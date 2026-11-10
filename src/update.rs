use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::UpdateMsg;

const REPO: &str = "Fel-2/riffle";
const CACHE_TTL: u64 = 24 * 60 * 60;

pub fn spawn_check(tx: Sender<UpdateMsg>, manual: bool) {
    std::thread::spawn(move || {
        let result = fetch_latest();
        let _ = tx.send(UpdateMsg { manual, result });
    });
}

fn fetch_latest() -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let output = match Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "6",
            "-H",
            "User-Agent: riffle-update-check",
            "-H",
            "Accept: application/vnd.github+json",
            &url,
        ])
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            match Command::new("wget")
                .args([
                    "-qO-",
                    "--timeout=6",
                    "--header=User-Agent: riffle-update-check",
                    &url,
                ])
                .stdin(Stdio::null())
                .output()
            {
                Ok(output) => output,
                Err(_) => return Err("curl or wget is required".into()),
            }
        }
        Err(err) => return Err(format!("curl: {err}")),
    };
    if !output.status.success() {
        return Err("could not reach GitHub".into());
    }
    let body = String::from_utf8_lossy(&output.stdout);
    parse_tag(&body).ok_or_else(|| "could not parse release metadata".into())
}

fn parse_tag(json: &str) -> Option<String> {
    let key = "\"tag_name\"";
    let start = json.find(key)? + key.len();
    let rest = &json[start..];
    let colon = rest.find(':')?;
    let rest = rest[colon + 1..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let tag = &rest[..end];
    if tag.is_empty() {
        None
    } else {
        Some(tag.to_string())
    }
}

pub fn latest_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(current), Some(latest)) => latest > current,
        _ => false,
    }
}

fn parse_version(version: &str) -> Option<[u64; 3]> {
    let core = version.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next()?;
    let mut out = [0u64; 3];
    let mut count = 0;
    for part in core.split('.') {
        if count == 3 {
            break;
        }
        out[count] = part.parse().ok()?;
        count += 1;
    }
    if count == 0 {
        None
    } else {
        Some(out)
    }
}

fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("riffle").join("update-check"))
}

pub fn read_cache() -> Option<(String, u64)> {
    let path = cache_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let latest = lines.next()?.trim().to_string();
    let timestamp = lines.next()?.trim().parse().ok()?;
    if latest.is_empty() {
        None
    } else {
        Some((latest, timestamp))
    }
}

pub fn write_cache(latest: &str) {
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, format!("{latest}\n{}\n", unix_now()));
}

pub fn cache_is_fresh(timestamp: u64) -> bool {
    unix_now().saturating_sub(timestamp) < CACHE_TTL
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_release_tag() {
        let json = r#"{"url":"x","tag_name":"v0.3.0","name":"x"}"#;
        assert_eq!(parse_tag(json), Some("v0.3.0".into()));
        assert_eq!(parse_tag("{}"), None);
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(latest_newer("0.2.0", "v0.2.1"));
        assert!(latest_newer("0.2.0", "v0.3.0"));
        assert!(latest_newer("0.9.9", "v1.0.0"));
        assert!(!latest_newer("0.2.1", "v0.2.1"));
        assert!(!latest_newer("0.3.0", "v0.2.9"));
        assert!(!latest_newer("0.2", "v0.2.0"));
    }
}
