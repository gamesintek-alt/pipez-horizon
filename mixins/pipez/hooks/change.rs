fn pipez_change(old: horizon_world::registry::BlockStateId, new: horizon_world::registry::BlockStateId) {
if old!=new{crate::game::pipez_runtime::changed();}
}
