fn pipez_register(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
commands.push(horizon_client::mod_api::NativeCommand::new("pipez settings <x:int> <y:int> <z:int> <side:word> <channel:word> <json:text>"));
}
