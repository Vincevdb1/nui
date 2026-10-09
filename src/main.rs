mod app;
mod backend;
mod features;
mod flake;
mod log;
mod pkgref;
mod shell_launch;
mod tui;
mod ui;

use app::{Action, App, Executor, Exit, Mode};
use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use ratatui::crossterm::event::{self, Event};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Build an ad-hoc `nix shell` instead of editing a flake.
    Shell,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let mode = match Cli::parse().command {
        Some(Command::Shell) => Mode::Shell,
        None => Mode::Flake,
    };
    backend::templates::ensure_dir();

    let (tx, rx) = mpsc::channel();
    let log_tx = tx.clone();
    log::init(move |entry| {
        let _ = log_tx.send(Action::Log(entry));
    });
    let executor = Executor::new(tx);
    let mut app = App::new(mode);

    let mut terminal = tui::init()?;
    let result = run(&mut terminal, &mut app, &executor, &rx);
    tui::restore()?;
    result?;

    if let Some(Exit::Shell(packages)) = &app.exit
        && !packages.is_empty()
    {
        shell_launch::launch(packages, &app.system.clone().unwrap_or_default())?;
    }
    Ok(())
}

fn run(
    terminal: &mut tui::Tui,
    app: &mut App,
    executor: &Executor,
    rx: &mpsc::Receiver<Action>,
) -> Result<()> {
    while app.exit.is_none() {
        for effect in app.effects.drain(..) {
            executor.run(effect);
        }
        terminal.draw(|frame| ui::render(app, frame))?;

        // Drain every queued keystroke before redrawing so a burst of typing lands in one frame.
        if event::poll(Duration::from_millis(16))? {
            let mut budget = 64;
            while budget > 0 && event::poll(Duration::ZERO)? {
                budget -= 1;
                if let Event::Key(key) = event::read()? {
                    app::update::update(app, Action::Key(key));
                }
            }
        }
        while let Ok(action) = rx.try_recv() {
            app::update::update(app, action);
        }
        app::update::update(app, Action::Tick);
    }
    Ok(())
}
