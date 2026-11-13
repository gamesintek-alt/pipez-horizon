fn pipez_slot(menu: &str, slot: usize, stack: &horizon_client::items::ItemStack, may_place: &mut bool, max_stack: &mut u32) {
if menu=="pipez:pipe"&&slot<6{*may_place=stack.item.starts_with("pipez:")&&stack.item.ends_with("_upgrade");*max_stack=1;}
}
