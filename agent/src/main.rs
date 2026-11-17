//! Vitals agent: samples every second, sends a summary every 60 s, and sends live
//! data only while someone is watching (docs/Architecture.md §2.2, FR-30, FR-31).
//! Convex client: https://docs.rs/convex/0.10.4 (clone shares the connection; the client
//! reconnects by itself with backoff, so only the first connection is retried here).

#[macro_use]
mod logging;
mod aggregator;
mod backend;
mod backlog;
mod battery;
mod clock;
mod collectors;
mod config;
mod control;
mod cputemp;
mod dashboard;
mod gpu;
mod hardware;
mod liveclock;
mod pair;
#[cfg(windows)]
mod pair_gui;
mod payload;
#[cfg(windows)]
mod service;
mod supervisor;
#[cfg(windows)]
mod tray;
mod update;
mod usage;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use aggregator::{Aggregator, Sample};
use backlog::Backlog;
use clock::{now_ms, now_secs};
use collectors::Proc;
use convex::Value;
use payload::{
    battery_value, capabilities_value, docker_value, drives_live, gpu_value, num, numbers,
    procs_value, ram_value, summary_value, text, usage_value,
};
use supervisor::Supervisor;
use sysinfo::{Components, CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};
use tokio::sync::Notify;
use usage::UsageTracker;

/// FR-30: one summary per minute, always.
const SUMMARY_EVERY: Duration = Duration::from_secs(60);
/// Offline buffering: keep up to a day of unsent summaries, send at most 60 per call.
const BACKLOG_MAX: usize = 24 * 60;
/// Live view refresh intervals for the slower readings, in seconds (ticks).
const PROCESS_EVERY: u64 = 5;
const DRIVES_EVERY: u64 = 10;
const DOCKER_EVERY: u64 = 10;
const BATTERY_EVERY: u64 = 10;

/// Hottest CPU sensor in Celsius. Raspberry Pi: cpu_thermal. Linux x86: coretemp/k10temp.
fn cpu_temp(components: &Components) -> Option<f64> {
    components
        .iter()
        .filter(|c| {
            let l = c.label().to_lowercase();
            l.contains("cpu") || l.contains("package") || l.contains("core")
        })
        .filter_map(|c| c.temperature())
        .fold(None, |max: Option<f64>, t| {
            Some(max.map_or(t as f64, |m| m.max(t as f64)))
        })
}

/// S2 spike: print every temperature sensor the OS exposes (Linux hwmon), then exit.
fn print_sensors() {
    match hardware::ram_speed_mts() {
        Some(mts) => say!("{:<40} {mts} MT/s", "memory speed"),
        None => say!("{:<40} n/a", "memory speed"),
    }
    // Base vs live CPU clock (live differs on Windows, where sysinfo only knows the base clock).
    let sys =
        System::new_with_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()))
            .expect("failed to read system info");
    let clock = liveclock::LiveClock::open();
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let (_, mut clocks) = collectors::per_core(&sys);
    if let Some(&base) = clocks.first() {
        clock.apply(&mut clocks);
        say!("{:<40} {base:.0} MHz", "cpu clock (base)");
        say!("{:<40} {:.0} MHz", "cpu clock (live)", clocks[0]);
    }
    let components = Components::new_with_refreshed_list().expect("failed to read sensors");
    let logical = sys.cpus().len();
    // Driver path first (Intel MSR / Ryzen SMU via PawnIO; needs an admin terminal), then Linux.
    let driver = cputemp::CpuTemp::open();
    match driver
        .read_cores(logical)
        .or_else(|| collectors::per_cpu_temps(&components, logical))
    {
        Some(t) => say!("{:<40} {t:?}", "per-core temps (°C)"),
        None => say!("{:<40} not reported by this chip/OS", "per-core temps"),
    }
    for c in &components {
        match c.temperature() {
            Some(t) => say!("{:<40} {t:.1} °C", c.label()),
            None => say!("{:<40} n/a", c.label()),
        }
    }
}

