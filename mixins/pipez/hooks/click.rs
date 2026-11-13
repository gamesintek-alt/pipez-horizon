fn pipez_click(game: &mut crate::game::Game<'_>, pos: [i32;3], face: [i32;3], canceled: &mut bool) {
crate::game::pipez_runtime::click(game,pos,face,canceled);
}
