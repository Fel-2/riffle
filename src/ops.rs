use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use chrono::Local;

use crate::utils::url_encode;

fn io_err(context: &str, err: std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::PermissionDenied => format!("{context}: permission denied"),
        std::io::ErrorKind::NotFound => format!("{context}: not found"),
        _ => format!("{context}: {err}"),
    }
}

pub fn delete_path(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| io_err("delete", e))?;
    if meta.is_dir() {
        fs::remove_dir_all(path).map_err(|e| io_err("delete", e))
    } else {
        fs::remove_file(path).map_err(|e| io_err("delete", e))
    }
}

pub fn create_file(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let name = validate_name(name)?;
    let path = dir.join(name);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                format!("'{name}' already exists")
            } else {
                io_err("create", e)
            }
        })?;
    Ok(path)
}

pub fn create_dir(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let name = validate_name(name)?;
    let path = dir.join(name);
    fs::create_dir(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            format!("'{name}' already exists")
        } else {
            io_err("create directory", e)
        }
    })?;
    Ok(path)
}

pub fn rename_path(old: &Path, new_name: &str) -> Result<PathBuf, String> {
    let new_name = validate_name(new_name)?;
    let parent = old.parent().ok_or("path has no parent")?;
    let new = parent.join(new_name);
    if new == old {
        return Ok(new);
    }
    if new.exists() {
        return Err(format!("'{new_name}' already exists"));
    }
    fs::rename(old, &new).map_err(|e| io_err("rename", e))?;
    Ok(new)
}

fn validate_name(name: &str) -> Result<&str, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("name is empty".into());
    }
    if trimmed == "." || trimmed == ".." {
        return Err("invalid name".into());
    }
    if trimmed.contains('/') || trimmed.contains('\0') {
        return Err("name cannot contain '/'".into());
    }
    Ok(trimmed)
}

pub fn unique_dest(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let mut dest = dir.join(name);
    if !dest.exists() {
        return dest;
    }
    let name = name.to_string_lossy();
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (name.to_string(), String::new()),
    };
    let mut i = 1u32;
    loop {
        let candidate = if i == 1 {
            format!("{stem} (copy){ext}")
        } else {
            format!("{stem} (copy {i}){ext}")
        };
        dest = dir.join(candidate);
        if !dest.exists() {
            return dest;
        }
        i += 1;
    }
}

pub fn copy_path(from: &Path, to: &Path) -> Result<(), String> {
    if from.is_dir() && to.starts_with(from) {
        return Err("cannot copy a directory into itself".into());
    }
    copy_entry(from, to).map_err(|e| io_err("copy", e))
}

fn copy_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(from)?;
    if meta.file_type().is_symlink() {
        let target = fs::read_link(from)?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, to)?;
        }
        #[cfg(not(unix))]
        {
            let resolved = from.parent().map(|p| p.join(&target)).unwrap_or(target);
            if resolved.is_dir() {
                fs::create_dir_all(to)?;
            } else {
                fs::copy(&resolved, to)?;
            }
        }
        return Ok(());
    }
    if meta.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_entry(&entry.path(), &to.join(entry.file_name()))?;
        }
        let _ = fs::set_permissions(to, meta.permissions());
        Ok(())
    } else {
        fs::copy(from, to)?;
        Ok(())
    }
}

pub fn move_path(from: &Path, to: &Path) -> Result<(), String> {
    if from.is_dir() && to.starts_with(from) {
        return Err("cannot move a directory into itself".into());
    }
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::CrossesDevices => {
            copy_path(from, to)?;
            delete_path(from)
        }
        Err(err) => Err(io_err("move", err)),
    }
}

pub fn trash_root() -> Result<PathBuf, String> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
        .ok_or("cannot determine data directory")?;
    Ok(data.join("Trash"))
}

#[derive(Debug, Clone)]
pub struct Trashed {
    pub original: PathBuf,
    pub trashed: PathBuf,
    pub info: PathBuf,
}