fn print_help() {
    let admin = if cfg!(windows) {
        "an admin terminal"
    } else {
        "sudo"
    };
    println!(
        "Vitals agent v{}  -  github.com/theallmyti/Vitals",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("Usage: vitals <command> [options]");
    println!();
    println!("Everyday");
    println!("  status            Is the agent running? Shows mode, last upload and live readings");
    println!("  pause [MINUTES]   Stop sending data for a while (default 24 h)");
    println!("  resume            Start sending data again");
    println!("  restart           Restart the background service (needs {admin})");
    println!();
    println!("Pairing");
    println!("  pair              Link this computer to your account: shows a QR code and a code");
    println!("                    to enter in the app. Options: --skip-if-paired");
    #[cfg(windows)]
    println!("                    --gui opens the pairing window instead of the terminal");
    println!("  unpair            Remove this computer from your account");
    println!();
    println!("Maintenance");
    println!("  update            Download and install the newest release (needs {admin})");
    println!("  sensors           List the temperatures, clocks and per-core readings found here");
    println!("  version           Print the installed version");
    println!("  help              Show this list");
    println!();
    #[cfg(windows)]
    {
        println!("Started by Windows (you normally don't run these)");
        println!("  service           Run as the VitalsAgent Windows service");
        println!("  tray              Show the system-tray icon");
        println!();
    }
    println!("Examples");
    println!("  vitals status");
    println!("  vitals pause 60          pause for one hour");
    println!("  vitals pair");
    println!();
    println!("With no command, the agent runs in this terminal using a config file path");
    println!("(default config.toml). Press Ctrl+C to stop.");
}

/// True for an argument that can only be a mistyped command, not a config file path.
fn looks_like_unknown_command(arg: &str) -> bool {
    if arg.starts_with('-') {
        return true;
    }
    let path_like = arg.contains('.') || arg.contains('/') || arg.contains('\\');
    !path_like && !Path::new(arg).exists()
}

pub(crate) fn resolve_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let dir = service::data_dir();
        if dir.join("status.json").exists() || dir.join("config.toml").exists() {
            dir
        } else if PathBuf::from("status.json").exists() || PathBuf::from("config.toml").exists() {
            PathBuf::from(".")
        } else {
            dir
        }
    }
    #[cfg(not(windows))]
    {
        let candidates = [
            PathBuf::from("/etc/vitals"),
            PathBuf::from("/var/lib/vitals"),
            // Installed before the rename and auto-updated since (binary swap only).
            PathBuf::from("/etc/nexus"),
            PathBuf::from("/var/lib/nexus"),
            PathBuf::from("/etc/pc-monitor"),
            PathBuf::from("/var/lib/pc-monitor-agent"),
            PathBuf::from("."),
        ];
        for c in &candidates {
            if c.join("status.json").exists() || c.join("config.toml").exists() {
                return c.clone();
            }
        }
        PathBuf::from("/etc/vitals")
    }
}

pub(crate) fn resolve_config_path(explicit: Option<&str>) -> PathBuf {
    if let Some(p) = explicit {
        return PathBuf::from(p);
    }
    let data_dir = resolve_data_dir();
    let cfg = data_dir.join("config.toml");
    if cfg.exists() {
        cfg
    } else if PathBuf::from("config.toml").exists() {
        PathBuf::from("config.toml")
    } else {
        data_dir.join("config.toml")
    }
}

fn print_status() {
    println!("Vitals Agent v{}", env!("CARGO_PKG_VERSION"));
    println!("-----------------------------");

    let dir = resolve_data_dir();

    match control::read_status(&dir) {
        Some(s) => {
            let now = now_secs();
            let age = now.saturating_sub(s.last_tick);
            let state_display = match s.state.as_str() {
                "active" if age > 15 => format!("Stalled (last tick {age}s ago)"),
                "active" => "Active (logging)".to_string(),
                "paused" => match s.paused_until {
                    Some(u) if u > now => format!("Paused (resumes in ~{}m)", (u - now) / 60 + 1),
                    _ => "Paused indefinitely".to_string(),
                },
                "unpaired" => "Unpaired (run `vitals pair`)".to_string(),
                "connecting" => "Connecting...".to_string(),
                "stopped" => "Stopped (service idle)".to_string(),
                other => other.to_string(),
            };
            println!("Status:       {state_display}");
            println!("CPU Usage:    {:.1}%", s.cpu_util);
            println!("RAM Usage:    {:.1}%", s.ram_pct);
            if let Some(t) = s.temp_c {
                println!("CPU Temp:     {t:.1} °C");
            }
            if let Some(u) = s.convex_url {
                println!("Backend:      {u}");
            }
            println!("Last Update:  {age}s ago");
            println!("Data Dir:     {}", dir.display());
        }
        None => {
            println!("Status:       Stopped or initializing (no status file found)");
            println!("Data Dir:     {}", dir.display());
        }
    }
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to start the async runtime")
        .block_on(f)
}

