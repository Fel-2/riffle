use std::time::Duration;

use crossterm::event;

use crate::action::{Effect, Suspend};
use crate::input;
use crate::proc;
use crate::state::{App, TaskMsg};
use crate::terminal::Tui;
use crate::ui;

pub fn run(tui: &mut Tui, app: &mut App) -> anyhow::Result<()> {
    loop {
        drain(app);
        app.tick();

        tui.terminal.draw(|frame| ui::render(frame, app))?;
        if let Err(err) = ui::image::draw(app) {
            app.image.ready = None;
            app.image.pending = false;
            app.image.error = Some(format!("image: {err}"));
            app.set_error(format!("image: {err}"));
        }

        if app.should_quit {
            break;
        }

        let timeout = if app.is_busy() { 40 } else { 1000 };
        if event::poll(Duration::from_millis(timeout))? {
            let event = event::read()?;
            match input::handle_event(app, event) {
                Effect::Quit => app.should_quit = true,
                Effect::Suspend(job) => suspend_and_run(tui, app, job)?,
                Effect::None => {}
            }
        }
    }
    Ok(())
}

fn suspend_and_run(tui: &mut Tui, app: &mut App, job: Suspend) -> anyhow::Result<()> {
    tui.suspend()?;
    let result = match job {
        Suspend::Editor(path) => proc::open_editor(&path, &app.config),
        Suspend::Shell(command) => {
            let dir = app.current_dir();
            proc::run_shell(&command, &dir, &app.config)
        }
    };
    tui.resume()?;
    match result {
        Ok(()) => app.set_info("Done"),
        Err(err) => app.set_error(err),
    }
    app.refresh_active();
    Ok(())
}

fn drain(app: &mut App) {
    while let Ok(message) = app.dir_rx.try_recv() {
        app.apply_dir(message);
    }
    while let Ok(message) = app.preview_rx.try_recv() {
        app.apply_preview(message);
    }
    while let Ok(message) = app.git_rx.try_recv() {
        app.apply_git(message);
    }
    while let Ok(message) = app.image_rx.try_recv() {
        app.apply_image(message);
    }
    while let Ok(message) = app.sidebar_rx.try_recv() {
        app.apply_sidebar_targets(message);
    }
    while let Ok(message) = app.update_rx.try_recv() {
        app.apply_update(message);
    }
    drain_task(app);
}

fn drain_task(app: &mut App) {
    use std::sync::mpsc::TryRecvError;
    let mut finished = None;
    if let Some(task) = app.task.as_mut() {
        loop {
            match task.rx.try_recv() {
                Ok(TaskMsg::Progress { done, current }) => {
                    task.done = done;
                    task.current = current;
                }
                Ok(TaskMsg::Done {
                    ok,
                    total,
                    errors,
                    note,
                }) => {
                    finished = Some((ok, total, errors, note));
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = Some((task.done, task.total, Vec::new(), None));
                    break;
                }
            }
        }
    }
    if let Some((ok, total, errors, note)) = finished {
        app.task = None;
        app.git_cache.clear();
        app.refresh();
        let cancelled = errors.iter().any(|error| error == "cancelled");
        if cancelled {
            app.set_warn("Cancelled");
        } else if errors.is_empty() {
            app.set_success(note.unwrap_or_else(|| format!("Done — {ok}/{total}")));
        } else {
            let summary = errors
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ");
            app.set_error(format!("{ok}/{total} — {summary}"));
        }
    }
}
