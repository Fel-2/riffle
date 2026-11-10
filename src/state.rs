use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::Line;

use crate::action::Action;
use crate::config::{self, Bookmark, Config};
use crate::filesystem::{
    filter_view, read_directory, sort_entries, total_size, FileClass, FileEntry, SortKey, ViewRow,
};
use crate::ops;
use crate::proc::{git_data, GitData, GitStatus, RepoInfo};
use crate::syntax::{tokenize, Lang, Token};
use crate::theme::Theme;
use crate::utils::{self, DateStyle};

pub const MAX_PREVIEW_LINES: usize = 1000;
pub const MAX_PREVIEW_LINE_CHARS: usize = 2000;
const MAX_PREVIEW_LINE_BYTES: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Files,
    Preview,
    Sidebar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Prompt,
    Confirm,
    Palette,
    Help,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Success,
    Warn,
    Error,
}

pub struct Status {
    pub text: String,
    pub kind: StatusKind,
    pub at: Instant,
    pub ttl: Duration,
}

impl Status {
    pub fn new(text: impl Into<String>, kind: StatusKind) -> Self {
        let ttl = match kind {
            StatusKind::Error => Duration::from_secs(8),
            StatusKind::Warn => Duration::from_secs(5),
            StatusKind::Info | StatusKind::Success => Duration::from_secs(3),
        };
        let raw = text.into();
        let text = raw
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("")
            .to_string();
        Self {
            text,
            kind,
            at: Instant::now(),
            ttl,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextInput {
    pub chars: Vec<char>,
    pub cursor: usize,
    anchor: Option<usize>,
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            chars: Vec::new(),
            cursor: 0,
            anchor: None,
        }
    }

    pub fn with_value(value: &str) -> Self {
        let mut input = Self::new();
        input.chars = value.chars().collect();
        input.cursor = input.chars.len();
        input
    }

    pub fn from_name_select_stem(name: &str) -> Self {
        let mut input = Self::with_value(name);
        let stem = match name.rfind('.') {
            Some(dot) if dot > 0 => name[..dot].chars().count(),
            _ => name.chars().count(),
        };
        input.anchor = Some(0);
        input.cursor = stem;
        input
    }

    pub fn value(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.chars.len();
    }

    pub fn selection(&self) -> Option<(usize, usize)> {
        match self.anchor {
            Some(anchor) if anchor != self.cursor => {
                Some((anchor.min(self.cursor), anchor.max(self.cursor)))
            }
            _ => None,
        }
    }

    fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection() {
            self.chars.drain(start..end);
            self.cursor = start;
            self.anchor = None;
            true
        } else {
            false
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        self.delete_selection();
        self.anchor = None;
        self.chars.insert(self.cursor, ch);
        self.cursor += 1;
    }

    pub fn insert_str(&mut self, text: &str) {
        self.delete_selection();
        self.anchor = None;
        for ch in text.chars().filter(|c| !c.is_control()) {
            self.chars.insert(self.cursor, ch);
            self.cursor += 1;
        }
    }

    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.anchor = None;
        if self.cursor > 0 {
            self.chars.remove(self.cursor - 1);
            self.cursor -= 1;
        }
    }

    pub fn delete(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.anchor = None;
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        if let Some((start, _)) = self.selection() {
            self.cursor = start;
        } else {
            self.cursor = self.cursor.saturating_sub(1);
        }
        self.anchor = None;
    }

    pub fn right(&mut self) {
        if let Some((_, end)) = self.selection() {
            self.cursor = end;
        } else {
            self.cursor = (self.cursor + 1).min(self.chars.len());
        }
        self.anchor = None;
    }

    pub fn home(&mut self) {
        self.cursor = 0;
        self.anchor = None;
    }

    pub fn end(&mut self) {
        self.cursor = self.chars.len();
        self.anchor = None;
    }

    pub fn word_left(&mut self) {
        self.anchor = None;
        while self.cursor > 0 && self.chars[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
        }
        while self.cursor > 0 && !self.chars[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
        }
    }

    pub fn word_right(&mut self) {
        self.anchor = None;
        while self.cursor < self.chars.len() && self.chars[self.cursor].is_whitespace() {
            self.cursor += 1;
        }
        while self.cursor < self.chars.len() && !self.chars[self.cursor].is_whitespace() {
            self.cursor += 1;
        }
    }

    pub fn backspace_word(&mut self) {
        if self.delete_selection() {
            return;
        }
        let end = self.cursor;
        self.word_left();
        self.chars.drain(self.cursor..end);
    }

    pub fn kill_to_start(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.chars.drain(..self.cursor);
        self.cursor = 0;
    }

    pub fn kill_to_end(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.chars.truncate(self.cursor);
    }
}

#[derive(Debug, Clone)]
pub enum PromptKind {
    Filter,
    GoTo,
    Rename { path: PathBuf },
    NewFile,
    NewDir,
    Shell,
}

pub struct Prompt {
    pub kind: PromptKind,
    pub title: String,
    pub input: TextInput,
    pub hint: String,
}

impl Prompt {
    pub fn filter(current: &str) -> Self {
        let mut input = TextInput::with_value(current);
        input.select_all();
        Self {
            kind: PromptKind::Filter,
            title: "Filter".into(),
            input,
            hint: "type to filter · ↑↓ move · ⏎ keep · esc clear".into(),
        }
    }

    pub fn goto(current: &Path) -> Self {
        let mut input = TextInput::with_value(&current.to_string_lossy());
        input.select_all();
        Self {
            kind: PromptKind::GoTo,
            title: "Go to path".into(),
            input,
            hint: "⏎ go · esc cancel · ~ and relative paths work".into(),
        }
    }

