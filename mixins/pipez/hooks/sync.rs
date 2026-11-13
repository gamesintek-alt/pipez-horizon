fn pipez_sync(pos: [i32;3], entity: &horizon_client::containers::ModBlockEntity, tag: &mut Option<horizon_world::nbt::Compound>) {
let _=pos;if entity.id=="pipez:pipe"{*tag=Some(entity.nbt.clone());}
}
