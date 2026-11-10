use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::action::{self, Action, Effect};
use crate::state::{App, Column, Focus, Mode};

pub fn handle_event(app: &mut App, event: Event) -> Effect {
    match event {
        Event::Key(key) => handle_key(app, key),
        Event::Mouse(mouse) => {
            if app.config.ui.mouse {
                handle_mouse(app, mouse);
            }
            Effect::None
        }
        Event::Paste(text) => {
            match app.mode {
                Mode::Prompt => {
                    if let Some(prompt) = &mut app.prompt {
                        prompt.input.insert_str(&text);
                    }
                    action::update_filter_live(app);
                }
                Mode::Palette => {
                    if let Some(palette) = &mut app.palette {
                        palette.input.insert_str(&text);
                        palette.refilter();
                    }
                }
                _ => {}
            }
            Effect::None
        }
        _ => Effect::None,
    }
}

fn handle_key(app: &mut App, key: KeyEvent) -> Effect {
    if key.kind == KeyEventKind::Release {
        return Effect::None;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    if ctrl && key.code == KeyCode::Char('c') {
        return match app.mode {
            Mode::Normal => Effect::Quit,
            Mode::Prompt => {
                action::cancel_prompt(app);
                Effect::None
            }
            Mode::Palette => {
                app.palette = None;
                app.mode = Mode::Normal;
                Effect::None
            }
            Mode::Confirm => {
                action::confirm_no(app);
                Effect::None
            }
            Mode::Help => {
                app.help = None;
                app.mode = Mode::Normal;
                Effect::None
            }
            Mode::Info => {
                app.info = None;
                app.mode = Mode::Normal;
                Effect::None
            }
        };
    }

    match app.mode {
        Mode::Prompt => return handle_prompt(app, key.code, ctrl),
        Mode::Confirm => {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    action::confirm_yes(app);
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    action::confirm_no(app);
                }
                _ => {}
            }
            return Effect::None;
        }
        Mode::Palette => return handle_palette(app, key.code, ctrl),
        Mode::Help => {
            handle_help(app, key.code);
            return Effect::None;
        }
        Mode::Info => {
            handle_info(app, key.code);
            return Effect::None;
        }
        Mode::Normal => {}
    }

    if app.task.is_some() && key.code == KeyCode::Esc {
        action::task_cancelled(app);
        app.set_info("Cancelling…");
        return Effect::None;
    }

    if key.code == KeyCode::F(1) || key.code == KeyCode::Char('?') {
        return action::dispatch(app, Action::Help);
    }

    if app.focus == Focus::Sidebar {
        return handle_sidebar(app, key.code);
    }
    if app.focus == Focus::Preview {
        return handle_preview(app, key.code);
    }

    if ctrl {
        return match key.code {
            KeyCode::Char('p') => action::dispatch(app, Action::Palette),
            KeyCode::Char('t') => action::dispatch(app, Action::NewTab),
            KeyCode::Char('w') => action::dispatch(app, Action::ToggleDual),
            KeyCode::Char('b') => action::dispatch(app, Action::Bookmark),
            KeyCode::Char('g') => action::dispatch(app, Action::GoTo),
            KeyCode::Char('z') => action::dispatch(app, Action::Undo),
            KeyCode::Char('d') => action::dispatch(app, Action::PageDown),
            KeyCode::Char('u') => action::dispatch(app, Action::PageUp),
            _ => Effect::None,
        };
    }

    if alt && key.code == KeyCode::Left {
        return action::dispatch(app, Action::Back);
    }
    if alt && key.code == KeyCode::Right {
        return action::dispatch(app, Action::Forward);
    }
    if alt && key.code == KeyCode::Up {
        return action::dispatch(app, Action::Parent);
    }

    if let KeyCode::Char(digit @ '1'..='9') = key.code {
        let index = digit as usize - '1' as usize;
        if index < app.tabs.len() {
            switch_tab(app, index);
        }
        return Effect::None;
    }

    let action = match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => Action::Parent,
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => Action::Open,
        KeyCode::Char('g') | KeyCode::Home => Action::Top,
        KeyCode::Char('G') | KeyCode::End => Action::Bottom,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::Char(' ') => Action::Select,
        KeyCode::Char('V') => Action::SelectAll,
        KeyCode::Esc => Action::ClearSelection,
        KeyCode::Char('y') => Action::Copy,
        KeyCode::Char('x') => Action::Cut,
        KeyCode::Char('p') => Action::Paste,
        KeyCode::Char('d') | KeyCode::Delete => Action::Trash,
        KeyCode::Char('D') => Action::Delete,
        KeyCode::Char('r') | KeyCode::F(2) => Action::Rename,
        KeyCode::Char('a') => Action::NewFile,
        KeyCode::Char('A') => Action::NewDir,
        KeyCode::Char('e') => Action::Edit,
        KeyCode::Char('i') => Action::Info,
        KeyCode::Char('Y') => Action::CopyPath,
        KeyCode::Char('T') => Action::OpenTerminal,
        KeyCode::Char('E') => Action::Extract,
        KeyCode::Char('Z') => Action::Compress,
        KeyCode::Char('K') => Action::Share,
        KeyCode::Char('/') => Action::Filter,
        KeyCode::Char(':') => Action::Shell,
        KeyCode::Char('~') => Action::Home,
        KeyCode::Char('R') | KeyCode::F(5) => Action::Refresh,
        KeyCode::Char('.') => Action::ToggleHidden,
        KeyCode::Char('P') => Action::TogglePreview,
        KeyCode::Char('B') => Action::ToggleSidebar,
        KeyCode::Char('b') => Action::FocusSidebar,
        KeyCode::Char('s') => Action::SortNext,
        KeyCode::Char('S') => Action::SortOrder,
        KeyCode::Char('X') => Action::CloseTab,
        KeyCode::Char(']') => Action::NextTab,
        KeyCode::Char('[') => Action::PrevTab,
        KeyCode::Char('-') => Action::Back,
        KeyCode::Tab => {
            if app.dual {
                Action::SwitchPane
            } else {
                app.focus_next(shift);
                return Effect::None;
            }
        }
        KeyCode::BackTab => {
            app.focus_next(true);
            return Effect::None;
        }
        _ => return Effect::None,
    };
    action::dispatch(app, action)
}

