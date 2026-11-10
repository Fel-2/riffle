use std::path::PathBuf;
use std::process::ExitCode;

mod action;
mod app;
mod config;
mod filesystem;
mod input;
mod ops;
mod proc;
mod state;
mod syntax;
mod terminal;
mod theme;
mod ui;
mod utils;

use crate::state::App;
use crate::terminal::Tui;

struct Options {
    start: PathBuf,
    select: Option<PathBuf>,
    print_cwd: bool,
}

fn main() -> ExitCode {
    let options = match parse_args() {
        Ok(Some(options)) => options,
        Ok(None) => return ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("riffle: {err}");
            eprintln!("Try 'riffle --help' for usage.");
            return ExitCode::from(2);
        }
    };

    let loaded = config::load();
    let config_error = loaded.error.clone();
    let print_cwd = options.print_cwd || loaded.config.behavior.print_cwd;
    let mut app = App::new(loaded.config, options.start, print_cwd, options.select);
    if let Some(err) = config_error {
        app.show_config_error(&err);
    }
    app.update_title();

    let mut tui = match Tui::enter(app.config.ui.mouse) {
        Ok(tui) => tui,
        Err(err) => {
            eprintln!("riffle: could not start terminal: {err}");
            return ExitCode::FAILURE;
        }
    };

    let result = app::run(&mut tui, &mut app);
    let final_dir = app.current_dir();
    let print_cwd = app.print_cwd;
    drop(tui);

    if print_cwd {
        println!("{}", final_dir.display());
    }

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("riffle: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args() -> Result<Option<Options>, String> {
    let args = std::env::args().skip(1);
    let mut path: Option<PathBuf> = None;
    let mut print_cwd = false;
    let mut positional_only = false;

    for arg in args {
        if positional_only {
            set_path(&mut path, &arg)?;
            continue;
        }
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("riffle {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "-p" | "--print-cwd" => print_cwd = true,
            "--" => positional_only = true,
            other if other.starts_with('-') && other != "-" => {
                return Err(format!("unknown option '{other}'"));
            }
            other => set_path(&mut path, other)?,
        }
    }

    let (start, select) = match path {
        Some(path) => {
            let meta =
                std::fs::metadata(&path).map_err(|err| format!("{}: {err}", path.display()))?;
            if meta.is_dir() {
                (path, None)
            } else {
                let parent = path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("/"));
                (parent, Some(path))
            }
        }
        None => (
            std::env::current_dir().map_err(|err| format!("current directory: {err}"))?,
            None,
        ),
    };

    Ok(Some(Options {
        start,
        select,
        print_cwd,
    }))
}

fn set_path(slot: &mut Option<PathBuf>, value: &str) -> Result<(), String> {
    if slot.is_some() {
        return Err("only one path can be opened".into());
    }
    *slot = Some(PathBuf::from(value));
    Ok(())
}

fn print_help() {
    println!(
        "\
riffle {version} — a fast terminal file manager

USAGE:
    riffle [OPTIONS] [PATH]

ARGS:
    PATH    Directory to open, or a file to reveal

OPTIONS:
    -p, --print-cwd    Print the final directory on exit (shell integration)
    -h, --help         Print help
    -V, --version      Print version

KEYS:
    j/k or ↑/↓         move            ⏎ / l        open
    h / ← / ⌫          parent          space        select
    y / x / p          copy/cut/paste  d / D        trash / delete
    r / F2             rename          a / A        new file / directory
    /                  fuzzy filter    ctrl+g       go to path
    P                  preview         B            sidebar
    ctrl+w             dual pane       tab          switch pane
    ctrl+t / X         new / close tab [ / ]        switch tab
    ctrl+p             command palette ?            all keys
    q                  quit

Config: ~/.config/riffle/config.toml
",
        version = env!("CARGO_PKG_VERSION")
    );
}
