//! Windows service mode (`vitals service`), started by the Service Control Manager.
//! The installer creates the service; SCM restarts it after a crash (NFR-06).
//! Config and log live in %ProgramData%\Vitals (docs/Security.md §6: owner-only ACL set by the installer).
//! API: windows-service 0.8 (define_windows_service!, service_dispatcher::start,
//! service_control_handler::register, set_service_status).

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};

pub const NAME: &str = "VitalsAgent";
/// Service names of earlier releases. An agent auto-updated from one of them keeps running under
/// that service (the updater only swaps the binary) until the Vitals installer migrates it. For an
/// own-process service the SCM ignores the name passed to the dispatcher, so NAME still works there.
pub const LEGACY_NAMES: [&str; 2] = ["NexusAgent", "PCMonitorAgent"];

define_windows_service!(ffi_service_main, service_main);

/// Hand control to the Service Control Manager. Only works when started as a service.
pub fn start() -> windows_service::Result<()> {
    service_dispatcher::start(NAME, ffi_service_main)
}

/// %ProgramData%\Vitals: config.toml and agent.log (or a pre-rename folder, see data_dir_in).
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    data_dir_in(&base)
}

/// Which folder under `base` (%ProgramData%) holds this install's config.toml.
/// Three generations exist: Vitals (now), Nexus (<= 0.2.5) and PCMonitor (before that).
/// An agent auto-updated from Nexus still runs from the Nexus folders, with its device secret
/// in `Nexus\config.toml`, until the Vitals installer moves it into `Vitals`.
fn data_dir_in(base: &std::path::Path) -> PathBuf {
    // Newest generation first; look for config.toml, since an uninstaller can leave an empty folder.
    ["Vitals", "Nexus", "PCMonitor"]
        .iter()
        .map(|dir| base.join(dir))
        .find(|dir| dir.join("config.toml").exists())
        .unwrap_or_else(|| base.join("Vitals"))
}

fn status(state: ServiceState) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: if state == ServiceState::Running {
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
        } else {
            ServiceControlAccept::empty()
        },
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }
}

fn service_main(_args: Vec<OsString>) {
    if let Err(e) = run() {
        crate::warn!("service error: {e}");
    }
}

fn run() -> windows_service::Result<()> {
    let stop = Arc::new(Notify::new());
    let on_stop = stop.clone();
    let handle = service_control_handler::register(NAME, move |control| match control {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            on_stop.notify_one();
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    })?;
    handle.set_service_status(status(ServiceState::Running))?;

    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = crate::logging::to_file(&dir.join("agent.log"));
    crate::say!("service started (version {})", env!("CARGO_PKG_VERSION"));

    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt.block_on(crate::run(dir.join("config.toml"), stop)),
        Err(e) => crate::warn!("cannot start runtime: {e}"),
    }

    crate::say!("service stopped");
    handle.set_service_status(status(ServiceState::Stopped))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::data_dir_in;
    use std::fs;
    use std::path::PathBuf;

    /// A fresh fake %ProgramData% with config.toml in each of `with_config`.
    fn base(name: &str, with_config: &[&str]) -> PathBuf {
        let base = std::env::temp_dir().join(format!("vitals_dd_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        for dir in with_config {
            fs::create_dir_all(base.join(dir)).unwrap();
            fs::write(base.join(dir).join("config.toml"), "").unwrap();
        }
        base
    }

    #[test]
    fn fresh_install_uses_vitals() {
        let b = base("fresh", &[]);
        assert_eq!(data_dir_in(&b), b.join("Vitals"));
    }

    #[test]
    fn auto_updated_from_nexus_keeps_its_config() {
        let b = base("nexus", &["Nexus"]);
        assert_eq!(data_dir_in(&b), b.join("Nexus"));
    }

    #[test]
    fn pc_monitor_install_keeps_its_config() {
        let b = base("pcm", &["PCMonitor"]);
        assert_eq!(data_dir_in(&b), b.join("PCMonitor"));
    }

    #[test]
    fn migrated_install_prefers_vitals() {
        let b = base("migrated", &["Vitals", "Nexus", "PCMonitor"]);
        assert_eq!(data_dir_in(&b), b.join("Vitals"));
    }

    #[test]
    fn nexus_wins_over_pc_monitor() {
        let b = base("both_old", &["Nexus", "PCMonitor"]);
        assert_eq!(data_dir_in(&b), b.join("Nexus"));
    }
}
