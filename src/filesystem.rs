use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileClass {
    Directory,
    Text,
    Code,
    Config,
    Document,
    Image,
    Video,
    Audio,
    Archive,
    Font,
    Executable,
    Other,
}

impl FileClass {
    pub fn label(self) -> &'static str {
        match self {
            Self::Directory => "Directory",
            Self::Text => "Text",
            Self::Code => "Source code",
            Self::Config => "Configuration",
            Self::Document => "Document",
            Self::Image => "Image",
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Archive => "Archive",
            Self::Font => "Font",
            Self::Executable => "Executable",
            Self::Other => "File",
        }
    }

    pub fn is_text(self) -> bool {
        matches!(self, Self::Text | Self::Code | Self::Config)
    }
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub broken: bool,
    pub symlink_target: Option<PathBuf>,
    pub hidden: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub mode: u32,
    pub class: FileClass,
    pub ext: String,
}

impl FileEntry {
    #[cfg(test)]
    pub fn from_path(path: &Path) -> Option<Self> {
        let link_meta = fs::symlink_metadata(path).ok()?;
        Self::build(path, link_meta)
    }

    fn from_dir_entry(entry: &fs::DirEntry) -> Option<Self> {
        let link_meta = entry.metadata().ok()?;
        Self::build(&entry.path(), link_meta)
    }

    fn build(path: &Path, link_meta: fs::Metadata) -> Option<Self> {
        let is_symlink = link_meta.file_type().is_symlink();
        let symlink_target = if is_symlink {
            fs::read_link(path).ok()
        } else {
            None
        };
        let (meta, broken) = if is_symlink {
            match fs::metadata(path) {
                Ok(m) => (m, false),
                Err(_) => (link_meta, true),
            }
        } else {
            (link_meta, false)
        };

        let name = path.file_name()?.to_string_lossy().into_owned();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let class = detect_class(path, &name, &ext, &meta);
        let hidden = name.starts_with('.');

        Some(Self {
            path: path.to_path_buf(),
            name,
            is_dir: meta.is_dir(),
            is_symlink,
            broken,
            symlink_target,
            hidden,
            size: meta.len(),
            modified: meta.modified().ok(),
            mode: file_mode(&meta),
            class,
            ext,
        })
    }

    pub fn icon(&self) -> &'static str {
        if self.broken {
            return "\u{f127}";
        }
        if self.is_symlink {
            return "\u{f0c1}";
        }
        if self.is_dir {
            return "\u{f07b}";
        }
        icon_for(&self.name, &self.ext, self.class)
    }

    pub fn is_text(&self) -> bool {
        self.class.is_text()
    }
}

#[cfg(unix)]
fn file_mode(meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.mode()
}

#[cfg(not(unix))]
fn file_mode(_meta: &fs::Metadata) -> u32 {
    0
}

fn detect_class(path: &Path, name: &str, ext: &str, meta: &fs::Metadata) -> FileClass {
    if meta.is_dir() {
        return FileClass::Directory;
    }
    if let Some(class) = class_for_name(name) {
        return class;
    }
    if let Some(class) = class_for_ext(ext) {
        return class;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o111 != 0 {
            return FileClass::Executable;
        }
    }
    let _ = path;
    FileClass::Other
}

fn class_for_name(name: &str) -> Option<FileClass> {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "makefile" | "dockerfile" | "justfile" | "rakefile" | "gemfile" | "procfile" => {
            Some(FileClass::Code)
        }
        "license" | "licence" | "readme" | "changelog" | "authors" | "copying" => {
            Some(FileClass::Text)
        }
        _ if lower.starts_with(".env") => Some(FileClass::Config),
        _ if lower.starts_with(".git") => Some(FileClass::Config),
        _ if lower.ends_with("rc") && lower.starts_with('.') => Some(FileClass::Config),
        _ => None,
    }
}