    pub fn rename(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            kind: PromptKind::Rename { path },
            title: "Rename".into(),
            input: TextInput::from_name_select_stem(&name),
            hint: "⏎ rename · esc cancel · ctrl+u clear".into(),
        }
    }

    pub fn new_file() -> Self {
        Self {
            kind: PromptKind::NewFile,
            title: "New file".into(),
            input: TextInput::new(),
            hint: "⏎ create · esc cancel".into(),
        }
    }

    pub fn new_dir() -> Self {
        Self {
            kind: PromptKind::NewDir,
            title: "New directory".into(),
            input: TextInput::new(),
            hint: "⏎ create · esc cancel".into(),
        }
    }

    pub fn shell() -> Self {
        Self {
            kind: PromptKind::Shell,
            title: "Shell command".into(),
            input: TextInput::new(),
            hint: "⏎ run · esc cancel · runs in the current directory".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    Delete(Vec<PathBuf>),
    Trash(Vec<PathBuf>),
}

pub struct Confirm {
    pub title: String,
    pub message: String,
    pub detail: String,
    pub action: ConfirmAction,
    pub destructive: bool,
}

pub struct Palette {
    pub input: TextInput,
    pub items: Vec<Action>,
    pub filtered: Vec<(usize, Vec<usize>)>,
    pub cursor: usize,
    pub scroll: usize,
}

impl Palette {
    pub fn new(items: Vec<Action>) -> Self {
        let mut palette = Self {
            input: TextInput::new(),
            items,
            filtered: Vec::new(),
            cursor: 0,
            scroll: 0,
        };
        palette.refilter();
        palette
    }

    pub fn refilter(&mut self) {
        use fuzzy_matcher::skim::SkimMatcherV2;
        use fuzzy_matcher::FuzzyMatcher;
        let query = self.input.value();
        if query.trim().is_empty() {
            self.filtered = self
                .items
                .iter()
                .enumerate()
                .map(|(index, _)| (index, Vec::new()))
                .collect();
        } else {
            let matcher = SkimMatcherV2::default().ignore_case();
            let mut hits = Vec::new();
            for (index, action) in self.items.iter().enumerate() {
                let haystack = format!("{} {}", action.label(), action.section());
                if let Some((score, positions)) = matcher.fuzzy_indices(&haystack, &query) {
                    hits.push((score, index, positions));
                }
            }
            hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            self.filtered = hits
                .into_iter()
                .map(|(_, index, positions)| (index, positions))
                .collect();
        }
        self.cursor = 0;
        self.scroll = 0;
    }

    pub fn selected(&self) -> Option<Action> {
        self.filtered
            .get(self.cursor)
            .map(|(index, _)| self.items[*index])
    }
}

pub struct HelpState {
    pub scroll: usize,
}

pub struct InfoState {
    pub title: String,
    pub rows: Vec<(String, String)>,
    pub scroll: usize,
}

pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub cut: bool,
}

impl Clone for Clipboard {
    fn clone(&self) -> Self {
        Self {
            paths: self.paths.clone(),
            cut: self.cut,
        }
    }
}

pub enum TaskMsg {
    Progress {
        done: usize,
        current: String,
    },
    Done {
        ok: usize,
        total: usize,
        errors: Vec<String>,
        note: Option<String>,
    },
}

pub struct Task {
    pub label: String,
    pub rx: Receiver<TaskMsg>,
    pub done: usize,
    pub total: usize,
    pub current: String,
    pub cancel: Arc<AtomicBool>,
    pub started: Instant,
}

#[derive(Debug, Clone)]
pub struct PreviewDirEntry {
    pub name: String,
    pub is_dir: bool,
    pub class: FileClass,
    pub icon: &'static str,
}

#[derive(Clone)]
pub enum PreviewContent {
    Empty,
    Loading,
    Directory {
        count: usize,
        entries: Vec<PreviewDirEntry>,
    },
    Text {
        tokens: Vec<Vec<Token>>,
        line_count: usize,
        truncated: bool,
        lang: Lang,
    },
    Image {
        dims: Option<(u32, u32)>,
        size: u64,
    },
    Binary {
        size: u64,
        hex: Vec<String>,
    },
    Unsupported {
        reason: String,
        size: u64,
    },
}

impl PreviewContent {
    pub fn line_count(&self) -> usize {
        match self {
            Self::Text { line_count, .. } => *line_count,
            Self::Binary { hex, .. } => hex.len(),
            Self::Directory { entries, .. } => entries.len(),
            _ => 0,
        }
    }
}

pub struct Preview {
    pub path: Option<PathBuf>,
    pub content: PreviewContent,
}

