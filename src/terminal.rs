use std::io::{self, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

static RESTORED: AtomicBool = AtomicBool::new(false);

pub fn restore_terminal() {
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste,
        Show
    );
    let _ = stdout.flush();
}

pub fn install_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        hook(info);
    }));
}

pub struct Tui {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
    mouse: bool,
}

impl Tui {
    pub fn enter(mouse: bool) -> io::Result<Self> {
        install_panic_hook();
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if mouse {
            execute!(
                stdout,
                EnterAlternateScreen,
                EnableMouseCapture,
                EnableBracketedPaste,
                Hide
            )?;
        } else {
            execute!(stdout, EnterAlternateScreen, EnableBracketedPaste, Hide)?;
        }
        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;
        Ok(Self { terminal, mouse })
    }

    pub fn suspend(&mut self) -> io::Result<()> {
        disable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste,
            Show
        )?;
        stdout.flush()
    }

    pub fn resume(&mut self) -> io::Result<()> {
        let mut stdout = io::stdout();
        if self.mouse {
            execute!(
                stdout,
                EnterAlternateScreen,
                EnableMouseCapture,
                EnableBracketedPaste,
                Hide
            )?;
        } else {
            execute!(stdout, EnterAlternateScreen, EnableBracketedPaste, Hide)?;
        }
        enable_raw_mode()?;
        self.terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        self.terminal.clear()?;
        while crossterm::event::poll(std::time::Duration::from_millis(0))? {
            let _ = crossterm::event::read()?;
        }
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        if !RESTORED.swap(true, Ordering::Relaxed) {
            restore_terminal();
        }
    }
}