fn class_for_ext(ext: &str) -> Option<FileClass> {
    let class = match ext {
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "tif"
        | "tga" | "avif" | "heic" | "qoi" => FileClass::Image,
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "m4v" | "flv" | "wmv" | "mpg" | "mpeg" => {
            FileClass::Video
        }
        "mp3" | "flac" | "ogg" | "wav" | "opus" | "m4a" | "aac" | "wma" | "mid" => FileClass::Audio,
        "zip" | "tar" | "gz" | "bz2" | "xz" | "zst" | "7z" | "rar" | "tgz" | "txz" | "tbz2"
        | "deb" | "rpm" | "apk" | "jar" | "iso" => FileClass::Archive,
        "pdf" | "doc" | "docx" | "odt" | "rtf" | "epub" | "xls" | "xlsx" | "ods" | "ppt"
        | "pptx" | "odp" => FileClass::Document,
        "rs" | "py" | "js" | "mjs" | "cjs" | "ts" | "jsx" | "tsx" | "go" | "java" | "c" | "cc"
        | "cpp" | "cxx" | "h" | "hpp" | "cs" | "rb" | "php" | "lua" | "sh" | "bash" | "zsh"
        | "fish" | "sql" | "html" | "htm" | "css" | "scss" | "sass" | "less" | "vue" | "svelte"
        | "swift" | "kt" | "kts" | "scala" | "hs" | "ml" | "ex" | "exs" | "erl" | "clj" | "elm"
        | "zig" | "nim" | "v" | "asm" | "s" | "pl" | "r" | "dart" | "groovy" | "gradle"
        | "cmake" | "nix" | "tf" | "proto" => FileClass::Code,
        "txt" | "md" | "markdown" | "rst" | "org" | "log" | "csv" | "tsv" | "diff" | "patch" => {
            FileClass::Text
        }
        "toml" | "yaml" | "yml" | "json" | "jsonc" | "ini" | "conf" | "cfg" | "env" | "xml"
        | "plist" | "desktop" | "service" | "lock" => FileClass::Config,
        "ttf" | "otf" | "woff" | "woff2" => FileClass::Font,
        "exe" | "bin" | "appimage" | "msi" | "dll" | "so" => FileClass::Executable,
        _ => return None,
    };
    Some(class)
}

fn icon_for(name: &str, ext: &str, class: FileClass) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower == "makefile" || lower == "dockerfile" || lower == "justfile" {
        return "\u{e673}";
    }
    if lower == "cargo.toml" || lower == "cargo.lock" {
        return "\u{e7a8}";
    }
    match ext {
        "rs" => "\u{e7a8}",
        "py" => "\u{e73c}",
        "js" | "mjs" | "cjs" | "jsx" => "\u{e781}",
        "ts" | "tsx" => "\u{e628}",
        "go" => "\u{e627}",
        "java" => "\u{e738}",
        "c" | "h" => "\u{e61e}",
        "cpp" | "cc" | "cxx" | "hpp" => "\u{e61d}",
        "rb" => "\u{e739}",
        "php" => "\u{e73d}",
        "lua" => "\u{e620}",
        "sh" | "bash" | "zsh" | "fish" => "\u{f489}",
        "html" | "htm" => "\u{e736}",
        "css" | "scss" | "sass" | "less" => "\u{e749}",
        "json" | "jsonc" => "\u{e60b}",
        "yaml" | "yml" | "toml" | "ini" | "conf" | "cfg" => "\u{e615}",
        "md" | "markdown" | "rst" => "\u{e73e}",
        "pdf" => "\u{f1c1}",
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "tif"
        | "tga" | "avif" | "heic" | "qoi" => "\u{f1c5}",
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "m4v" | "flv" | "wmv" | "mpg" | "mpeg" => {
            "\u{f1c8}"
        }
        "mp3" | "flac" | "ogg" | "wav" | "opus" | "m4a" | "aac" | "wma" | "mid" => "\u{f1c7}",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "zst" | "7z" | "rar" | "tgz" | "txz" | "tbz2"
        | "deb" | "rpm" | "apk" | "jar" | "iso" => "\u{f1c6}",
        "ttf" | "otf" | "woff" | "woff2" => "\u{f031}",
        "exe" | "bin" | "appimage" | "msi" | "dll" | "so" => "\u{f013}",
        "lock" => "\u{f023}",
        _ => match class {
            FileClass::Directory => "\u{f07b}",
            FileClass::Text | FileClass::Document => "\u{f15c}",
            FileClass::Code => "\u{f121}",
            FileClass::Config => "\u{e615}",
            FileClass::Image => "\u{f1c5}",
            FileClass::Video => "\u{f1c8}",
            FileClass::Audio => "\u{f1c7}",
            FileClass::Archive => "\u{f1c6}",
            FileClass::Font => "\u{f031}",
            FileClass::Executable => "\u{f013}",
            FileClass::Other => "\u{f15b}",
        },
    }
}

