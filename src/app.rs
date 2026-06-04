use std::io::Stdout;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::commands;
use crate::filesystem::{read_directory, sort_entries, FileType};
use crate::state::{ActiveTask, AppState, InputMode, PreviewContent, TabState, TaskMsg};
use crate::ui;

#[derive(Debug)]
enum LoopAction {
    Continue,
    Quit,
    OpenEditor(PathBuf),
    RunShell(String),
}

/// Leave alternate screen for subprocess, then create fresh Terminal on return.
/// Destroys and recreates Terminal so all internal buffers start clean.
fn run_subprocess<F>(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    f: F,
) -> anyhow::Result<()>
where
    F: FnOnce() -> Result<(), String>,
{
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::cursor::Show,
    )?;

    let _ = f();

    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
        crossterm::cursor::Hide,
    )?;
    crossterm::terminal::enable_raw_mode()?;

    // Replace terminal with fresh one — all buffers start empty
    *terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    // Drain stale input events from subprocess
    while crossterm::event::poll(std::time::Duration::from_millis(0))? {
        let _ = crossterm::event::read()?;
    }

    Ok(())
}

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    let mut state = AppState::new();
    refresh_tab(&mut state.tabs[0], state.show_hidden);
    update_preview(&mut state);

    loop {
        // Expire status messages
        if let Some((_, t)) = &state.status {
            if t.elapsed().as_secs() >= 3 { state.status = None; }
        }

        // Drain progress from background copy/move worker
        drain_task(&mut state);

        terminal.draw(|f| ui::render(f, &mut state))?;

        // Image preview: 150ms debounce then send via kitty protocol or chafa fallback
        if state.preview_enabled {
            if let PreviewContent::Image(ref path) = state.preview_content.clone() {
                let path = path.clone();
                let already_shown = state.last_kitty_image.as_ref() == Some(&path);
                if !already_shown {
                    let ready = match &state.preview_image_pending {
                        Some((p, t)) if p == &path => t.elapsed().as_millis() >= 150,
                        _ => {
                            state.preview_image_pending = Some((path.clone(), std::time::Instant::now()));
                            false
                        }
                    };
                    if ready {
                        state.preview_image_pending = None;
                        if ui::preview::is_kitty() {
                            ui::preview::clear_kitty();
                            if let Some((x, y, w, h)) = state.preview_rect {
                                let _ = ui::preview::send_kitty_image(&path, x, y, w, h);
                            }
                        } else if ui::preview::is_chafa_available() {
                            if let Some((_, _, w, h)) = state.preview_rect {
                                state.cached_chafa_output =
                                    ui::preview::render_chafa_image(&path, w, h);
                            }
                        }
                        state.last_kitty_image = Some(path.clone());
                    }
                }
                // Chafa output must be re-written every frame (ratatui overwrites it)
                if !ui::preview::is_kitty() {
                    if let (Some(ref output), Some((x, y, _, h))) =
                        (state.cached_chafa_output.clone(), state.preview_rect)
                    {
                        ui::preview::write_chafa_output(output, x, y, h);
                    }
                }
            } else {
                if state.last_kitty_image.is_some() {
                    if ui::preview::is_kitty() {
                        ui::preview::clear_kitty();
                    } else if state.cached_chafa_output.is_some() {
                        if let Some((x, y, w, h)) = state.preview_rect {
                            ui::preview::clear_chafa(x, y, w, h);
                        }
                    }
                    state.last_kitty_image = None;
                }
                state.cached_chafa_output = None;
                state.preview_image_pending = None;
            }
        } else if state.last_kitty_image.is_some() {
            if ui::preview::is_kitty() {
                ui::preview::clear_kitty();
            } else if state.cached_chafa_output.is_some() {
                if let Some((x, y, w, h)) = state.preview_rect {
                    ui::preview::clear_chafa(x, y, w, h);
                }
            }
            state.last_kitty_image = None;
            state.cached_chafa_output = None;
            state.preview_image_pending = None;
        }

        if event::poll(Duration::from_millis(100))? {
            let ev = event::read()?;

            // Mouse events
            if let Event::Mouse(mouse) = ev {
                use crossterm::event::MouseEventKind;
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        state.update_cursor(3);
                        update_preview(&mut state);
                    }
                    MouseEventKind::ScrollUp => {
                        state.update_cursor(-3);
                        update_preview(&mut state);
                    }
                    _ => {}
                }
                continue;
            }

            if let Event::Key(key) = ev {
                match handle_key(&mut state, key.code, key.modifiers) {
                    LoopAction::Quit => {
                        ui::preview::clear_kitty();
                        break;
                    }
                    LoopAction::OpenEditor(path) => {
                        run_subprocess(terminal, || {
                            commands::open_in_editor(&path, &state.config)
                        })?;
                        let tab_idx = focused_tab_idx(&state);
                        refresh_tab(&mut state.tabs[tab_idx], state.show_hidden);
                        update_preview(&mut state);
                    }
                    LoopAction::RunShell(cmd) => {
                        let dir = state.focused_tab().current_dir.clone();
                        run_subprocess(terminal, || {
                            commands::run_shell_cmd(&cmd, &dir)
                        })?;
                        let tab_idx = focused_tab_idx(&state);
                        refresh_tab(&mut state.tabs[tab_idx], state.show_hidden);
                        update_preview(&mut state);
                    }
                    LoopAction::Continue => {}
                }
            }
        }
    }
    Ok(())
}

