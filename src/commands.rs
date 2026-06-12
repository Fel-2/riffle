use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use crate::config::Config;

/// Detach a spawned child from the TUI: any stdout/stderr it produces would
/// otherwise be written straight over the ratatui screen.
fn detached(cmd: &mut Command) -> &mut Command {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
}

/// Map a spawn error to a friendlier message when the binary is missing.
fn spawn_err(tool: &str, e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        format!("'{}' not installed", tool)
    } else {
        format!("Failed to run '{}': {}", tool, e)
    }
}

/// Known text extensions that should be opened in an editor
fn is_text_extension(ext: &str) -> bool {
    matches!(ext,
        "rs"|"js"|"ts"|"py"|"go"|"java"|"c"|"cpp"|"h"|"css"|"html"
        |"json"|"toml"|"yaml"|"yml"|"sh"|"txt"|"md"|"conf"|"cfg"|"ini"|"log"
        |"xml"|"sql"|"rb"|"php"|"lua"|"vim"|"el"|"lisp"|"hs"|"ml"|"ex"|"exs"
        |"dockerfile"|"makefile"|"gitignore"|"env"|"bashrc"|"zshrc"
    )
}

/// Text files must go through the editor, which needs the terminal —
/// the caller has to suspend the TUI first (see `LoopAction::OpenEditor`).
pub fn wants_editor(path: &Path) -> bool {
    let ext = path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    is_text_extension(&ext)
}

/// Open a non-text file with its configured app or xdg-open.
/// Text files are the caller's job (route through `wants_editor`).
pub fn open_file(path: &PathBuf, config: &Config) -> Result<(), String> {
    let ext = path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // Check config for a specific opener for this extension
    if let Some(app) = config.opener_for_ext(&ext) {
        detached(Command::new(&app).arg(path))
            .spawn()
            .map_err(|e| spawn_err(&app, e))?;
        return Ok(());
    }

    // Fallback: xdg-open
    detached(Command::new("xdg-open").arg(path))
        .spawn()
        .map_err(|e| spawn_err("xdg-open", e))?;
    Ok(())
}

pub fn open_in_editor(path: &Path, config: &Config) -> Result<(), String> {
    let editor = config.editor();
    let status = Command::new(&editor)
        .arg(path)
        .status()
        .map_err(|e| spawn_err(&editor, e))?;
    if !status.success() {
        return Err(format!("Editor exited with status: {}", status));
    }
    Ok(())
}

pub fn run_shell_cmd(cmd: &str, dir: &Path) -> Result<(), String> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .status()
        .map_err(|e| format!("Failed to run shell command: {}", e))?;
    if !status.success() {
        return Err(format!("Command exited with status: {}", status));
    }
    Ok(())
}

/// Move `path` to the trash. Returns the path it was moved to (for undo).
pub fn trash_file(path: &Path) -> Result<PathBuf, String> {
    let trash_dir = dirs::home_dir()
        .ok_or("Cannot find home directory")?
        .join(".local/share/Trash/files");
    trash_file_to(path, &trash_dir)
}

/// Implementation of `trash_file` with the trash directory injected, so tests
/// don't have to touch the real `~/.local/share/Trash`.
fn trash_file_to(path: &Path, trash_dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(trash_dir).map_err(|e| e.to_string())?;

    let file_name = path.file_name()
        .ok_or("File has no name")?
        .to_string_lossy()
        .into_owned();

    let mut dest = trash_dir.join(&file_name);

    // Handle conflicts
    if dest.exists() {
        let mut i = 1u32;
        loop {
            dest = trash_dir.join(format!("{}.{}", file_name, i));
            if !dest.exists() { break; }
            i += 1;
        }
    }

    if let Err(e) = std::fs::rename(path, &dest) {
        // Cross-device: copy then delete
        if e.kind() == std::io::ErrorKind::CrossesDevices {
            if path.is_dir() {
                copy_dir_all(&path.to_path_buf(), &dest)
                    .and_then(|_| std::fs::remove_dir_all(path))
                    .map_err(|e| e.to_string())?;
            } else {
                std::fs::copy(path, &dest)
                    .and_then(|_| std::fs::remove_file(path))
                    .map(|_| ())
                    .map_err(|e| e.to_string())?;
            }
        } else {
            return Err(e.to_string());
        }
    }

    Ok(dest)
}

