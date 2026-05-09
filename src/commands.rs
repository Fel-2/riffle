use std::path::{Path, PathBuf};
use std::process::Command;
use crate::config::Config;

/// Known text extensions that should be opened in an editor
fn is_text_extension(ext: &str) -> bool {
    matches!(ext,
        "rs"|"js"|"ts"|"py"|"go"|"java"|"c"|"cpp"|"h"|"css"|"html"
        |"json"|"toml"|"yaml"|"yml"|"sh"|"txt"|"md"|"conf"|"cfg"|"ini"|"log"
        |"xml"|"sql"|"rb"|"php"|"lua"|"vim"|"el"|"lisp"|"hs"|"ml"|"ex"|"exs"
        |"dockerfile"|"makefile"|"gitignore"|"env"|"bashrc"|"zshrc"
    )
}

pub fn open_file(path: &PathBuf, config: &Config) -> Result<(), String> {
    let ext = path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if is_text_extension(&ext) {
        return open_in_editor(path, config);
    }

    // Check config for a specific opener for this extension
    if let Some(app) = config.opener_for_ext(&ext) {
        Command::new(&app)
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to launch '{}': {}", app, e))?;
        return Ok(());
    }

    // Fallback: xdg-open
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn open_in_editor(path: &Path, config: &Config) -> Result<(), String> {
    let editor = config.editor();
    let status = Command::new(&editor)
        .arg(path)
        .status()
        .map_err(|e| format!("Failed to launch editor '{}': {}", editor, e))?;
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

pub fn trash_file(path: &Path) -> Result<(), String> {
    let trash_dir = dirs::home_dir()
        .ok_or("Cannot find home directory")?
        .join(".local/share/Trash/files");

    std::fs::create_dir_all(&trash_dir).map_err(|e| e.to_string())?;

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

    Ok(())
}

pub fn extract_archive(path: &Path) -> Result<(), String> {
    let parent = path.parent().unwrap_or(Path::new("."));

    // Determine extension chain
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();

    let status = if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Command::new("tar")
            .arg("-xzf")
            .arg(path)
            .current_dir(parent)
            .status()
    } else if name.ends_with(".tar.bz2") {
        Command::new("tar")
            .arg("-xjf")
            .arg(path)
            .current_dir(parent)
            .status()
    } else if name.ends_with(".tar.xz") {
        Command::new("tar")
            .arg("-xJf")
            .arg(path)
            .current_dir(parent)
            .status()
    } else if name.ends_with(".tar") {
        Command::new("tar")
            .arg("-xf")
            .arg(path)
            .current_dir(parent)
            .status()
    } else if name.ends_with(".zip") {
        Command::new("unzip")
            .arg("-d")
            .arg(parent)
            .arg(path)
            .status()
    } else if name.ends_with(".7z") {
        Command::new("7z")
            .arg("x")
            .arg(path)
            .current_dir(parent)
            .status()
    } else if name.ends_with(".gz") {
        Command::new("gunzip")
            .arg(path)
            .current_dir(parent)
            .status()
    } else if name.ends_with(".bz2") {
        Command::new("bunzip2")
            .arg(path)
            .current_dir(parent)
            .status()
    } else if name.ends_with(".xz") {
        Command::new("unxz")
            .arg(path)
            .current_dir(parent)
            .status()
    } else if name.ends_with(".rar") {
        Command::new("unrar")
            .arg("x")
            .arg(path)
            .current_dir(parent)
            .status()
    } else {
        return Err(format!("Unknown archive format: {}", name));
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("Archive extraction failed with status: {}", s)),
        Err(e) => Err(format!("Failed to run extraction command: {}", e)),
    }
}

pub fn open_terminal(path: &PathBuf, config: &Config) -> Result<(), String> {
    // If user configured a specific terminal, try it first
    if !config.apps.terminal.is_empty() {
        if Command::new(&config.apps.terminal).current_dir(path).spawn().is_ok() {
            return Ok(());
        }
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
                if Command::new(term).current_dir(path).spawn().is_ok() {
                    return Ok(());
                }
            } else if Command::new(term).arg(flag).arg(path).spawn().is_ok() {
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
            if Command::new(term).current_dir(path).spawn().is_ok() {
                return Ok(());
            }
        }

        return Err("Could not find a supported terminal emulator".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-a")
            .arg("Terminal")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/K", "cd", "/D", &path.to_string_lossy()])
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
            let _ = Command::new("kdeconnect-cli")
                .arg("--share")
                .arg(path)
                .arg("--device")
                .arg(id)
                .spawn();
        }
    }

    Ok(format!("Shared {} file(s) to {} device(s)", n_files, n_devices))
}

pub fn compress_zip(path: &Path) -> Result<(), String> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let zip_name = format!("{}.zip", name);
    let parent = path.parent().unwrap_or(Path::new("."));
    let output = Command::new("zip")
        .arg("-r")
        .arg(&zip_name)
        .arg(path.file_name().unwrap())
        .current_dir(parent)
        .output();

    if output.is_err() || !output.unwrap().status.success() {
        let tar_name = format!("{}.tar.gz", name);
        Command::new("tar")
            .arg("-czf")
            .arg(&tar_name)
            .arg(path.file_name().unwrap())
            .current_dir(parent)
            .spawn()
            .map_err(|e| format!("Both zip and tar failed. Zip error: {}", e))?;
    }
    Ok(())
}
