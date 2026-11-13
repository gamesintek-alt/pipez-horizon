fn pipez_button(game: &mut crate::game::Game<'_>, pos: [i32;3], entity: &mut horizon_client::containers::ModBlockEntity, id: i32, carried: &mut Option<horizon_client::items::ItemStack>) {
crate::game::pipez_runtime::button(game,pos,entity,id,carried);
}