fn handle_prompt(app: &mut App, code: KeyCode, ctrl: bool) -> Effect {
    if app.prompt.is_none() {
        app.mode = Mode::Normal;
        return Effect::None;
    }
    let mut close = false;
    let mut submit = false;
    let mut nav = 0i32;
    {
        let prompt = app.prompt.as_mut().unwrap();
        if ctrl {
            match code {
                KeyCode::Char('a') => prompt.input.select_all(),
                KeyCode::Char('u') => prompt.input.kill_to_start(),
                KeyCode::Char('k') => prompt.input.kill_to_end(),
                KeyCode::Char('w') => prompt.input.backspace_word(),
                KeyCode::Char('e') => prompt.input.end(),
                KeyCode::Left => prompt.input.word_left(),
                KeyCode::Right => prompt.input.word_right(),
                _ => {}
            }
        } else {
            match code {
                KeyCode::Esc => close = true,
                KeyCode::Enter => submit = true,
                KeyCode::Up => nav = -1,
                KeyCode::Down => nav = 1,
                KeyCode::Backspace => prompt.input.backspace(),
                KeyCode::Delete => prompt.input.delete(),
                KeyCode::Left => prompt.input.left(),
                KeyCode::Right => prompt.input.right(),
                KeyCode::Home => prompt.input.home(),
                KeyCode::End => prompt.input.end(),
                KeyCode::Char(ch) => prompt.input.insert_char(ch),
                _ => {}
            }
        }
    }
    if close {
        action::cancel_prompt(app);
        return Effect::None;
    }
    if submit {
        return action::submit_prompt(app);
    }
    if nav != 0 {
        let action = if nav > 0 {
            Action::MoveDown
        } else {
            Action::MoveUp
        };
        return action::dispatch(app, action);
    }
    action::update_filter_live(app);
    Effect::None
}