fn focused_tab_idx(state: &AppState) -> usize {
    if state.dual_pane && state.right_focused { state.right_tab } else { state.active_tab }
}

pub fn refresh_tab(tab: &mut TabState, show_hidden: bool) {
    tab.entries = read_directory(&tab.current_dir, show_hidden);
    sort_entries(&mut tab.entries, tab.sort_by, tab.sort_ascending);
    apply_filter(tab);
    tab.clamp_cursor();
    refresh_git_status(tab);
}

pub fn apply_filter(tab: &mut TabState) {
    if tab.search_query.is_empty() {
        tab.display_indices = (0..tab.entries.len()).collect();
    } else {
        let q = tab.search_query.to_lowercase();
        tab.display_indices = tab.entries.iter().enumerate()
            .filter(|(_, e)| e.name.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
    }
}

fn refresh_git_status(tab: &mut TabState) {
    tab.git_status.clear();

    let output = std::process::Command::new("git")
        .args(["-C", &tab.current_dir.to_string_lossy(), "status", "--porcelain", "--no-renames"])
        .output();

    let Ok(out) = output else { return; };
    if !out.status.success() && out.stdout.is_empty() { return; }

    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        if line.len() < 3 { continue; }
        let xy = &line[0..2];
        // The filename starts at position 3
        let file_part = &line[3..];
        // Take only the first path component (for files in subdirs, show at dir level)
        let first_component = file_part.split('/').next().unwrap_or(file_part).trim();
        if first_component.is_empty() { continue; }

        // XY: X = index status, Y = worktree status
        let x_ch = xy.chars().next().unwrap_or(' ');
        let y_ch = xy.chars().nth(1).unwrap_or(' ');

        // Priority: M > A > D > R > U > ? > ' '
        fn priority(c: char) -> u8 {
            match c {
                'M' => 6,
                'A' => 5,
                'D' => 4,
                'R' => 3,
                'U' => 2,
                '?' => 1,
                _   => 0,
            }
        }

        let ch = if priority(x_ch) >= priority(y_ch) { x_ch } else { y_ch };
        if ch != ' ' {
            // Only insert if not already there with higher priority
            let entry = tab.git_status.entry(first_component.to_string()).or_insert(ch);
            if priority(ch) > priority(*entry) {
                *entry = ch;
            }
        }
    }
}

/// Pull progress messages from the background worker without blocking.
/// On completion, refresh the view and report the outcome.
fn drain_task(state: &mut AppState) {
    use std::sync::mpsc::TryRecvError;
    let mut finished: Option<(usize, usize, Option<String>)> = None;
    if let Some(task) = state.task.as_mut() {
        loop {
            match task.rx.try_recv() {
                Ok(TaskMsg::Progress { done, total, current }) => {
                    task.done = done;
                    task.total = total;
                    task.current = current;
                }
                Ok(TaskMsg::Done { ok, total, first_err }) => {
                    finished = Some((ok, total, first_err));
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = Some((task.done, task.total, None));
                    break;
                }
            }
        }
    }
    if let Some((ok, total, first_err)) = finished {
        state.task = None;
        let show = state.show_hidden;
        // Worker may have written into any tab's directory; refresh them all
        for tab in &mut state.tabs { refresh_tab(tab, show); }
        update_preview(state);
        match first_err {
            None => state.set_status(format!("Done — {} item(s)", total)),
            Some(e) => state.set_status(format!("Done {}/{} — {}", ok, total, e)),
        }
    }
}

