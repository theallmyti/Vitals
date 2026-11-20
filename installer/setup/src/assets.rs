//! The installer's screens, embedded in the exe and served to the WebView at
//! `http://vitals.localhost/<path>` (wry's custom protocol on Windows).
//!
//! `installer/ui/index.html` is written as a page fragment (it is also the design prototype,
//! published as an artifact), so it is wrapped here in a full document. Inside the real window the
//! pixel font comes from the embedded file instead of Google Fonts, so setup works offline.

use std::borrow::Cow;

const INDEX: &str = include_str!("../../ui/index.html");
const FONT: &[u8] = include_bytes!("../../ui/fonts/PressStart2P.ttf");
/// QR Code Generator for JavaScript 1.4.4 (MIT): draws the pairing link as a themed QR code.
const QR_JS: &[u8] = include_bytes!("../../ui/vendor/qrcode.js");
/// The app logo, shown in the middle of the pairing QR code.
const LOGO: &[u8] = include_bytes!("../../ui/logo.png");

const SOUNDS: [(&str, &[u8]); 8] = [
    ("click", include_bytes!("../../ui/sounds/click.wav")),
    ("toggle", include_bytes!("../../ui/sounds/toggle.wav")),
    ("tick", include_bytes!("../../ui/sounds/tick.wav")),
    ("start", include_bytes!("../../ui/sounds/start.wav")),
    ("paired", include_bytes!("../../ui/sounds/paired.wav")),
    ("complete", include_bytes!("../../ui/sounds/complete.wav")),
    (
        "uninstalled",
        include_bytes!("../../ui/sounds/uninstalled.wav"),
    ),
    ("error", include_bytes!("../../ui/sounds/error.wav")),
];

/// The page as a full document: standards mode, the local font, and the real-window flag set before
/// any of the page's own script runs.
pub fn page() -> String {
    let body = INDEX
        .lines()
        .filter(|l| !l.contains("fonts.googleapis.com"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<!doctype html><html lang=\"en\" class=\"real\"><head><meta charset=\"utf-8\">\
         <style>@font-face{{font-family:\"Press Start 2P\";src:url(fonts/PressStart2P.ttf) format(\"truetype\");}}</style>\
         <script>window.VITALS_REAL=true;</script></head><body>{body}</body></html>"
    )
}

/// The file at `path` and its content type, or `None` for an unknown path.
pub fn get(path: &str) -> Option<(Cow<'static, [u8]>, &'static str)> {
    let path = path.trim_start_matches('/');
    if path.is_empty() || path == "index.html" {
        return Some((Cow::Owned(page().into_bytes()), "text/html; charset=utf-8"));
    }
    match path {
        "fonts/PressStart2P.ttf" => return Some((Cow::Borrowed(FONT), "font/ttf")),
        "vendor/qrcode.js" => return Some((Cow::Borrowed(QR_JS), "text/javascript")),
        "logo.png" => return Some((Cow::Borrowed(LOGO), "image/png")),
        _ => {}
    }
    let name = path.strip_prefix("sounds/")?.strip_suffix(".wav")?;
    SOUNDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, bytes)| (Cow::Borrowed(*bytes), "audio/wav"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_page_font_and_sounds() {
        assert!(get("/").is_some());
        assert!(get("/index.html").unwrap().1.starts_with("text/html"));
        assert_eq!(get("/fonts/PressStart2P.ttf").unwrap().1, "font/ttf");
        assert_eq!(get("/vendor/qrcode.js").unwrap().1, "text/javascript");
        assert_eq!(get("/logo.png").unwrap().1, "image/png");
        for (name, _) in SOUNDS {
            assert_eq!(get(&format!("/sounds/{name}.wav")).unwrap().1, "audio/wav");
        }
        assert!(get("/sounds/missing.wav").is_none());
        assert!(get("/../Cargo.toml").is_none());
    }

    #[test]
    fn page_is_a_full_offline_document() {
        let page = page();
        assert!(page.starts_with("<!doctype html>"));
        assert!(page.contains("window.VITALS_REAL=true"));
        assert!(
            !page.contains("fonts.googleapis.com"),
            "setup must not need the internet for its font"
        );
    }
}