pub fn read_directory(dir: &Path, show_hidden: bool) -> Result<Vec<FileEntry>, String> {
    let reader = fs::read_dir(dir).map_err(|e| format!("{}: {}", dir.display(), io_message(&e)))?;
    let mut entries = Vec::new();
    for item in reader.flatten() {
        if let Some(entry) = FileEntry::from_dir_entry(&item) {
            if show_hidden || !entry.hidden {
                entries.push(entry);
            }
        }
    }
    Ok(entries)
}

pub fn io_message(err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::PermissionDenied => "permission denied".to_string(),
        std::io::ErrorKind::NotFound => "not found".to_string(),
        _ => err.to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Modified,
    Kind,
}

impl SortKey {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "size" => Self::Size,
            "date" | "modified" => Self::Modified,
            "type" | "kind" => Self::Kind,
            _ => Self::Name,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Size => "size",
            Self::Modified => "modified",
            Self::Kind => "type",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::Size,
            Self::Size => Self::Modified,
            Self::Modified => Self::Kind,
            Self::Kind => Self::Name,
        }
    }
}

pub fn sort_entries(entries: &mut [FileEntry], key: SortKey, ascending: bool) {
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return if a.is_dir {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let order = match key {
            SortKey::Name => natural_cmp(&a.name, &b.name),
            SortKey::Size => a.size.cmp(&b.size),
            SortKey::Modified => a.modified.cmp(&b.modified),
            SortKey::Kind => a.class.label().cmp(b.class.label()),
        };
        let order = if ascending { order } else { order.reverse() };
        if order == Ordering::Equal {
            natural_cmp(&a.name, &b.name)
        } else {
            order
        }
    });
}

fn natural_cmp(a: &str, b: &str) -> Ordering {
    if !a.is_ascii() || !b.is_ascii() {
        return a.to_lowercase().cmp(&b.to_lowercase());
    }
    let a = a.to_ascii_lowercase();
    let b = b.to_ascii_lowercase();
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < ab.len() && j < bb.len() {
        if ab[i].is_ascii_digit() && bb[j].is_ascii_digit() {
            let si = i;
            while i < ab.len() && ab[i].is_ascii_digit() {
                i += 1;
            }
            let sj = j;
            while j < bb.len() && bb[j].is_ascii_digit() {
                j += 1;
            }
            let na = a[si..i].trim_start_matches('0');
            let nb = b[sj..j].trim_start_matches('0');
            let order = na.len().cmp(&nb.len()).then_with(|| na.cmp(nb));
            if order != Ordering::Equal {
                return order;
            }
        } else {
            let order = ab[i].cmp(&bb[j]);
            if order != Ordering::Equal {
                return order;
            }
            i += 1;
            j += 1;
        }
    }
    (ab.len() - i).cmp(&(bb.len() - j))
}

#[derive(Debug, Clone)]
pub struct ViewRow {
    pub index: usize,
    pub matches: Vec<usize>,
}