pub fn extract_archive(path: &Path) -> Result<(), String> {
    let parent = path.parent().unwrap_or(Path::new("."));

    // Determine extension chain
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();

    let (tool, args): (&str, Vec<&std::ffi::OsStr>) = if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        ("tar", vec!["-xzf".as_ref(), path.as_os_str()])
    } else if name.ends_with(".tar.bz2") {
        ("tar", vec!["-xjf".as_ref(), path.as_os_str()])
    } else if name.ends_with(".tar.xz") {
        ("tar", vec!["-xJf".as_ref(), path.as_os_str()])
    } else if name.ends_with(".tar") {
        ("tar", vec!["-xf".as_ref(), path.as_os_str()])
    } else if name.ends_with(".zip") {
        ("unzip", vec!["-d".as_ref(), parent.as_os_str(), path.as_os_str()])
    } else if name.ends_with(".7z") {
        ("7z", vec!["x".as_ref(), "-y".as_ref(), path.as_os_str()])
    } else if name.ends_with(".gz") {
        ("gunzip", vec![path.as_os_str()])
    } else if name.ends_with(".bz2") {
        ("bunzip2", vec![path.as_os_str()])
    } else if name.ends_with(".xz") {
        ("unxz", vec![path.as_os_str()])
    } else if name.ends_with(".rar") {
        ("unrar", vec!["x".as_ref(), path.as_os_str()])
    } else {
        return Err(format!("Unknown archive format: {}", name));
    };

    // .output() captures stdout/stderr — .status() would let the tool
    // print over the TUI.
    let out = Command::new(tool)
        .args(&args)
        .current_dir(parent)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| spawn_err(tool, e))?;

    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let detail = stderr.lines().last().unwrap_or("").trim();
        if detail.is_empty() {
            Err(format!("{} failed with status {}", tool, out.status))
        } else {
            Err(format!("{}: {}", tool, detail))
        }
    }
}

