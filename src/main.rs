use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use nui::app::{self, Action, App, Executor, Exit, Mode};
use nui::{backend, log, shell_launch, tui, ui};
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
    backend::process::shutdown();
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
    let mut dirty = true;
    while app.exit.is_none() {
        for effect in app.effects.drain(..) {
            executor.run(effect);
        }
        if dirty {
            terminal.draw(|frame| ui::render(app, frame))?;
            dirty = false;
        }

        // Poll fast only while the throbber spins; idle frames are not redrawn.
        let timeout = if app.flake.loading { 16 } else { 50 };
        // Drain every queued keystroke before redrawing so a burst of typing lands in one frame.
        if event::poll(Duration::from_millis(timeout))? {
            let mut budget = 64;
            while budget > 0 && event::poll(Duration::ZERO)? {
                budget -= 1;
                match event::read()? {
                    Event::Key(key) => app::update::update(app, Action::Key(key)),
                    Event::Resize(..) => {}
                    _ => continue,
                }
                dirty = true;
            }
        }
        while let Ok(action) = rx.try_recv() {
            app::update::update(app, action);
            dirty = true;
        }
        dirty |= app::update::tick(app);
    }
    Ok(())
}
