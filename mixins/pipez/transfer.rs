use super::*;
impl Game<'_> {
    pub(in crate::game) fn pipez_move_slot(
        &mut self,
        dimension: &str,
        from: [i32; 3],
        from_face: Direction,
        slot: usize,
        expected: &ItemStack,
        to: [i32; 3],
        to_face: Direction,
        max: u32,
    ) -> u32 {
        let Some(source) = self.native_item_source(dimension, from) else {
            return 0
        };
        let Some(target) = self.containers().view((to[0], to[1], to[2]), false) else {
            return 0
        };
        if source.parts.iter().any(|p| target.parts.contains(p))
            || !source.slots_for_face(Some(from_face)).contains(&slot)
        {
            return 0;
        }
        let into = target.slots_to_add(Some(to_face));
        let max = if matches!(target.kind, Kind::Composter(_)) {
            max.min(1)
        } else {
            max
        };
        let mut moved = 0;
        while moved < max {
            let c = self.containers();
            let Some(stack) = c.get(&source, slot) else { break };
            let mut identity = stack.clone();
            identity.count = expected.count;
            if identity != *expected || !c.can_take(&source, slot, stack, from_face)
            {
                break;
            }
            let Some(one) = self.take_one(&source, slot) else { break };
            if let Some(rest) = self.add_one_quietly(&target, &into, one, to_face) {
                self.put_back(&source, slot, rest);
                break;
            }
            moved += 1;
        }
        if moved > 0 {
            self.view_changed(&source);
            self.view_changed(&target);
        }
        moved
    }
}
