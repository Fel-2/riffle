use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Instant;

use crate::filesystem::{FileClass, SortKey};
use crate::ops;
use crate::proc;
use crate::state::{
    App, Clipboard, Confirm, ConfirmAction, Focus, HelpState, InfoState, Mode, Palette, Prompt,
    PromptKind, Task, TaskMsg,
};
use crate::utils;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveDown,
    MoveUp,
    PageDown,
    PageUp,
    Top,
    Bottom,
    Open,
    Parent,
    Back,
    Forward,
    Home,
    Select,
    SelectAll,
    ClearSelection,
    Copy,
    Cut,
    Paste,
    Trash,
    Delete,
    Rename,
    NewFile,
    NewDir,
    Edit,
    Info,
    CopyPath,
    OpenTerminal,
    Extract,
    Compress,
    Share,
    Filter,
    GoTo,
    Shell,
    Refresh,
    ToggleHidden,
    TogglePreview,
    ToggleSidebar,
    ToggleDual,
    SwitchPane,
    FocusSidebar,
    SortNext,
    SortOrder,
    SortByName,
    SortBySize,
    SortByModified,
    SortByKind,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    Bookmark,
    RemoveBookmark,
    Undo,
    ReloadConfig,
    CheckUpdates,
    Help,
    Palette,
    Quit,
    PreviewDown,
    PreviewUp,
    PreviewPageDown,
    PreviewPageUp,
    PreviewTop,
    PreviewBottom,
    PreviewLeft,
    PreviewRight,
    SidebarDown,
    SidebarUp,
    SidebarTop,
    SidebarBottom,
    SidebarOpen,
}

pub enum Effect {
    None,
    Quit,
    Suspend(Suspend),
}

pub enum Suspend {
    Editor(PathBuf),
    Shell(String),
}

