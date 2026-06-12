//! Keyboard input handling: one function per input mode, dispatched by
//! `handle_key`. Split out of `app.rs` to keep the event loop readable.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{apply_filter, focused_tab_idx, navigate_to, refresh_tab, update_preview};
use crate::commands;
use crate::filesystem::{sort_entries, FileType};
use crate::state::{ActiveTask, AppState, InputMode, TabState, TaskMsg};

/// What the event loop should do after a key press. Editor and shell need the
/// terminal, so they bubble up to `run` which suspends the TUI first.
#[derive(Debug)]
pub enum LoopAction {
    Continue,
    Quit,
    OpenEditor(PathBuf),
    RunShell(String),
}

pub fn handle_key(state: &mut AppState, key: KeyCode, mods: KeyModifiers) -> LoopAction {
    let ctrl = mods.contains(KeyModifiers::CONTROL);

    // Global: quit (but 'q' just leaves the sidebar when it has focus)
    if key == KeyCode::Char('q') && state.input_mode == InputMode::Normal && !ctrl && !state.sidebar_focused {
        return LoopAction::Quit;
    }

    // Help toggle
    if key == KeyCode::F(1) {
        state.show_help = !state.show_help;
        return LoopAction::Continue;
    }
    if state.show_help {
        state.show_help = false;
        return LoopAction::Continue;
    }

    match state.input_mode {
        InputMode::Search  => { handle_search_key(state, key); LoopAction::Continue }
        InputMode::Rename  => { handle_rename_key(state, key); LoopAction::Continue }
        InputMode::GoTo    => { handle_goto_key(state, key); LoopAction::Continue }
        InputMode::Shell   => handle_shell_key(state, key),
        InputMode::Create  => { handle_create_key(state, key); LoopAction::Continue }
        InputMode::Confirm => { handle_confirm_key(state, key); LoopAction::Continue }
        InputMode::Normal  => handle_normal_key(state, key, ctrl),
    }
}

fn handle_search_key(state: &mut AppState, key: KeyCode) {
    let tab_idx = focused_tab_idx(state);
    match key {
        KeyCode::Esc => {
            state.focused_tab_mut().search_query.clear();
            apply_filter(&mut state.tabs[tab_idx]);
            state.focused_tab_mut().cursor = 0;
            state.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            state.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            state.focused_tab_mut().search_query.pop();
            apply_filter(&mut state.tabs[tab_idx]);
            state.tabs[tab_idx].cursor = 0;
        }
        KeyCode::Char(c) => {
            state.focused_tab_mut().search_query.push(c);
            apply_filter(&mut state.tabs[tab_idx]);
            state.tabs[tab_idx].cursor = 0;
        }
        _ => {}
    }
}

fn handle_rename_key(state: &mut AppState, key: KeyCode) {
    let tab_idx = focused_tab_idx(state);
    match key {
        KeyCode::Esc => {
            state.focused_tab_mut().renaming = None;
            state.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            if let Some((display_idx, new_name, _)) = state.tabs[tab_idx].renaming.take() {
                let old_path = state.tabs[tab_idx].get_display_entry(display_idx).map(|e| e.path.clone());
                if let Some(old) = old_path {
                    if let Err(e) = commands::rename_file(&old, new_name.trim()) {
                        state.set_status(format!("Rename failed: {}", e));
                    }
                }
                let show = state.show_hidden;
                refresh_tab(&mut state.tabs[tab_idx], show, &mut state.git);
            }
            state.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            if let Some((_, ref mut buf, ref mut cur)) = state.focused_tab_mut().renaming {
                if *cur > 0 {
                    let byte = buf.char_indices().nth(*cur - 1).map(|(b, _)| b).unwrap_or(0);
                    buf.remove(byte);
                    *cur -= 1;
                }
            }
        }
        KeyCode::Left => {
            if let Some((_, _, ref mut cur)) = state.focused_tab_mut().renaming {
                *cur = cur.saturating_sub(1);
            }
        }
        KeyCode::Right => {
            if let Some((_, ref buf, ref mut cur)) = state.focused_tab_mut().renaming {
                *cur = (*cur + 1).min(buf.chars().count());
            }
        }
        KeyCode::Home => {
            if let Some((_, _, ref mut cur)) = state.focused_tab_mut().renaming { *cur = 0; }
        }
        KeyCode::End => {
            if let Some((_, ref buf, ref mut cur)) = state.focused_tab_mut().renaming {
                *cur = buf.chars().count();
            }
        }
        KeyCode::Char(c) => {
            if let Some((_, ref mut buf, ref mut cur)) = state.focused_tab_mut().renaming {
                let byte = buf.char_indices().nth(*cur).map(|(b, _)| b).unwrap_or(buf.len());
                buf.insert(byte, c);
                *cur += 1;
            }
        }
        _ => {}
    }
}