impl Preview {
    pub fn empty() -> Self {
        Self {
            path: None,
            content: PreviewContent::Empty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Kitty,
    Chafa,
    Blocks,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageKey {
    pub path: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub mode: RenderMode,
    pub bg: Color,
    pub bg_rgb: (u8, u8, u8),
}

pub enum ImagePayload {
    Kitty { data: String },
    Chafa { output: String },
    Blocks { lines: Vec<Line<'static>> },
    Failed { reason: String },
}

pub struct ImageRequest {
    pub generation: u64,
    pub key: ImageKey,
}

pub struct ImageMsg {
    pub generation: u64,
    pub key: ImageKey,
    pub dims: Option<(u32, u32)>,
    pub payload: ImagePayload,
}

pub struct ImageState {
    pub key: Option<ImageKey>,
    pub ready: Option<ImageMsg>,
    pub drawn: Option<(u64, PathBuf, Rect)>,
    pub generation: u64,
    pub pending: bool,
    pub error: Option<String>,
}

impl ImageState {
    pub fn new() -> Self {
        Self {
            key: None,
            ready: None,
            drawn: None,
            generation: 0,
            pending: false,
            error: None,
        }
    }

    pub fn reset(&mut self) {
        self.key = None;
        self.ready = None;
        self.pending = false;
        self.error = None;
    }
}

pub struct DirMsg {
    pub generation: u64,
    pub tab: usize,
    pub pane: usize,
    pub dir: PathBuf,
    pub result: Result<Vec<FileEntry>, String>,
}

pub struct PreviewRequest {
    pub generation: u64,
    pub entry: FileEntry,
}

pub struct PreviewMsg {
    pub generation: u64,
    pub path: PathBuf,
    pub content: PreviewContent,
}

pub struct GitMsg {
    pub dir: PathBuf,
    pub data: Option<GitData>,
}

pub struct SidebarMsg {
    pub generation: u64,
    pub targets: Vec<(String, PathBuf)>,
}

pub struct Pane {
    pub dir: PathBuf,
    pub entries: Vec<FileEntry>,
    pub view: Vec<ViewRow>,
    pub cursor: usize,
    pub scroll: usize,
    pub height: usize,
    pub selection: HashSet<PathBuf>,
    pub filter: String,
    pub sort: SortKey,
    pub ascending: bool,
    pub history: Vec<PathBuf>,
    pub hist_pos: usize,
    pub loading: bool,
    pub generation: u64,
    pub error: Option<String>,
    pub repo: Option<RepoInfo>,
    pub git: HashMap<String, GitStatus>,
    pub total_size: u64,
    pub dir_count: usize,
    pub file_count: usize,
}

impl Pane {
    pub fn new(dir: PathBuf, sort: SortKey, ascending: bool) -> Self {
        Self {
            dir: dir.clone(),
            entries: Vec::new(),
            view: Vec::new(),
            cursor: 0,
            scroll: 0,
            height: 10,
            selection: HashSet::new(),
            filter: String::new(),
            sort,
            ascending,
            history: vec![dir],
            hist_pos: 0,
            loading: false,
            generation: 0,
            error: None,
            repo: None,
            git: HashMap::new(),
            total_size: 0,
            dir_count: 0,
            file_count: 0,
        }
    }

    pub fn rebuild(&mut self, fuzzy: bool) {
        sort_entries(&mut self.entries, self.sort, self.ascending);
        self.view = filter_view(&self.entries, &self.filter, fuzzy);
        self.total_size = total_size(&self.entries);
        self.dir_count = self.entries.iter().filter(|e| e.is_dir).count();
        self.file_count = self.entries.len().saturating_sub(self.dir_count);
        if self.view.is_empty() {
            self.cursor = 0;
            self.scroll = 0;
        } else if self.cursor >= self.view.len() {
            self.cursor = self.view.len() - 1;
        }
        self.ensure_scroll();
    }

    pub fn view_len(&self) -> usize {
        self.view.len()
    }

    pub fn get_display_entry(&self, row: usize) -> Option<&FileEntry> {
        self.view
            .get(row)
            .and_then(|view| self.entries.get(view.index))
    }

    pub fn cursor_entry(&self) -> Option<&FileEntry> {
        self.get_display_entry(self.cursor)
    }

    pub fn ensure_scroll(&mut self) {
        let height = self.height.max(1);
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + height {
            self.scroll = self.cursor + 1 - height;
        }
        let max_scroll = self.view.len().saturating_sub(height);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
    }

    pub fn select_path(&mut self, path: &Path) {
        if let Some(row) = self
            .view
            .iter()
            .position(|view| self.entries[view.index].path == path)
        {
            self.cursor = row;
            self.ensure_scroll();
        }
    }

    pub fn selected_paths(&self) -> Vec<PathBuf> {
        if self.selection.is_empty() {
            self.cursor_entry()
                .map(|entry| vec![entry.path.clone()])
                .unwrap_or_default()
        } else {
            let mut paths: Vec<PathBuf> = self.selection.iter().cloned().collect();
            paths.sort();
            paths
        }
    }

    pub fn clear_filter(&mut self, fuzzy: bool) {
        if !self.filter.is_empty() {
            self.filter.clear();
            self.rebuild(fuzzy);
        }
    }
}

pub struct Tab {
    pub panes: [Pane; 2],
    pub focus_right: bool,
}

impl Tab {
    pub fn new(dir: PathBuf, sort: SortKey, ascending: bool) -> Self {
        Self {
            panes: [
                Pane::new(dir.clone(), sort, ascending),
                Pane::new(dir, sort, ascending),
            ],
            focus_right: false,
        }
    }

    pub fn pane_index(&self, dual: bool) -> usize {
        if dual && self.focus_right {
            1
        } else {
            0
        }
    }

    pub fn pane(&self, index: usize) -> &Pane {
        &self.panes[index]
    }

    pub fn pane_mut(&mut self, index: usize) -> &mut Pane {
        &mut self.panes[index]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Name,
    Size,
    Modified,
}

#[derive(Default, Clone)]
pub struct PaneLayout {
    pub header: Rect,
    pub list: Rect,
    pub crumbs: Vec<(Rect, PathBuf)>,
    pub columns: Vec<(Rect, Column)>,
}

#[derive(Default, Clone)]
pub struct Layout {
    pub sidebar: Option<Rect>,
    pub panes: [Option<PaneLayout>; 2],
    pub preview: Option<Rect>,
    pub preview_content: Option<Rect>,
    pub tab_hits: Vec<(Rect, usize)>,
    pub sidebar_hits: Vec<(Rect, usize)>,
}

pub struct App {
    pub tabs: Vec<Tab>,
    pub tab: usize,
    pub dual: bool,
    pub focus: Focus,
    pub show_sidebar: bool,
    pub show_preview: bool,
    pub mode: Mode,
    pub prompt: Option<Prompt>,
    pub confirm: Option<Confirm>,
    pub palette: Option<Palette>,
    pub help: Option<HelpState>,
    pub info: Option<InfoState>,
    pub status: Option<Status>,
    pub clipboard: Option<Clipboard>,
    pub task: Option<Task>,
    pub undo: Vec<ops::Trashed>,
    pub show_hidden: bool,
    pub bookmarks: Vec<Bookmark>,
    pub persisted_bookmarks: Vec<Bookmark>,
    pub sidebar_targets_cache: Vec<(String, PathBuf)>,
    pub sidebar_tx: Sender<SidebarMsg>,
    pub sidebar_rx: Receiver<SidebarMsg>,
    pub sidebar_gen: u64,
    pub sidebar_cursor: usize,
    pub sidebar_scroll: usize,
    pub theme: Theme,
    pub config: Config,
    pub preview: Preview,
    pub preview_scroll: usize,
    pub preview_hscroll: usize,
    pub image: ImageState,
    pub image_tx: Sender<ImageRequest>,
    pub image_rx: Receiver<ImageMsg>,
    pub dir_gen: u64,
    pub preview_gen: u64,
    pub dir_tx: Sender<DirMsg>,
    pub dir_rx: Receiver<DirMsg>,
    pub preview_tx: Sender<PreviewRequest>,
    pub preview_rx: Receiver<PreviewMsg>,
    pub git_tx: Sender<GitMsg>,
    pub git_rx: Receiver<GitMsg>,
    pub git_pending: HashSet<PathBuf>,
    pub git_cache: HashMap<PathBuf, Option<GitData>>,
    pub layout: Layout,
    pub should_quit: bool,
    pub print_cwd: bool,
    pub pending_select: Option<PathBuf>,
    pub last_click: Option<(Instant, u16, u16)>,
}

impl App {
    pub fn new(
        config: Config,
        start: PathBuf,
        print_cwd: bool,
        pending_select: Option<PathBuf>,
    ) -> Self {
        let theme = Theme::from_config(&config.theme);
        let sort = SortKey::parse(&config.ui.sort_by);
        let show_hidden = config.ui.show_hidden;
        let fuzzy = config.behavior.fuzzy;
        let mut persisted = config::load_persisted_bookmarks();
        persisted.retain(|b| !b.path.is_empty());
        let mut bookmarks = config.bookmarks.clone();
        for bookmark in &persisted {
            if !bookmarks.iter().any(|b| b.path == bookmark.path) {
                bookmarks.push(bookmark.clone());
            }
        }
        for bookmark in default_bookmarks() {
            if !bookmarks.iter().any(|b| b.path == bookmark.path) {
                bookmarks.push(bookmark);
            }
        }

        let sidebar_cache = fast_sidebar_targets(&bookmarks);
        let mut tab = Tab::new(start.clone(), sort, config.ui.sort_ascending);
        for pane in &mut tab.panes {
            pane.rebuild(fuzzy);
        }
        let (dir_tx, dir_rx) = std::sync::mpsc::channel();
        let (preview_tx, preview_req_rx) = std::sync::mpsc::channel();
        let (preview_res_tx, preview_rx) = std::sync::mpsc::channel();
        spawn_preview_worker(preview_req_rx, preview_res_tx);
        let (git_tx, git_rx) = std::sync::mpsc::channel();
        let (image_tx, image_req_rx) = std::sync::mpsc::channel();
        let (image_res_tx, image_rx) = std::sync::mpsc::channel();
        crate::ui::image::spawn_worker(image_req_rx, image_res_tx);
        let (sidebar_tx, sidebar_rx) = std::sync::mpsc::channel();
        spawn_sidebar_scan(sidebar_tx.clone(), 1, bookmarks.clone());

        let mut app = Self {
            tabs: vec![tab],
            tab: 0,
            dual: false,
            focus: Focus::Files,
            show_sidebar: config.ui.sidebar,
            show_preview: config.ui.preview,
            mode: Mode::Normal,
            prompt: None,
            confirm: None,
            palette: None,
            help: None,
            info: None,
            status: None,
            clipboard: None,
            task: None,
            undo: Vec::new(),
            show_hidden,
            bookmarks,
            persisted_bookmarks: persisted,
            sidebar_targets_cache: sidebar_cache,
            sidebar_tx,
            sidebar_rx,
            sidebar_gen: 1,
            sidebar_cursor: 0,
            sidebar_scroll: 0,
            theme,
            config,
            preview: Preview::empty(),
            preview_scroll: 0,
            preview_hscroll: 0,
            image: ImageState::new(),
            image_tx,
            image_rx,
            dir_gen: 0,
            preview_gen: 0,
            dir_tx,
            dir_rx,
            preview_tx,
            preview_rx,
            git_tx,
            git_rx,
            git_pending: HashSet::new(),
            git_cache: HashMap::new(),
            layout: Layout::default(),
            should_quit: false,
            print_cwd,
            pending_select,
            last_click: None,
        };
        app.load_pane(0, 0);
        app.request_git(0, 0);
        app
    }

    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.tab]
    }

    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.tab]
    }

    pub fn pane_index(&self) -> usize {
        self.active_tab().pane_index(self.dual)
    }

    pub fn active_pane(&self) -> &Pane {
        self.active_tab().pane(self.pane_index())
    }

    pub fn active_pane_mut(&mut self) -> &mut Pane {
        let index = self.pane_index();
        self.active_tab_mut().pane_mut(index)
    }

    pub fn current_dir(&self) -> PathBuf {
        self.active_pane().dir.clone()
    }

    pub fn fuzzy(&self) -> bool {
        self.config.behavior.fuzzy
    }

    pub fn date_style(&self) -> DateStyle {
        self.config.date_style()
    }

    pub fn set_status(&mut self, text: impl Into<String>, kind: StatusKind) {
        self.status = Some(Status::new(text, kind));
    }

    pub fn set_info(&mut self, text: impl Into<String>) {
        self.set_status(text, StatusKind::Info);
    }

    pub fn set_error(&mut self, text: impl Into<String>) {
        self.set_status(text, StatusKind::Error);
    }

    pub fn set_success(&mut self, text: impl Into<String>) {
        self.set_status(text, StatusKind::Success);
    }

    pub fn set_warn(&mut self, text: impl Into<String>) {
        self.set_status(text, StatusKind::Warn);
    }

    pub fn show_config_error(&mut self, error: &str) {
        let mut rows = vec![(
            "File".to_string(),
            config::config_path().display().to_string(),
        )];
        for (index, line) in error
            .lines()
            .filter(|line| !line.trim().is_empty())
            .enumerate()
        {
            rows.push((
                if index == 0 { "Error" } else { "" }.to_string(),
                line.trim().to_string(),
            ));
        }
        rows.push((
            "Hint".to_string(),
            "fix the file, then run Reload configuration from ctrl+p".to_string(),
        ));
        self.info = Some(InfoState {
            title: "Configuration error".to_string(),
            rows,
            scroll: 0,
        });
        self.mode = Mode::Info;
        self.set_error("configuration error");
    }

    pub fn tick(&mut self) {
        if let Some(status) = &self.status {
            if status.at.elapsed() >= status.ttl {
                self.status = None;
            }
        }
    }

    pub fn is_busy(&self) -> bool {
        self.task.is_some()
            || self.image.pending
            || !self.git_pending.is_empty()
            || self
                .tabs
                .iter()
                .any(|tab| tab.panes.iter().any(|pane| pane.loading))
            || matches!(self.preview.content, PreviewContent::Loading)
    }

    pub fn load_pane(&mut self, tab: usize, pane: usize) {
        let Some(tab_state) = self.tabs.get(tab) else {
            return;
        };
        let target = &tab_state.panes[pane];
        let dir = target.dir.clone();
        self.dir_gen += 1;
        let generation = self.dir_gen;
        self.tabs[tab].panes[pane].generation = generation;
        self.tabs[tab].panes[pane].loading = true;
        self.tabs[tab].panes[pane].error = None;
        let tx = self.dir_tx.clone();
        let show_hidden = self.show_hidden;
        std::thread::spawn(move || {
            let result = read_directory(&dir, show_hidden);
            let _ = tx.send(DirMsg {
                generation,
                tab,
                pane,
                dir,
                result,
            });
        });
    }

    pub fn apply_dir(&mut self, msg: DirMsg) {
        let fuzzy = self.fuzzy();
        let Some(tab) = self.tabs.get_mut(msg.tab) else {
            return;
        };
        let pane = &mut tab.panes[msg.pane];
        if pane.generation != msg.generation || pane.dir != msg.dir {
            return;
        }
        pane.loading = false;
        match msg.result {
            Ok(entries) => {
                let previous = pane.cursor_entry().map(|entry| entry.path.clone());
                pane.entries = entries;
                pane.selection
                    .retain(|path| pane.entries.iter().any(|entry| &entry.path == path));
                pane.error = None;
                pane.rebuild(fuzzy);
                if let Some(previous) = previous {
                    pane.select_path(&previous);
                }
            }
            Err(err) => {
                pane.entries.clear();
                pane.view.clear();
                pane.error = Some(err);
                pane.rebuild(fuzzy);
            }
        }
        if let Some(select) = self.pending_select.clone() {
            if select.parent() == Some(pane.dir.as_path()) {
                pane.select_path(&select);
                self.pending_select = None;
            }
        }
        if msg.tab == self.tab && msg.pane == self.pane_index() {
            self.preview_scroll = 0;
            self.preview_hscroll = 0;
            self.reload_preview();
            self.update_title();
        }
    }

    pub fn reload_preview(&mut self) {
        self.preview.path = None;
        self.request_preview();
    }

    pub fn request_preview(&mut self) {
        if !self.show_preview {
            self.preview = Preview::empty();
            self.image.reset();
            return;
        }
        let entry = self.active_pane().cursor_entry().cloned();
        let path = entry.as_ref().map(|entry| entry.path.clone());
        if self.preview.path == path
            && !matches!(
                self.preview.content,
                PreviewContent::Empty | PreviewContent::Loading
            )
        {
            return;
        }
        self.preview_gen += 1;
        let generation = self.preview_gen;
        self.preview = Preview {
            path: path.clone(),
            content: if path.is_some() {
                PreviewContent::Loading
            } else {
                PreviewContent::Empty
            },
        };
        self.preview_scroll = 0;
        self.preview_hscroll = 0;
        self.image.reset();
        let Some(entry) = entry else {
            return;
        };
        let _ = self.preview_tx.send(PreviewRequest { generation, entry });
    }

    pub fn apply_preview(&mut self, msg: PreviewMsg) {
        if msg.generation != self.preview_gen || self.preview.path.as_ref() != Some(&msg.path) {
            return;
        }
        self.preview.content = msg.content;
        self.image.reset();
    }

    pub fn apply_image(&mut self, msg: ImageMsg) {
        if msg.generation != self.image.generation {
            return;
        }
        if self.image.key.as_ref() != Some(&msg.key) {
            return;
        }
        self.image.pending = false;
        self.image.error = match &msg.payload {
            ImagePayload::Failed { reason } => Some(reason.clone()),
            _ => None,
        };
        self.image.ready = Some(msg);
    }

    pub fn request_git(&mut self, tab: usize, pane: usize) {
        if !self.config.ui.git {
            if let Some(tab) = self.tabs.get_mut(tab) {
                let pane = &mut tab.panes[pane];
                pane.repo = None;
                pane.git.clear();
            }
            return;
        }
        let Some(tab_state) = self.tabs.get(tab) else {
            return;
        };
        let dir = tab_state.panes[pane].dir.clone();
        if let Some(cached) = self.git_cache.get(&dir) {
            let data = cached.clone();
            self.apply_git_data(tab, pane, data);
            return;
        }
        if !self.git_pending.insert(dir.clone()) {
            return;
        }
        let tx = self.git_tx.clone();
        std::thread::spawn(move || {
            let data = git_data(&dir);
            let _ = tx.send(GitMsg {
                dir: dir.clone(),
                data,
            });
        });
    }

    pub fn apply_git(&mut self, msg: GitMsg) {
        self.git_pending.remove(&msg.dir);
        self.git_cache.insert(msg.dir.clone(), msg.data.clone());
        for tab_index in 0..self.tabs.len() {
            for pane_index in 0..2 {
                if self.tabs[tab_index].panes[pane_index].dir == msg.dir {
                    self.apply_git_data(tab_index, pane_index, msg.data.clone());
                }
            }
        }
    }

    fn apply_git_data(&mut self, tab: usize, pane: usize, data: Option<GitData>) {
        if let Some(tab) = self.tabs.get_mut(tab) {
            let pane = &mut tab.panes[pane];
            match data {
                Some(data) => {
                    pane.repo = Some(data.repo);
                    pane.git = data.status;
                }
                None => {
                    pane.repo = None;
                    pane.git.clear();
                }
            }
        }
    }

    pub fn refresh(&mut self) {
        self.refresh_sidebar_targets();
        self.git_cache.clear();
        for tab in 0..self.tabs.len() {
            for pane in 0..2 {
                self.load_pane(tab, pane);
                self.request_git(tab, pane);
            }
        }
        self.request_preview();
    }

    pub fn refresh_active(&mut self) {
        let tab = self.tab;
        let pane = self.pane_index();
        let dir = self.active_pane().dir.clone();
        self.git_cache.remove(&dir);
        self.load_pane(tab, pane);
        self.request_git(tab, pane);
    }

    pub fn navigate(&mut self, path: PathBuf) -> Result<(), String> {
        self.go_to(path, true)
    }

    pub fn go_to(&mut self, path: PathBuf, push: bool) -> Result<(), String> {
        let base = self.current_dir();
        let target = utils::absolutize(&path, &base);
        if !target.is_dir() {
            return Err(format!("not a directory: {}", target.display()));
        }
        let tab = self.tab;
        let pane_index = self.pane_index();
        {
            let pane = &mut self.tabs[tab].panes[pane_index];
            if pane.dir == target {
                return Ok(());
            }
            if push {
                pane.history.truncate(pane.hist_pos + 1);
                if pane.history.last() != Some(&target) {
                    pane.history.push(target.clone());
                    pane.hist_pos = pane.history.len() - 1;
                }
            }
            pane.dir = target;
            pane.filter.clear();
            pane.cursor = 0;
            pane.scroll = 0;
            pane.selection.clear();
            pane.entries.clear();
            pane.view.clear();
        }
        self.preview = Preview::empty();
        self.image.reset();
        self.load_pane(tab, pane_index);
        self.request_git(tab, pane_index);
        self.request_preview();
        self.update_title();
        Ok(())
    }

    pub fn history(&mut self, delta: isize) {
        let tab = self.tab;
        let pane_index = self.pane_index();
        let (target, position) = {
            let pane = &self.tabs[tab].panes[pane_index];
            let next = pane.hist_pos as isize + delta;
            if next < 0 || next as usize >= pane.history.len() {
                return;
            }
            (pane.history[next as usize].clone(), next as usize)
        };
        {
            let pane = &mut self.tabs[tab].panes[pane_index];
            pane.hist_pos = position;
            pane.dir = target;
            pane.filter.clear();
            pane.cursor = 0;
            pane.scroll = 0;
            pane.selection.clear();
            pane.entries.clear();
            pane.view.clear();
        }
        self.preview = Preview::empty();
        self.image.reset();
        self.load_pane(tab, pane_index);
        self.request_git(tab, pane_index);
        self.request_preview();
        self.update_title();
    }

    pub fn parent(&mut self) -> Result<(), String> {
        let parent = self
            .current_dir()
            .parent()
            .map(|p| p.to_path_buf())
            .ok_or("already at the root")?;
        self.navigate(parent)
    }

    pub fn update_title(&mut self) {
        let dir = self.current_dir();
        crate::proc::set_title(&format!("riffle — {}", utils::shorten_home(&dir)));
    }

    pub fn focus_next(&mut self, reverse: bool) {
        let (width, _) = crossterm::terminal::size().unwrap_or((120, 40));
        let mut order = vec![Focus::Files];
        if self.show_sidebar && width >= 60 {
            order.push(Focus::Sidebar);
        }
        if self.show_preview && !self.dual && width >= 90 {
            order.push(Focus::Preview);
        }
        let index = order.iter().position(|f| *f == self.focus).unwrap_or(0);
        let next = if reverse {
            (index + order.len() - 1) % order.len()
        } else {
            (index + 1) % order.len()
        };
        self.focus = order[next];
    }

    pub fn toggle_preview(&mut self) {
        self.show_preview = !self.show_preview;
        if self.show_preview {
            self.request_preview();
        } else {
            if self.focus == Focus::Preview {
                self.focus = Focus::Files;
            }
            self.preview = Preview::empty();
            self.image.reset();
        }
    }

    pub fn toggle_sidebar(&mut self) {
        self.show_sidebar = !self.show_sidebar;
        if !self.show_sidebar && self.focus == Focus::Sidebar {
            self.focus = Focus::Files;
        }
    }

    pub fn toggle_dual(&mut self) {
        self.dual = !self.dual;
        self.focus = Focus::Files;
        let tab = self.tab;
        self.active_tab_mut().focus_right = false;
        if self.dual {
            let current = self.tabs[tab].panes[0].dir.clone();
            if self.tabs[tab].panes[1].generation == 0 {
                let pane = &mut self.tabs[tab].panes[1];
                pane.dir = current.clone();
                pane.history = vec![current];
                pane.hist_pos = 0;
                pane.filter.clear();
                pane.cursor = 0;
                pane.scroll = 0;
                pane.selection.clear();
            }
            for pane in 0..2 {
                self.load_pane(tab, pane);
                self.request_git(tab, pane);
            }
        } else {
            self.load_pane(tab, 0);
            self.request_git(tab, 0);
        }
        self.request_preview();
        self.update_title();
    }

    pub fn load_tab_panes(&mut self, tab: usize) {
        self.load_pane(tab, 0);
        self.request_git(tab, 0);
        if self.dual {
            self.load_pane(tab, 1);
            self.request_git(tab, 1);
        }
    }

    pub fn switch_pane(&mut self) {
        if self.dual {
            let tab = self.active_tab_mut();
            tab.focus_right = !tab.focus_right;
            self.focus = Focus::Files;
            self.request_preview();
            self.update_title();
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        for tab in 0..self.tabs.len() {
            for pane in 0..2 {
                self.load_pane(tab, pane);
            }
        }
        self.request_preview();
    }

    pub fn add_bookmark(&mut self) {
        let path = self.current_dir();
        if self.bookmarks.iter().any(|b| Path::new(&b.path) == path) {
            self.set_info("Already bookmarked");
            return;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let bookmark = Bookmark {
            name,
            path: path.to_string_lossy().into_owned(),
        };
        self.bookmarks.push(bookmark.clone());
        if !self
            .persisted_bookmarks
            .iter()
            .any(|b| b.path == bookmark.path)
        {
            self.persisted_bookmarks.push(bookmark);
        }
        self.refresh_sidebar_targets();
        match config::save_persisted_bookmarks(&self.persisted_bookmarks) {
            Ok(()) => self.set_success("Bookmark added"),
            Err(err) => self.set_error(err),
        }
    }

    pub fn remove_bookmark(&mut self, path: &Path) {
        self.bookmarks.retain(|b| Path::new(&b.path) != path);
        self.persisted_bookmarks
            .retain(|b| Path::new(&b.path) != path);
        if let Err(err) = config::save_persisted_bookmarks(&self.persisted_bookmarks) {
            self.set_error(err);
        } else {
            self.set_success("Bookmark removed");
        }
    }

    pub fn undo_trash(&mut self) {
        let Some(item) = self.undo.pop() else {
            self.set_info("Nothing to undo");
            return;
        };
        match ops::restore_trashed(&item) {
            Ok(()) => {
                self.set_success(format!(
                    "Restored {}",
                    item.original
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ));
                self.refresh_active();
            }
            Err(err) => {
                self.undo.push(item);
                self.set_error(err);
            }
        }
    }

    pub fn reload_config(&mut self) {
        let loaded = config::load();
        self.config = loaded.config;
        self.theme = Theme::from_config(&self.config.theme);
        self.show_hidden = self.config.ui.show_hidden;
        self.show_preview = self.config.ui.preview;
        self.show_sidebar = self.config.ui.sidebar;
        let mut persisted = config::load_persisted_bookmarks();
        persisted.retain(|b| !b.path.is_empty());
        self.persisted_bookmarks = persisted;
        let mut bookmarks = self.config.bookmarks.clone();
        for bookmark in &self.persisted_bookmarks {
            if !bookmarks.iter().any(|b| b.path == bookmark.path) {
                bookmarks.push(bookmark.clone());
            }
        }
        for bookmark in default_bookmarks() {
            if !bookmarks.iter().any(|b| b.path == bookmark.path) {
                bookmarks.push(bookmark);
            }
        }
        self.bookmarks = bookmarks;
        self.refresh_sidebar_targets();
        if !self.show_sidebar && self.focus == Focus::Sidebar {
            self.focus = Focus::Files;
        }
        if !self.show_preview && self.focus == Focus::Preview {
            self.focus = Focus::Files;
        }
        self.git_cache.clear();
        self.refresh();
        self.image.reset();
        match loaded.error {
            Some(err) => self.set_error(err),
            None => self.set_success("Config reloaded"),
        }
    }

    pub fn sidebar_targets(&self) -> Vec<(String, PathBuf)> {
        self.sidebar_targets_cache.clone()
    }

    pub fn refresh_sidebar_targets(&mut self) {
        self.sidebar_gen += 1;
        spawn_sidebar_scan(
            self.sidebar_tx.clone(),
            self.sidebar_gen,
            self.bookmarks.clone(),
        );
        self.clamp_sidebar_cursor();
    }

    pub fn apply_sidebar_targets(&mut self, msg: SidebarMsg) {
        if msg.generation != self.sidebar_gen {
            return;
        }
        self.sidebar_targets_cache = msg.targets;
        self.clamp_sidebar_cursor();
    }

    fn clamp_sidebar_cursor(&mut self) {
        let len = self.sidebar_targets_cache.len();
        if len == 0 {
            self.sidebar_cursor = 0;
        } else if self.sidebar_cursor >= len {
            self.sidebar_cursor = len - 1;
        }
    }
}

fn spawn_sidebar_scan(tx: Sender<SidebarMsg>, generation: u64, bookmarks: Vec<Bookmark>) {
    std::thread::spawn(move || {
        let targets = compute_sidebar_targets(&bookmarks);
        let _ = tx.send(SidebarMsg {
            generation,
            targets,
        });
    });
}

fn fast_sidebar_targets(bookmarks: &[Bookmark]) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = bookmarks
        .iter()
        .map(|bookmark| (bookmark.name.clone(), PathBuf::from(&bookmark.path)))
        .collect();
    let root = PathBuf::from("/");
    if !out.iter().any(|(_, path)| path == &root) {
        out.push(("/ (root)".into(), root));
    }
    out
}

fn compute_sidebar_targets(bookmarks: &[Bookmark]) -> Vec<(String, PathBuf)> {
    let mut out = fast_sidebar_targets(bookmarks);
    for base in ["/media", "/mnt", "/run/media"] {
        let base_path = PathBuf::from(base);
        if !base_path.is_dir() {
            continue;
        }
        let search = std::env::var("USER")
            .ok()
            .map(|user| base_path.join(user))
            .filter(|path| path.is_dir())
            .unwrap_or_else(|| base_path.clone());
        if let Ok(reader) = std::fs::read_dir(&search) {
            let mut mounts: Vec<(String, PathBuf)> = reader
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .filter_map(|path| {
                    let name = path.file_name()?.to_string_lossy().into_owned();
                    Some((name, path))
                })
                .collect();
            mounts.sort_by(|a, b| a.0.cmp(&b.0));
            for mount in mounts {
                if !out.iter().any(|(_, path)| path == &mount.1) {
                    out.push(mount);
                }
            }
        }
    }
    out
}

fn default_bookmarks() -> Vec<Bookmark> {
    let mut out = Vec::new();
    let home = utils::home_dir();
    out.push(Bookmark {
        name: "Home".into(),
        path: home.to_string_lossy().into_owned(),
    });
    for (name, path) in [
        ("Downloads", dirs::download_dir()),
        ("Documents", dirs::document_dir()),
        ("Pictures", dirs::picture_dir()),
        ("Music", dirs::audio_dir()),
        ("Videos", dirs::video_dir()),
    ] {
        if let Some(path) = path {
            if path.is_dir() {
                out.push(Bookmark {
                    name: name.into(),
                    path: path.to_string_lossy().into_owned(),
                });
            }
        }
    }
    out
}

fn spawn_preview_worker(requests: Receiver<PreviewRequest>, results: Sender<PreviewMsg>) {
    std::thread::spawn(move || {
        while let Ok(mut request) = requests.recv() {
            while let Ok(next) = requests.try_recv() {
                request = next;
            }
            let content = load_preview(&request.entry);
            if results
                .send(PreviewMsg {
                    generation: request.generation,
                    path: request.entry.path,
                    content,
                })
                .is_err()
            {
                break;
            }
        }
    });
}

fn load_preview(entry: &FileEntry) -> PreviewContent {
    if entry.is_dir {
        let mut entries = match read_directory(&entry.path, false) {
            Ok(entries) => entries,
            Err(err) => {
                return PreviewContent::Unsupported {
                    reason: err,
                    size: 0,
                }
            }
        };
        sort_entries(&mut entries, SortKey::Name, true);
        let count = entries.len();
        let entries = entries
            .into_iter()
            .take(200)
            .map(|entry| PreviewDirEntry {
                name: entry.name.clone(),
                is_dir: entry.is_dir,
                class: entry.class,
                icon: entry.icon(),
            })
            .collect();
        return PreviewContent::Directory { count, entries };
    }

    match entry.class {
        FileClass::Image => {
            let dims = image::image_dimensions(&entry.path).ok();
            return PreviewContent::Image {
                dims,
                size: entry.size,
            };
        }
        FileClass::Audio | FileClass::Video | FileClass::Archive | FileClass::Font => {
            let reason = match entry.class {
                FileClass::Audio => "audio file",
                FileClass::Video => "video file",
                FileClass::Archive => "archive",
                FileClass::Font => "font file",
                _ => "file",
            };
            return PreviewContent::Unsupported {
                reason: reason.to_string(),
                size: entry.size,
            };
        }
        FileClass::Document => {
            return PreviewContent::Unsupported {
                reason: "document".to_string(),
                size: entry.size,
            };
        }
        _ => {}
    }

    let Ok(mut file) = std::fs::File::open(&entry.path) else {
        return PreviewContent::Unsupported {
            reason: "cannot open file".into(),
            size: entry.size,
        };
    };
    let mut probe = [0u8; 4096];
    let read = file.read(&mut probe).unwrap_or(0);
    if probe[..read].contains(&0) {
        return binary_preview(&entry.path, entry.size);
    }
    let _ = file.seek(SeekFrom::Start(0));
    let mut lines = Vec::new();
    let mut truncated = false;
    let mut current = Vec::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).unwrap_or(0);
        if read == 0 {
            break;
        }
        for &byte in &buffer[..read] {
            if byte == b'\n' {
                if lines.len() >= MAX_PREVIEW_LINES {
                    truncated = true;
                    break;
                }
                let (line, cut) = finalize_line(&current);
                truncated |= cut;
                lines.push(line);
                current.clear();
            } else if current.len() < MAX_PREVIEW_LINE_BYTES {
                current.push(byte);
            } else {
                truncated = true;
            }
        }
        if lines.len() >= MAX_PREVIEW_LINES {
            truncated = true;
            break;
        }
    }
    if lines.len() < MAX_PREVIEW_LINES && !current.is_empty() {
        let (line, cut) = finalize_line(&current);
        truncated |= cut;
        lines.push(line);
    }
    let lang = Lang::from_path(&entry.path, entry.class);
    let tokens = tokenize(&lines, lang);
    let line_count = tokens.len();
    PreviewContent::Text {
        tokens,
        line_count,
        truncated,
        lang,
    }
}