pub fn open_terminal(path: &PathBuf, config: &Config) -> Result<(), String> {
    // If user configured a specific terminal, try it first
    if !config.apps.terminal.is_empty()
        && detached(Command::new(&config.apps.terminal).current_dir(path)).spawn().is_ok() {
            return Ok(());
        }

    #[cfg(target_os = "linux")]
    {
        let configs = [
            ("konsole", "--workdir"),
            ("gnome-terminal", "--working-directory"),
            ("alacritty", "--working-directory"),
            ("kitty", "--directory"),
            ("xfce4-terminal", "--working-directory"),
            ("xterm", "-e"),
        ];

        for (term, flag) in configs {
            if term == "xterm" {
                if detached(Command::new(term).current_dir(path)).spawn().is_ok() {
                    return Ok(());
                }
            } else if detached(Command::new(term).arg(flag).arg(path)).spawn().is_ok() {
                return Ok(());
            }
        }

        let fallback_terms = [
            "konsole",
            "gnome-terminal",
            "alacritty",
            "kitty",
            "xterm",
            "bash",
        ];
        for term in fallback_terms {
            if detached(Command::new(term).current_dir(path)).spawn().is_ok() {
                return Ok(());
            }
        }

        return Err("Could not find a supported terminal emulator".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        detached(Command::new("open").arg("-a").arg("Terminal").arg(path))
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        detached(Command::new("cmd").args(["/K", "cd", "/D", &path.to_string_lossy()]))
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Unsupported operating system".to_string())
}

pub fn rename_file(old_path: &PathBuf, new_name: &str) -> Result<PathBuf, String> {
    let parent = old_path.parent().ok_or("No parent directory")?;
    let new_path = parent.join(new_name);
    std::fs::rename(old_path, &new_path).map_err(|e| e.to_string())?;
    Ok(new_path)
}

pub fn delete_file(path: &PathBuf) -> Result<(), String> {
    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        std::fs::remove_file(path).map_err(|e| e.to_string())
    }
}

pub fn create_directory(path: &Path, name: &str) -> Result<PathBuf, String> {
    let new_dir = path.join(name);
    std::fs::create_dir_all(&new_dir).map_err(|e| e.to_string())?;
    Ok(new_dir)
}

pub fn create_file(path: &Path, name: &str) -> Result<PathBuf, String> {
    let new_file = path.join(name);
    std::fs::File::create(&new_file).map_err(|e| e.to_string())?;
    Ok(new_file)
}

/// Return a non-colliding path in `dir` for `name`, inserting " (copy)" /
/// " (copy N)" before the extension when needed. Never overwrites.
pub fn unique_dest(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let mut dest = dir.join(name);
    if !dest.exists() {
        return dest;
    }
    let name = name.to_string_lossy();
    let (stem, ext) = match name.rsplit_once('.') {
        // Hidden files like ".bashrc" have no real extension
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{}", e)),
        _ => (name.to_string(), String::new()),
    };
    let mut i = 1u32;
    loop {
        let candidate = if i == 1 {
            format!("{} (copy){}", stem, ext)
        } else {
            format!("{} (copy {}){}", stem, i, ext)
        };
        dest = dir.join(candidate);
        if !dest.exists() {
            return dest;
        }
        i += 1;
    }
}

pub fn copy_file(from: &PathBuf, to: &PathBuf) -> Result<(), String> {
    if from.is_dir() {
        copy_dir_all(from, to).map_err(|e| e.to_string())
    } else {
        std::fs::copy(from, to).map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn copy_dir_all(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

pub fn move_file(from: &PathBuf, to: &PathBuf) -> Result<(), String> {
    std::fs::rename(from, to).map_err(|e| {
        if e.kind() == std::io::ErrorKind::CrossesDevices {
            copy_file(from, to)
                .and_then(|_| delete_file(from))
                .err()
                .unwrap_or(e.to_string())
        } else {
            e.to_string()
        }
    })
}

pub fn share_kdeconnect(paths: &[PathBuf]) -> Result<String, String> {
    let out = Command::new("kdeconnect-cli")
        .arg("-l")
        .output()
        .map_err(|_| "kdeconnect-cli not found — install kdeconnect".to_string())?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut device_ids: Vec<String> = Vec::new();
    for line in stdout.lines() {
        // Format: "- Device Name: <id> on <ip> via LAN (paired and reachable)"
        if line.contains("reachable") {
            if let Some(colon_pos) = line.find(": ") {
                let rest = &line[colon_pos + 2..];
                let id_end = rest.find(" on ").or_else(|| rest.find(" (")).unwrap_or(rest.len());
                let id = rest[..id_end].trim().to_string();
                if !id.is_empty() {
                    device_ids.push(id);
                }
            }
        }
    }

    if device_ids.is_empty() {
        return Err("No KDE Connect devices reachable".to_string());
    }

    let n_devices = device_ids.len();
    let n_files = paths.len();
    for id in &device_ids {
        for path in paths {
            let _ = detached(
                Command::new("kdeconnect-cli")
                    .arg("--share")
                    .arg(path)
                    .arg("--device")
                    .arg(id),
            )
            .spawn();
        }
    }

    Ok(format!("Shared {} file(s) to {} device(s)", n_files, n_devices))
}

pub fn compress_zip(path: &Path) -> Result<(), String> {
    let file_name = path.file_name().ok_or("File has no name")?;
    let name = file_name.to_string_lossy();
    let parent = path.parent().unwrap_or(Path::new("."));

    let zip_name = format!("{}.zip", name);
    let zip_out = Command::new("zip")
        .arg("-r")
        .arg(&zip_name)
        .arg(file_name)
        .current_dir(parent)
        .stdin(Stdio::null())
        .output();
    if matches!(&zip_out, Ok(o) if o.status.success()) {
        return Ok(());
    }

    // Fall back to tar.gz; .output() so nothing prints over the TUI
    let tar_name = format!("{}.tar.gz", name);
    let tar_out = Command::new("tar")
        .arg("-czf")
        .arg(&tar_name)
        .arg(file_name)
        .current_dir(parent)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| spawn_err("tar", e))?;
    if tar_out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "zip and tar both failed: {}",
            String::from_utf8_lossy(&tar_out.stderr).lines().last().unwrap_or("").trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, b"data").unwrap();
        p
    }

    // ── unique_dest ─────────────────────────────────────────────────────────

    #[test]
    fn unique_dest_no_conflict_keeps_name() {
        let t = tmp();
        let d = unique_dest(t.path(), "a.txt".as_ref());
        assert_eq!(d, t.path().join("a.txt"));
    }

    #[test]
    fn unique_dest_conflict_appends_copy() {
        let t = tmp();
        touch(t.path(), "a.txt");
        let d = unique_dest(t.path(), "a.txt".as_ref());
        assert_eq!(d, t.path().join("a (copy).txt"));
    }

    #[test]
    fn unique_dest_second_conflict_counts() {
        let t = tmp();
        touch(t.path(), "a.txt");
        touch(t.path(), "a (copy).txt");
        let d = unique_dest(t.path(), "a.txt".as_ref());
        assert_eq!(d, t.path().join("a (copy 2).txt"));
    }

    #[test]
    fn unique_dest_dotfile_has_no_extension() {
        let t = tmp();
        touch(t.path(), ".bashrc");
        let d = unique_dest(t.path(), ".bashrc".as_ref());
        assert_eq!(d, t.path().join(".bashrc (copy)"));
    }

    #[test]
    fn unique_dest_no_extension() {
        let t = tmp();
        touch(t.path(), "Makefile");
        let d = unique_dest(t.path(), "Makefile".as_ref());
        assert_eq!(d, t.path().join("Makefile (copy)"));
    }

    // ── rename / create / delete ────────────────────────────────────────────

    #[test]
    fn rename_file_moves_within_parent() {
        let t = tmp();
        let old = touch(t.path(), "old.txt");
        let new = rename_file(&old, "new.txt").unwrap();
        assert!(!old.exists());
        assert_eq!(new, t.path().join("new.txt"));
        assert_eq!(fs::read(&new).unwrap(), b"data");
    }

    #[test]
    fn create_file_and_directory() {
        let t = tmp();
        let f = create_file(t.path(), "f.txt").unwrap();
        assert!(f.is_file());
        let d = create_directory(t.path(), "sub").unwrap();
        assert!(d.is_dir());
    }

    #[test]
    fn delete_file_removes_file_and_dir() {
        let t = tmp();
        let f = touch(t.path(), "f.txt");
        delete_file(&f).unwrap();
        assert!(!f.exists());

        let d = t.path().join("sub");
        fs::create_dir(&d).unwrap();
        touch(&d, "inner.txt");
        delete_file(&d).unwrap();
        assert!(!d.exists());
    }

    // ── copy / move ─────────────────────────────────────────────────────────

    #[test]
    fn copy_file_copies_contents() {
        let t = tmp();
        let src = touch(t.path(), "src.txt");
        let dst = t.path().join("dst.txt");
        copy_file(&src, &dst).unwrap();
        assert!(src.exists());
        assert_eq!(fs::read(&dst).unwrap(), b"data");
    }

    #[test]
    fn copy_file_copies_directory_recursively() {
        let t = tmp();
        let src = t.path().join("dir");
        fs::create_dir_all(src.join("nested")).unwrap();
        touch(&src, "a.txt");
        touch(&src.join("nested"), "b.txt");

        let dst = t.path().join("dir2");
        copy_file(&src, &dst).unwrap();
        assert!(dst.join("a.txt").is_file());
        assert!(dst.join("nested/b.txt").is_file());
    }

    #[test]
    fn move_file_renames() {
        let t = tmp();
        let src = touch(t.path(), "src.txt");
        let dst = t.path().join("dst.txt");
        move_file(&src, &dst).unwrap();
        assert!(!src.exists());
        assert_eq!(fs::read(&dst).unwrap(), b"data");
    }

    // ── trash ───────────────────────────────────────────────────────────────

    #[test]
    fn trash_moves_file_into_trash_dir() {
        let t = tmp();
        let trash = t.path().join("trash");
        let f = touch(t.path(), "f.txt");
        let dest = trash_file_to(&f, &trash).unwrap();
        assert!(!f.exists());
        assert_eq!(dest, trash.join("f.txt"));
        assert!(dest.is_file());
    }

    #[test]
    fn trash_conflict_appends_counter() {
        let t = tmp();
        let trash = t.path().join("trash");
        let f1 = touch(t.path(), "f.txt");
        trash_file_to(&f1, &trash).unwrap();
        let f2 = touch(t.path(), "f.txt");
        let dest = trash_file_to(&f2, &trash).unwrap();
        assert_eq!(dest, trash.join("f.txt.1"));
        assert!(dest.is_file());
    }

    #[test]
    fn trash_handles_directories() {
        let t = tmp();
        let trash = t.path().join("trash");
        let d = t.path().join("dir");
        fs::create_dir(&d).unwrap();
        touch(&d, "inner.txt");
        let dest = trash_file_to(&d, &trash).unwrap();
        assert!(!d.exists());
        assert!(dest.join("inner.txt").is_file());
    }

    // ── routing ─────────────────────────────────────────────────────────────

    #[test]
    fn text_files_want_editor_binaries_do_not() {
        assert!(wants_editor(Path::new("/x/main.rs")));
        assert!(wants_editor(Path::new("/x/notes.md")));
        assert!(!wants_editor(Path::new("/x/photo.png")));
        assert!(!wants_editor(Path::new("/x/movie.mp4")));
    }
}