fn handle_palette(app: &mut App, code: KeyCode, ctrl: bool) -> Effect {
    if app.palette.is_none() {
        app.mode = Mode::Normal;
        return Effect::None;
    }
    let mut close = false;
    let mut run = None;
    {
        let palette = app.palette.as_mut().unwrap();
        if ctrl {
            match code {
                KeyCode::Char('a') => palette.input.select_all(),
                KeyCode::Char('u') => palette.input.kill_to_start(),
                KeyCode::Char('w') => palette.input.backspace_word(),
                _ => {}
            }
            palette.refilter();
            return Effect::None;
        }
        match code {
            KeyCode::Esc => close = true,
            KeyCode::Enter => {
                run = palette.selected();
                close = true;
            }
            KeyCode::Up => {
                palette.cursor = palette.cursor.saturating_sub(1);
                if palette.cursor < palette.scroll {
                    palette.scroll = palette.cursor;
                }
            }
            KeyCode::Down => {
                if !palette.filtered.is_empty() {
                    palette.cursor = (palette.cursor + 1).min(palette.filtered.len() - 1);
                }
            }
            KeyCode::PageUp => palette.cursor = palette.cursor.saturating_sub(10),
            KeyCode::PageDown => {
                if !palette.filtered.is_empty() {
                    palette.cursor = (palette.cursor + 10).min(palette.filtered.len() - 1);
                }
            }
            KeyCode::Backspace => {
                palette.input.backspace();
                palette.refilter();
            }
            KeyCode::Delete => {
                palette.input.delete();
                palette.refilter();
            }
            KeyCode::Left => palette.input.left(),
            KeyCode::Right => palette.input.right(),
            KeyCode::Home => palette.input.home(),
            KeyCode::End => palette.input.end(),
            KeyCode::Char(ch) => {
                palette.input.insert_char(ch);
                palette.refilter();
            }
            _ => {}
        }
    }
    if close {
        app.palette = None;
        app.mode = Mode::Normal;
        if let Some(action) = run {
            return action::dispatch(app, action);
        }
    }
    Effect::None
}

fn handle_help(app: &mut App, code: KeyCode) {
    if app.help.is_none() {
        app.mode = Mode::Normal;
        return;
    }
    let mut close = false;
    {
        let help = app.help.as_mut().unwrap();
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::F(1) => close = true,
            KeyCode::Char('j') | KeyCode::Down => help.scroll = help.scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => help.scroll = help.scroll.saturating_sub(1),
            KeyCode::PageDown => help.scroll = help.scroll.saturating_add(10),
            KeyCode::PageUp => help.scroll = help.scroll.saturating_sub(10),
            KeyCode::Char('g') | KeyCode::Home => help.scroll = 0,
            KeyCode::Char('G') | KeyCode::End => help.scroll = usize::MAX,
            _ => {}
        }
    }
    if close {
        app.help = None;
        app.mode = Mode::Normal;
    }
}

fn handle_info(app: &mut App, code: KeyCode) {
    if app.info.is_none() {
        app.mode = Mode::Normal;
        return;
    }
    let mut close = false;
    {
        let info = app.info.as_mut().unwrap();
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('i') | KeyCode::Enter => close = true,
            KeyCode::Char('j') | KeyCode::Down => info.scroll = info.scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => info.scroll = info.scroll.saturating_sub(1),
            KeyCode::PageDown => info.scroll = info.scroll.saturating_add(10),
            KeyCode::PageUp => info.scroll = info.scroll.saturating_sub(10),
            KeyCode::Char('g') | KeyCode::Home => info.scroll = 0,
            KeyCode::Char('G') | KeyCode::End => info.scroll = usize::MAX,
            _ => {}
        }
    }
    if close {
        app.info = None;
        app.mode = Mode::Normal;
    }
}

