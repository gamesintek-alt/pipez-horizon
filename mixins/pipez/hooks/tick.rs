fn pipez_tick(game: &mut crate::game::Game<'_>, pos: [i32;3], entity: &mut horizon_client::containers::ModBlockEntity) {
crate::game::pipez_runtime::tick(game,pos,entity);
}