pub fn filter_view(entries: &[FileEntry], query: &str, fuzzy: bool) -> Vec<ViewRow> {
    if query.trim().is_empty() {
        return (0..entries.len())
            .map(|index| ViewRow {
                index,
                matches: Vec::new(),
            })
            .collect();
    }
    let matcher = SkimMatcherV2::default().ignore_case();
    let mut hits: Vec<(i64, usize, Vec<usize>)> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if !fuzzy {
            let lower = entry.name.to_lowercase();
            if let Some(pos) = lower.find(&query.to_lowercase()) {
                let start = lower[..pos].chars().count();
                let matches = (start..start + query.chars().count()).collect();
                hits.push((0, index, matches));
            }
            continue;
        }
        if let Some((score, positions)) = matcher.fuzzy_indices(&entry.name, query) {
            hits.push((score, index, positions));
        }
    }
    if !fuzzy {
        hits.sort_by_key(|(_, index, _)| *index);
    } else {
        hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    }
    hits.into_iter()
        .map(|(_, index, matches)| ViewRow { index, matches })
        .collect()
}

pub fn total_size(entries: &[FileEntry]) -> u64 {
    entries.iter().filter(|e| !e.is_dir).map(|e| e.size).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str, len: usize) {
        fs::write(dir.join(name), vec![b'x'; len]).unwrap();
    }

    #[test]
    fn filters_hidden_files() {
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), "visible.txt", 1);
        touch(t.path(), ".hidden", 1);
        let shown = read_directory(t.path(), false).unwrap();
        assert_eq!(shown.len(), 1);
        let all = read_directory(t.path(), true).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn directories_sort_first() {
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), "aaa.txt", 1);
        fs::create_dir(t.path().join("zzz")).unwrap();
        let mut entries = read_directory(t.path(), false).unwrap();
        sort_entries(&mut entries, SortKey::Name, true);
        assert!(entries[0].is_dir);
    }

    #[test]
    fn natural_sort_orders_numbers() {
        let t = tempfile::tempdir().unwrap();
        for name in ["file10", "file2", "file1"] {
            touch(t.path(), name, 1);
        }
        let mut entries = read_directory(t.path(), false).unwrap();
        sort_entries(&mut entries, SortKey::Name, true);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["file1", "file2", "file10"]);
    }

    #[test]
    fn size_sort_descending() {
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), "small", 1);
        touch(t.path(), "big", 100);
        let mut entries = read_directory(t.path(), false).unwrap();
        sort_entries(&mut entries, SortKey::Size, false);
        assert_eq!(entries[0].name, "big");
    }

    #[test]
    fn detects_symlinks_and_targets() {
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), "target.txt", 1);
        std::os::unix::fs::symlink(t.path().join("target.txt"), t.path().join("link.txt")).unwrap();
        let entry = FileEntry::from_path(&t.path().join("link.txt")).unwrap();
        assert!(entry.is_symlink);
        assert!(!entry.broken);
        assert_eq!(entry.symlink_target, Some(t.path().join("target.txt")));
    }

    #[test]
    fn detects_classes() {
        let t = tempfile::tempdir().unwrap();
        for (name, expect) in [
            ("a.png", FileClass::Image),
            ("a.pdf", FileClass::Document),
            ("a.mkv", FileClass::Video),
            ("a.flac", FileClass::Audio),
            ("a.rs", FileClass::Code),
            ("a.zip", FileClass::Archive),
            ("a.toml", FileClass::Config),
            ("a.md", FileClass::Text),
            ("Makefile", FileClass::Code),
        ] {
            touch(t.path(), name, 1);
            let e = FileEntry::from_path(&t.path().join(name)).unwrap();
            assert_eq!(e.class, expect, "{}", name);
        }
    }

    #[test]
    fn fuzzy_filter_ranks_subsequence_matches() {
        let t = tempfile::tempdir().unwrap();
        for name in ["main.rs", "domain.txt", "readme.md"] {
            touch(t.path(), name, 1);
        }
        let entries = read_directory(t.path(), false).unwrap();
        let view = filter_view(&entries, "mnrs", true);
        assert!(!view.is_empty());
        assert_eq!(entries[view[0].index].name, "main.rs");
        assert!(!view[0].matches.is_empty());
    }

    #[test]
    fn substring_filter_positions() {
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), "hello.txt", 1);
        let entries = read_directory(t.path(), false).unwrap();
        let view = filter_view(&entries, "ell", false);
        assert_eq!(view.len(), 1);
        assert_eq!(view[0].matches, vec![1, 2, 3]);
    }
}