pub fn trash_file(path: &Path) -> Result<Trashed, String> {
    let root = trash_root()?;
    trash_file_in(path, &root)
}

fn trash_file_in(path: &Path, root: &Path) -> Result<Trashed, String> {
    let files = root.join("files");
    let info_dir = root.join("info");
    fs::create_dir_all(&files).map_err(|e| io_err("trash", e))?;
    fs::create_dir_all(&info_dir).map_err(|e| io_err("trash", e))?;

    let name = path
        .file_name()
        .ok_or("path has no file name")?
        .to_string_lossy()
        .into_owned();
    let mut dest = files.join(&name);
    let mut counter = 1u32;
    while dest.exists()
        || info_dir
            .join(format!(
                "{}.trashinfo",
                dest.file_name().unwrap().to_string_lossy()
            ))
            .exists()
    {
        dest = files.join(format!("{name}.{counter}"));
        counter += 1;
    }

    if let Err(err) = fs::rename(path, &dest) {
        if err.kind() == std::io::ErrorKind::CrossesDevices {
            copy_path(path, &dest)?;
            delete_path(path)?;
        } else {
            return Err(io_err("trash", err));
        }
    }

    let info = info_dir.join(format!(
        "{}.trashinfo",
        dest.file_name().unwrap().to_string_lossy()
    ));
    let body = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        url_encode(&path.to_string_lossy()),
        Local::now().format("%Y-%m-%dT%H:%M:%S")
    );
    if let Err(err) = fs::write(&info, body) {
        let _ = fs::rename(&dest, path);
        return Err(io_err("trash metadata", err));
    }

    Ok(Trashed {
        original: path.to_path_buf(),
        trashed: dest,
        info,
    })
}

pub fn restore_trashed(item: &Trashed) -> Result<(), String> {
    if item.original.exists() {
        return Err(format!("'{}' already exists", item.original.display()));
    }
    if let Some(parent) = item.original.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err("restore", e))?;
    }
    move_path(&item.trashed, &item.original)?;
    let _ = fs::remove_file(&item.info);
    Ok(())
}

pub fn extract_archive(path: &Path) -> Result<PathBuf, String> {
    let name = path
        .file_name()
        .ok_or("archive has no name")?
        .to_string_lossy()
        .to_ascii_lowercase();
    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = archive_stem(&name);
    let dest = unique_dest(parent, std::ffi::OsStr::new(&stem));
    fs::create_dir(&dest).map_err(|e| io_err("extract", e))?;

    let result = extract_into(path, &name, &dest);
    if let Err(err) = result {
        let _ = fs::remove_dir_all(&dest);
        return Err(err);
    }
    Ok(dest)
}

fn extract_into(path: &Path, name: &str, dest: &Path) -> Result<(), String> {
    let run = |program: &str, args: &[&std::ffi::OsStr]| -> Result<(), String> {
        let output = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    format!("'{program}' is not installed")
                } else {
                    format!("{program}: {e}")
                }
            })?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.lines().last().unwrap_or("").trim();
            Err(if detail.is_empty() {
                format!("{program} failed")
            } else {
                format!("{program}: {detail}")
            })
        }
    };

    if name.ends_with(".tar.gz")
        || name.ends_with(".tgz")
        || name.ends_with(".tar.bz2")
        || name.ends_with(".tbz2")
        || name.ends_with(".tar.xz")
        || name.ends_with(".txz")
        || name.ends_with(".tar.zst")
        || name.ends_with(".tar")
    {
        return run(
            "tar",
            &[
                std::ffi::OsStr::new("-xf"),
                path.as_os_str(),
                std::ffi::OsStr::new("-C"),
                dest.as_os_str(),
            ],
        );
    }
    if name.ends_with(".zip") || name.ends_with(".jar") {
        return run(
            "unzip",
            &[
                std::ffi::OsStr::new("-q"),
                std::ffi::OsStr::new("-o"),
                path.as_os_str(),
                std::ffi::OsStr::new("-d"),
                dest.as_os_str(),
            ],
        );
    }
    if name.ends_with(".7z") {
        let out_arg = format!("-o{}", dest.display());
        return run(
            "7z",
            &[
                std::ffi::OsStr::new("x"),
                std::ffi::OsStr::new("-y"),
                std::ffi::OsStr::new(&out_arg),
                path.as_os_str(),
            ],
        );
    }
    if name.ends_with(".rar") {
        return run(
            "unrar",
            &[
                std::ffi::OsStr::new("x"),
                std::ffi::OsStr::new("-o+"),
                path.as_os_str(),
                dest.as_os_str(),
            ],
        );
    }

    let (program, flag) = if name.ends_with(".gz") {
        ("gunzip", "-c")
    } else if name.ends_with(".bz2") {
        ("bunzip2", "-c")
    } else if name.ends_with(".xz") {
        ("unxz", "-c")
    } else {
        return Err(format!("unsupported archive: {name}"));
    };
    let output = Command::new(program)
        .arg(flag)
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("'{program}' is not installed")
            } else {
                format!("{program}: {e}")
            }
        })?;
    if !output.status.success() {
        return Err(format!("{program} failed"));
    }
    let out_name = stem_without_compression(name);
    fs::write(dest.join(out_name), output.stdout).map_err(|e| io_err("extract", e))
}