fn handle_sidebar(app: &mut App, code: KeyCode) -> Effect {
    match code {
        KeyCode::Char('j') | KeyCode::Down => action::dispatch(app, Action::SidebarDown),
        KeyCode::Char('k') | KeyCode::Up => action::dispatch(app, Action::SidebarUp),
        KeyCode::Char('g') | KeyCode::Home => action::dispatch(app, Action::SidebarTop),
        KeyCode::Char('G') | KeyCode::End => action::dispatch(app, Action::SidebarBottom),
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            action::dispatch(app, Action::SidebarOpen)
        }
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('b') => {
            app.focus = Focus::Files;
            Effect::None
        }
        KeyCode::Tab => {
            app.focus_next(false);
            Effect::None
        }
        KeyCode::BackTab => {
            app.focus_next(true);
            Effect::None
        }
        KeyCode::Char('q') => Effect::Quit,
        _ => Effect::None,
    }
}

fn handle_preview(app: &mut App, code: KeyCode) -> Effect {
    match code {
        KeyCode::Char('j') | KeyCode::Down => action::dispatch(app, Action::PreviewDown),
        KeyCode::Char('k') | KeyCode::Up => action::dispatch(app, Action::PreviewUp),
        KeyCode::PageDown => action::dispatch(app, Action::PreviewPageDown),
        KeyCode::PageUp => action::dispatch(app, Action::PreviewPageUp),
        KeyCode::Char('g') | KeyCode::Home => action::dispatch(app, Action::PreviewTop),
        KeyCode::Char('G') | KeyCode::End => action::dispatch(app, Action::PreviewBottom),
        KeyCode::Char('h') | KeyCode::Left => action::dispatch(app, Action::PreviewLeft),
        KeyCode::Char('l') | KeyCode::Right => action::dispatch(app, Action::PreviewRight),
        KeyCode::Esc => {
            app.focus = Focus::Files;
            Effect::None
        }
        KeyCode::Tab => {
            app.focus_next(false);
            Effect::None
        }
        KeyCode::BackTab => {
            app.focus_next(true);
            Effect::None
        }
        KeyCode::Char('q') => Effect::Quit,
        _ => Effect::None,
    }
}

fn switch_tab(app: &mut App, index: usize) {
    if index >= app.tabs.len() {
        return;
    }
    app.tab = index;
    app.focus = Focus::Files;
    let tab = app.tab;
    app.load_tab_panes(tab);
    app.request_preview();
}

fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    match app.mode {
        Mode::Help => {
            if let Some(help) = &mut app.help {
                match mouse.kind {
                    MouseEventKind::ScrollDown => help.scroll = help.scroll.saturating_add(3),
                    MouseEventKind::ScrollUp => help.scroll = help.scroll.saturating_sub(3),
                    _ => {}
                }
            }
            return;
        }
        Mode::Info => {
            if let Some(info) = &mut app.info {
                match mouse.kind {
                    MouseEventKind::ScrollDown => info.scroll = info.scroll.saturating_add(3),
                    MouseEventKind::ScrollUp => info.scroll = info.scroll.saturating_sub(3),
                    _ => {}
                }
            }
            return;
        }
        Mode::Prompt | Mode::Palette | Mode::Confirm => return,
        Mode::Normal => {}
    }

    let column = mouse.column;
    let row = mouse.row;
    let preview_rect = app.layout.preview;
    let sidebar_rect = app.layout.sidebar;

    if matches!(
        mouse.kind,
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
    ) {
        let delta = if mouse.kind == MouseEventKind::ScrollDown {
            3
        } else {
            -3
        };
        if preview_rect.is_some_and(|rect| rect_contains(rect, column, row)) {
            app.focus = Focus::Preview;
            if delta > 0 {
                app.preview_scroll = app.preview_scroll.saturating_add(delta as usize);
            } else {
                app.preview_scroll = app.preview_scroll.saturating_sub((-delta) as usize);
            }
            return;
        }
        if sidebar_rect.is_some_and(|rect| rect_contains(rect, column, row)) {
            app.focus = Focus::Sidebar;
            let len = app.sidebar_targets().len();
            if len > 0 {
                let next = app.sidebar_cursor as isize + if delta > 0 { 1 } else { -1 };
                app.sidebar_cursor = next.clamp(0, len as isize - 1) as usize;
            }
            return;
        }
        if let Some(index) = pane_at(app, column, row) {
            focus_pane(app, index);
            let len = app.active_pane().view_len();
            if len > 0 {
                let current = app.active_pane().cursor as isize + delta;
                app.active_pane_mut().cursor = current.clamp(0, len as isize - 1) as usize;
                app.active_pane_mut().ensure_scroll();
                app.request_preview();
            }
        }
        return;
    }

    if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
        return;
    }

    let tab_hit = app
        .layout
        .tab_hits
        .iter()
        .find(|(rect, _)| rect_contains(*rect, column, row))
        .map(|(_, index)| *index);
    if let Some(index) = tab_hit {
        switch_tab(app, index);
        return;
    }

    if sidebar_rect.is_some_and(|rect| rect_contains(rect, column, row)) {
        let hit = app
            .layout
            .sidebar_hits
            .iter()
            .find(|(rect, _)| rect_contains(*rect, column, row))
            .map(|(_, index)| *index);
        if let Some(index) = hit {
            app.sidebar_cursor = index;
            app.focus = Focus::Files;
            action::dispatch(app, Action::SidebarOpen);
        }
        return;
    }

    for pane in 0..2 {
        let Some(layout) = app.layout.panes[pane].clone() else {
            continue;
        };
        if !rect_contains(layout.header, column, row) {
            continue;
        }
        let crumb = layout
            .crumbs
            .iter()
            .find(|(rect, _)| rect_contains(*rect, column, row))
            .map(|(_, path)| path.clone());
        if let Some(path) = crumb {
            focus_pane(app, pane);
            if let Err(err) = app.navigate(path) {
                app.set_error(err);
            }
            return;
        }
        let column_hit = layout
            .columns
            .iter()
            .find(|(rect, _)| rect_contains(*rect, column, row))
            .map(|(_, sort_column)| *sort_column);
        if let Some(sort_column) = column_hit {
            focus_pane(app, pane);
            let key = match sort_column {
                Column::Name => crate::filesystem::SortKey::Name,
                Column::Size => crate::filesystem::SortKey::Size,
                Column::Modified => crate::filesystem::SortKey::Modified,
            };
            if app.active_pane().sort == key {
                action::dispatch(app, Action::SortOrder);
            } else {
                let fuzzy = app.fuzzy();
                let active = app.active_pane_mut();
                active.sort = key;
                active.rebuild(fuzzy);
            }
            return;
        }
        return;
    }

    if let Some(index) = pane_at(app, column, row) {
        let Some(list_rect) = app.layout.panes[index].as_ref().map(|layout| layout.list) else {
            return;
        };
        if !rect_contains(list_rect, column, row) {
            return;
        }
        focus_pane(app, index);
        let row_in_list = (row - list_rect.y) as usize;
        let target = app.active_pane().scroll + row_in_list;
        if target < app.active_pane().view_len() {
            let double = app.last_click.is_some_and(|(at, last_col, last_row)| {
                last_col == column
                    && last_row == row
                    && (at.elapsed().as_millis() as u64) < app.config.behavior.double_click_ms
            });
            app.last_click = Some((std::time::Instant::now(), column, row));
            app.active_pane_mut().cursor = target;
            app.active_pane_mut().ensure_scroll();
            app.request_preview();
            if double {
                let _ = action::dispatch(app, Action::Open);
            }
        }
        return;
    }

    if preview_rect.is_some_and(|rect| rect_contains(rect, column, row)) {
        app.focus = Focus::Preview;
    }
}

fn pane_at(app: &App, column: u16, row: u16) -> Option<usize> {
    for index in 0..2 {
        if let Some(layout) = &app.layout.panes[index] {
            if rect_contains(layout.list, column, row) || rect_contains(layout.header, column, row)
            {
                return Some(index);
            }
        }
    }
    None
}

fn focus_pane(app: &mut App, index: usize) {
    if app.dual {
        app.active_tab_mut().focus_right = index == 1;
    }
    app.focus = Focus::Files;
}

fn rect_contains(rect: ratatui::layout::Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}