fn main() {
    let first = std::env::args().nth(1);
    match first.as_deref() {
        // Helper mode: read the GPU in a separate process (see gpu.rs).
        Some("gpu") => {
            let every_ms = std::env::args().nth(2).and_then(|v| v.parse().ok());
            exit(gpu::run_helper(every_ms));
        }
        Some("sensors") => {
            print_sensors();
            return;
        }
        Some("status") => {
            print_status();
            return;
        }
        Some("pause") => {
            let data_dir = resolve_data_dir();
            let mins: Option<u64> = std::env::args().nth(2).and_then(|m| m.parse().ok());
            let duration_secs = mins.map(|m| m.saturating_mul(60));
            control::set_pause(&data_dir, duration_secs, "cli");
            match mins {
                Some(m) => println!(
                    "Vitals monitoring paused for {m} minutes. Run `vitals resume` to resume."
                ),
                None => {
                    println!(
                        "Vitals monitoring paused for 24 hours. Run `vitals resume` to resume."
                    )
                }
            }
            return;
        }
        Some("resume") => {
            let data_dir = resolve_data_dir();
            control::clear_pause(&data_dir);
            println!("Vitals monitoring resumed.");
            return;
        }
        Some("restart") => {
            #[cfg(windows)]
            println!("Restarting the Vitals Windows service...");
            #[cfg(not(windows))]
            println!("Restarting the Vitals systemd service...");
            match update::restart_service() {
                Ok(()) => println!("Restart command issued successfully."),
                #[cfg(windows)]
                Err(_) => println!(
                    "Failed to trigger service restart. Ensure you have administrator permissions."
                ),
                #[cfg(not(windows))]
                Err(_) => println!(
                    "Failed to restart service. Run: sudo systemctl restart {}",
                    update::systemd_unit()
                ),
            }
            return;
        }
        Some("update") | Some("upgrade") => {
            let data_dir = resolve_data_dir();
            let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("vitals"));
            let own_version = env!("CARGO_PKG_VERSION");
            println!("Vitals Update Manager");
            println!("-------------------");
            println!("Installed version: v{own_version}");
            println!("Checking GitHub Releases (https://github.com/theallmyti/Vitals)...");

            match update::fetch_latest_release() {
                Ok(release) => {
                    let latest_ver = release.tag_name.trim_start_matches('v');
                    if update::is_newer(latest_ver, own_version) {
                        println!("Found newer release: v{latest_ver}");
                        println!("Downloading release assets and verifying SHA-256 checksums...");
                        match block_on(update::check_and_apply_update(
                            &data_dir,
                            own_version,
                            &current_exe,
                            true,
                        )) {
                            Ok(Some(v)) => {
                                println!("Successfully updated to Vitals v{v}!");
                                println!("Service restart command triggered.");
                            }
                            Ok(None) => println!("Vitals is already up to date."),
                            Err(e) => println!("Update failed: {e}"),
                        }
                    } else {
                        println!("Vitals is already up to date (version v{own_version}).");
                    }
                }
                Err(e) => println!("Update check failed: {e}"),
            }
            return;
        }
        Some("help") | Some("--help") | Some("-h") => {
            print_help();
            return;
        }
        Some("version") | Some("--version") | Some("-V") => {
            println!("vitals {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        // Remove this device from Vitals (run by the uninstaller).
        Some("unpair") => {
            let explicit = std::env::args().nth(2);
            let path = resolve_config_path(explicit.as_deref());
            block_on(pair::unpair(&path));
            return;
        }
        // Show a QR code and wait for the owner to confirm.
        // `pair [--gui] [--skip-if-paired] [--ui <dir>] [config]`
        Some("pair") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            let mut skip_if_paired = false;
            #[cfg(windows)]
            let mut use_gui = false;
            let mut config_arg: Option<String> = None;
            let mut ui_dir: Option<PathBuf> = None;

            let mut iter = rest.into_iter();
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--skip-if-paired" => skip_if_paired = true,
                    #[cfg(windows)]
                    "--gui" => use_gui = true,
                    #[cfg(not(windows))]
                    "--gui" => {
                        println!(
                            "Note: GUI pairing window is for Windows. Running in terminal mode."
                        );
                    }
                    "--ui" => {
                        if let Some(dir) = iter.next() {
                            ui_dir = Some(PathBuf::from(dir));
                        }
                    }
                    other if !other.starts_with("--") => {
                        config_arg = Some(other.to_string());
                    }
                    _ => {}
                }
            }

            let path = resolve_config_path(config_arg.as_deref());

            #[cfg(windows)]
            if use_gui {
                let data_dir = resolve_data_dir();
                pair_gui::launch_pairing_gui(data_dir, path);
                return;
            }

            let ui = match ui_dir {
                Some(dir) => pair::Ui::Files(dir),
                None => pair::Ui::Terminal,
            };

            block_on(pair::run(&path, ui, skip_if_paired));
            return;
        }
        // Started by the Windows Service Control Manager (installed by the setup program).
        #[cfg(windows)]
        Some("service") => {
            if let Err(e) = service::start() {
                warn!("run as a service only: {e}");
                exit(1);
            }
            return;
        }
        // Started in the user desktop session to show taskbar tray icon.
        #[cfg(windows)]
        Some("tray") => {
            tray::run();
            return;
        }
        Some(arg) if looks_like_unknown_command(arg) => {
            eprintln!("Unknown command '{arg}'. Run 'vitals help' to see all commands.");
            exit(2);
        }
        _ => {}
    }
    let path = resolve_config_path(first.as_deref());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to start the async runtime");
    let stop = Arc::new(Notify::new());
    let stop_signal = stop.clone();
    runtime.spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            stop_signal.notify_one();
        }
    });
    // Runs until the process is stopped; the service passes a real stop signal instead.
    runtime.block_on(run(path, stop));
}

