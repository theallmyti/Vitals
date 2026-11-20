//! Pure text work the installer does, kept free of Windows calls so it can be unit tested:
//! the agent config, the machine PATH, the pairing status file and version numbers.

/// The agent config after install. Keeps everything already there (above all `device_secret`,
/// which keeps the computer paired), points it at this build's backend and sets `auto_update`.
/// Port of WriteConfig from the former Inno Setup installer.
pub fn config_toml(existing: Option<&str>, convex_url: &str, auto_update: bool) -> String {
    let url_line = format!("convex_url = \"{convex_url}\"");
    let update_line = format!("auto_update = {auto_update}");
    let Some(existing) = existing else {
        return format!(
            "# Vitals agent config, written by the installer.\n{url_line}\nlive_interval_secs = 1\n{update_line}\n"
        );
    };
    let mut lines: Vec<String> = Vec::new();
    let (mut has_url, mut has_update) = (false, false);
    for line in existing.lines() {
        let key = line.split('=').next().unwrap_or("").trim();
        if key == "convex_url" {
            lines.push(url_line.clone());
            has_url = true;
        } else if key == "auto_update" {
            lines.push(update_line.clone());
            has_update = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !has_url {
        lines.push(url_line);
    }
    if !has_update {
        lines.push(update_line);
    }
    lines.join("\n") + "\n"
}

/// The machine PATH with `dir` added at the end (unchanged if already there, any case).
pub fn path_with(path: &str, dir: &str) -> String {
    if path_entries(path).any(|e| e.eq_ignore_ascii_case(dir)) {
        return path.to_string();
    }
    let trimmed = path.trim_end_matches(';');
    if trimmed.is_empty() {
        dir.to_string()
    } else {
        format!("{trimmed};{dir}")
    }
}

/// The machine PATH with every entry equal to `dir` (any case, trailing slash or not) removed.
pub fn path_without(path: &str, dir: &str) -> String {
    let dir = dir.trim_end_matches('\\');
    path.split(';')
        .filter(|e| !e.trim_end_matches('\\').eq_ignore_ascii_case(dir))
        .collect::<Vec<_>>()
        .join(";")
}

fn path_entries(path: &str) -> impl Iterator<Item = &str> {
    path.split(';')
        .map(|e| e.trim_end_matches('\\'))
        .filter(|e| !e.is_empty())
}

/// One reading of the agent's `pair --ui` status file (agent/src/pair.rs, Ui::status).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PairStatus {
    /// connecting | pending | paired | error
    pub state: String,
    /// The 6-character code without its display space, e.g. "ABC234".
    pub code: String,
    /// Seconds until the code expires.
    pub left: u64,
    pub message: String,
}

pub fn parse_pair_status(text: &str) -> PairStatus {
    let mut s = PairStatus::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "state" => s.state = value.trim().to_string(),
            "code" => {
                s.code = value
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .collect()
            }
            "left" => s.left = value.trim().parse().unwrap_or(0),
            "message" => s.message = value.trim().to_string(),
            _ => {}
        }
    }
    s
}

/// The link in the pairing QR code, the same one the agent builds (agent/src/pair.rs,
/// pairing_url): a phone camera opens it, and that page opens the app or downloads it.
pub fn pairing_url(convex_url: &str, code: &str) -> String {
    let base = convex_url
        .trim_end_matches('/')
        .replace(".convex.cloud", ".convex.site");
    format!("{base}/pair?code={code}")
}

/// Compares "x.y.z" versions; anything unreadable counts as 0.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|p| p.trim().parse().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parts(a), parts(b));
    for i in 0..a.len().max(b.len()) {
        let ord = a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0));
        if ord.is_ne() {
            return ord;
        }
    }
    std::cmp::Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering::*;

    const URL: &str = "https://tidy-newt-65.convex.cloud";

    #[test]
    fn new_config_has_the_backend_and_update_choice() {
        let c = config_toml(None, URL, false);
        assert!(c.contains("convex_url = \"https://tidy-newt-65.convex.cloud\""));
        assert!(c.contains("live_interval_secs = 1"));
        assert!(c.contains("auto_update = false"));
    }

    #[test]
    fn upgrade_keeps_the_device_secret_and_other_settings() {
        let old = "convex_url = \"https://old.convex.cloud\"\nlive_interval_secs = 2\ndevice_secret = \"s3cret\"\n";
        let c = config_toml(Some(old), URL, true);
        assert!(
            c.contains("device_secret = \"s3cret\""),
            "losing it would unpair the computer"
        );
        assert!(c.contains("live_interval_secs = 2"));
        assert!(c.contains(&format!("convex_url = \"{URL}\"")));
        assert!(!c.contains("old.convex.cloud"));
        assert_eq!(c.matches("auto_update").count(), 1);
    }

    #[test]
    fn auto_update_is_replaced_not_duplicated() {
        let c = config_toml(Some("convex_url = \"x\"\nauto_update = true\n"), URL, false);
        assert_eq!(c.matches("auto_update").count(), 1);
        assert!(c.contains("auto_update = false"));
    }

    #[test]
    fn path_add_is_idempotent_and_remove_is_exact() {
        let dir = r"C:\Program Files\Vitals";
        let p = path_with(r"C:\Windows;C:\Tools;", dir);
        assert_eq!(p, r"C:\Windows;C:\Tools;C:\Program Files\Vitals");
        assert_eq!(
            path_with(&p, &dir.to_uppercase()),
            p,
            "already there, any case"
        );
        assert_eq!(path_with("", dir), dir);
        let back = path_without(&format!("{p};C:\\Program Files\\Vitals\\"), dir);
        assert_eq!(back, r"C:\Windows;C:\Tools");
        assert_eq!(
            path_without(r"C:\Program Files\VitalsExtra;C:\x", dir),
            r"C:\Program Files\VitalsExtra;C:\x"
        );
    }

    #[test]
    fn reads_the_agent_status_file() {
        let s = parse_pair_status("state=pending\ncode=ABC 234\nleft=588\nmessage=Waiting\n");
        assert_eq!(
            s,
            PairStatus {
                state: "pending".into(),
                code: "ABC234".into(),
                left: 588,
                message: "Waiting".into()
            }
        );
        assert_eq!(
            parse_pair_status("state=paired\ncode=\nleft=0\nmessage=\n").state,
            "paired"
        );
        assert_eq!(parse_pair_status("garbage").state, "");
    }

    #[test]
    fn pairing_url_matches_the_agent() {
        assert_eq!(
            pairing_url("https://tidy-newt-65.convex.cloud/", "ABC234"),
            "https://tidy-newt-65.convex.site/pair?code=ABC234"
        );
    }

    #[test]
    fn versions_compare_numerically() {
        assert_eq!(compare_versions("1.0.10", "1.0.9"), Greater);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Equal);
        assert_eq!(compare_versions("0.2.5", "1.0.0"), Less);
        assert_eq!(compare_versions("1.0", "1.0.0"), Equal);
    }
}