impl Action {
    pub fn all() -> &'static [Action] {
        &[
            Action::Open,
            Action::Edit,
            Action::Info,
            Action::Copy,
            Action::Cut,
            Action::Paste,
            Action::Trash,
            Action::Delete,
            Action::Rename,
            Action::NewFile,
            Action::NewDir,
            Action::Select,
            Action::SelectAll,
            Action::ClearSelection,
            Action::CopyPath,
            Action::OpenTerminal,
            Action::Extract,
            Action::Compress,
            Action::Share,
            Action::Filter,
            Action::GoTo,
            Action::Shell,
            Action::Refresh,
            Action::ToggleHidden,
            Action::TogglePreview,
            Action::ToggleSidebar,
            Action::ToggleDual,
            Action::SwitchPane,
            Action::FocusSidebar,
            Action::SortNext,
            Action::SortOrder,
            Action::SortByName,
            Action::SortBySize,
            Action::SortByModified,
            Action::SortByKind,
            Action::NewTab,
            Action::CloseTab,
            Action::NextTab,
            Action::PrevTab,
            Action::Bookmark,
            Action::RemoveBookmark,
            Action::Undo,
            Action::ReloadConfig,
            Action::CheckUpdates,
            Action::Help,
            Action::Palette,
            Action::Parent,
            Action::Back,
            Action::Forward,
            Action::Home,
            Action::Top,
            Action::Bottom,
            Action::PageDown,
            Action::PageUp,
            Action::Quit,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::MoveDown => "Move down",
            Action::MoveUp => "Move up",
            Action::PageDown => "Page down",
            Action::PageUp => "Page up",
            Action::Top => "Go to top",
            Action::Bottom => "Go to bottom",
            Action::Open => "Open",
            Action::Parent => "Go to parent directory",
            Action::Back => "Go back",
            Action::Forward => "Go forward",
            Action::Home => "Go home",
            Action::Select => "Select and move down",
            Action::SelectAll => "Select all",
            Action::ClearSelection => "Clear selection",
            Action::Copy => "Copy",
            Action::Cut => "Cut",
            Action::Paste => "Paste",
            Action::Trash => "Move to trash",
            Action::Delete => "Delete permanently",
            Action::Rename => "Rename",
            Action::NewFile => "New file",
            Action::NewDir => "New directory",
            Action::Edit => "Open in editor",
            Action::Info => "File info",
            Action::CopyPath => "Copy path to clipboard",
            Action::OpenTerminal => "Open terminal here",
            Action::Extract => "Extract archive",
            Action::Compress => "Compress to archive",
            Action::Share => "Share via KDE Connect",
            Action::Filter => "Filter files",
            Action::GoTo => "Go to path",
            Action::Shell => "Run shell command",
            Action::Refresh => "Refresh",
            Action::ToggleHidden => "Toggle hidden files",
            Action::TogglePreview => "Toggle preview",
            Action::ToggleSidebar => "Toggle sidebar",
            Action::ToggleDual => "Toggle dual pane",
            Action::SwitchPane => "Switch pane",
            Action::FocusSidebar => "Focus sidebar",
            Action::SortNext => "Next sort field",
            Action::SortOrder => "Reverse sort order",
            Action::SortByName => "Sort by name",
            Action::SortBySize => "Sort by size",
            Action::SortByModified => "Sort by modified",
            Action::SortByKind => "Sort by type",
            Action::NewTab => "New tab",
            Action::CloseTab => "Close tab",
            Action::NextTab => "Next tab",
            Action::PrevTab => "Previous tab",
            Action::Bookmark => "Bookmark current directory",
            Action::RemoveBookmark => "Remove bookmark for current directory",
            Action::Undo => "Undo last trash",
            Action::ReloadConfig => "Reload configuration",
            Action::CheckUpdates => "Check for updates",
            Action::Help => "Show keybindings",
            Action::Palette => "Command palette",
            Action::Quit => "Quit",
            Action::PreviewDown => "Scroll preview down",
            Action::PreviewUp => "Scroll preview up",
            Action::PreviewPageDown => "Preview page down",
            Action::PreviewPageUp => "Preview page up",
            Action::PreviewTop => "Preview to top",
            Action::PreviewBottom => "Preview to bottom",
            Action::PreviewLeft => "Scroll preview left",
            Action::PreviewRight => "Scroll preview right",
            Action::SidebarDown => "Sidebar down",
            Action::SidebarUp => "Sidebar up",
            Action::SidebarTop => "Sidebar top",
            Action::SidebarBottom => "Sidebar bottom",
            Action::SidebarOpen => "Open sidebar item",
        }
    }

    pub fn section(self) -> &'static str {
        match self {
            Action::Open
            | Action::Edit
            | Action::Info
            | Action::CopyPath
            | Action::OpenTerminal
            | Action::Parent
            | Action::Back
            | Action::Forward
            | Action::Home
            | Action::Top
            | Action::Bottom
            | Action::PageDown
            | Action::PageUp
            | Action::MoveDown
            | Action::MoveUp
            | Action::SidebarDown
            | Action::SidebarUp
            | Action::SidebarTop
            | Action::SidebarBottom
            | Action::SidebarOpen
            | Action::PreviewDown
            | Action::PreviewUp
            | Action::PreviewPageDown
            | Action::PreviewPageUp
            | Action::PreviewTop
            | Action::PreviewBottom
            | Action::PreviewLeft
            | Action::PreviewRight => "Navigate",
            Action::Copy
            | Action::Cut
            | Action::Paste
            | Action::Trash
            | Action::Delete
            | Action::Rename
            | Action::NewFile
            | Action::NewDir
            | Action::Select
            | Action::SelectAll
            | Action::ClearSelection
            | Action::Extract
            | Action::Compress
            | Action::Share
            | Action::Undo => "Files",
            Action::Filter
            | Action::GoTo
            | Action::Shell
            | Action::Refresh
            | Action::ToggleHidden
            | Action::TogglePreview
            | Action::ToggleSidebar
            | Action::SortNext
            | Action::SortOrder
            | Action::SortByName
            | Action::SortBySize
            | Action::SortByModified
            | Action::SortByKind
            | Action::Bookmark
            | Action::RemoveBookmark => "View",
            Action::ToggleDual
            | Action::SwitchPane
            | Action::FocusSidebar
            | Action::NewTab
            | Action::CloseTab
            | Action::NextTab
            | Action::PrevTab => "Layout",
            Action::ReloadConfig
            | Action::CheckUpdates
            | Action::Help
            | Action::Palette
            | Action::Quit => "App",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Action::MoveDown => "j ↓",
            Action::MoveUp => "k ↑",
            Action::PageDown => "PgDn",
            Action::PageUp => "PgUp",
            Action::Top => "g",
            Action::Bottom => "G",
            Action::Open => "⏎ l →",
            Action::Parent => "h ← ⌫",
            Action::Back => "alt+←",
            Action::Forward => "alt+→",
            Action::Home => "~",
            Action::Select => "space",
            Action::SelectAll => "V",
            Action::ClearSelection => "esc",
            Action::Copy => "y",
            Action::Cut => "x",
            Action::Paste => "p",
            Action::Trash => "d del",
            Action::Delete => "D",
            Action::Rename => "r F2",
            Action::NewFile => "a",
            Action::NewDir => "A",
            Action::Edit => "e",
            Action::Info => "i",
            Action::CopyPath => "Y",
            Action::OpenTerminal => "T",
            Action::Extract => "E",
            Action::Compress => "Z",
            Action::Share => "K",
            Action::Filter => "/",
            Action::GoTo => "ctrl+g",
            Action::Shell => ":",
            Action::Refresh => "R F5",
            Action::ToggleHidden => ".",
            Action::TogglePreview => "P",
            Action::ToggleSidebar => "B",
            Action::ToggleDual => "ctrl+w",
            Action::SwitchPane => "tab",
            Action::FocusSidebar => "b",
            Action::SortNext => "s",
            Action::SortOrder => "S",
            Action::SortByName => "",
            Action::SortBySize => "",
            Action::SortByModified => "",
            Action::SortByKind => "",
            Action::NewTab => "ctrl+t",
            Action::CloseTab => "X",
            Action::NextTab => "]",
            Action::PrevTab => "[",
            Action::Bookmark => "ctrl+b",
            Action::RemoveBookmark => "",
            Action::Undo => "ctrl+z",
            Action::ReloadConfig => "",
            Action::CheckUpdates => "",
            Action::Help => "? F1",
            Action::Palette => "ctrl+p",
            Action::Quit => "q",
            Action::PreviewDown => "j",
            Action::PreviewUp => "k",
            Action::PreviewPageDown => "PgDn",
            Action::PreviewPageUp => "PgUp",
            Action::PreviewTop => "g",
            Action::PreviewBottom => "G",
            Action::PreviewLeft => "h",
            Action::PreviewRight => "l",
            Action::SidebarDown => "j",
            Action::SidebarUp => "k",
            Action::SidebarTop => "g",
            Action::SidebarBottom => "G",
            Action::SidebarOpen => "⏎",
        }
    }
}

