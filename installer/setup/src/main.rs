//! Vitals Setup: the Windows installer. A borderless 720×480 window showing the screens in
//! `installer/ui/index.html` through WebView2. The page asks for actions over IPC (JSON with a
//! `cmd`); work runs on a background thread and reports back as page events (`vitalsEvent(...)`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use vitals_core::ops::{self, Kind, Options};
use vitals_core::{assets, pair, payload, text};
use wry::WebViewBuilder;
use wry::http::{Response, header::CONTENT_TYPE};

/// Messages for the window thread: requests from the page, and events for the page.
#[derive(Debug)]
enum UserEvent {
    Page(Request),
    Emit(Value),
}

/// What the page can ask for, e.g. `{"cmd":"run","kind":"install","options":{...}}`.
#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
enum Request {
    /// The page has loaded: which screen should it start on?
    Hello,
    Run {
        kind: Kind,
        options: Option<Options>,
    },
    /// Re-run the failed step of the last run and carry on.
    Retry,
    PairStart,
    PickFolder,
    Drag,
    Minimize,
    Close,
    Open {
        url: String,
    },
}

/// The run in progress (or the last one): what, with which options, and where it stopped.
struct Job {
    kind: Kind,
    options: Options,
    steps: Vec<ops::PlannedStep>,
    next: usize,
}

const NAVY: (u8, u8, u8, u8) = (0x0F, 0x1B, 0x2E, 0xFF);

fn main() {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Vitals Setup")
        .with_inner_size(LogicalSize::new(720.0, 480.0))
        .with_resizable(false)
        .with_decorations(false)
        .with_visible(false)
        .build(&event_loop)
        .expect("create the setup window");
    style_window(&window);
    center(&window);

    // WebView2 keeps browser data in a folder. By default that is next to the exe, which would
    // leave "vitals-setup.exe.WebView2" in Program Files (or wherever setup ran from): use temp
    // instead, and delete it when setup closes (see `exit`).
    let web_data = std::env::temp_dir().join("VitalsSetup.WebView2");
    let mut web_context = wry::WebContext::new(Some(web_data.clone()));

    let webview = match WebViewBuilder::new_with_web_context(&mut web_context)
        .with_background_color(NAVY)
        .with_devtools(cfg!(debug_assertions))
        .with_custom_protocol("vitals".into(), |_id, request| {
            match assets::get(request.uri().path()) {
                Some((body, mime)) => Response::builder().header(CONTENT_TYPE, mime).body(body),
                None => Response::builder().status(404).body(Vec::new().into()),
            }
            .expect("build response")
        })
        .with_ipc_handler(page_requests(proxy.clone()))
        .with_url("vitals://localhost/index.html")
        .build(&window)
    {
        Ok(w) => w,
        Err(e) => {
            message_box(&format!(
                "Vitals Setup needs Microsoft Edge WebView2, which is part of Windows 10 and 11.\n\n\
                 Install it from https://go.microsoft.com/fwlink/p/?LinkId=2124703 and run setup again.\n\n({e})"
            ));
            return;
        }
    };
    window.set_visible(true);

    let job: Arc<Mutex<Option<Job>>> = Arc::new(Mutex::new(None));
    let mut pairing: Option<pair::Pairing> = None;
    let mut installed_unpaired = false;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        let exit = |control_flow: &mut ControlFlow, pairing: &mut Option<pair::Pairing>, installed_unpaired: bool| {
            if let Some(p) = pairing.take()
                && !p.is_done()
            {
                p.cancel();
            }
            // Closed before pairing: start the agent anyway; it waits until it is paired.
            if installed_unpaired {
                let _ = ops::start_service(false);
            }
            ops::finish_uninstall();
            delete_after_exit(&web_data);
            *control_flow = ControlFlow::Exit;
        };
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                exit(control_flow, &mut pairing, installed_unpaired)
            }
            Event::UserEvent(UserEvent::Emit(value)) => {
                if value["event"] == "finished" && value["kind"] == "install" {
                    installed_unpaired = true;
                }
                if value["event"] == "paired" {
                    installed_unpaired = false;
                }
                let _ = webview.evaluate_script(&format!("window.vitalsEvent && window.vitalsEvent({value})"));
            }
            Event::UserEvent(UserEvent::Page(request)) => match request {
                Request::Hello => emit(&proxy, hello()),
                Request::Run { kind, options } => start_job(&job, &proxy, kind, options),
                Request::Retry => continue_job(job.clone(), proxy.clone()),
                Request::PairStart => {
                    let dir = job.lock().unwrap().as_ref().map(|j| PathBuf::from(&j.options.dir));
                    let dir = dir.or_else(|| ops::installed().map(|i| PathBuf::from(i.dir)));
                    let Some(dir) = dir else { return };
                    let p = proxy.clone();
                    match pair::start(&dir, move |v| emit(&p, v)) {
                        Ok(handle) => pairing = Some(handle),
                        Err(e) => emit(&proxy, json!({ "event": "pairInfo", "message": format!("Couldn't start pairing: {e}") })),
                    }
                }
                Request::PickFolder => {
                    if let Some(folder) = rfd::FileDialog::new().set_title("Install Vitals in").pick_folder() {
                        let mut path = folder.to_string_lossy().to_string();
                        // Picking "C:\Program Files" means "in Program Files", not "loose in it".
                        if !path.to_ascii_lowercase().trim_end_matches('\\').ends_with("vitals") {
                            path = format!("{}\\Vitals", path.trim_end_matches('\\'));
                        }
                        emit(&proxy, json!({ "event": "folder", "dir": path }));
                    }
                }
                Request::Drag => {
                    let _ = window.drag_window();
                }
                Request::Minimize => window.set_minimized(true),
                Request::Close => exit(control_flow, &mut pairing, installed_unpaired),
                Request::Open { url } => open_in_browser(&url),
            },
            _ => {}
        }
    });
}

