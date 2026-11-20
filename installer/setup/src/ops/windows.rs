//! What the installer does to the computer: a port of the former Inno Setup installer to Rust.
//!
//! A run (install, update, repair, settings, remove) is a list of steps. Every step can be run
//! again safely, so "Try again" on the page simply re-runs the step that failed and carries on.
#![cfg(windows)]

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use winreg::RegKey;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, KEY_READ, REG_EXPAND_SZ};

use crate::payload;
use crate::text;

pub const SERVICE: &str = "VitalsAgent";
const LEGACY_SERVICES: [&str; 2] = ["NexusAgent", "PCMonitorAgent"];
const STATE_KEY: &str = r"SOFTWARE\Vitals";
const UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Vitals";
/// The Inno Setup entry (same AppId as the Nexus installer): taken over on upgrade.
const INNO_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{F9FF55E1-801A-4E78-96FA-550A8B46C103}_is1";
const PAWNIO_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO";
const RUN_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Vitals Tray";
const APP_PATHS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\vitals.exe";
const ENVIRONMENT: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The choices on the Custom / Change settings screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub dir: String,
    pub startup: bool,
    pub auto_update: bool,
    pub driver: bool,
    pub shortcut: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Install,
    Update,
    Repair,
    Settings,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    StopOld,
    CopyFiles,
    Config,
    Driver,
    Service,
    Integrate,
    StartService,
    Unpair,
    RemoveService,
    RemoveDriver,
    RemoveFiles,
}

/// One line of the checklist on the page.
#[derive(Debug, Serialize)]
pub struct PlannedStep {
    pub label: String,
    /// Shown as skipped (e.g. "Already installed") instead of being run.
    pub note: Option<String>,
    #[serde(skip)]
    step: Option<Step>,
}

/// What a failed step tells the person: a plain sentence and the technical detail.
#[derive(Debug, Serialize)]
pub struct StepError {
    pub what: String,
    pub detail: String,
}

type StepResult = Result<(), StepError>;

fn fail(what: &str, detail: impl ToString) -> StepError {
    StepError {
        what: what.to_string(),
        detail: detail.to_string(),
    }
}

/// What is on this computer before setup runs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub version: String,
    pub dir: String,
    pub options: Options,
}

/// Paths everything else is relative to.
fn program_data() -> PathBuf {
    PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into()))
}
pub fn data_dir() -> PathBuf {
    program_data().join("Vitals")
}
pub fn config_path() -> PathBuf {
    data_dir().join("config.toml")
}
fn system32(exe: &str) -> PathBuf {
    PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
        .join("System32")
        .join(exe)
}
pub fn default_dir() -> String {
    let pf = std::env::var("ProgramW6432")
        .or_else(|_| std::env::var("ProgramFiles"))
        .unwrap_or_else(|_| r"C:\Program Files".into());
    format!(r"{pf}\Vitals")
}
fn hklm() -> RegKey {
    RegKey::predef(HKEY_LOCAL_MACHINE)
}

/// Runs a system tool hidden and returns (exit code, combined output).
fn run(exe: &Path, args: &[&str]) -> (i32, String) {
    match Command::new(exe)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(out) => {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            (out.status.code().unwrap_or(-1), text.trim().to_string())
        }
        Err(e) => (-1, e.to_string()),
    }
}
fn sc(args: &[&str]) -> (i32, String) {
    run(&system32("sc.exe"), args)
}

fn pawnio_installed() -> bool {
    hklm().open_subkey(PAWNIO_KEY).is_ok()
}
fn state_dword(name: &str) -> Option<u32> {
    hklm()
        .open_subkey(STATE_KEY)
        .ok()?
        .get_value::<u32, _>(name)
        .ok()
}