pub fn dispatch(app: &mut App, action: Action) -> Effect {
    match action {
        Action::MoveDown => move_cursor(app, 1),
        Action::MoveUp => move_cursor(app, -1),
        Action::PageDown => page(app, 1),
        Action::PageUp => page(app, -1),
        Action::Top => set_cursor(app, 0),
        Action::Bottom => set_cursor(app, usize::MAX),
        Action::Open => return open_current(app),
        Action::Parent => {
            if let Err(err) = app.parent() {
                app.set_error(err);
            }
        }
        Action::Back => app.history(-1),
        Action::Forward => app.history(1),
        Action::Home => {
            if let Err(err) = app.navigate(utils::home_dir()) {
                app.set_error(err);
            }
        }
        Action::Select => toggle_select(app),
        Action::SelectAll => select_all(app),
        Action::ClearSelection => {
            let fuzzy = app.fuzzy();
            app.active_pane_mut().selection.clear();
            app.active_pane_mut().clear_filter(fuzzy);
            app.request_preview();
        }
        Action::Copy => set_clipboard(app, false),
        Action::Cut => set_clipboard(app, true),
        Action::Paste => paste(app),
        Action::Trash => request_trash(app),
        Action::Delete => request_delete(app),
        Action::Rename => start_rename(app),
        Action::NewFile => open_prompt(app, Prompt::new_file()),
        Action::NewDir => open_prompt(app, Prompt::new_dir()),
        Action::Edit => return edit_current(app),
        Action::Info => show_info(app),
        Action::CopyPath => copy_paths(app),
        Action::OpenTerminal => {
            if let Err(err) = proc::open_terminal(&app.current_dir(), &app.config) {
                app.set_error(err);
            }
        }
        Action::Extract => extract(app),
        Action::Compress => compress(app),
        Action::Share => share(app),
        Action::Filter => open_prompt(app, Prompt::filter(&app.active_pane().filter)),
        Action::GoTo => open_prompt(app, Prompt::goto(&app.current_dir())),
        Action::Shell => open_prompt(app, Prompt::shell()),
        Action::Refresh => {
            app.refresh_active();
            app.set_info("Refreshed");
        }
        Action::ToggleHidden => {
            app.toggle_hidden();
            let state = if app.show_hidden { "shown" } else { "hidden" };
            app.set_info(format!("Hidden files {state}"));
        }
        Action::TogglePreview => app.toggle_preview(),
        Action::ToggleSidebar => app.toggle_sidebar(),
        Action::ToggleDual => app.toggle_dual(),
        Action::SwitchPane => app.switch_pane(),
        Action::FocusSidebar => focus_sidebar(app),
        Action::SortNext => {
            let next = app.active_pane().sort.next();
            set_sort(app, next);
        }
        Action::SortOrder => {
            let ascending = !app.active_pane().ascending;
            let fuzzy = app.fuzzy();
            {
                let pane = app.active_pane_mut();
                pane.ascending = ascending;
                pane.rebuild(fuzzy);
            }
            app.request_preview();
            app.set_info(format!(
                "Sort {}",
                if ascending { "ascending" } else { "descending" }
            ));
        }
        Action::SortByName => set_sort(app, SortKey::Name),
        Action::SortBySize => set_sort(app, SortKey::Size),
        Action::SortByModified => set_sort(app, SortKey::Modified),
        Action::SortByKind => set_sort(app, SortKey::Kind),
        Action::NewTab => new_tab(app),
        Action::CloseTab => close_tab(app),
        Action::NextTab => switch_tab(app, 1),
        Action::PrevTab => switch_tab(app, -1),
        Action::Bookmark => app.add_bookmark(),
        Action::RemoveBookmark => {
            let path = app.current_dir();
            app.remove_bookmark(&path);
        }
        Action::Undo => app.undo_trash(),
        Action::ReloadConfig => app.reload_config(),
        Action::CheckUpdates => app.check_updates(),
        Action::Help => {
            app.help = Some(HelpState { scroll: 0 });
            app.mode = Mode::Help;
        }
        Action::Palette => {
            app.palette = Some(Palette::new(Action::all().to_vec()));
            app.mode = Mode::Palette;
        }
        Action::Quit => return Effect::Quit,
        Action::PreviewDown => {
            app.preview_scroll = app.preview_scroll.saturating_add(1);
        }
        Action::PreviewUp => {
            app.preview_scroll = app.preview_scroll.saturating_sub(1);
        }
        Action::PreviewPageDown => {
            app.preview_scroll = app.preview_scroll.saturating_add(10);
        }
        Action::PreviewPageUp => {
            app.preview_scroll = app.preview_scroll.saturating_sub(10);
        }
        Action::PreviewTop => app.preview_scroll = 0,
        Action::PreviewBottom => app.preview_scroll = usize::MAX,
        Action::PreviewLeft => {
            app.preview_hscroll = app.preview_hscroll.saturating_sub(2);
        }
        Action::PreviewRight => {
            app.preview_hscroll = app.preview_hscroll.saturating_add(2);
        }
        Action::SidebarDown => sidebar_move(app, 1),
        Action::SidebarUp => sidebar_move(app, -1),
        Action::SidebarTop => app.sidebar_cursor = 0,
        Action::SidebarBottom => {
            let len = app.sidebar_targets().len();
            app.sidebar_cursor = len.saturating_sub(1);
        }
        Action::SidebarOpen => open_sidebar(app),
    }
    Effect::None
}

