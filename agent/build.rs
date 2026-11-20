//! Embeds the Vitals icons into vitals.exe (Windows only): the app icon for Start and search, and
//! the tray icons (src/tray.rs loads them by id).

fn main() {
    println!("cargo:rerun-if-changed=vitals.rc");
    println!("cargo:rerun-if-changed=icons");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("vitals.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