pub fn update_preview(state: &mut AppState) {
    if !state.preview_enabled {
        state.preview_content = PreviewContent::None;
        return;
    }

    let entry = match state.focused_tab().cursor_entry() {
        Some(e) => e.clone(),
        None => {
            state.preview_content = PreviewContent::None;
            return;
        }
    };

    if entry.is_dir {
        // Count sub-entries
        let count = std::fs::read_dir(&entry.path)
            .map(|rd| rd.flatten().count())
            .unwrap_or(0);
        state.preview_content = PreviewContent::Directory { count };
        return;
    }

    if entry.file_type == FileType::Image {
        state.preview_content = PreviewContent::Image(entry.path.clone());
        return;
    }

    // Try to read as text
    let size = entry.size;
    let mut file = match std::fs::File::open(&entry.path) {
        Ok(f) => f,
        Err(_) => {
            state.preview_content = PreviewContent::Binary { size };
            return;
        }
    };

    use std::io::Read;
    // Read first 512 bytes to detect binary
    let mut probe = [0u8; 512];
    let n = file.read(&mut probe).unwrap_or(0);
    if probe[..n].contains(&0u8) {
        state.preview_content = PreviewContent::Binary { size };
        return;
    }

    // Read up to 200 lines
    use std::io::{BufRead, BufReader, Seek, SeekFrom};
    let _ = file.seek(SeekFrom::Start(0));
    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    for line in reader.lines().take(200) {
        match line {
            Ok(l) => lines.push(l),
            Err(_) => {
                // Probably binary after all
                state.preview_content = PreviewContent::Binary { size };
                return;
            }
        }
    }
    state.preview_content = PreviewContent::Text(lines);
}

fn navigate_to(state: &mut AppState, path: PathBuf) {
    if path.is_dir() {
        let show = state.show_hidden;
        let tab = state.focused_tab_mut();
        tab.current_dir = path;
        tab.search_query.clear();
        tab.cursor = 0;
        tab.scroll = 0;
        tab.selection.clear();
        let tab_idx = focused_tab_idx(state);
        refresh_tab(&mut state.tabs[tab_idx], show);
        update_preview(state);
    }
}

fn handle_key(state: &mut AppState, key: KeyCode, mods: KeyModifiers) -> LoopAction {
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
                refresh_tab(&mut state.tabs[tab_idx], show);
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
        KeyCode::Char('j') | KeyCode::Down => {
            if n > 0 { state.sidebar_cursor = (state.sidebar_cursor + 1).min(n - 1); }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.sidebar_cursor = state.sidebar_cursor.saturating_sub(1);
        }
        KeyCode::Char('g') => { state.sidebar_cursor = 0; }
        KeyCode::Char('G') => { if n > 0 { state.sidebar_cursor = n - 1; } }
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
            refresh_tab(&mut state.tabs[tab_idx], show);
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
                        refresh_tab(&mut state.tabs[tab_idx], show);
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
    refresh_tab(&mut state.tabs[tab_idx], show);
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
    refresh_tab(&mut state.tabs[tab_idx], show);
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
                } else {
                    let _ = commands::open_file(&path, &state.config);
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
            for tab in &mut state.tabs { refresh_tab(tab, sh); }
            update_preview(state);
        }

        // Refresh
        KeyCode::F(5) => {
            let tab_idx2 = focused_tab_idx(state);
            refresh_tab(&mut state.tabs[tab_idx2], show);
            update_preview(state);
        }
        KeyCode::Char('R') => {
            let tab_idx2 = focused_tab_idx(state);
            refresh_tab(&mut state.tabs[tab_idx2], show);
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
                    refresh_tab(&mut state.tabs[tab_idx2], show);
                } else {
                    state.set_status("Not an archive file");
                }
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
                refresh_tab(&mut t, show);
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
        KeyCode::Char('[') if !ctrl => {
            if state.tabs.len() > 1 {
                state.active_tab = if state.active_tab == 0 {
                    state.tabs.len() - 1
                } else {
                    state.active_tab - 1
                };
                update_preview(state);
            }
        }
        KeyCode::Char(']') if !ctrl => {
            if state.tabs.len() > 1 {
                state.active_tab = (state.active_tab + 1) % state.tabs.len();
                update_preview(state);
            }
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
            refresh_tab(&mut t, show);
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
            let _ = commands::open_terminal(&dir, &state.config);
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