fn move_cursor(app: &mut App, delta: isize) {
    let len = app.active_pane().view_len();
    if len == 0 {
        return;
    }
    let current = app.active_pane().cursor as isize;
    let next = (current + delta).clamp(0, len as isize - 1) as usize;
    app.active_pane_mut().cursor = next;
    app.active_pane_mut().ensure_scroll();
    app.request_preview();
}

fn page(app: &mut App, direction: isize) {
    let height = app.active_pane().height.saturating_sub(1).max(1) as isize;
    move_cursor(app, height * direction);
}

fn set_cursor(app: &mut App, row: usize) {
    let len = app.active_pane().view_len();
    if len == 0 {
        return;
    }
    let row = row.min(len - 1);
    app.active_pane_mut().cursor = row;
    app.active_pane_mut().ensure_scroll();
    app.request_preview();
}

fn toggle_select(app: &mut App) {
    let Some(path) = app.active_pane().cursor_entry().map(|e| e.path.clone()) else {
        return;
    };
    let mut selection = app.active_pane().selection.clone();
    if !selection.remove(&path) {
        selection.insert(path);
    }
    app.active_pane_mut().selection = selection;
    move_cursor(app, 1);
}

fn select_all(app: &mut App) {
    let paths: Vec<PathBuf> = app
        .active_pane()
        .view
        .iter()
        .filter_map(|row| {
            app.active_pane()
                .entries
                .get(row.index)
                .map(|e| e.path.clone())
        })
        .collect();
    let count = paths.len();
    app.active_pane_mut().selection.extend(paths);
    app.set_info(format!("Selected {count}"));
}

