use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::Config;
use crate::filesystem::FileClass;

pub fn split_command(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match quote {
            Some(q) => {
                if ch == '\\' && q == '"' {
                    escaped = true;
                } else if ch == q {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => match ch {
                '\\' => escaped = true,
                '"' | '\'' => quote = Some(ch),
                c if c.is_whitespace() => {
                    if !current.is_empty() {
                        parts.push(std::mem::take(&mut current));
                    }
                }
                c => current.push(c),
            },
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

fn detached(cmd: &mut Command) -> &mut Command {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
}

fn spawn_error(program: &str, err: std::io::Error) -> String {
    if err.kind() == std::io::ErrorKind::NotFound {
        format!("'{program}' is not installed")
    } else {
        format!("{program}: {err}")
    }
}

pub fn open_editor(path: &Path, config: &Config) -> Result<(), String> {
    let parts = split_command(&config.editor());
    let Some(program) = parts.first() else {
        return Err("no editor configured".into());
    };
    let status = Command::new(program)
        .args(&parts[1..])
        .arg(path)
        .status()
        .map_err(|e| spawn_error(program, e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("editor exited with {status}"))
    }
}

pub fn open_file(path: &Path, class: FileClass, config: &Config) -> Result<(), String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let configured = if class == FileClass::Document && ext != "pdf" {
        None
    } else {
        config.opener_for(class)
    };
    if let Some(app) = configured {
        let parts = split_command(&app);
        if let Some(program) = parts.first() {
            detached(Command::new(program).args(&parts[1..]).arg(path))
                .spawn()
                .map_err(|e| spawn_error(program, e))?;
            return Ok(());
        }
    }
    detached(Command::new("xdg-open").arg(path))
        .spawn()
        .map_err(|e| spawn_error("xdg-open", e))?;
    Ok(())
}

pub fn run_shell(command: &str, dir: &Path, config: &Config) -> Result<(), String> {
    let shell = config.shell();
    let status = Command::new(&shell)
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .status()
        .map_err(|e| spawn_error(&shell, e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command exited with {status}"))
    }
}

pub fn open_terminal(dir: &Path, config: &Config) -> Result<(), String> {
    if !config.apps.terminal.trim().is_empty() {
        let parts = split_command(&config.apps.terminal);
        if let Some(program) = parts.first() {
            if detached(Command::new(program).args(&parts[1..]).current_dir(dir))
                .spawn()
                .is_ok()
            {
                return Ok(());
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let candidates: [(&str, &[&str]); 6] = [
            ("kitty", &["--directory"]),
            ("alacritty", &["--working-directory"]),
            ("konsole", &["--workdir"]),
            ("gnome-terminal", &["--working-directory"]),
            ("xfce4-terminal", &["--working-directory"]),
            ("xterm", &[]),
        ];
        for (program, flags) in candidates {
            let mut cmd = Command::new(program);
            for flag in flags {
                cmd.arg(flag);
            }
            if !flags.is_empty() {
                cmd.arg(dir);
            }
            if detached(&mut cmd).current_dir(dir).spawn().is_ok() {
                return Ok(());
            }
        }
        return Err("no terminal emulator found".into());
    }

    #[cfg(target_os = "macos")]
    {
        detached(Command::new("open").arg("-a").arg("Terminal").arg(dir))
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        detached(Command::new("cmd").args(["/K", "cd", "/D"]).arg(dir))
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("unsupported platform".into())
}

pub fn set_title(title: &str) {
    use std::io::Write;
    let clean: String = title.chars().filter(|c| !c.is_control()).collect();
    let mut stdout = std::io::stdout().lock();
    let _ = write!(stdout, "\x1b]2;{}\x07", clean);
    let _ = stdout.flush();
}

pub fn share_kdeconnect(paths: &[PathBuf]) -> Result<String, String> {
    if paths.is_empty() {
        return Err("nothing selected".into());
    }
    let output = Command::new("kdeconnect-cli")
        .arg("-l")
        .output()
        .map_err(|_| "kdeconnect-cli not found — install kdeconnect".to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    for line in stdout.lines() {
        if !line.contains("reachable") {
            continue;
        }
        if let Some(pos) = line.find(": ") {
            let rest = &line[pos + 2..];
            let end = rest
                .find(" on ")
                .or_else(|| rest.find(" ("))
                .unwrap_or(rest.len());
            let id = rest[..end].trim();
            if !id.is_empty() {
                devices.push(id.to_string());
            }
        }
    }
    if devices.is_empty() {
        return Err("no reachable KDE Connect devices".into());
    }
    for device in &devices {
        for path in paths {
            let _ = detached(
                Command::new("kdeconnect-cli")
                    .arg("--share")
                    .arg(path)
                    .arg("--device")
                    .arg(device),
            )
            .spawn();
        }
    }
    Ok(format!(
        "shared {} to {} device(s)",
        crate::ops::describe_paths(paths),
        devices.len()
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

impl GitStatus {
    pub fn marker(self) -> char {
        match self {
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Untracked => '?',
            Self::Conflicted => 'U',
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::Conflicted => 6,
            Self::Modified => 5,
            Self::Added => 4,
            Self::Deleted => 3,
            Self::Renamed => 2,
            Self::Untracked => 1,
        }
    }

    fn from_char(c: char) -> Option<Self> {
        match c {
            'M' => Some(Self::Modified),
            'A' => Some(Self::Added),
            'D' => Some(Self::Deleted),
            'R' | 'C' => Some(Self::Renamed),
            'U' => Some(Self::Conflicted),
            '?' => Some(Self::Untracked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub branch: String,
}

#[derive(Debug, Clone)]
pub struct GitData {
    pub repo: RepoInfo,
    pub status: HashMap<String, GitStatus>,
}

pub fn git_data(dir: &Path) -> Option<GitData> {
    let root_output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--show-toplevel"])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !root_output.status.success() {
        return None;
    }
    let root = PathBuf::from(String::from_utf8_lossy(&root_output.stdout).trim());
    if root.as_os_str().is_empty() {
        return None;
    }
    let branch = git_branch(&root);

    let canonical_dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let canonical_root = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    let relative = canonical_dir
        .strip_prefix(&canonical_root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let pathspec = if relative.is_empty() {
        ".".to_string()
    } else {
        relative.clone()
    };

    let status_output = Command::new("git")
        .arg("-C")
        .arg(&canonical_root)
        .args([
            "status",
            "--porcelain",
            "-z",
            "--no-renames",
            "--untracked-files=normal",
            "--",
            &pathspec,
        ])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let status = if status_output.status.success() {
        parse_status(&status_output.stdout, &relative)
    } else {
        HashMap::new()
    };

    Some(GitData {
        repo: RepoInfo { branch },
        status,
    })
}

fn git_branch(root: &Path) -> String {
    let symbolic = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["symbolic-ref", "--short", "HEAD"])
        .stdin(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|branch| !branch.is_empty());
    if let Some(branch) = symbolic {
        return branch;
    }
    let detached = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--short", "HEAD"])
        .stdin(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|hash| !hash.is_empty());
    detached.unwrap_or_else(|| "HEAD".into())
}

fn parse_status(raw: &[u8], relative: &str) -> HashMap<String, GitStatus> {
    let mut map: HashMap<String, GitStatus> = HashMap::new();
    for record in raw.split(|b| *b == 0) {
        if record.len() < 4 {
            continue;
        }
        let text = String::from_utf8_lossy(record);
        let (flags, path) = text.split_at(3);
        let mut path = path.trim_start_matches('/');
        if !relative.is_empty() {
            let prefix = format!("{relative}/");
            let Some(stripped) = path.strip_prefix(&prefix) else {
                continue;
            };
            path = stripped;
        }
        let Some(first) = path.split('/').next() else {
            continue;
        };
        if first.is_empty() {
            continue;
        }
        let mut best: Option<GitStatus> = None;
        for c in flags.chars() {
            if let Some(status) = GitStatus::from_char(c) {
                best = match best {
                    Some(current) if current.priority() >= status.priority() => Some(current),
                    _ => Some(status),
                };
            }
        }
        if let Some(status) = best {
            match map.get(first) {
                Some(existing) if existing.priority() >= status.priority() => {}
                _ => {
                    map.insert(first.to_string(), status);
                }
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_commands() {
        assert_eq!(
            split_command("code --wait"),
            vec!["code".to_string(), "--wait".to_string()]
        );
        assert_eq!(
            split_command(r#""/opt/My App/bin" --flag 'a b'"#),
            vec![
                "/opt/My App/bin".to_string(),
                "--flag".to_string(),
                "a b".to_string()
            ]
        );
    }

    #[test]
    fn parses_git_status_records() {
        let raw = b" M sub/a.txt\0?? sub/new.txt\0A  top.txt\0";
        let map = parse_status(raw, "sub");
        assert_eq!(map.get("a.txt"), Some(&GitStatus::Modified));
        assert_eq!(map.get("new.txt"), Some(&GitStatus::Untracked));
        assert_eq!(map.get("top.txt"), None);
    }

    #[test]
    fn parses_status_at_root() {
        let raw = b"?? top.txt\0 M nested/file\0";
        let map = parse_status(raw, "");
        assert_eq!(map.get("top.txt"), Some(&GitStatus::Untracked));
        assert_eq!(map.get("nested"), Some(&GitStatus::Modified));
    }
}