fn handle_sidebar_key(state: &mut AppState, key: KeyCode) -> LoopAction {
    let targets = crate::ui::sidebar::nav_targets(state);
    let n = targets.len();
    match key {
        KeyCode::Char('j') | KeyCode::Down
            if n > 0 => { state.sidebar_cursor = (state.sidebar_cursor + 1).min(n - 1); }
        KeyCode::Char('k') | KeyCode::Up => {
            state.sidebar_cursor = state.sidebar_cursor.saturating_sub(1);
        }
        KeyCode::Char('g') => { state.sidebar_cursor = 0; }
        KeyCode::Char('G') if n > 0 => { state.sidebar_cursor = n - 1; }
        // Jump to the selected location
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            if let Some((_, path)) = targets.get(state.sidebar_cursor).cloned() {
                state.sidebar_focused = false;
                navigate_to(state, path);
            }
        }
        // Leave sidebar focus, back to the file list
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left
        | KeyCode::Char('q') | KeyCode::Char('b') | KeyCode::Tab => {
            state.sidebar_focused = false;
        }
        _ => {}
    }
    LoopAction::Continue
}

fn start_rename(state: &mut AppState) {
    let cursor = state.focused_tab().cursor;
    let name = state.focused_tab().cursor_entry().map(|e| e.name.clone());
    if let Some(n) = name {
        let at = rename_cursor_at_stem(&n);
        state.focused_tab_mut().renaming = Some((cursor, n, at));
        state.input_mode = InputMode::Rename;
    }
}

/// Char index just before a filename's extension dot (so the stem is "selected"
/// for quick editing); falls back to end of name for dotfiles / no extension.
fn rename_cursor_at_stem(name: &str) -> usize {
    match name.rfind('.') {
        Some(dot) if dot > 0 => name[..dot].chars().count(),
        _ => name.chars().count(),
    }
}

fn handle_goto_key(state: &mut AppState, key: KeyCode) {
    match key {
        KeyCode::Esc => {
            state.goto_buf.clear();
            state.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            let path = PathBuf::from(state.goto_buf.clone());
            state.goto_buf.clear();
            state.input_mode = InputMode::Normal;
            navigate_to(state, path);
        }
        KeyCode::Backspace => { state.goto_buf.pop(); }
        KeyCode::Char(c)   => { state.goto_buf.push(c); }
        _ => {}
    }
}

fn handle_shell_key(state: &mut AppState, key: KeyCode) -> LoopAction {
    match key {
        KeyCode::Esc => {
            state.shell_buf.clear();
            state.input_mode = InputMode::Normal;
            LoopAction::Continue
        }
        KeyCode::Enter => {
            let cmd = state.shell_buf.clone();
            state.shell_buf.clear();
            state.input_mode = InputMode::Normal;
            if !cmd.is_empty() {
                LoopAction::RunShell(cmd)
            } else {
                LoopAction::Continue
            }
        }
        KeyCode::Backspace => { state.shell_buf.pop(); LoopAction::Continue }
        KeyCode::Char(c)   => { state.shell_buf.push(c); LoopAction::Continue }
        _ => LoopAction::Continue
    }
}

fn handle_create_key(state: &mut AppState, key: KeyCode) {
    let show = state.show_hidden;
    match key {
        KeyCode::Esc => {
            state.create_buf.clear();
            state.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            let name = state.create_buf.trim().to_string();
            state.create_buf.clear();
            state.input_mode = InputMode::Normal;
            if name.is_empty() { return; }
            let dir = state.focused_tab().current_dir.clone();
            let res = if state.create_is_dir {
                commands::create_directory(&dir, &name).map(|_| ())
            } else {
                commands::create_file(&dir, &name).map(|_| ())
            };
            let tab_idx = focused_tab_idx(state);
            refresh_tab(&mut state.tabs[tab_idx], show, &mut state.git);
            match res {
                Ok(_) => state.set_status(format!("Created '{}'", name)),
                Err(e) => state.set_status(format!("Create failed: {}", e)),
            }
        }
        KeyCode::Backspace => { state.create_buf.pop(); }
        KeyCode::Char(c)   => { state.create_buf.push(c); }
        _ => {}
    }
}