fn set_clipboard(app: &mut App, cut: bool) {
    let paths = app.active_pane().selected_paths();
    if paths.is_empty() {
        return;
    }
    let count = paths.len();
    app.clipboard = Some(Clipboard { paths, cut });
    app.set_info(format!(
        "{} {count} item(s)",
        if cut { "Cut" } else { "Copied" }
    ));
}

fn open_current(app: &mut App) -> Effect {
    let Some(entry) = app.active_pane().cursor_entry().cloned() else {
        return Effect::None;
    };
    if entry.is_dir {
        if let Err(err) = app.navigate(entry.path.clone()) {
            app.set_error(err);
        }
        return Effect::None;
    }
    if entry.is_text() {
        return Effect::Suspend(Suspend::Editor(entry.path));
    }
    match proc::open_file(&entry.path, entry.class, &app.config) {
        Ok(()) => app.set_info(format!("Opened {}", entry.name)),
        Err(err) => app.set_error(err),
    }
    Effect::None
}

fn edit_current(app: &mut App) -> Effect {
    let Some(entry) = app.active_pane().cursor_entry().cloned() else {
        return Effect::None;
    };
    if entry.is_dir {
        app.set_error("Cannot edit a directory");
        return Effect::None;
    }
    Effect::Suspend(Suspend::Editor(entry.path))
}

fn copy_paths(app: &mut App) {
    let paths = app.active_pane().selected_paths();
    if paths.is_empty() {
        return;
    }
    let text = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    match utils::osc52(&text) {
        Ok(()) => app.set_success(format!("Copied {} path(s)", paths.len())),
        Err(err) => app.set_error(format!("clipboard: {err}")),
    }
}

fn start_rename(app: &mut App) {
    let Some(entry) = app.active_pane().cursor_entry() else {
        return;
    };
    let prompt = Prompt::rename(entry.path.clone());
    open_prompt(app, prompt);
}

fn open_prompt(app: &mut App, prompt: Prompt) {
    app.prompt = Some(prompt);
    app.mode = Mode::Prompt;
}

fn set_sort(app: &mut App, sort: SortKey) {
    let fuzzy = app.fuzzy();
    let ascending = {
        let pane = app.active_pane_mut();
        pane.sort = sort;
        pane.rebuild(fuzzy);
        pane.ascending
    };
    app.request_preview();
    let label = sort.label();
    app.set_info(format!(
        "Sorted by {label} ({})",
        if ascending { "ascending" } else { "descending" }
    ));
}

fn focus_sidebar(app: &mut App) {
    let (width, _) = crossterm::terminal::size().unwrap_or((120, 40));
    if width < 60 {
        app.set_info("Sidebar is hidden on narrow terminals");
        return;
    }
    if !app.show_sidebar {
        app.show_sidebar = true;
    }
    let current = app.current_dir();
    let targets = app.sidebar_targets();
    app.sidebar_cursor = targets
        .iter()
        .position(|(_, path)| *path == current)
        .unwrap_or(0);
    app.focus = Focus::Sidebar;
}

fn sidebar_move(app: &mut App, delta: isize) {
    let len = app.sidebar_targets().len();
    if len == 0 {
        return;
    }
    let current = app.sidebar_cursor as isize;
    app.sidebar_cursor = (current + delta).clamp(0, len as isize - 1) as usize;
}

fn open_sidebar(app: &mut App) {
    let targets = app.sidebar_targets();
    let Some((_, path)) = targets.get(app.sidebar_cursor).cloned() else {
        return;
    };
    app.focus = Focus::Files;
    if let Err(err) = app.navigate(path) {
        app.set_error(err);
    }
}

fn new_tab(app: &mut App) {
    let dir = app.current_dir();
    let sort = app.active_pane().sort;
    let ascending = app.active_pane().ascending;
    app.tabs.push(crate::state::Tab::new(dir, sort, ascending));
    app.tab = app.tabs.len() - 1;
    app.focus = Focus::Files;
    let tab = app.tab;
    app.load_tab_panes(tab);
    app.request_preview();
}

fn close_tab(app: &mut App) {
    if app.tabs.len() == 1 {
        app.set_info("Cannot close the last tab");
        return;
    }
    app.tabs.remove(app.tab);
    if app.tab >= app.tabs.len() {
        app.tab = app.tabs.len() - 1;
    }
    app.focus = Focus::Files;
    let tab = app.tab;
    app.load_tab_panes(tab);
    app.request_preview();
}

