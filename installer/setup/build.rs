//! Embeds the icon, the admin manifest and the version details into vitals-setup.exe, and rebuilds
//! when the screens or sounds in installer/ui change (they are included with include_bytes!).

fn main() {
    println!("cargo:rerun-if-changed=setup.rc");
    println!("cargo:rerun-if-changed=setup.manifest");
    println!("cargo:rerun-if-changed=vitals.ico");
    println!("cargo:rerun-if-changed=../ui");
    // The payload (src/payload.rs): rebuild when the agent or the backend address changes.
    println!("cargo:rerun-if-changed=../../agent/target/release/vitals.exe");
    println!("cargo:rerun-if-changed=../convex-url.txt");

    let url = std::fs::read_to_string("../convex-url.txt")
        .expect("installer/convex-url.txt is missing: put the production Convex URL in it (CI writes it from the CONVEX_URL secret)");
    let url = url.trim();
    assert!(
        url.starts_with("https://") && url.ends_with(".convex.cloud"),
        "installer/convex-url.txt must hold an https://<name>.convex.cloud URL"
    );
    println!("cargo:rustc-env=VITALS_CONVEX_URL={url}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = env!("CARGO_PKG_VERSION");
        let parts: Vec<&str> = version.split('.').collect();
        let defines = [
            format!("VERSION_MAJOR={}", parts[0]),
            format!("VERSION_MINOR={}", parts[1]),
            format!("VERSION_PATCH={}", parts[2]),
            format!("VERSION_STR=\"{version}\""),
        ];
        // Only the setup binary: the test runner must not get the admin manifest, or `cargo test`
        // fails with "requires elevation" (os error 740). Required, not optional: without the
        // manifest Windows would not ask for admin rights.
        embed_resource::compile_for("setup.rc", ["vitals-setup"], &defines)
            .manifest_required()
            .unwrap();
    }
}