/// The first screen: uninstall (asked for), welcome (not installed), update (older version) or
/// manage (this version or newer).
fn hello() -> Value {
    let installed = ops::installed();
    let uninstall = std::env::args().any(|a| a == "--uninstall")
        || std::env::current_exe()
            .ok()
            .and_then(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_ascii_lowercase())
            })
            .is_some_and(|n| n.contains("uninstall"));
    let screen = match &installed {
        // The uninstaller with nothing to remove says so (and never guesses a folder).
        None if uninstall && !own_folder_is_install() => "none",
        _ if uninstall => "remove",
        None => "welcome",
        Some(i) if text::compare_versions(payload::VERSION, &i.version).is_gt() => "update",
        Some(_) => "manage",
    };
    // An uninstaller whose registry entry is gone still names the folder it would empty: its own.
    let dir = match &installed {
        None if uninstall && own_folder_is_install() => own_folder()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(ops::default_dir),
        _ => ops::default_dir(),
    };
    let defaults = Options {
        dir,
        startup: true,
        auto_update: true,
        driver: true,
        shortcut: false,
    };
    json!({
        "event": "hello",
        "screen": screen,
        "version": payload::VERSION,
        "installed": installed,
        "defaults": defaults,
    })
}

/// The folder this program runs from.
fn own_folder() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|d| d.to_path_buf())
}

fn own_folder_is_install() -> bool {
    own_folder().is_some_and(|d| ops::is_install_dir(&d))
}

fn start_job(
    job: &Arc<Mutex<Option<Job>>>,
    proxy: &EventLoopProxy<UserEvent>,
    kind: Kind,
    options: Option<Options>,
) {
    let installed = ops::installed();
    let mut options = match (options, &installed) {
        (Some(o), _) => o,
        (None, Some(i)) => i.options.clone(),
        (None, None) => Options {
            dir: ops::default_dir(),
            startup: true,
            auto_update: true,
            driver: true,
            shortcut: false,
        },
    };
    // Everything but a fresh install works on the folder Vitals is already in.
    if kind != Kind::Install
        && let Some(i) = &installed
    {
        options.dir = i.dir.clone();
    }
    if kind == Kind::Remove && installed.is_none() {
        // Registry entries already gone (a half-finished uninstall): this program's own folder,
        // but only if it really is a Vitals folder. Otherwise there is nothing to remove.
        match own_folder().filter(|d| ops::is_install_dir(d)) {
            Some(dir) => options.dir = dir.to_string_lossy().to_string(),
            None => {
                emit(
                    proxy,
                    json!({ "event": "hello", "screen": "none", "version": payload::VERSION }),
                );
                return;
            }
        }
    }
    let steps = ops::plan(
        kind,
        &options,
        installed.as_ref().map(|i| i.version.as_str()),
    );
    emit(
        proxy,
        json!({ "event": "plan", "kind": kind, "steps": steps }),
    );
    *job.lock().unwrap() = Some(Job {
        kind,
        options,
        steps,
        next: 0,
    });
    continue_job(job.clone(), proxy.clone());
}