fn switch_tab(app: &mut App, delta: isize) {
    if app.tabs.len() < 2 {
        return;
    }
    let len = app.tabs.len() as isize;
    app.tab = ((app.tab as isize + delta).rem_euclid(len)) as usize;
    app.focus = Focus::Files;
    let tab = app.tab;
    app.load_tab_panes(tab);
    app.request_preview();
}

fn show_info(app: &mut App) {
    let Some(entry) = app.active_pane().cursor_entry().cloned() else {
        return;
    };
    let mut rows = vec![
        ("Name".to_string(), entry.name.clone()),
        ("Path".to_string(), entry.path.display().to_string()),
        ("Type".to_string(), entry.class.label().to_string()),
    ];
    if entry.is_dir {
        rows.push(("Contents".to_string(), directory_summary(&entry.path)));
    } else {
        rows.push(("Size".to_string(), utils::format_size(entry.size)));
    }
    if let Some(modified) = entry.modified {
        rows.push((
            "Modified".to_string(),
            utils::format_time(modified, crate::utils::DateStyle::Iso),
        ));
    }
    if let Ok(meta) = std::fs::symlink_metadata(&entry.path) {
        rows.push(("Mode".to_string(), utils::format_type_mode(&meta)));
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            rows.push((
                "Owner".to_string(),
                format!("{}:{}", meta.uid(), meta.gid()),
            ));
            rows.push(("Links".to_string(), meta.nlink().to_string()));
            rows.push(("Inode".to_string(), meta.ino().to_string()));
        }
    } else {
        rows.push(("Mode".to_string(), utils::format_mode(entry.mode)));
    }
    if let Some(target) = &entry.symlink_target {
        rows.push(("Target".to_string(), target.display().to_string()));
    }
    if entry.class == FileClass::Image {
        if let Ok((w, h)) = image::image_dimensions(&entry.path) {
            rows.push(("Dimensions".to_string(), format!("{w} × {h}")));
        }
    }
    app.info = Some(InfoState {
        title: entry.name,
        rows,
        scroll: 0,
    });
    app.mode = Mode::Info;
}

fn directory_summary(path: &std::path::Path) -> String {
    match std::fs::read_dir(path) {
        Ok(reader) => {
            let count = reader.flatten().take(1001).count();
            if count > 1000 {
                "1000+ items".to_string()
            } else {
                utils::format_count(count, "item")
            }
        }
        Err(err) => err.to_string(),
    }
}

fn request_trash(app: &mut App) {
    if app.task.is_some() {
        app.set_error("Another operation is running");
        return;
    }
    let paths = app.active_pane().selected_paths();
    if paths.is_empty() {
        return;
    }
    if app.config.behavior.confirm_trash {
        app.confirm = Some(Confirm {
            title: "Move to trash?".into(),
            message: format!("Move {} to the trash?", ops::describe_paths(&paths)),
            detail: "You can restore it with Ctrl+Z.".into(),
            action: ConfirmAction::Trash(paths),
            destructive: false,
        });
        app.mode = Mode::Confirm;
    } else {
        trash_paths(app, paths);
    }
}

fn request_delete(app: &mut App) {
    if app.task.is_some() {
        app.set_error("Another operation is running");
        return;
    }
    let paths = app.active_pane().selected_paths();
    if paths.is_empty() {
        return;
    }
    app.confirm = Some(Confirm {
        title: "Delete permanently?".into(),
        message: format!("Delete {}?", ops::describe_paths(&paths)),
        detail: "This cannot be undone.".into(),
        action: ConfirmAction::Delete(paths),
        destructive: true,
    });
    app.mode = Mode::Confirm;
}

pub fn confirm_yes(app: &mut App) {
    let Some(confirm) = app.confirm.take() else {
        app.mode = Mode::Normal;
        return;
    };
    app.mode = Mode::Normal;
    match confirm.action {
        ConfirmAction::Trash(paths) => trash_paths(app, paths),
        ConfirmAction::Delete(paths) => spawn_delete(app, paths),
    }
}

pub fn confirm_no(app: &mut App) {
    app.confirm = None;
    app.mode = Mode::Normal;
}

