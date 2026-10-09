use super::{Action, App, Effect, Exit, Mode, Popup};
use crate::features::{add_input, details, flake, help, search, shell, templates};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub fn update(app: &mut App, action: Action) {
    match action {
        Action::Key(key) => on_key(app, key),
        Action::Tick => {
            app.throbber.calc_next();
            search::tick(app);
        }
        Action::Log(entry) => app.log.push(entry),
        Action::SystemProbed(system, tools) => {
            if tools.nxv.is_some() {
                crate::log::output("NXV", "Starting background update check...");
                app.effects.push(Effect::UpdateNxv);
            }
            app.system = Some(system);
            app.tools = tools;
        }
        Action::FlakeLoaded(inputs, outputs) => flake::loaded(app, inputs, outputs),
        Action::FlakeChanged => flake::reload(app),
        Action::PackagesLoaded(generation, result) => {
            flake::packages_loaded(app, generation, result)
        }
        Action::LatestVersion { attr, version } => {
            search::latest_version(app, &attr, &version);
            app.flake.updates.insert(attr, version);
        }
        Action::SearchFinished(generation, result) => search::finished(app, generation, result),
        Action::LockedVersion {
            generation,
            attr,
            channel,
            version,
        } => search::locked_version(app, generation, &attr, &channel, version),
        Action::VersionsLoaded(attr, result) => search::versions_loaded(app, &attr, result),
        Action::ShellMetadata(reference, metadata) => {
            app.shell.apply_metadata(&reference, metadata)
        }
        Action::ShellTemplateLoaded(packages) => {
            shell::add_from_template(app, packages);
            app.popup = Popup::None;
        }
        Action::BranchesLoaded(result) => {
            if let Popup::AddInput(form) = &mut app.popup {
                form.branches_loaded(result, &app.flake.inputs);
            }
        }
        Action::TemplatesListed(list) => app.templates = list,
        Action::NxvProgress(progress) => app.nxv_progress = progress,
    }
}

fn on_key(app: &mut App, key: KeyEvent) {
    // Handlers own the popup while they run and return the one to show next.
    app.popup = match std::mem::take(&mut app.popup) {
        Popup::None => {
            main_key(app, key);
            return;
        }
        Popup::Help => help::on_key(key),
        Popup::Templates(picker) => templates::picker_key(app, picker, key),
        Popup::ConfirmTemplate(name) => templates::confirm_key(app, name, key),
        Popup::SaveTemplate(form) => templates::save_key(app, form, key),
        Popup::AddInput(form) => add_input::on_key(app, form, key),
        Popup::Search(s) => search::on_key(app, s, key),
        Popup::Details(result) => details::on_key(result, key),
    };
}

fn main_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') => app.exit = Some(Exit::Quit),
        KeyCode::Char('t') => templates::open(app),
        KeyCode::Char('m') => switch_mode(app),
        KeyCode::Char('?') => app.popup = Popup::Help,
        _ => match app.mode {
            Mode::Flake => flake::keys::on_key(app, key),
            Mode::Shell => shell::keys::on_key(app, key),
        },
    }
}

fn switch_mode(app: &mut App) {
    match app.mode {
        Mode::Flake => {
            app.mode = Mode::Shell;
            app.flake = Default::default();
            if app.shell.packages.is_empty() {
                search::open(app);
            }
        }
        Mode::Shell => {
            app.mode = Mode::Flake;
            app.focus = super::Focus::Title;
            flake::load(app);
        }
    }
}