fn archive_stem(name: &str) -> String {
    for suffix in [
        ".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tgz", ".tbz2", ".txz", ".zip", ".7z",
        ".rar", ".tar", ".gz", ".bz2", ".xz",
    ] {
        if let Some(stem) = name.strip_suffix(suffix) {
            if !stem.is_empty() {
                return stem.to_string();
            }
        }
    }
    name.to_string()
}

fn stem_without_compression(name: &str) -> String {
    for suffix in [".gz", ".bz2", ".xz"] {
        if let Some(stem) = name.strip_suffix(suffix) {
            if !stem.is_empty() {
                return stem.to_string();
            }
        }
    }
    name.to_string()
}

pub fn compress(paths: &[PathBuf]) -> Result<PathBuf, String> {
    let first = paths.first().ok_or("nothing selected")?;
    let parent = first.parent().ok_or("path has no parent")?.to_path_buf();
    let base = if paths.len() == 1 {
        let name = first.file_name().unwrap_or_default().to_string_lossy();
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem.to_string(),
            _ => name.into_owned(),
        }
    } else {
        parent
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "archive".into())
    };

    let names: Vec<String> = paths
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .map(|n| format!("./{n}"))
        .collect();
    if names.is_empty() {
        return Err("nothing selected".into());
    }

    let zip_path = unique_dest(&parent, std::ffi::OsStr::new(&format!("{base}.zip")));
    let zip_ok = Command::new("zip")
        .arg("-r")
        .arg("-q")
        .arg(&zip_path)
        .args(&names)
        .current_dir(&parent)
        .stdin(Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if zip_ok {
        return Ok(zip_path);
    }
    let _ = fs::remove_file(&zip_path);

    let tar_path = unique_dest(&parent, std::ffi::OsStr::new(&format!("{base}.tar.gz")));
    let output = Command::new("tar")
        .arg("-czf")
        .arg(&tar_path)
        .args(&names)
        .current_dir(&parent)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "neither zip nor tar is installed".to_string()
            } else {
                format!("tar: {e}")
            }
        })?;
    if output.status.success() {
        Ok(tar_path)
    } else {
        let _ = fs::remove_file(&tar_path);
        Err("could not create archive".into())
    }
}