fn handle_confirm_key(state: &mut AppState, key: KeyCode) {
    let show = state.show_hidden;
    match key {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            if let Some((_, action)) = state.confirm.take() {
                match action {
                    crate::state::ConfirmAction::DeletePermanent(paths) => {
                        let n = paths.len();
                        let mut errs = 0;
                        for p in &paths {
                            if commands::delete_file(p).is_err() { errs += 1; }
                        }
                        state.focused_tab_mut().selection.clear();
                        let tab_idx = focused_tab_idx(state);
                        refresh_tab(&mut state.tabs[tab_idx], show, &mut state.git);
                        update_preview(state);
                        if errs == 0 {
                            state.set_status(format!("Deleted {} item(s)", n));
                        } else {
                            state.set_status(format!("Deleted {}/{} — {} failed", n - errs, n, errs));
                        }
                    }
                }
            }
            state.input_mode = InputMode::Normal;
        }
        _ => {
            // Anything else cancels — safe default
            state.confirm = None;
            state.input_mode = InputMode::Normal;
            state.set_status("Cancelled");
        }
    }
}

/// Move selection (or cursor entry) to trash, surfacing any failures.
/// Records the original→trashed paths so the batch can be undone with Ctrl+Z.
fn trash_selection(state: &mut AppState, show: bool) {
    let paths = selected_or_cursor(state);
    let n = paths.len();
    if n == 0 { return; }
    let mut ok = 0;
    let mut last_err = String::new();
    let mut batch: Vec<(PathBuf, PathBuf)> = Vec::new();
    for p in &paths {
        match commands::trash_file(p) {
            Ok(dest) => { ok += 1; batch.push((p.clone(), dest)); }
            Err(e) => last_err = e,
        }
    }
    if !batch.is_empty() { state.trash_undo.push(batch); }
    state.focused_tab_mut().selection.clear();
    let tab_idx = focused_tab_idx(state);
    refresh_tab(&mut state.tabs[tab_idx], show, &mut state.git);
    update_preview(state);
    if ok == n {
        state.set_status(format!("Trashed {} item(s) — Ctrl+Z to undo", n));
    } else {
        state.set_status(format!("Trashed {}/{} — {}", ok, n, last_err));
    }
}

/// Restore the most recent trash batch from the trash back to its origin.
fn undo_trash(state: &mut AppState, show: bool) {
    let Some(batch) = state.trash_undo.pop() else {
        state.set_status("Nothing to undo");
        return;
    };
    let n = batch.len();
    let mut ok = 0;
    let mut last_err = String::new();
    for (orig, trashed) in &batch {
        match commands::move_file(trashed, orig) {
            Ok(_) => ok += 1,
            Err(e) => last_err = e,
        }
    }
    let tab_idx = focused_tab_idx(state);
    refresh_tab(&mut state.tabs[tab_idx], show, &mut state.git);
    update_preview(state);
    if ok == n {
        state.set_status(format!("Restored {} item(s)", n));
    } else {
        state.set_status(format!("Restored {}/{} — {}", ok, n, last_err));
    }
}