fn trash_paths(app: &mut App, paths: Vec<PathBuf>) {
    let mut errors = Vec::new();
    let mut restored = 0;
    for path in &paths {
        match ops::trash_file(path) {
            Ok(item) => {
                app.undo.push(item);
                restored += 1;
            }
            Err(err) => errors.push(err),
        }
    }
    app.active_pane_mut().selection.clear();
    app.refresh_active();
    if errors.is_empty() {
        app.set_success(format!("Trashed {restored} item(s) — Ctrl+Z to undo"));
    } else {
        app.set_error(format!(
            "Trashed {restored}/{} — {}",
            paths.len(),
            errors.join("; ")
        ));
    }
}

fn spawn_delete(app: &mut App, paths: Vec<PathBuf>) {
    let total = paths.len();
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_worker = cancel.clone();
    std::thread::spawn(move || {
        let mut ok = 0;
        let mut errors = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            if cancel_worker.load(Ordering::Relaxed) {
                errors.push("cancelled".into());
                break;
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let _ = tx.send(TaskMsg::Progress {
                done: index,
                current: name,
            });
            match ops::delete_path(path) {
                Ok(()) => ok += 1,
                Err(err) => errors.push(err),
            }
        }
        let _ = tx.send(TaskMsg::Done {
            ok,
            total,
            errors,
            note: None,
        });
    });
    app.task = Some(Task {
        label: "Deleting".into(),
        rx,
        done: 0,
        total,
        current: String::new(),
        cancel,
        started: Instant::now(),
    });
}

fn paste(app: &mut App) {
    if app.task.is_some() {
        app.set_error("Another operation is running");
        return;
    }
    let Some(clipboard) = app.clipboard.clone() else {
        app.set_info("Clipboard is empty");
        return;
    };
    let dest = app.current_dir();
    let total = clipboard.paths.len();
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_worker = cancel.clone();
    let label = if clipboard.cut { "Moving" } else { "Copying" };
    let is_cut = clipboard.cut;
    std::thread::spawn(move || {
        let mut ok = 0;
        let mut errors = Vec::new();
        for (index, path) in clipboard.paths.iter().enumerate() {
            if cancel_worker.load(Ordering::Relaxed) {
                errors.push("cancelled".into());
                break;
            }
            let name = path.file_name().unwrap_or_default();
            let name_text = name.to_string_lossy().into_owned();
            let _ = tx.send(TaskMsg::Progress {
                done: index,
                current: name_text.clone(),
            });
            let target = ops::unique_dest(&dest, name);
            let result = if is_cut {
                ops::move_path(path, &target)
            } else {
                ops::copy_path(path, &target)
            };
            match result {
                Ok(()) => ok += 1,
                Err(err) => errors.push(format!("{name_text}: {err}")),
            }
        }
        let _ = tx.send(TaskMsg::Done {
            ok,
            total,
            errors,
            note: None,
        });
    });
    if is_cut {
        app.clipboard = None;
    }
    app.active_pane_mut().selection.clear();
    app.task = Some(Task {
        label: label.into(),
        rx,
        done: 0,
        total,
        current: String::new(),
        cancel,
        started: Instant::now(),
    });
}

fn extract(app: &mut App) {
    let Some(entry) = app.active_pane().cursor_entry().cloned() else {
        return;
    };
    if entry.class != FileClass::Archive {
        app.set_error("Not an archive");
        return;
    }
    if app.task.is_some() {
        app.set_error("Another operation is running");
        return;
    }
    let path = entry.path.clone();
    let name = entry.name.clone();
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    std::thread::spawn(move || {
        let _ = tx.send(TaskMsg::Progress {
            done: 0,
            current: name,
        });
        match ops::extract_archive(&path) {
            Ok(dest) => {
                let note = format!(
                    "Extracted to {}",
                    dest.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| dest.display().to_string())
                );
                let _ = tx.send(TaskMsg::Done {
                    ok: 1,
                    total: 1,
                    errors: Vec::new(),
                    note: Some(note),
                });
            }
            Err(err) => {
                let _ = tx.send(TaskMsg::Done {
                    ok: 0,
                    total: 1,
                    errors: vec![err],
                    note: None,
                });
            }
        }
    });
    app.task = Some(Task {
        label: "Extracting".into(),
        rx,
        done: 0,
        total: 1,
        current: String::new(),
        cancel,
        started: Instant::now(),
    });
}