/// Runs the job's remaining steps on a background thread, reporting each one.
fn continue_job(job: Arc<Mutex<Option<Job>>>, proxy: EventLoopProxy<UserEvent>) {
    std::thread::spawn(move || {
        loop {
            let mut guard = job.lock().unwrap();
            let Some(j) = guard.as_mut() else { return };
            if j.next >= j.steps.len() {
                emit(&proxy, json!({ "event": "finished", "kind": j.kind }));
                return;
            }
            let i = j.next;
            emit(
                &proxy,
                json!({ "event": "step", "index": i, "state": "active" }),
            );
            match ops::run_step(&j.steps[i], j.kind, &j.options) {
                Ok(()) => {
                    j.next += 1;
                    emit(
                        &proxy,
                        json!({ "event": "step", "index": i, "state": "done", "total": j.steps.len() }),
                    );
                }
                Err(e) => {
                    emit(
                        &proxy,
                        json!({ "event": "failed", "index": i, "what": e.what, "detail": e.detail }),
                    );
                    return;
                }
            }
            drop(guard);
        }
    });
}

fn emit(proxy: &EventLoopProxy<UserEvent>, value: Value) {
    let _ = proxy.send_event(UserEvent::Emit(value));
}

/// Parses each IPC message from the page and forwards it to the window thread.
fn page_requests(
    proxy: EventLoopProxy<UserEvent>,
) -> impl Fn(wry::http::Request<String>) + 'static {
    move |message| match serde_json::from_str::<Request>(message.body()) {
        Ok(request) => {
            let _ = proxy.send_event(UserEvent::Page(request));
        }
        Err(e) => eprintln!("ignored page message {:?}: {e}", message.body()),
    }
}

/// Opens a web link in the default browser. Only https links: the page has no reason to open
/// anything else, so nothing else is passed to the shell.
fn open_in_browser(url: &str) {
    if !url.starts_with("https://") || url.contains('"') {
        return;
    }
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

/// WebView2's helper processes hold its data folder until shortly after the window closes, so a
/// hidden command waits a moment and then removes it.
fn delete_after_exit(dir: &std::path::Path) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let dir = dir.to_string_lossy();
    if !dir.ends_with("VitalsSetup.WebView2") || dir.contains('"') {
        return; // only ever our own folder
    }
    let _ = std::process::Command::new("cmd.exe")
        .raw_arg(format!(
            "/c ping -n 4 127.0.0.1 >nul & rmdir /s /q \"{dir}\""
        ))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

fn message_box(text: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    let wide = |s: &str| {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let (text, title) = (wide(text), wide("Vitals Setup"));
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn center(window: &Window) {
    if let Some(monitor) = window.current_monitor() {
        let screen = monitor.size();
        let size = window.outer_size();
        let x = monitor.position().x + (screen.width as i32 - size.width as i32) / 2;
        let y = monitor.position().y + (screen.height as i32 - size.height as i32) / 2;
        window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
    }
}

/// A borderless window still gets the system shadow and, on Windows 11, rounded corners.
fn style_window(window: &Window) {
    use tao::platform::windows::WindowExtWindows;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    window.set_undecorated_shadow(true);
    let preference = DWMWCP_ROUND;
    // Ignored (harmless) on Windows 10, which has no rounded corners.
    unsafe {
        DwmSetWindowAttribute(
            window.hwnd() as _,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &preference as *const _ as *const _,
            std::mem::size_of_val(&preference) as u32,
        );
    }
}
