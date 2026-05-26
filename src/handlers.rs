pub mod ui;
pub mod nix;

use crate::state::AppState;
use crate::action::Action;
use crate::context::Context;

pub fn handle_action(state: &mut AppState, context: &Context, action: Action) {
    ui::handle_ui_action(state, context, action.clone());
    nix::handle_nix_action(state, context, action);
}