pub fn describe_paths(paths: &[PathBuf]) -> String {
    match paths {
        [] => "nothing".into(),
        [one] => one
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| one.display().to_string()),
        many => format!("{} items", many.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, b"data").unwrap();
        path
    }

    #[test]
    fn unique_dest_names() {
        let t = tmp();
        assert_eq!(
            unique_dest(t.path(), "a.txt".as_ref()),
            t.path().join("a.txt")
        );
        touch(t.path(), "a.txt");
        assert_eq!(
            unique_dest(t.path(), "a.txt".as_ref()),
            t.path().join("a (copy).txt")
        );
        touch(t.path(), "a (copy).txt");
        assert_eq!(
            unique_dest(t.path(), "a.txt".as_ref()),
            t.path().join("a (copy 2).txt")
        );
        touch(t.path(), ".bashrc");
        assert_eq!(
            unique_dest(t.path(), ".bashrc".as_ref()),
            t.path().join(".bashrc (copy)")
        );
    }

    #[test]
    fn create_refuses_to_overwrite() {
        let t = tmp();
        touch(t.path(), "exists.txt");
        assert!(create_file(t.path(), "exists.txt").is_err());
        assert_eq!(fs::read(t.path().join("exists.txt")).unwrap(), b"data");
    }

    #[test]
    fn rename_refuses_to_overwrite() {
        let t = tmp();
        let a = touch(t.path(), "a.txt");
        touch(t.path(), "b.txt");
        assert!(rename_path(&a, "b.txt").is_err());
        assert!(a.exists());
    }

    #[test]
    fn delete_symlink_does_not_touch_target() {
        let t = tmp();
        let target = touch(t.path(), "target.txt");
        let link = t.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        delete_path(&link).unwrap();
        assert!(!link.exists());
        assert!(target.exists());
    }

    #[test]
    fn copy_preserves_symlinks() {
        let t = tmp();
        let target = touch(t.path(), "target.txt");
        let link = t.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let dest = t.path().join("copied-link.txt");
        copy_path(&link, &dest).unwrap();
        let meta = fs::symlink_metadata(&dest).unwrap();
        assert!(meta.file_type().is_symlink());
        assert_eq!(fs::read_link(&dest).unwrap(), target);
    }

    #[test]
    fn trash_writes_info_and_restores() {
        let t = tmp();
        let root = t.path().join("Trash");
        let file = touch(t.path(), "file.txt");
        let trashed = trash_file_in(&file, &root).unwrap();
        assert!(!file.exists());
        assert!(trashed.trashed.exists());
        let info = fs::read_to_string(&trashed.info).unwrap();
        assert!(info.contains("[Trash Info]"));
        assert!(info.contains("Path="));
        restore_trashed(&trashed).unwrap();
        assert!(file.exists());
        assert!(!trashed.info.exists());
    }

    #[test]
    fn trash_conflicts_get_suffix() {
        let t = tmp();
        let root = t.path().join("Trash");
        let a = touch(t.path(), "file.txt");
        let first = trash_file_in(&a, &root).unwrap();
        let b = touch(t.path(), "file.txt");
        let second = trash_file_in(&b, &root).unwrap();
        assert_ne!(first.trashed, second.trashed);
    }

    #[test]
    fn move_and_copy_directories() {
        let t = tmp();
        let src = t.path().join("dir");
        fs::create_dir_all(src.join("nested")).unwrap();
        touch(&src, "a.txt");
        touch(&src.join("nested"), "b.txt");
        let dst = t.path().join("copy");
        copy_path(&src, &dst).unwrap();
        assert!(dst.join("nested/b.txt").is_file());
        let moved = t.path().join("moved");
        move_path(&dst, &moved).unwrap();
        assert!(moved.join("a.txt").is_file());
        assert!(!dst.exists());
    }

    #[test]
    fn refuses_copy_into_self() {
        let t = tmp();
        let src = t.path().join("dir");
        fs::create_dir(&src).unwrap();
        assert!(copy_path(&src, &src.join("inner")).is_err());
    }

    #[test]
    fn invalid_names_rejected() {
        let t = tmp();
        assert!(create_file(t.path(), "").is_err());
        assert!(create_file(t.path(), "a/b").is_err());
        assert!(create_file(t.path(), "..").is_err());
    }

    #[test]
    fn archive_stems() {
        assert_eq!(archive_stem("foo.tar.gz"), "foo");
        assert_eq!(archive_stem("foo.zip"), "foo");
        assert_eq!(archive_stem("foo.txt"), "foo.txt");
    }
}