fn compress(app: &mut App) {
    if app.task.is_some() {
        app.set_error("Another operation is running");
        return;
    }
    let paths = app.active_pane().selected_paths();
    if paths.is_empty() {
        return;
    }
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    std::thread::spawn(move || {
        let _ = tx.send(TaskMsg::Progress {
            done: 0,
            current: String::new(),
        });
        match ops::compress(&paths) {
            Ok(archive) => {
                let note = format!(
                    "Created {}",
                    archive
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| archive.display().to_string())
                );
                let _ = tx.send(TaskMsg::Done {
                    ok: 1,
                    total: 1,
                    errors: Vec::new(),
                    note: Some(note),
                });
            }
            Err(err) => {
                let _ = tx.send(TaskMsg::Done {
                    ok: 0,
                    total: 1,
                    errors: vec![err],
                    note: None,
                });
            }
        }
    });
    app.task = Some(Task {
        label: "Compressing".into(),
        rx,
        done: 0,
        total: 1,
        current: String::new(),
        cancel,
        started: Instant::now(),
    });
}

fn share(app: &mut App) {
    let paths = app.active_pane().selected_paths();
    match proc::share_kdeconnect(&paths) {
        Ok(message) => app.set_success(message),
        Err(err) => app.set_error(err),
    }
}

pub fn submit_prompt(app: &mut App) -> Effect {
    let Some(prompt) = app.prompt.take() else {
        app.mode = Mode::Normal;
        return Effect::None;
    };
    let value = prompt.input.value();
    match &prompt.kind {
        PromptKind::Filter => {
            app.mode = Mode::Normal;
            app.active_pane_mut().filter = value;
            let fuzzy = app.fuzzy();
            app.active_pane_mut().rebuild(fuzzy);
            app.request_preview();
        }
        PromptKind::GoTo => {
            let expanded = expand_path(&value);
            match app.go_to(expanded, true) {
                Ok(()) => app.mode = Mode::Normal,
                Err(err) => {
                    app.set_error(err);
                    app.prompt = Some(prompt);
                }
            }
        }
        PromptKind::Rename { path } => match ops::rename_path(path, &value) {
            Ok(new_path) => {
                app.mode = Mode::Normal;
                app.set_success(format!(
                    "Renamed to {}",
                    new_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ));
                app.refresh_active();
            }
            Err(err) => {
                app.set_error(err);
                app.prompt = Some(prompt);
            }
        },
        PromptKind::NewFile => match ops::create_file(&app.current_dir(), &value) {
            Ok(path) => {
                app.mode = Mode::Normal;
                app.set_success(format!(
                    "Created {}",
                    path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ));
                app.pending_select = Some(path);
                app.refresh_active();
            }
            Err(err) => {
                app.set_error(err);
                app.prompt = Some(prompt);
            }
        },
        PromptKind::NewDir => match ops::create_dir(&app.current_dir(), &value) {
            Ok(path) => {
                app.mode = Mode::Normal;
                app.set_success(format!(
                    "Created {}",
                    path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ));
                app.pending_select = Some(path);
                app.refresh_active();
            }
            Err(err) => {
                app.set_error(err);
                app.prompt = Some(prompt);
            }
        },
        PromptKind::Shell => {
            app.mode = Mode::Normal;
            if value.trim().is_empty() {
                return Effect::None;
            }
            return Effect::Suspend(Suspend::Shell(value));
        }
    }
    Effect::None
}

pub fn cancel_prompt(app: &mut App) {
    if let Some(prompt) = app.prompt.take() {
        if matches!(prompt.kind, PromptKind::Filter) {
            let fuzzy = app.fuzzy();
            let pane = app.active_pane_mut();
            pane.filter.clear();
            pane.rebuild(fuzzy);
            app.request_preview();
        }
    }
    app.mode = Mode::Normal;
}

pub fn update_filter_live(app: &mut App) {
    let Some(prompt) = &app.prompt else {
        return;
    };
    if !matches!(prompt.kind, PromptKind::Filter) {
        return;
    }
    let value = prompt.input.value();
    let fuzzy = app.fuzzy();
    let pane = app.active_pane_mut();
    pane.filter = value;
    pane.cursor = 0;
    pane.scroll = 0;
    pane.rebuild(fuzzy);
    app.request_preview();
}

fn expand_path(value: &str) -> PathBuf {
    let trimmed = value.trim();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return utils::home_dir().join(rest);
    }
    if trimmed == "~" {
        return utils::home_dir();
    }
    PathBuf::from(trimmed)
}

pub fn task_cancelled(app: &mut App) {
    if let Some(task) = &app.task {
        task.cancel.store(true, Ordering::Relaxed);
    }
}