fn finalize_line(bytes: &[u8]) -> (String, bool) {
    let text = String::from_utf8_lossy(bytes).replace('\t', "    ");
    if text.chars().count() > MAX_PREVIEW_LINE_CHARS {
        (text.chars().take(MAX_PREVIEW_LINE_CHARS).collect(), true)
    } else {
        (text, false)
    }
}

fn binary_preview(path: &Path, size: u64) -> PreviewContent {
    let mut bytes = vec![0u8; 512];
    let count = std::fs::File::open(path)
        .and_then(|mut file| file.read(&mut bytes))
        .unwrap_or(0);
    bytes.truncate(count);
    PreviewContent::Binary {
        size,
        hex: utils::hex_dump(&bytes, 32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_editing() {
        let mut input = TextInput::new();
        input.insert_str("hello");
        assert_eq!(input.value(), "hello");
        input.left();
        input.left();
        input.insert_char('X');
        assert_eq!(input.value(), "helXlo");
        input.backspace();
        assert_eq!(input.value(), "hello");
        input.home();
        input.delete();
        assert_eq!(input.value(), "ello");
    }

    #[test]
    fn rename_selects_stem() {
        let mut input = TextInput::from_name_select_stem("photo.jpg");
        assert_eq!(input.selection(), Some((0, 5)));
        input.insert_char('x');
        assert_eq!(input.value(), "x.jpg");
    }

    #[test]
    fn typing_into_empty_select_all_keeps_every_char() {
        let mut input = TextInput::new();
        input.select_all();
        for ch in "main".chars() {
            input.insert_char(ch);
        }
        assert_eq!(input.value(), "main");
    }

    #[test]
    fn word_navigation() {
        let mut input = TextInput::with_value("one two three");
        input.word_left();
        assert_eq!(input.cursor, 8);
        input.word_left();
        assert_eq!(input.cursor, 4);
        input.backspace_word();
        assert_eq!(input.value(), "two three");
    }

    #[test]
    fn filter_view_on_pane() {
        let mut pane = Pane::new(PathBuf::from("/tmp"), SortKey::Name, true);
        pane.entries = vec![
            FileEntry {
                path: PathBuf::from("/tmp/a.txt"),
                name: "a.txt".into(),
                is_dir: false,
                is_symlink: false,
                broken: false,
                symlink_target: None,
                hidden: false,
                size: 1,
                modified: None,
                mode: 0o644,
                class: FileClass::Text,
                ext: "txt".into(),
            },
            FileEntry {
                path: PathBuf::from("/tmp/b.txt"),
                name: "b.txt".into(),
                is_dir: false,
                is_symlink: false,
                broken: false,
                symlink_target: None,
                hidden: false,
                size: 2,
                modified: None,
                mode: 0o644,
                class: FileClass::Text,
                ext: "txt".into(),
            },
        ];
        pane.filter = "b".into();
        pane.rebuild(true);
        assert_eq!(pane.view_len(), 1);
        assert_eq!(pane.cursor_entry().unwrap().name, "b.txt");
    }

    #[test]
    fn palette_filters_actions() {
        let palette = Palette::new(crate::action::Action::all().to_vec());
        assert!(!palette.filtered.is_empty());
    }
}
