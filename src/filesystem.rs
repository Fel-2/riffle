use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use crate::state::SortBy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileType {
    Directory, Image, Document, Video, Audio, Code, Archive, Executable, Other,
}

impl FileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileType::Directory  => "dir",
            FileType::Image      => "img",
            FileType::Document   => "doc",
            FileType::Video      => "vid",
            FileType::Audio      => "aud",
            FileType::Code       => "code",
            FileType::Archive    => "arc",
            FileType::Executable => "exe",
            FileType::Other      => "oth",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: SystemTime,
    pub file_type: FileType,
    pub is_hidden: bool,
    pub is_symlink: bool,
    pub symlink_target: Option<PathBuf>,
}

impl FileEntry {
    pub fn from_path(path: &PathBuf) -> Option<Self> {
        // Use symlink_metadata to detect symlinks without following them
        let symlink_meta = fs::symlink_metadata(path).ok()?;
        let is_symlink = symlink_meta.file_type().is_symlink();

        let symlink_target = if is_symlink {
            fs::read_link(path).ok()
        } else {
            None
        };

        // Follow symlinks for actual metadata (size, type, etc.)
        let metadata = fs::metadata(path).unwrap_or(symlink_meta);

        let name = path.file_name()?.to_string_lossy().into_owned();
        let is_hidden = name.starts_with('.');
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let file_type = detect_file_type(path, &metadata);
        Some(Self {
            path: path.clone(),
            name,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified,
            file_type,
            is_hidden,
            is_symlink,
            symlink_target,
        })
    }

    pub fn type_icon(&self) -> &'static str {
        if self.is_symlink { return "󰉹"; }
        if self.is_dir { return "󰉋"; }
        match self.file_type {
            FileType::Image      => "󰋩",
            FileType::Document   => "󰈙",
            FileType::Video      => "󰎁",
            FileType::Audio      => "󰎈",
            FileType::Code       => "󰅴",
            FileType::Archive    => "󰗄",
            FileType::Executable => "󰆍",
            _                    => "󰈔",
        }
    }
}

fn detect_file_type(path: &PathBuf, metadata: &fs::Metadata) -> FileType {
    if metadata.is_dir() { return FileType::Directory; }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext.is_empty() {
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 != 0 { return FileType::Executable; }
        }
        return FileType::Other;
    }
    match ext.as_str() {
        "jpg"|"jpeg"|"png"|"gif"|"bmp"|"webp"|"svg" => FileType::Image,
        "pdf"|"doc"|"docx"|"txt"|"md"|"rtf"|"odt"  => FileType::Document,
        "mp4"|"avi"|"mkv"|"mov"|"wmv"|"webm"        => FileType::Video,
        "mp3"|"wav"|"flac"|"ogg"|"m4a"|"aac"        => FileType::Audio,
        "rs"|"js"|"ts"|"py"|"go"|"java"|"c"|"cpp"|"h"|"css"|"html"|"json"|"toml"|"yaml"|"sh" => FileType::Code,
        "zip"|"tar"|"gz"|"bz2"|"xz"|"7z"|"rar"     => FileType::Archive,
        _ => FileType::Other,
    }
}

pub fn read_directory(path: &PathBuf, show_hidden: bool) -> Vec<FileEntry> {
    let Ok(rd) = fs::read_dir(path) else { return Vec::new(); };
    rd.flatten()
        .filter_map(|e| FileEntry::from_path(&e.path()))
        .filter(|e| show_hidden || !e.is_hidden)
        .collect()
}

pub fn sort_entries(entries: &mut Vec<FileEntry>, sort_by: SortBy, ascending: bool) {
    entries.sort_by(|a, b| {
        // Dirs always first
        if a.is_dir != b.is_dir {
            return if a.is_dir { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
        }
        let cmp = match sort_by {
            SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortBy::Size => a.size.cmp(&b.size),
            SortBy::Date => a.modified.cmp(&b.modified),
            SortBy::Type => a.file_type.as_str().cmp(b.file_type.as_str()),
        };
        if ascending { cmp } else { cmp.reverse() }
    });
}