/// Vitals already on this computer: this installer's entry, else the old Inno Setup one.
pub fn installed() -> Option<Installed> {
    let (version, dir) = if let Ok(k) = hklm().open_subkey_with_flags(UNINSTALL_KEY, KEY_READ) {
        (
            k.get_value::<String, _>("DisplayVersion").ok()?,
            k.get_value::<String, _>("InstallLocation").ok()?,
        )
    } else {
        let k = hklm().open_subkey_with_flags(INNO_KEY, KEY_READ).ok()?;
        let dir: String = k
            .get_value("Inno Setup: App Path")
            .or_else(|_| k.get_value("InstallLocation"))
            .ok()?;
        (
            k.get_value::<String, _>("DisplayVersion")
                .unwrap_or_else(|_| "0.0.0".into()),
            dir,
        )
    };
    let dir = dir.trim_end_matches('\\').to_string();
    let config = fs::read_to_string(config_path()).unwrap_or_default();
    let options = Options {
        startup: hklm()
            .open_subkey(RUN_KEY)
            .and_then(|k| k.get_value::<String, _>(RUN_VALUE))
            .is_ok()
            || state_dword("Startup") == Some(1),
        auto_update: !config
            .lines()
            .any(|l| l.replace(' ', "") == "auto_update=false"),
        driver: pawnio_installed(),
        shortcut: shortcut_path().exists(),
        dir: dir.clone(),
    };
    Some(Installed {
        version,
        dir,
        options,
    })
}

/// The checklist for a run. Steps that will not run are listed with a note, so the page shows
/// the whole job.
pub fn plan(kind: Kind, opts: &Options, from_version: Option<&str>) -> Vec<PlannedStep> {
    let s = |label: &str, step: Step| PlannedStep {
        label: label.into(),
        note: None,
        step: Some(step),
    };
    let note = |label: &str, note: &str| PlannedStep {
        label: label.into(),
        note: Some(note.into()),
        step: None,
    };
    let driver = if !opts.driver {
        note("Installing the sensor driver", "Skipped")
    } else if pawnio_installed() {
        note("Installing the sensor driver", "Already installed")
    } else {
        s("Installing the sensor driver", Step::Driver)
    };
    match kind {
        Kind::Install => vec![
            s("Getting this PC ready", Step::StopOld),
            s("Copying Vitals files", Step::CopyFiles),
            s("Saving your settings", Step::Config),
            driver,
            s("Setting up the Vitals service", Step::Service),
            s("Adding Vitals to Windows", Step::Integrate),
        ],
        Kind::Update => vec![
            s(
                &format!("Stopping Vitals {}", from_version.unwrap_or("")),
                Step::StopOld,
            ),
            s(
                &format!("Copying Vitals {}", payload::VERSION),
                Step::CopyFiles,
            ),
            s("Updating your settings", Step::Config),
            s("Setting up the Vitals service", Step::Service),
            s("Adding Vitals to Windows", Step::Integrate),
            s("Starting Vitals", Step::StartService),
        ],
        Kind::Repair => vec![
            s("Stopping Vitals", Step::StopOld),
            s("Copying Vitals files", Step::CopyFiles),
            s("Checking your settings", Step::Config),
            driver,
            s("Setting up the Vitals service", Step::Service),
            s("Adding Vitals to Windows", Step::Integrate),
            s("Starting Vitals", Step::StartService),
        ],
        Kind::Settings => vec![
            s("Applying your settings", Step::Config),
            driver,
            s("Updating Windows startup and shortcuts", Step::Integrate),
            s("Restarting Vitals", Step::StartService),
        ],
        Kind::Remove => vec![
            s("Stopping Vitals", Step::StopOld),
            s("Unlinking this PC from your phone", Step::Unpair),
            s("Removing the Vitals service", Step::RemoveService),
            if state_dword("InstalledPawnIO") == Some(1) && pawnio_installed() {
                s("Removing the sensor driver", Step::RemoveDriver)
            } else {
                note("Removing the sensor driver", "Not needed")
            },
            s("Deleting Vitals files", Step::RemoveFiles),
        ],
    }
}

/// Runs one planned step. Steps listed with a note do nothing.
pub fn run_step(planned: &PlannedStep, kind: Kind, opts: &Options) -> StepResult {
    let Some(step) = planned.step else {
        return Ok(());
    };
    let dir = PathBuf::from(&opts.dir);
    match step {
        Step::StopOld => stop_everything(),
        Step::CopyFiles => copy_files(&dir),
        Step::Config => write_config(opts),
        Step::Driver => install_driver(),
        Step::Service => create_service(&dir),
        Step::Integrate => integrate(&dir, opts),
        Step::StartService => start_service(kind == Kind::Settings),
        Step::Unpair => unpair(&dir),
        Step::RemoveService => remove_services(),
        Step::RemoveDriver => remove_driver(),
        Step::RemoveFiles => remove_files(&dir),
    }
}

// ---- install steps ------------------------------------------------------------------------