/// The agent: load config, register, then sample / summary / live loop until `stop` fires.
pub async fn run(path: PathBuf, stop: Arc<Notify>) {
    let mut config = match config::Config::load(&path) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "{e} (expected at {}; copy config.example.toml)",
                path.display()
            );
            exit(1);
        }
    };
    let secret = match config.ensure_device_secret(&path) {
        Ok(s) => s,
        Err(e) => {
            warn!("{e}");
            exit(1);
        }
    };
    let secret_hash = config::secret_hash(&secret);

    // Readers first, so registration can report capabilities.
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    )
    .expect("failed to read system info");
    // No NVIDIA driver means no GPU stats; the rest of the agent keeps running (NFR-07).
    let mut gpu_reader = gpu::GpuReader::new().await;
    // Windows no-driver mode has no temperature sensors, and listing them goes through WMI,
    // which loads heavy system libraries. Keep an empty list there (Task 16, NFR-01).
    // Windows full mode: CPU temperature through PawnIO (admin). Logs why when unavailable.
    let cpu_temp_reader = cputemp::CpuTemp::open();
    let live_clock = liveclock::LiveClock::open();
    let mut components = if cfg!(windows) {
        Components::new().expect("failed to create sensor list")
    } else {
        Components::new_with_refreshed_list().expect("failed to read sensors")
    };
    let mut disks = Disks::new_with_refreshed_list().ok();
    let mut drives = disks.as_ref().map(collectors::drives).unwrap_or_default();
    // None on machines without a battery; then no battery data is sent.
    let mut battery_reader = battery::BatteryReader::new();
    let mut battery_now: Option<battery::BatteryStats> = None;
    let mut gpu_now: Option<gpu::GpuStats> = None;

    let device_name = System::host_name().unwrap_or_else(|_| "unknown".to_string());
    say!(
        "device={device_name} convex={} gpu={} drives={}",
        config.convex_url,
        if gpu_reader.present() {
            "nvidia"
        } else {
            "none"
        },
        drives.len()
    );

    let data_dir = path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();

    let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("vitals"));
    let own_version = env!("CARGO_PKG_VERSION");
    update::check_startup_health(&data_dir, &current_exe, own_version);

    if config.auto_update {
        update::spawn_background_updater(
            data_dir.clone(),
            own_version.to_string(),
            current_exe.clone(),
            stop.clone(),
        );
    }

    let verify_data_dir = data_dir.clone();
    let verify_exe = current_exe.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        update::mark_verified(&verify_data_dir, &verify_exe);
    });
    control::write_status(
        &data_dir,
        &control::AgentStatus::idle("connecting", Some(config.convex_url.clone())),
    );

    let mut client = backend::connect(&config.convex_url).await;

    // Say hello: the device must be paired (docs/Flow.md §3). Until then, retry every minute,
    // so a service started before pairing picks it up without a restart.
    let mut hello_args = BTreeMap::new();
    hello_args.insert("secretHash".to_string(), text(&secret_hash));
    hello_args.insert(
        "capabilities".to_string(),
        capabilities_value(
            gpu_reader.present(),
            battery_reader.is_some(),
            cpu_temp_reader.available(),
            &components,
            &drives,
            sys.cpus()
                .first()
                .map(|c| c.brand().trim())
                .filter(|b| !b.is_empty()),
        ),
    );
    hello_args.insert("agentVersion".to_string(), text(env!("CARGO_PKG_VERSION")));
    let hello = loop {
        if let Some(answer) = backend::call(&mut client, "agent:hello", hello_args.clone()).await {
            break answer;
        }
        control::write_status(
            &data_dir,
            &control::AgentStatus::idle("unpaired", Some(config.convex_url.clone())),
        );
        say!("not connected yet (see above; if not paired, run `vitals pair`); retrying in 60 s");
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(60)) => {}
            _ = stop.notified() => {
                say!("stopping agent during connection retry");
                control::write_status(
                    &data_dir,
                    &control::AgentStatus::idle("stopped", Some(config.convex_url.clone())),
                );
                return;
            }
        }
    };
    say!("connected as a paired device");
    if let Some(latest) = backend::newer_version(&hello, own_version) {
        say!(
            "update available: version {latest} (this is {own_version}); it installs automatically unless auto_update = false"
        );
    }

    let viewer_until = Arc::new(AtomicU64::new(0));
    backend::watch_viewer(client.clone(), secret_hash.clone(), viewer_until.clone());
    if config.auto_update {
        backend::watch_release(
            client.clone(),
            &secret_hash,
            data_dir.clone(),
            current_exe.clone(),
        );
    }

    // Container counts are fetched every 10 s (they change slowly).
    let mut docker: Option<dashboard::DockerSummary> = None;
    let mut top: Vec<Proc> = Vec::new();

    let boot_time = collectors::boot_time();
    let uptime_init = collectors::uptime_secs().unwrap_or(0);
    let mut usage_tracker = UsageTracker::new(boot_time, uptime_init, now_secs());

    let mut supervisor = Supervisor::new();
    supervisor.register("cpu");
    supervisor.register("memory");
    if disks.is_some() {
        supervisor.register("disks");
    }
    if cpu_temp_reader.available() || !cfg!(windows) {
        supervisor.register("cpuTemp");
    }
    if gpu_reader.present() {
        supervisor.register("gpu");
    }
    if battery_reader.is_some() {
        supervisor.register("battery");
    }
    if config.dashboard_url.is_some() {
        supervisor.register("docker");
    }

    // CPU usage needs two samples separated by a short delay.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut aggregator = Aggregator::default();
    let mut backlog: Backlog<Value> = Backlog::new(BACKLOG_MAX);
    // First summary after 10 s, so a newly paired computer shows data at once; then every 60 s.
    let mut next_summary = Instant::now() + Duration::from_secs(10);
    let mut was_live = false;
    let mut was_paused = false;
    let mut tick: u64 = 0;
    let mut ticker = tokio::time::interval(Duration::from_secs(config.live_interval_secs));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = stop.notified() => {
                say!("stopping agent, performing clean flush...");
                control::write_status(
                    &data_dir,
                    &control::AgentStatus::idle("stopped", Some(config.convex_url.clone())),
                );
                if let Some(s) = aggregator.take() {
                    let col_status = supervisor.status_map();
                    backlog.push(summary_value(
                        now_ms() / 60_000 * 60_000,
                        &s,
                        &drives,
                        &col_status,
                    ));
                }
                if !backlog.is_empty() {
                    let _ = tokio::time::timeout(
                        Duration::from_secs(3),
                        backend::send_backlog(&mut client, &secret_hash, &mut backlog),
                    )
                    .await;
                }
                say!("agent cleanly stopped");
                return;
            }
        }
        tick += 1;
        let now_wall = now_secs();
        let uptime_now = collectors::uptime_secs().unwrap_or(0);

        let pause_ctrl = control::read_control(&data_dir);
        if pause_ctrl.paused {
            if !was_paused {
                say!("monitoring paused by user");
                was_paused = true;
            }
            if tick.is_multiple_of(2) {
                control::write_status(
                    &data_dir,
                    &control::AgentStatus {
                        paused_until: pause_ctrl.until,
                        last_tick: now_wall,
                        ..control::AgentStatus::idle("paused", Some(config.convex_url.clone()))
                    },
                );
            }
            continue;
        } else if was_paused {
            say!("monitoring resumed");
            was_paused = false;
        }

        usage_tracker.tick(now_wall);

        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let cpu_util = sys.global_cpu_usage() as f64;
        if cpu_util.is_nan() {
            supervisor.record_failure("cpu");
        } else {
            supervisor.record_success("cpu");
        }

        let total_mem = sys.total_memory();
        let ram_pct = if total_mem > 0 {
            supervisor.record_success("memory");
            sys.used_memory() as f64 / total_mem as f64 * 100.0
        } else {
            supervisor.record_failure("memory");
            0.0
        };

        let mut cpu_t = None;
        if supervisor.can_run("cpuTemp") {
            if !cfg!(windows) {
                components.refresh(false);
            }
            cpu_t = cpu_temp_reader.read().or_else(|| cpu_temp(&components));
            if cpu_t.is_some() {
                supervisor.record_success("cpuTemp");
            } else if cpu_temp_reader.available() {
                supervisor.record_failure("cpuTemp");
            }
        }

        if tick.is_multiple_of(2) {
            control::write_status(
                &data_dir,
                &control::AgentStatus {
                    cpu_util,
                    ram_pct,
                    temp_c: cpu_t,
                    last_tick: now_wall,
                    ..control::AgentStatus::idle("active", Some(config.convex_url.clone()))
                },
            );
        }

        if let Some(url) = config
            .dashboard_url
            .as_deref()
            .filter(|_| tick % DOCKER_EVERY == 1 && supervisor.can_run("docker"))
        {
            match dashboard::docker_summary(url) {
                Ok(d) => {
                    docker = Some(d);
                    supervisor.record_success("docker");
                }
                Err(e) => {
                    warn!("dashboard read failed: {e}");
                    supervisor.record_failure("docker");
                }
            }
        }

        // Battery changes slowly: read it every 10 s.
        if tick % BATTERY_EVERY == 1
            && supervisor.can_run("battery")
            && let Some(r) = battery_reader.as_mut()
        {
            if let Some(b) = r.read() {
                battery_now = Some(b);
                supervisor.record_success("battery");
            } else {
                supervisor.record_failure("battery");
            }
        }

        let now = now_ms();
        // Live: only while a viewer is present (FR-31).
        let live = viewer_until.load(Ordering::Relaxed) > now;

        // GPU: every second while live; every 30 s otherwise, without keeping NVML loaded.
        let gpu = if gpu_reader.present() {
            if supervisor.can_run("gpu") {
                let g = gpu_reader.read(live).await;
                if g.is_some() {
                    gpu_now = g.clone();
                    supervisor.record_success("gpu");
                    g
                } else {
                    if live && gpu_now.is_none() {
                        supervisor.record_failure("gpu");
                    }
                    gpu_now.clone()
                }
            } else {
                gpu_now.clone()
            }
        } else {
            None
        };

        aggregator.push(Sample {
            cpu_util,
            ram_pct,
            cpu_temp: cpu_t,
            gpu_temp: gpu.as_ref().map(|g| f64::from(g.temp_c)),
            gpu_util: gpu.as_ref().map(|g| f64::from(g.util_pct)),
            battery_pct: battery_now.as_ref().map(|b| b.pct),
        });

        let summary_due = Instant::now() >= next_summary;

        if live != was_live {
            say!(
                "{}",
                if live {
                    "viewer present: live on"
                } else {
                    "no viewer: live off"
                }
            );
            was_live = live;
        }

        // Drives: every 10 s while live, and before each summary.
        if ((live && tick % DRIVES_EVERY == 1) || summary_due)
            && supervisor.can_run("disks")
            && let Some(d) = disks.as_mut()
        {
            d.refresh(true);
            drives = collectors::drives(d);
            if !drives.is_empty() {
                supervisor.record_success("disks");
            } else {
                supervisor.record_failure("disks");
            }
        }

        if live {
            // Processes only while someone is watching, every 5 s (keeps idle CPU low).
            if config.share_activity && (top.is_empty() || tick % PROCESS_EVERY == 1) {
                collectors::refresh_processes(&mut sys);
                top = collectors::top_processes(&sys, 5);
            }
            let (per_core, mut clocks) = collectors::per_core(&sys);
            live_clock.apply(&mut clocks);
            // Per-core temperatures where the chip has them (Intel via PawnIO, Linux coretemp).
            let core_temps = cpu_temp_reader
                .read_cores(per_core.len())
                .or_else(|| collectors::per_cpu_temps(&components, per_core.len()));
            let ram = collectors::ram_detail(&sys);

            let mut args = BTreeMap::new();
            args.insert("secretHash".to_string(), text(&secret_hash));
            args.insert("ts".to_string(), num(now as f64));
            args.insert("cpuUtil".to_string(), num(cpu_util));
            args.insert("ramUsedPct".to_string(), num(ram_pct));
            args.insert("perCoreUtil".to_string(), numbers(&per_core));
            args.insert("clocks".to_string(), numbers(&clocks));
            if let Some(t) = &core_temps {
                args.insert("coreTemps".to_string(), numbers(t));
            }
            args.insert("ram".to_string(), ram_value(&ram));
            if config.share_activity {
                args.insert("topProcs".to_string(), procs_value(&top));
            }
            if !drives.is_empty() {
                args.insert("disks".to_string(), drives_live(&drives));
            }
            if let Some(up) = collectors::uptime_secs() {
                args.insert("uptimeSec".to_string(), num(up as f64));
            }
            if config.share_activity {
                let usage_stats = usage_tracker.stats(uptime_now);
                args.insert("usage".to_string(), usage_value(&usage_stats));
            }
            if let Some(t) = cpu_t {
                args.insert("cpuTemp".to_string(), num(t));
            }
            if let Some(d) = docker {
                args.insert("docker".to_string(), docker_value(&d));
            }
            let effective_gpu = gpu.as_ref().or(gpu_now.as_ref());
            if let Some(g) = effective_gpu {
                args.insert("gpu".to_string(), gpu_value(g));
            }
            if let Some(b) = &battery_now {
                args.insert("battery".to_string(), battery_value(b));
            }
            backend::call(&mut client, "agent:pushLatest", args).await;
        } else if !top.is_empty() {
            // Free the process list when nobody is watching.
            top.clear();
        }

        // Summary: every 60 s, always (FR-30). Queued first, so nothing is lost while offline.
        if summary_due {
            next_summary += SUMMARY_EVERY;
            if let Some(s) = aggregator.take() {
                say!(
                    "summary cpu avg {:.1}% max {:.1}% ram avg {:.1}%",
                    s.cpu_util_avg,
                    s.cpu_util_max,
                    s.ram_pct_avg
                );
                let col_status = supervisor.status_map();
                if backlog.push(summary_value(
                    now / 60_000 * 60_000,
                    &s,
                    &drives,
                    &col_status,
                )) {
                    warn!("offline for over a day: dropped the oldest waiting summary");
                }
            }
            if !backlog.is_empty() {
                backend::send_backlog(&mut client, &secret_hash, &mut backlog).await;
            }
        }
    }
}
