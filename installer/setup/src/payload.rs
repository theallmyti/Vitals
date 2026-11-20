//! What the installer puts on the computer, embedded in the exe at build time (as the Inno setup's
//! [Files] section did). Build the agent first: `cargo build --release` in agent/.

/// The backend this build talks to, from installer/convex-url.txt (read by build.rs; that file
/// is kept out of the repo, and CI writes it from the CONVEX_URL secret).
pub const CONVEX_URL: &str = env!("VITALS_CONVEX_URL");

/// This installer's version (Cargo.toml), shown in the window and written to Apps & features.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const AGENT_EXE: &[u8] = include_bytes!("../../../agent/target/release/vitals.exe");
pub const PAWNIO_SETUP: &[u8] = include_bytes!("../../vendor/pawnio/PawnIO_setup.exe");

/// (path inside the install folder, contents)
pub const FILES: &[(&str, &[u8])] = &[
    ("LICENSE.txt", include_bytes!("../../../LICENSE")),
    (
        "docs/LEGAL_AGREEMENT.txt",
        include_bytes!("../../LEGAL_AGREEMENT.txt"),
    ),
    ("docs/LEGAL.md", include_bytes!("../../../LEGAL.md")),
    ("docs/TERMS.md", include_bytes!("../../../docs/TERMS.md")),
    (
        "docs/PRIVACY.md",
        include_bytes!("../../../docs/PRIVACY.md"),
    ),
    (
        "docs/COOKIE_POLICY.md",
        include_bytes!("../../../docs/COOKIE_POLICY.md"),
    ),
    // Signed PawnIO modules (LGPL-2.1, github.com/namazso/PawnIO.Modules), fetched by
    // installer/fetch-pawnio.ps1: CPU temperatures on Intel and AMD.
    (
        "pawnio/AMDFamily17.bin",
        include_bytes!("../../vendor/pawnio/AMDFamily17.bin"),
    ),
    (
        "pawnio/IntelMSR.bin",
        include_bytes!("../../vendor/pawnio/IntelMSR.bin"),
    ),
    (
        "pawnio/RyzenSMU.bin",
        include_bytes!("../../vendor/pawnio/RyzenSMU.bin"),
    ),
    (
        "pawnio/COPYING-PawnIO-Modules.txt",
        include_bytes!("../../vendor/pawnio/COPYING"),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_is_complete() {
        assert!(CONVEX_URL.starts_with("https://") && CONVEX_URL.ends_with(".convex.cloud"));
        assert_eq!(
            &AGENT_EXE[..2],
            b"MZ",
            "the agent must be a Windows program"
        );
        assert_eq!(&PAWNIO_SETUP[..2], b"MZ");
        assert!(FILES.iter().all(|(_, bytes)| !bytes.is_empty()));
    }
}