/// Stops the agent, its tray icon and any older Vitals / Nexus / PC Monitor service, and waits
/// until they are gone, so their files can be replaced. Port of PrepareToInstall.
fn stop_everything() -> StepResult {
    for exe in ["vitals.exe", "nexus.exe", "pc-monitor-agent.exe"] {
        run(&system32("taskkill.exe"), &["/F", "/IM", exe]);
    }
    for svc in std::iter::once(SERVICE).chain(LEGACY_SERVICES) {
        sc(&["stop", svc]);
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let running = std::iter::once(SERVICE).chain(LEGACY_SERVICES).any(|svc| {
            sc(&["query", svc]).1.contains("STOP_PENDING")
                || sc(&["query", svc]).1.contains("RUNNING")
        });
        if !running {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    Err(fail(
        "Vitals didn't stop in time.",
        "The VitalsAgent service is still running after 20 seconds.",
    ))
}

/// Writes the agent, documents and sensor modules, and a copy of this program (it is also the
/// uninstaller and the Manage screen). Then only SYSTEM and Administrators may change the folder:
/// the agent runs as LocalSystem, so a user-writable folder would let anyone replace it (LockDir).
fn copy_files(dir: &Path) -> StepResult {
    let what = "Couldn't copy the Vitals files.";
    fs::create_dir_all(dir).map_err(|e| fail(what, format!("{}: {e}", dir.display())))?;
    write_retrying(&dir.join("vitals.exe"), payload::AGENT_EXE).map_err(|e| fail(what, e))?;
    for (name, bytes) in payload::FILES {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| fail(what, e))?;
        }
        write_retrying(&path, bytes).map_err(|e| fail(what, e))?;
    }
    let me = std::env::current_exe().map_err(|e| fail(what, e))?;
    let target = dir.join("vitals-setup.exe");
    if !same_file(&me, &target) {
        fs::copy(&me, &target).map_err(|e| fail(what, format!("{}: {e}", target.display())))?;
    }
    lock_dir(dir, true);
    Ok(())
}

