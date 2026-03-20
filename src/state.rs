use std::collections::{HashSet, HashMap};
use std::path::PathBuf;
use crate::filesystem::FileEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy { Name, Size, Date, Type }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode { Normal, Search, Rename, GoTo, Shell }

#[derive(Debug, Clone)]
pub enum PreviewContent {
    None,
    Text(Vec<String>),
    Image(PathBuf),
    Binary { size: u64 },
    Directory { count: usize },
}

pub struct TabState {
    pub current_dir: PathBuf,
    pub entries: Vec<FileEntry>,
    pub display_indices: Vec<usize>,
    pub search_query: String,
    pub sort_by: SortBy,
    pub sort_ascending: bool,
    pub cursor: usize,
    pub scroll: usize,
    pub selection: HashSet<PathBuf>,
    pub renaming: Option<(usize, String)>, // (display_idx, new_name_buf)
    pub git_status: HashMap<String, char>,
}

impl TabState {
    pub fn new(path: PathBuf) -> Self {
        Self {
            current_dir: path,
            entries: Vec::new(),
            display_indices: Vec::new(),
            search_query: String::new(),
            sort_by: SortBy::Name,
            sort_ascending: true,
            cursor: 0,
            scroll: 0,
            selection: HashSet::new(),
            renaming: None,
            git_status: HashMap::new(),
        }
    }

    pub fn display_count(&self) -> usize { self.display_indices.len() }

    pub fn get_display_entry(&self, i: usize) -> Option<&FileEntry> {
        self.display_indices.get(i).and_then(|&idx| self.entries.get(idx))
    }

    pub fn cursor_entry(&self) -> Option<&FileEntry> {
        self.get_display_entry(self.cursor)
    }

    pub fn clamp_cursor(&mut self) {
        let len = self.display_count();
        if len == 0 { self.cursor = 0; }
        else if self.cursor >= len { self.cursor = len - 1; }
    }

    pub fn ensure_scroll(&mut self, visible_rows: usize) {
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + visible_rows && visible_rows > 0 {
            self.scroll = self.cursor + 1 - visible_rows;
        }
    }

    pub fn toggle_selection(&mut self, path: PathBuf) {
        if !self.selection.remove(&path) { self.selection.insert(path); }
    }
}

pub struct AppState {
    pub tabs: Vec<TabState>,
    pub active_tab: usize,
    pub right_tab: usize,
    pub dual_pane: bool,
    pub right_focused: bool,
    pub show_hidden: bool,
    pub clipboard: Option<(Vec<PathBuf>, bool)>, // (paths, is_cut)
    pub input_mode: InputMode,
    pub goto_buf: String,
    pub shell_buf: String,
    pub bookmarks: Vec<(String, PathBuf)>,
    pub status: Option<(String, std::time::Instant)>,
    pub show_help: bool,
    pub sidebar_width: u16,
    pub preview_enabled: bool,
    pub preview_content: PreviewContent,
    pub preview_rect: Option<(u16, u16, u16, u16)>, // x,y,w,h (inner area)
    pub last_kitty_image: Option<PathBuf>,
    pub preview_image_pending: Option<(PathBuf, std::time::Instant)>,
}

impl AppState {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let mut bookmarks = vec![("Home".into(), home.clone())];
        if let Some(p) = dirs::download_dir() { bookmarks.push(("Downloads".into(), p)); }
        if let Some(p) = dirs::document_dir() { bookmarks.push(("Documents".into(), p)); }
        if let Some(p) = dirs::picture_dir()  { bookmarks.push(("Pictures".into(), p)); }
        if let Some(p) = dirs::audio_dir()    { bookmarks.push(("Music".into(), p)); }
        if let Some(p) = dirs::video_dir()    { bookmarks.push(("Videos".into(), p)); }

        Self {
            tabs: vec![TabState::new(home)],
            active_tab: 0,
            right_tab: 0,
            dual_pane: false,
            right_focused: false,
            show_hidden: false,
            clipboard: None,
            input_mode: InputMode::Normal,
            goto_buf: String::new(),
            shell_buf: String::new(),
            bookmarks,
            status: None,
            show_help: false,
            sidebar_width: 20,
            preview_enabled: false,
            preview_content: PreviewContent::None,
            preview_rect: None,
            last_kitty_image: None,
            preview_image_pending: None,
        }
    }

    pub fn focused_tab(&self) -> &TabState {
        if self.dual_pane && self.right_focused {
            self.tabs.get(self.right_tab).unwrap_or(&self.tabs[self.active_tab])
        } else {
            &self.tabs[self.active_tab]
        }
    }

    pub fn focused_tab_mut(&mut self) -> &mut TabState {
        if self.dual_pane && self.right_focused {
            let i = self.right_tab; &mut self.tabs[i]
        } else {
            let i = self.active_tab; &mut self.tabs[i]
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), std::time::Instant::now()));
    }

    pub fn update_cursor(&mut self, delta: isize) {
        let len = self.focused_tab().display_count() as isize;
        if len == 0 { return; }
        let c = self.focused_tab().cursor as isize;
        let new = (c + delta).clamp(0, len - 1) as usize;
        self.focused_tab_mut().cursor = new;
    }
}
