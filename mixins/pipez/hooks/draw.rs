fn pipez_draw(game: &crate::game::Game<'_>, block: &str, data: &[i32], batch: &mut horizon_client::hud::HudBatch, origin: [f32;2]) {
crate::game::pipez_runtime::draw(game,block,data,batch,origin);
}