/// A file held open for a moment by a closing process (or an antivirus scan) is retried.
fn write_retrying(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut last = String::new();
    for _ in 0..20 {
        match fs::write(path, bytes) {
            Ok(()) => return Ok(()),
            Err(e) => last = format!("{}: {e}", path.display()),
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(last)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn lock_dir(dir: &Path, users_can_read: bool) {
    let dir = dir.to_string_lossy();
    let mut args = vec![
        dir.as_ref(),
        "/inheritance:r",
        "/grant:r",
        "*S-1-5-18:(OI)(CI)F",
        "*S-1-5-32-544:(OI)(CI)F",
    ];
    if users_can_read {
        args.push("*S-1-5-32-545:(OI)(CI)RX");
    }
    run(&system32("icacls.exe"), &args);
}

/// The agent config: kept (with its device secret) or created, pointed at this build's backend.
/// config.toml is for SYSTEM and Administrators only; control.json may be changed by users (the
/// tray's pause/resume). Port of WriteConfig and the ssPostInstall permissions.
fn write_config(opts: &Options) -> StepResult {
    let what = "Couldn't save the Vitals settings.";
    let data = data_dir();
    fs::create_dir_all(&data).map_err(|e| fail(what, e))?;
    let cfg = config_path();
    if !cfg.exists() {
        // Upgrade from Nexus or PC Monitor: keep the device secret, so the computer stays paired.
        for old in [
            program_data().join(r"Nexus\config.toml"),
            program_data().join(r"PCMonitor\config.toml"),
        ] {
            if old.exists() {
                let _ = fs::copy(&old, &cfg);
                break;
            }
        }
    }
    let existing = fs::read_to_string(&cfg).ok();
    let body = text::config_toml(existing.as_deref(), payload::CONVEX_URL, opts.auto_update);
    fs::write(&cfg, body).map_err(|e| fail(what, format!("{}: {e}", cfg.display())))?;
    lock_dir(&data, true);
    let control = data.join("control.json");
    if !control.exists() {
        let _ = fs::write(&control, r#"{"paused":false}"#);
    }
    let icacls = system32("icacls.exe");
    for file in ["control.json", "status.json"] {
        let path = data.join(file);
        if path.exists() {
            run(
                &icacls,
                &[&path.to_string_lossy(), "/grant", "*S-1-5-32-545:M"],
            );
        }
    }
    run(
        &icacls,
        &[
            &cfg.to_string_lossy(),
            "/inheritance:r",
            "/grant:r",
            "*S-1-5-18:F",
            "*S-1-5-32-544:F",
        ],
    );
    Ok(())
}

/// The signed PawnIO driver (CPU temperatures), installed only when it isn't there yet, and
/// remembered so the uninstaller removes only what it added. Port of InstallPawnIO.
fn install_driver() -> StepResult {
    let what = "Couldn't install the sensor driver.";
    if pawnio_installed() {
        return Ok(());
    }
    let setup = std::env::temp_dir().join("vitals-PawnIO_setup.exe");
    fs::write(&setup, payload::PAWNIO_SETUP).map_err(|e| fail(what, e))?;
    let (code, out) = run(&setup, &["-install", "-silent"]);
    let _ = fs::remove_file(&setup);
    if !pawnio_installed() {
        return Err(fail(
            what,
            format!(
                "PawnIO setup exited with {code}. {out} You can untick the sensor driver in Custom setup and try again."
            ),
        ));
    }
    let (key, _) = hklm().create_subkey(STATE_KEY).map_err(|e| fail(what, e))?;
    let _ = key.set_value("InstalledPawnIO", &1u32);
    Ok(())
}

/// The Windows service, created (or pointed at this folder) and set to restart after a crash;
/// older Nexus / PC Monitor services are removed. It is started after pairing, so the agent
/// starts as a paired device. Port of CreateService.
fn create_service(dir: &Path) -> StepResult {
    let what = "Couldn't set up the Vitals service.";
    let bin = format!("\"{}\" service", dir.join("vitals.exe").display());
    let (exists, _) = sc(&["query", SERVICE]);
    let (code, out) = if exists == 0 {
        sc(&[
            "config",
            SERVICE,
            "binPath=",
            &bin,
            "start=",
            "delayed-auto",
        ])
    } else {
        sc(&[
            "create",
            SERVICE,
            "binPath=",
            &bin,
            "start=",
            "delayed-auto",
            "DisplayName=",
            "Vitals Agent",
        ])
    };
    if code != 0 {
        return Err(fail(what, format!("sc.exe exited with {code}: {out}")));
    }
    sc(&[
        "description",
        SERVICE,
        "Sends this computer's hardware stats to Vitals.",
    ]);
    // Restart after a crash: 5 s, 5 s, then 60 s; reset the count after a day (NFR-06).
    sc(&[
        "failure",
        SERVICE,
        "reset=",
        "86400",
        "actions=",
        "restart/5000/restart/5000/restart/60000",
    ]);
    for old in LEGACY_SERVICES {
        sc(&["stop", old]);
        sc(&["delete", old]);
    }
    Ok(())
}

/// Starts (or restarts) the service. 1056 = already running.
pub fn start_service(restart: bool) -> StepResult {
    if restart {
        sc(&["stop", SERVICE]);
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline && sc(&["query", SERVICE]).1.contains("STOP_PENDING") {
            std::thread::sleep(Duration::from_millis(300));
        }
    }
    let (code, out) = sc(&["start", SERVICE]);
    if code == 0 || code == 1056 {
        Ok(())
    } else {
        Err(fail(
            "Couldn't start the Vitals service.",
            format!("sc.exe exited with {code}: {out}"),
        ))
    }
}

/// Tray at sign-in, the `vitals` command in every terminal (PATH, App Paths), the desktop
/// shortcut and the entry in Settings > Apps. Takes over an Inno Setup install's entry.
fn integrate(dir: &Path, opts: &Options) -> StepResult {
    let what = "Couldn't add Vitals to Windows.";
    let exe = dir.join("vitals.exe");
    let setup = dir.join("vitals-setup.exe");
    let dir_s = dir.to_string_lossy().to_string();

    let run_key = hklm()
        .open_subkey_with_flags(RUN_KEY, KEY_ALL_ACCESS)
        .map_err(|e| fail(what, e))?;
    if opts.startup {
        run_key
            .set_value(RUN_VALUE, &format!("\"{}\" tray", exe.display()))
            .map_err(|e| fail(what, e))?;
    } else {
        let _ = run_key.delete_value(RUN_VALUE);
    }
    // The Inno setup started the tray from a Startup-folder shortcut instead: remove it, or the
    // tray would start twice.
    let common_startup = program_data().join(r"Microsoft\Windows\Start Menu\Programs\Startup");
    let _ = fs::remove_file(common_startup.join("Vitals Agent Tray.lnk"));
    let _ = fs::remove_file(common_startup.join("Nexus Agent Tray.lnk"));

    set_machine_path(|p| text::path_with(p, &dir_s)).map_err(|e| fail(what, e))?;
    let (app_paths, _) = hklm().create_subkey(APP_PATHS).map_err(|e| fail(what, e))?;
    let _ = app_paths.set_value("", &exe.to_string_lossy().to_string());
    let _ = app_paths.set_value("Path", &dir_s);

    // Start menu: always. Windows search finds Vitals through it, shows the logo from vitals.exe,
    // and offers Uninstall because the target sits in the Settings > Apps entry's InstallLocation.
    write_tray_shortcut(&exe, &start_menu_path()).map_err(|e| fail(what, e))?;
    if opts.shortcut {
        let _ = write_tray_shortcut(&exe, &shortcut_path());
    } else {
        let _ = fs::remove_file(shortcut_path());
    }

    let (k, _) = hklm()
        .create_subkey(UNINSTALL_KEY)
        .map_err(|e| fail(what, e))?;
    let quoted = format!("\"{}\"", setup.display());
    let size_kb = (payload::AGENT_EXE.len() / 1024) as u32 + 2048;
    for (name, value) in [
        ("DisplayName", "Vitals".to_string()),
        ("DisplayVersion", payload::VERSION.to_string()),
        (
            "Publisher",
            "Aditya Prasad, Tirth Patel, Pratham Vasava".to_string(),
        ),
        (
            "URLInfoAbout",
            "https://github.com/theallmyti/Vitals".to_string(),
        ),
        ("DisplayIcon", setup.to_string_lossy().to_string()),
        ("InstallLocation", dir_s.clone()),
        ("UninstallString", format!("{quoted} --uninstall")),
        ("ModifyPath", quoted.clone()),
    ] {
        k.set_value(name, &value).map_err(|e| fail(what, e))?;
    }
    let _ = k.set_value("EstimatedSize", &size_kb);
    let _ = k.set_value("NoRepair", &1u32);

    let (state, _) = hklm().create_subkey(STATE_KEY).map_err(|e| fail(what, e))?;
    let _ = state.set_value("InstallDir", &dir_s);
    let _ = state.set_value("Version", &payload::VERSION.to_string());
    let _ = state.set_value("Startup", &(opts.startup as u32));

    take_over_inno_install(dir);
    launch_tray();
    Ok(())
}

/// A shortcut that opens the tray, named "Vitals".
fn write_tray_shortcut(exe: &Path, lnk: &Path) -> Result<(), String> {
    let mut link = mslnk::ShellLink::new(exe).map_err(|e| e.to_string())?;
    link.set_arguments(Some("tray".into()));
    link.set_name(Some("Vitals".into()));
    link.set_working_dir(exe.parent().map(|d| d.to_string_lossy().to_string()));
    link.create_lnk(lnk).map_err(|e| e.to_string())
}

/// Starts the tray for the person at the desk. Setup runs as admin, and a tray started directly
/// would be admin too (and outlive a later sign-out oddly); opening the Start menu shortcut through
/// explorer.exe hands it to the already-running desktop shell, which starts it as the normal user.
/// A tray that is already running exits by itself (agent/src/tray.rs).
fn launch_tray() {
    let lnk = start_menu_path();
    if lnk.exists() {
        let explorer =
            PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
                .join("explorer.exe");
        let _ = Command::new(explorer).arg(&lnk).spawn();
    }
}

/// Removes what an Inno Setup install of Vitals or Nexus leaves that this installer replaces:
/// its uninstall entry and uninstaller files, its Start-menu folder, the Nexus leftovers.
fn take_over_inno_install(dir: &Path) {
    if let Ok(k) = hklm().open_subkey(INNO_KEY) {
        if let Ok(old) = k.get_value::<String, _>("Inno Setup: App Path") {
            let old = PathBuf::from(old);
            for f in ["unins000.exe", "unins000.dat", "nexus.exe", "nexus.old"] {
                let _ = fs::remove_file(old.join(f));
            }
            if !same_file(&old, dir) {
                let _ = set_machine_path(|p| text::path_without(p, &old.to_string_lossy()));
            }
        }
        let _ = hklm().delete_subkey_all(INNO_KEY);
    }
    let menu = program_data().join(r"Microsoft\Windows\Start Menu\Programs");
    for group in ["Vitals", "Nexus"] {
        let _ = fs::remove_dir_all(menu.join(group));
    }
    let _ =
        hklm().delete_subkey_all(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\nexus.exe");
    let _ = hklm().delete_subkey_all(r"SOFTWARE\Nexus");
    if config_path().exists() {
        let _ = fs::remove_dir_all(program_data().join("Nexus"));
    }
}

fn start_menu_path() -> PathBuf {
    program_data().join(r"Microsoft\Windows\Start Menu\Programs\Vitals.lnk")
}

fn shortcut_path() -> PathBuf {
    let public = std::env::var("PUBLIC").unwrap_or_else(|_| r"C:\Users\Public".into());
    PathBuf::from(public).join(r"Desktop\Vitals.lnk")
}

/// Edits the machine PATH, keeping it REG_EXPAND_SZ (it holds %SystemRoot% and friends), and
/// tells running programs so new terminals see the change.
fn set_machine_path(edit: impl Fn(&str) -> String) -> Result<(), String> {
    let env = hklm()
        .open_subkey_with_flags(ENVIRONMENT, KEY_ALL_ACCESS)
        .map_err(|e| e.to_string())?;
    let current: String = env.get_value("Path").map_err(|e| e.to_string())?;
    let new = edit(&current);
    if new == current {
        return Ok(());
    }
    let bytes: Vec<u8> = new
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(|u| u.to_le_bytes())
        .collect();
    env.set_raw_value(
        "Path",
        &winreg::RegValue {
            bytes: bytes.into(),
            vtype: REG_EXPAND_SZ,
        },
    )
    .map_err(|e| e.to_string())?;
    broadcast_environment_change();
    Ok(())
}

fn broadcast_environment_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };
    let what: Vec<u16> = "Environment"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            what.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            3000,
            &mut result,
        );
    }
}

// ---- uninstall steps ----------------------------------------------------------------------

/// Removes this computer from Vitals (needs the config, so before files go). Not fatal: without
/// internet the device is simply removed in the app later.
fn unpair(dir: &Path) -> StepResult {
    let exe = dir.join("vitals.exe");
    if exe.exists() && config_path().exists() {
        run(&exe, &["unpair", &config_path().to_string_lossy()]);
    }
    Ok(())
}

fn remove_services() -> StepResult {
    for svc in std::iter::once(SERVICE).chain(LEGACY_SERVICES) {
        sc(&["delete", svc]);
    }
    Ok(())
}

fn remove_driver() -> StepResult {
    if let Ok(k) = hklm().open_subkey(PAWNIO_KEY)
        && let Ok(cmd) = k.get_value::<String, _>("QuietUninstallString")
    {
        let (exe, args) = split_command_line(&cmd);
        run(
            Path::new(&exe),
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
        );
    }
    Ok(())
}

/// "C:\path\x.exe" -a -b  ->  (C:\path\x.exe, [-a, -b])
fn split_command_line(cmd: &str) -> (String, Vec<String>) {
    let cmd = cmd.trim();
    let (exe, rest) = if let Some(stripped) = cmd.strip_prefix('"') {
        let end = stripped.find('"').unwrap_or(stripped.len());
        (
            stripped[..end].to_string(),
            stripped.get(end + 1..).unwrap_or(""),
        )
    } else {
        let end = cmd.find(' ').unwrap_or(cmd.len());
        (cmd[..end].to_string(), &cmd[end..])
    };
    (exe, rest.split_whitespace().map(String::from).collect())
}

/// Files Vitals puts in its folder: what setup copies, plus what the agent's self-update
/// leaves (agent/src/update.rs). The uninstaller deletes exactly these, never "everything in the
/// folder": the folder could be anywhere, and other files there are not ours to delete.
fn our_files() -> Vec<&'static str> {
    let mut files = vec![
        "vitals.exe",
        "vitals.old",
        "vitals.update.tmp",
        "unins000.exe",
        "unins000.dat",
    ];
    files.extend(payload::FILES.iter().map(|(name, _)| *name));
    files
}