fn handle_normal_key(state: &mut AppState, key: KeyCode, ctrl: bool) -> LoopAction {
    let show = state.show_hidden;
    let tab_idx = focused_tab_idx(state);

    // When the sidebar holds focus, all normal keys drive it instead
    if state.sidebar_focused {
        return handle_sidebar_key(state, key);
    }

    match key {
        // Navigation
        KeyCode::Char('j') | KeyCode::Down  => {
            state.update_cursor(1);
            update_preview(state);
        }
        KeyCode::Char('k') | KeyCode::Up    => {
            state.update_cursor(-1);
            update_preview(state);
        }
        KeyCode::Char('g') if !ctrl => {
            state.focused_tab_mut().cursor = 0;
            update_preview(state);
        }
        KeyCode::Char('G') => {
            let len = state.focused_tab().display_count();
            if len > 0 { state.focused_tab_mut().cursor = len - 1; }
            update_preview(state);
        }
        KeyCode::PageDown => {
            state.update_cursor(20);
            update_preview(state);
        }
        KeyCode::PageUp   => {
            state.update_cursor(-20);
            update_preview(state);
        }

        // Enter directory / open file
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
            let entry = state.focused_tab().cursor_entry().map(|e| (e.path.clone(), e.is_dir));
            if let Some((path, is_dir)) = entry {
                if is_dir {
                    navigate_to(state, path);
                } else if commands::wants_editor(&path) {
                    // Editor needs the real terminal — must suspend the TUI
                    return LoopAction::OpenEditor(path);
                } else if let Err(e) = commands::open_file(&path, &state.config) {
                    state.set_status(format!("Open failed: {}", e));
                }
            }
        }

        // Open in editor (always)
        KeyCode::Char('e') if !ctrl => {
            let entry = state.focused_tab().cursor_entry().map(|e| (e.path.clone(), e.is_dir));
            if let Some((path, is_dir)) = entry {
                if !is_dir {
                    return LoopAction::OpenEditor(path);
                }
            }
        }

        // Go up
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
            let parent = state.focused_tab().current_dir.parent().map(|p| p.to_path_buf());
            if let Some(p) = parent { navigate_to(state, p); }
        }

        // Search
        KeyCode::Char('/') => {
            state.input_mode = InputMode::Search;
        }

        // Shell command
        KeyCode::Char(':') => {
            state.shell_buf.clear();
            state.input_mode = InputMode::Shell;
        }

        // Go to path (Ctrl+G)
        KeyCode::Char('g') if ctrl => {
            state.goto_buf = state.focused_tab().current_dir.to_string_lossy().to_string();
            state.input_mode = InputMode::GoTo;
        }

        // Escape: clear selection/search
        KeyCode::Esc => {
            state.focused_tab_mut().selection.clear();
            let tab_idx2 = focused_tab_idx(state);
            if !state.focused_tab().search_query.is_empty() {
                state.focused_tab_mut().search_query.clear();
                apply_filter(&mut state.tabs[tab_idx2]);
            }
        }

        // Select
        KeyCode::Char(' ') => {
            let path = state.focused_tab().cursor_entry().map(|e| e.path.clone());
            if let Some(p) = path {
                state.focused_tab_mut().toggle_selection(p);
                state.update_cursor(1);
            }
        }

        // Yank / Cut / Paste
        KeyCode::Char('y') if !ctrl => {
            let paths = selected_or_cursor(state);
            let n = paths.len();
            if !paths.is_empty() {
                state.clipboard = Some((paths, false));
                state.set_status(format!("Yanked {} item(s)", n));
            }
        }
        KeyCode::Char('x') if !ctrl => {
            let paths = selected_or_cursor(state);
            let n = paths.len();
            if !paths.is_empty() {
                state.clipboard = Some((paths, true));
                state.set_status(format!("Cut {} item(s)", n));
            }
        }
        KeyCode::Char('p') if !ctrl => {
            if state.task.is_some() {
                state.set_status("Busy — another operation is running");
            } else if let Some((paths, is_cut)) = state.clipboard.clone() {
                let dest = state.focused_tab().current_dir.clone();
                let (tx, rx) = std::sync::mpsc::channel();
                let total = paths.len();
                let label = if is_cut { "Moving" } else { "Copying" };

                // Worker thread: copy/move each item, report progress, never block UI
                std::thread::spawn(move || {
                    let mut ok = 0usize;
                    let mut first_err: Option<String> = None;
                    for (i, path) in paths.iter().enumerate() {
                        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                        let _ = tx.send(TaskMsg::Progress { done: i, total, current: name.clone() });
                        let d = commands::unique_dest(&dest, path.file_name().unwrap_or_default());
                        let res = if is_cut {
                            commands::move_file(path, &d)
                        } else {
                            commands::copy_file(path, &d)
                        };
                        match res {
                            Ok(_) => ok += 1,
                            Err(e) => if first_err.is_none() {
                                first_err = Some(format!("{}: {}", name, e));
                            }
                        }
                    }
                    let _ = tx.send(TaskMsg::Done { ok, total, first_err });
                });

                if is_cut { state.clipboard = None; }
                state.focused_tab_mut().selection.clear();
                state.task = Some(ActiveTask { rx, label, done: 0, total, current: String::new() });
            }
        }

        // Trash (recoverable) — safe default, like Dolphin's Delete key
        KeyCode::Char('d') | KeyCode::Delete if !ctrl => {
            trash_selection(state, show);
        }

        // Permanent delete — requires confirmation modal
        KeyCode::Char('D') => {
            let paths = selected_or_cursor(state);
            if !paths.is_empty() {
                let prompt = if paths.len() == 1 {
                    format!("Permanently delete '{}'?", paths[0].file_name().unwrap_or_default().to_string_lossy())
                } else {
                    format!("Permanently delete {} items?", paths.len())
                };
                state.confirm = Some((prompt, crate::state::ConfirmAction::DeletePermanent(paths)));
                state.input_mode = InputMode::Confirm;
            }
        }

        // Rename
        KeyCode::Char('r') if !ctrl => { start_rename(state); }
        KeyCode::F(2) => { start_rename(state); }

        // Create file — prompt for name
        KeyCode::Char('a') if !ctrl => {
            state.create_buf.clear();
            state.create_is_dir = false;
            state.input_mode = InputMode::Create;
        }
        // Create directory — prompt for name
        KeyCode::Char('A') => {
            state.create_buf.clear();
            state.create_is_dir = true;
            state.input_mode = InputMode::Create;
        }

        // Toggle hidden
        KeyCode::Char('.') => {
            state.show_hidden = !state.show_hidden;
            let sh = state.show_hidden;
            for tab in &mut state.tabs { refresh_tab(tab, sh, &mut state.git); }
            update_preview(state);
        }

        // Refresh
        KeyCode::F(5) => {
            let tab_idx2 = focused_tab_idx(state);
            refresh_tab(&mut state.tabs[tab_idx2], show, &mut state.git);
            update_preview(state);
        }
        KeyCode::Char('R') => {
            let tab_idx2 = focused_tab_idx(state);
            refresh_tab(&mut state.tabs[tab_idx2], show, &mut state.git);
            update_preview(state);
        }

        // Toggle preview pane
        KeyCode::Char('P') => {
            state.preview_enabled = !state.preview_enabled;
            update_preview(state);
        }

        // Extract archive
        KeyCode::Char('E') => {
            let entry = state.focused_tab().cursor_entry().map(|e| (e.path.clone(), e.is_dir, e.file_type.clone()));
            if let Some((path, is_dir, ft)) = entry {
                if !is_dir && ft == FileType::Archive {
                    match commands::extract_archive(&path) {
                        Ok(_) => state.set_status("Archive extracted"),
                        Err(e) => state.set_status(format!("Extract failed: {}", e)),
                    }
                    let tab_idx2 = focused_tab_idx(state);
                    refresh_tab(&mut state.tabs[tab_idx2], show, &mut state.git);
                } else {
                    state.set_status("Not an archive file");
                }
            }
        }

        // Compress to zip (tar.gz fallback)
        KeyCode::Char('Z') => {
            let entry = state.focused_tab().cursor_entry().map(|e| e.path.clone());
            if let Some(path) = entry {
                match commands::compress_zip(&path) {
                    Ok(_) => state.set_status("Compressed"),
                    Err(e) => state.set_status(format!("Compress failed: {}", e)),
                }
                let tab_idx2 = focused_tab_idx(state);
                refresh_tab(&mut state.tabs[tab_idx2], show, &mut state.git);
            }
        }

        // Dual pane (Ctrl+W)
        KeyCode::Char('w') if ctrl => {
            if state.dual_pane {
                state.dual_pane = false;
                state.right_focused = false;
            } else {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                let mut t = TabState::new(home);
                refresh_tab(&mut t, show, &mut state.git);
                state.tabs.push(t);
                state.right_tab = state.tabs.len() - 1;
                state.dual_pane = true;
                state.right_focused = true;
            }
        }

        // Switch pane
        KeyCode::Tab if state.dual_pane => {
            state.right_focused = !state.right_focused;
        }

        // Cycle tabs: [ prev, ] next
        KeyCode::Char('[') if !ctrl
            && state.tabs.len() > 1 => {
                state.active_tab = if state.active_tab == 0 {
                    state.tabs.len() - 1
                } else {
                    state.active_tab - 1
                };
                update_preview(state);
            }
        KeyCode::Char(']') if !ctrl
            && state.tabs.len() > 1 => {
                state.active_tab = (state.active_tab + 1) % state.tabs.len();
                update_preview(state);
            }

        // Close tab (X)
        KeyCode::Char('X') => {
            if state.tabs.len() > 1 {
                state.tabs.remove(tab_idx);
                if state.active_tab >= state.tabs.len() {
                    state.active_tab = state.tabs.len() - 1;
                }
                // Fix right_tab if it refers to a removed index
                if state.right_tab >= state.tabs.len() {
                    state.right_tab = state.tabs.len() - 1;
                }
                update_preview(state);
                state.set_status("Tab closed");
            } else {
                state.set_status("Cannot close last tab");
            }
        }

        // New tab (Ctrl+T)
        KeyCode::Char('t') if ctrl => {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            let mut t = TabState::new(home);
            refresh_tab(&mut t, show, &mut state.git);
            state.tabs.push(t);
            state.active_tab = state.tabs.len() - 1;
            update_preview(state);
        }

        // Sort cycle
        KeyCode::Char('s') if !ctrl => {
            use crate::state::SortBy::*;
            let tab_idx2 = focused_tab_idx(state);
            let tab = &mut state.tabs[tab_idx2];
            tab.sort_by = match tab.sort_by { Name => Size, Size => Date, Date => Type, Type => Name };
            sort_entries(&mut tab.entries, tab.sort_by, tab.sort_ascending);
            apply_filter(tab);
        }

        // Sort order toggle
        KeyCode::Char('S') => {
            let tab_idx2 = focused_tab_idx(state);
            let tab = &mut state.tabs[tab_idx2];
            tab.sort_ascending = !tab.sort_ascending;
            sort_entries(&mut tab.entries, tab.sort_by, tab.sort_ascending);
            apply_filter(tab);
        }

        // Open terminal
        KeyCode::Char('T') => {
            let dir = state.focused_tab().current_dir.clone();
            if let Err(e) = commands::open_terminal(&dir, &state.config) {
                state.set_status(format!("Terminal: {}", e));
            }
        }

        // Share via KDE Connect
        KeyCode::Char('K') => {
            let paths = selected_or_cursor(state);
            match commands::share_kdeconnect(&paths) {
                Ok(msg) => state.set_status(msg),
                Err(e)  => state.set_status(format!("KDE Connect: {}", e)),
            }
        }

        // Help
        KeyCode::Char('?') => { state.show_help = true; }

        // Undo last trash (Ctrl+Z)
        KeyCode::Char('z') if ctrl => {
            undo_trash(state, show);
        }

        // Focus the sidebar (bookmarks / drives)
        KeyCode::Char('b') if !ctrl => {
            let targets = crate::ui::sidebar::nav_targets(state);
            if !targets.is_empty() {
                let cur = state.focused_tab().current_dir.clone();
                state.sidebar_cursor = targets.iter().position(|(_, p)| *p == cur).unwrap_or(0);
                state.sidebar_focused = true;
            }
        }

        // Bookmark current dir (Ctrl+B)
        KeyCode::Char('b') if ctrl => {
            let cur = state.focused_tab().current_dir.clone();
            let name = cur.file_name().and_then(|n| n.to_str()).unwrap_or("bm").to_string();
            if !state.bookmarks.iter().any(|(_, p)| p == &cur) {
                state.bookmarks.push((name, cur));
                state.set_status("Bookmark added");
            }
        }

        _ => {}
    }

    // Suppress unused variable warning for tab_idx
    let _ = tab_idx;

    LoopAction::Continue
}

fn selected_or_cursor(state: &AppState) -> Vec<PathBuf> {
    let tab = state.focused_tab();
    if tab.selection.is_empty() {
        tab.cursor_entry().map(|e| vec![e.path.clone()]).unwrap_or_default()
    } else {
        tab.selection.iter().cloned().collect()
    }
}
