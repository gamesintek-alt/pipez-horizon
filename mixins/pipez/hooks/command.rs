fn pipez_command(game: &mut crate::game::Game<'_>, player: &horizon_client::mod_api::NativeServerPlayer, command: &str, ran: &mut bool) {
crate::game::pipez_runtime::perform(game,player,command,ran);
}