/// Is `dir` a Vitals install folder? It must hold the agent. (The uninstaller refuses anything
/// else, so a stray copy of it in Downloads can never empty Downloads.)
pub fn is_install_dir(dir: &Path) -> bool {
    dir.join("vitals.exe").is_file()
}

/// Everything else: startup, PATH, shortcuts, registry entries, settings and logs, and the
/// files Vitals put in its folder. This running program and the then-empty folders are deleted
/// after it exits.
fn remove_files(dir: &Path) -> StepResult {
    if !is_install_dir(dir) && hklm().open_subkey(UNINSTALL_KEY).is_err() {
        return Err(fail(
            "Couldn't find the Vitals folder.",
            format!(
                "{} doesn't contain vitals.exe, so nothing there was deleted.",
                dir.display()
            ),
        ));
    }
    if let Ok(k) = hklm().open_subkey_with_flags(RUN_KEY, KEY_ALL_ACCESS) {
        let _ = k.delete_value(RUN_VALUE);
    }
    let _ = set_machine_path(|p| text::path_without(p, &dir.to_string_lossy()));
    let _ = fs::remove_file(shortcut_path());
    let _ = fs::remove_file(start_menu_path());
    for key in [APP_PATHS, UNINSTALL_KEY, STATE_KEY, INNO_KEY] {
        let _ = hklm().delete_subkey_all(key);
    }
    let _ = fs::remove_dir_all(data_dir()); // %ProgramData%\Vitals: config, logs, pairing files
    for name in our_files() {
        let _ = fs::remove_file(dir.join(name));
    }
    for sub in ["docs", "pawnio"] {
        let _ = fs::remove_dir(dir.join(sub)); // only if now empty
    }
    *DELETE_AFTER_EXIT.lock().unwrap() = Some(dir.to_path_buf());
    Ok(())
}

