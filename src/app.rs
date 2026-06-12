use std::io::Stdout;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::commands;
use crate::filesystem::{read_directory, sort_entries, FileType};
use crate::input::{handle_key, LoopAction};
use crate::state::{AppState, GitWorker, PreviewContent, TabState, TaskMsg};
use crate::ui;

/// Leave alternate screen for subprocess, then create fresh Terminal on return.
/// Destroys and recreates Terminal so all internal buffers start clean.
/// Returns the subprocess error (if any) so the caller can show it.
fn run_subprocess<F>(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    f: F,
) -> anyhow::Result<Option<String>>
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

    let sub_err = f().err();

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

    Ok(sub_err)
}

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    let mut state = AppState::new();
    refresh_tab(&mut state.tabs[0], state.show_hidden, &mut state.git);
    update_preview(&mut state);

    loop {
        // Expire status messages
        if let Some((_, t)) = &state.status {
            if t.elapsed().as_secs() >= 3 { state.status = None; }
        }

        // Drain progress from background copy/move worker
        drain_task(&mut state);

        // Apply finished background git status results
        drain_git(&mut state);

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
                        let err = run_subprocess(terminal, || {
                            commands::open_in_editor(&path, &state.config)
                        })?;
                        if let Some(e) = err { state.set_status(e); }
                        let tab_idx = focused_tab_idx(&state);
                        refresh_tab(&mut state.tabs[tab_idx], state.show_hidden, &mut state.git);
                        update_preview(&mut state);
                    }
                    LoopAction::RunShell(cmd) => {
                        let dir = state.focused_tab().current_dir.clone();
                        let err = run_subprocess(terminal, || {
                            commands::run_shell_cmd(&cmd, &dir)
                        })?;
                        if let Some(e) = err { state.set_status(e); }
                        let tab_idx = focused_tab_idx(&state);
                        refresh_tab(&mut state.tabs[tab_idx], state.show_hidden, &mut state.git);
                        update_preview(&mut state);
                    }
                    LoopAction::Continue => {}
                }
            }
        }
    }
    Ok(())
}

pub fn focused_tab_idx(state: &AppState) -> usize {
    if state.dual_pane && state.right_focused { state.right_tab } else { state.active_tab }
}

pub fn refresh_tab(tab: &mut TabState, show_hidden: bool, git: &mut GitWorker) {
    tab.entries = read_directory(&tab.current_dir, show_hidden);
    sort_entries(&mut tab.entries, tab.sort_by, tab.sort_ascending);
    apply_filter(tab);
    tab.clamp_cursor();
    request_git_status(tab, git);
}

/// Show the cached git status immediately (possibly stale) and kick off a
/// fresh `git status` on a background thread so navigation never blocks.
fn request_git_status(tab: &mut TabState, git: &mut GitWorker) {
    let dir = tab.current_dir.clone();
    tab.git_status = git.cache.get(&dir).cloned().unwrap_or_default();
    if git.pending.insert(dir.clone()) {
        let tx = git.tx.clone();
        std::thread::spawn(move || {
            let map = compute_git_status(&dir);
            let _ = tx.send((dir, map));
        });
    }
}

/// Apply finished background `git status` results to every tab showing that
/// directory.
fn drain_git(state: &mut AppState) {
    while let Ok((dir, map)) = state.git.rx.try_recv() {
        state.git.pending.remove(&dir);
        state.git.cache.insert(dir.clone(), map.clone());
        for tab in &mut state.tabs {
            if tab.current_dir == dir {
                tab.git_status = map.clone();
            }
        }
    }
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

fn compute_git_status(dir: &std::path::Path) -> std::collections::HashMap<String, char> {
    let mut status = std::collections::HashMap::new();

    let output = std::process::Command::new("git")
        .args(["-C", &dir.to_string_lossy(), "status", "--porcelain", "--no-renames"])
        .output();

    let Ok(out) = output else { return status; };
    if !out.status.success() && out.stdout.is_empty() { return status; }

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
            let entry = status.entry(first_component.to_string()).or_insert(ch);
            if priority(ch) > priority(*entry) {
                *entry = ch;
            }
        }
    }
    status
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
        for tab in &mut state.tabs { refresh_tab(tab, show, &mut state.git); }
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

pub fn navigate_to(state: &mut AppState, path: PathBuf) {
    if path.is_dir() {
        let show = state.show_hidden;
        let tab = state.focused_tab_mut();
        tab.current_dir = path;
        tab.search_query.clear();
        tab.cursor = 0;
        tab.scroll = 0;
        tab.selection.clear();
        let tab_idx = focused_tab_idx(state);
        refresh_tab(&mut state.tabs[tab_idx], show, &mut state.git);
        update_preview(state);
    }
}

