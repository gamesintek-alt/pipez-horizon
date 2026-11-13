fn pipez_ticks(block: &str, ticks: &mut bool) {
if crate::game::pipez_rules::pipe(block){*ticks=true;}
}