static DELETE_AFTER_EXIT: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Called as the program exits: a running exe can't delete itself, so a hidden command waits a
/// moment, deletes this program's copy in the install folder and then the folder, but only if it
/// is empty by then (`rmdir` without /s refuses a folder with anything else in it).
pub fn finish_uninstall() {
    if let Some(dir) = DELETE_AFTER_EXIT.lock().unwrap().take() {
        let dir = dir.to_string_lossy().to_string();
        if dir.len() > 3 && !dir.contains('"') {
            let _ = Command::new(system32("cmd.exe"))
                .raw_arg(format!(
                    "/c ping -n 3 127.0.0.1 >nul & del /f /q \"{dir}\\vitals-setup.exe\" & rmdir \"{dir}\""
                ))
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uninstall_only_touches_a_real_install_folder() {
        let tmp = std::env::temp_dir().join(format!("vitals-test-{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("holiday-photo.jpg"), b"not ours").unwrap();
        assert!(
            !is_install_dir(&tmp),
            "a folder without vitals.exe is never treated as Vitals"
        );
        fs::write(tmp.join("vitals.exe"), b"MZ").unwrap();
        assert!(is_install_dir(&tmp));
        assert!(!our_files().contains(&"holiday-photo.jpg"));
        assert!(our_files().contains(&"docs/TERMS.md") && our_files().contains(&"vitals.old"));
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn splits_uninstall_command_lines() {
        assert_eq!(
            split_command_line(r#""C:\Program Files\PawnIO\uninstall.exe" -uninstall -silent"#),
            (
                r"C:\Program Files\PawnIO\uninstall.exe".to_string(),
                vec!["-uninstall".into(), "-silent".into()]
            )
        );
        assert_eq!(
            split_command_line(r"C:\x.exe /S"),
            (r"C:\x.exe".to_string(), vec!["/S".into()])
        );
    }

    #[test]
    fn plans_list_every_step_and_skip_the_driver_when_unticked() {
        let opts = Options {
            dir: default_dir(),
            startup: true,
            auto_update: true,
            driver: false,
            shortcut: false,
        };
        let install = plan(Kind::Install, &opts, None);
        let labels: Vec<&str> = install.iter().map(|s| s.label.as_str()).collect();
        assert!(labels.contains(&"Copying Vitals files"));
        let driver = install
            .iter()
            .find(|s| s.label.contains("sensor driver"))
            .unwrap();
        assert_eq!(driver.note.as_deref(), Some("Skipped"));
        assert!(
            plan(Kind::Update, &opts, Some("1.0.0"))[0]
                .label
                .ends_with("1.0.0")
        );
        assert_eq!(
            plan(Kind::Remove, &opts, None).last().unwrap().label,
            "Deleting Vitals files"
        );
    }
}
