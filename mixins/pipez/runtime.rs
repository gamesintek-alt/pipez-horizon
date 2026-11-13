use crate::game::{Game, pipez_rules as rules};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    items::ItemStack, hud::HudBatch, mod_api::NativeScreen, lang,
};
use horizon_core::geometry::Direction;
use crate::game::native_block_entity_draw::{BlockEntityDraw, CubeFace};
use horizon_world::nbt::{Compound, Tag};
use rules::{Pos, Endpoint, adjacent, opposite};
use std::{cell::RefCell, collections::HashMap, sync::atomic::{AtomicU64, Ordering}};
const SIDES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];
const CHANNELS: [&str; 4] = ["item", "fluid", "energy", "gas"];
fn dir(i: usize) -> Direction {
    [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ][i]
}
use crate::game::pipez_storage::*;
static REVISION: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
struct Cache {
    generation: u64,
    expires: i64,
    ends: Vec<Endpoint>,
}
thread_local! {
    static CACHE : RefCell < HashMap < (String, Pos), Cache >>=
    RefCell::new(HashMap::new());
}
pub(in crate::game) fn changed() {
    REVISION.fetch_add(1, Ordering::Relaxed);
}
pub(in crate::game) fn clear() {
    changed();
    CACHE.with(|c| c.borrow_mut().clear());
}
fn network(
    game: &Game<'_>,
    pos: Pos,
    e: &ModBlockEntity,
    name: &str,
    now: i64,
) -> Vec<Endpoint> {
    let key = (game.dim.id().to_owned(), pos);
    let generation = REVISION.load(Ordering::Relaxed);
    if let Some(c) = CACHE
        .with(|c| c.borrow().get(&key).cloned())
        .filter(|c| c.generation == generation && now < c.expires)
    {
        return c.ends;
    }
    let ends = rules::network(
        pos,
        name,
        16384,
        |p| {
            let state = game.world.block(p[0], p[1], p[2]);
            let block = game.res.facts().get(state).name.to_string();
            if block == "minecraft:air" {
                return None;
            }
            let m = if p == pos {
                modes(e)
            } else {
                game.native_mod_block_entity(p).as_ref().map_or([0; 6], modes)
            };
            Some((block, m))
        },
    );
    CACHE
        .with(|c| {
            let mut c = c.borrow_mut();
            if c.len() > 8192 {
                c.clear();
            }
            c.insert(
                key,
                Cache {
                    generation,
                    expires: now + 100,
                    ends: ends.clone(),
                },
            );
        });
    ends
}
pub(in crate::game) fn entity(block: &str, out: &mut Option<(String, Compound)>) {
    if rules::pipe(block) && out.is_none() {
        *out = Some(("pipez:pipe".into(), Compound::default()));
    }
}
pub(in crate::game) fn container(block: &str, out: &mut Option<ModContainer>) {
    if rules::pipe(block) {
        *out = Some(ModContainer {
            slots: 6,
            menu: Some("pipez:pipe".into()),
            ..Default::default()
        });
    }
}
fn destination(c: &Compound, end: &Endpoint) -> bool {
    if int(c, "target", 0) == 0 {
        return true;
    }
    end.pos
        == [int(c, "tx", 0) as i32, int(c, "ty", 0) as i32, int(c, "tz", 0) as i32]
        && (int(c, "tf", -1) < 0 || end.side == int(c, "tf", -1) as usize)
}
fn item_filter(
    game: &Game<'_>,
    c: &Compound,
    s: &ItemStack,
    end: &Endpoint,
    tier: usize,
) -> bool {
    if tier < 3 {
        return true;
    }
    let filters = list(c, "filters");
    let mut positive = false;
    let mut matched = false;
    for f in filters {
        let Tag::Compound(f) = f else { continue };
        if text(&f, "id").is_empty() || !destination(&f, end) {
            continue;
        }
        let id = text(&f, "id");
        let idmatch = if let Some(tag) = id.strip_prefix('#') {
            game.res.items().item_in_tag(tag, &s.item)
        } else {
            id.is_empty() || id == s.item.as_ref()
        };
        let metadata = f.get(&"components".into());
        let stack = stack_tag(s);
        let Tag::Compound(sc) = stack else { unreachable!() };
        let metadatamatch = match metadata {
            None => true,
            Some(m) => {
                match sc.get(&"components".into()) {
                    None => false,
                    Some(v) => {
                        if int(&f, "exact", 0) != 0 { m == v } else { subset(m, v) }
                    }
                }
            }
        };
        let hit = idmatch && metadatamatch;
        let inverted = int(&f, "invert", 0) != 0;
        if inverted && hit {
            return false;
        }
        if !inverted {
            positive = true;
            matched |= hit;
        }
    }
    let hit = !positive || matched;
    if int(c, "blacklist", 0) != 0 { !hit } else { hit }
}
fn fluid_filter(c: &Compound, id: &str, end: &Endpoint, tier: usize) -> bool {
    if tier < 3 {
        return true;
    }
    let mut positive = false;
    let mut matched = false;
    for f in list(c, "filters") {
        let Tag::Compound(f) = f else { continue };
        if text(&f, "id").is_empty() || !destination(&f, end) {
            continue;
        }
        let key = text(&f, "id");
        let hit = key == id;
        if int(&f, "invert", 0) != 0 && hit {
            return false;
        }
        if int(&f, "invert", 0) == 0 {
            positive = true;
            matched |= hit;
        }
    }
    let hit = !positive || matched;
    if int(c, "blacklist", 0) != 0 { !hit } else { hit }
}
fn has_capability(game: &Game<'_>, p: Pos, face: usize, ch: usize) -> bool {
    let d = game.dim.id();
    match ch {
        0 => game.native_container_slots(d, p, dir(face)).is_some(),
        1 => game.native_fluid_tanks(d, p, dir(face)).is_some(),
        2 => game.native_energy(d, p, dir(face)).is_some(),
        _ => {
            game.native_mod_block_entity(p)
                .is_some_and(|e| e.nbt.contains_key(&"pipez_chemical".into()))
        }
    }
}
fn chemical_move(game: &mut Game<'_>, from: Pos, to: Pos, max: u32) -> u32 {
    if from == to {
        return 0;
    }
    let Some(source) = game.native_mod_block_entity(from) else { return 0 };
    let Some(target) = game.native_mod_block_entity(to) else { return 0 };
    let Some(Tag::Compound(a)) = source.nbt.get(&"pipez_chemical".into()) else {
        return 0
    };
    let Some(Tag::Compound(b)) = target.nbt.get(&"pipez_chemical".into()) else {
        return 0
    };
    let id = text(a, "id");
    let target_id = text(b, "id");
    if id.is_empty() || (!target_id.is_empty() && id != target_id)
        || int(a, "extract", 1) == 0 || int(b, "insert", 1) == 0
    {
        return 0;
    }
    let amount = int(a, "amount", 0)
        .max(0)
        .min((int(b, "capacity", 0) - int(b, "amount", 0)).max(0))
        .min(max as i64);
    if amount == 0 {
        return 0;
    }
    let mut a = a.clone();
    let mut b = b.clone();
    let left = int(&a, "amount", 0) - amount;
    let filled = int(&b, "amount", 0) + amount;
    put(&mut a, "amount", left);
    put(&mut b, "amount", filled);
    b.insert("id".into(), Tag::String(id.as_str().into()));
    if !game
        .native_change_mod_block_entity(
            from,
            &mut |e| {
                e.nbt.insert("pipez_chemical".into(), Tag::Compound(a.clone()));
            },
        )
    {
        return 0;
    }
    if !game
        .native_change_mod_block_entity(
            to,
            &mut |e| {
                e.nbt.insert("pipez_chemical".into(), Tag::Compound(b.clone()));
            },
        )
    {
        game.native_change_mod_block_entity(
            from,
            &mut |e| {
                e.nbt = source.nbt.clone();
            },
        );
        return 0;
    }
    game.native_block_entity_sync(from);
    game.native_block_entity_sync(to);
    amount as u32
}
fn refresh(game: &mut Game<'_>, p: Pos, e: &ModBlockEntity, name: &str) {
    let m = modes(e);
    for side in 0..6 {
        let q = adjacent(p, side);
        let other = game.native_block(q);
        let connected = m[side] != 2
            && if other == name {
                game.native_mod_block_entity(q)
                    .as_ref()
                    .is_none_or(|e| modes(e)[opposite(side)] != 2)
            } else if rules::pipe(&other) {
                false
            } else {
                rules::channels(name)
                    .into_iter()
                    .any(|ch| has_capability(game, q, opposite(side), ch))
            };
        game.native_set_block_property_with(
            p,
            SIDES[side],
            if connected { "true" } else { "false" },
            2,
        );
    }
}
pub(in crate::game) fn tick(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity) {
    if e.id != "pipez:pipe" {
        return;
    }
    let Some(clock) = game.native_clock() else { return };
    let now = clock.game_time;
    let name = game.native_block(pos);
    if !rules::pipe(&name) {
        return;
    }
    e.items.resize(6, None);
    load_upgrades(e);
    e.data.resize(40, 0);
    e.data[30..33].copy_from_slice(&pos);
    if now.rem_euclid(20)
        == (i64::from(pos[0]) + i64::from(pos[1]) + i64::from(pos[2])).rem_euclid(20)
    {
        refresh(game, pos, e, &name);
    }
    let m = modes(e);
    if !m.contains(&1) {
        update_data(e);
        return;
    }
    let ends = network(game, pos, e, &name, now);
    let dim = game.dim.id();
    for side in 0..6 {
        if m[side] != 1 {
            continue;
        }
        let source = adjacent(pos, side);
        let source_face = opposite(side);
        let tier = upgrade(e, side);
        for ch in rules::channels(&name) {
            let mut c = settings(e, side, ch);
            let next = int(&c, "next", 0);
            if now < next
                || !rules::powered_allowed(
                    tier,
                    int(&c, "redstone", 0) as i32,
                    game.native_neighbor_signal(dim, pos) > 0,
                )
            {
                continue;
            }
            let order = if tier >= 2 {
                int(&c, "distribution", 0) as i32
            } else {
                0
            };
            let cursor = int(&c, "cursor", 0).max(0) as usize;
            let random = (now as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(pos[0] as u64)
                .wrapping_add(side as u64 + 1);
            let targets = rules::ordered(
                ends
                    .iter()
                    .copied()
                    .filter(|x| {
                        x.pos != source && has_capability(game, x.pos, x.side, ch)
                    })
                    .collect(),
                order,
                cursor,
                random,
            );
            let (total, last) = rules::distribute(
                &targets,
                rules::rate(tier, ch),
                order == 2 && ch == 0,
                order == 2,
                |end, allowance| {
                    let moved = match ch {
                        0 => {
                            let mut moved = 0;
                            if let Some(slots) = game
                                .native_container_slots(dim, source, dir(source_face))
                            {
                                for slot in slots {
                                    let Some(stack) = slot.stack else { continue };
                                    if !slot.can_take
                                        || !item_filter(game, &c, &stack, end, tier)
                                    {
                                        continue;
                                    }
                                    moved
                                        += game
                                            .pipez_move_slot(
                                                dim,
                                                source,
                                                dir(source_face),
                                                slot.slot,
                                                &stack,
                                                end.pos,
                                                dir(end.side),
                                                allowance - moved,
                                            );
                                    if moved >= allowance {
                                        break;
                                    }
                                }
                            }
                            moved
                        }
                        1 => {
                            let first = game
                                .native_fluid_tanks(dim, source, dir(source_face))
                                .and_then(|t| {
                                    t.into_iter().find(|t| t.can_drain && t.amount > 0)
                                });
                            if first
                                .as_ref()
                                .and_then(|t| t.fluid.as_deref())
                                .is_some_and(|id| fluid_filter(&c, id, end, tier))
                            {
                                game.native_move_fluids(
                                        dim,
                                        source,
                                        dir(source_face),
                                        end.pos,
                                        dir(end.side),
                                        allowance.min(i32::MAX as u32) as i32,
                                    )
                                    .map_or(0, |(_, n)| n.max(0) as u32)
                            } else {
                                0
                            }
                        }
                        2 => {
                            game
                                .native_move_energy(
                                    dim,
                                    source,
                                    dir(source_face),
                                    end.pos,
                                    dir(end.side),
                                    allowance.min(i32::MAX as u32) as i32,
                                )
                                .max(0) as u32
                        }
                        _ => {
                            let id = game
                                .native_mod_block_entity(source)
                                .and_then(|e| match e.nbt.get(&"pipez_chemical".into()) {
                                    Some(Tag::Compound(c)) => Some(text(c, "id")),
                                    _ => None,
                                });
                            if id.is_some_and(|id| fluid_filter(&c, &id, end, tier)) {
                                chemical_move(game, source, end.pos, allowance)
                            } else {
                                0
                            }
                        }
                    };
                    moved
                },
            );
            if let Some(index) = last {
                put(&mut c, "cursor", (cursor + index + 1) as i64);
            }
            if ch == 0 {
                let mut back = rules::Backoff {
                    delay: int(&c, "delay", 0),
                    next: 0,
                };
                back.result(now, total > 0, 2, 32);
                put(&mut c, "delay", back.delay);
                put(&mut c, "next", back.next.max(now + rules::interval(tier, ch)));
            } else {
                put(&mut c, "next", now + rules::interval(tier, ch));
            }
            put(&mut c, "moved", total as i64);
            save(e, side, ch, c);
        }
    }
    update_data(e);
}
fn update_data(e: &mut ModBlockEntity) {
    let side = int(&e.nbt, "selected_side", 0).clamp(0, 5) as usize;
    let ch = int(&e.nbt, "selected_channel", 0).clamp(0, 3) as usize;
    let c = settings(e, side, ch);
    let mut data = vec![0; 40];
    for i in 30..33 {
        data[i] = e.data.get(i).copied().unwrap_or(0);
    }
    data[0] = side as i32;
    data[1] = ch as i32;
    data[2] = modes(e)[side] as i32;
    data[3] = int(&c, "redstone", 0) as i32;
    data[4] = int(&c, "distribution", 0) as i32;
    data[5] = int(&c, "blacklist", 0) as i32;
    data[6] = upgrade(e, side) as i32;
    data[7] = int(&c, "moved", 0) as i32;
    data[8] = int(&c, "components_mode", 0) as i32;
    for (i, f) in list(&c, "filters").iter().take(18).enumerate() {
        if let Tag::Compound(f) = f {
            data[10 + i] = if text(f, "id").is_empty() { 0 } else { 1 };
        }
    }
    e.data = data;
}
pub(in crate::game) fn click(
    game: &mut Game<'_>,
    pos: Pos,
    face: Pos,
    canceled: &mut bool,
) {
    let name = game.native_block(pos);
    if game
        .held()
        .is_some_and(|s| s.item.as_ref() == "pipez:filter_destination_tool")
    {
        *canceled = true;
        if !game.native_authoritative() {
            return;
        }
        let side = rules::OFFSETS.iter().position(|x| *x == face).unwrap_or(1);
        if let Some(stack) = game.held_mut() {
            let mut custom = stack.custom_data().cloned().unwrap_or_default();
            custom
                .insert(
                    "pipez_destination".into(),
                    Tag::compound([
                        ("target", Tag::Long(1)),
                        ("tx", Tag::Long(pos[0] as i64)),
                        ("ty", Tag::Long(pos[1] as i64)),
                        ("tz", Tag::Long(pos[2] as i64)),
                        ("tf", Tag::Long(side as i64)),
                    ]),
                );
            stack
                .data_mut()
                .set_other("minecraft:custom_data", Some(Tag::Compound(custom)));
        }
        return;
    }
    if !rules::pipe(&name) {
        return;
    }
    let Some(held) = game.held().cloned() else {
        return;
    };
    let fallback_side = rules::OFFSETS.iter().position(|x| *x == face).unwrap_or(1);
    let side = game.pick().filter(|h| h.pos == pos).map_or(fallback_side, |hit| {
        let lx = hit.point[0] - f64::from(pos[0]);
        let ly = hit.point[1] - f64::from(pos[1]);
        let lz = hit.point[2] - f64::from(pos[2]);
        if ly < 0.3124 {
            0
        } else if ly > 0.6876 {
            1
        } else if lz < 0.3124 {
            2
        } else if lz > 0.6876 {
            3
        } else if lx < 0.3124 {
            4
        } else if lx > 0.6876 {
            5
        } else {
            fallback_side
        }
    });
    if held.item.as_ref() == "pipez:wrench" {
        *canceled = true;
        if !game.native_authoritative() {
            return;
        }
        let mut next_mode = 0u8;
        game.native_change_mod_block_entity(
            pos,
            &mut |e| {
                let m = modes(e);
                next_mode = (m[side] + 1) % 3;
                put(&mut e.nbt, &format!("side{side}"), i64::from(next_mode));
                put(&mut e.nbt, "selected_side", side as i64);
                update_data(e);
            },
        );
        changed();
        if let Some(e) = game.native_mod_block_entity(pos) {
            refresh(game, pos, &e, &name);
        }
        game.native_block_entity_sync(pos);
        game.swing_arm();
        let center = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + 0.5,
            f64::from(pos[2]) + 0.5,
        ];
        let block_source = horizon_client::settings::SOUND_SOURCES
            .iter()
            .position(|s| *s == "block")
            .unwrap_or(0);
        game.native_play_sound(
            "minecraft:block.lever.click",
            center,
            block_source,
            0.5,
            1.0 + f32::from(next_mode) * 0.15,
        );
        if game.lan_acting.is_none() {
            let mode_key = ["insert", "extract", "disabled"][next_mode as usize];
            let side_label = lang::get(&format!("pipez.value.side.{}", SIDES[side]));
            let mode_label = lang::get(&format!("pipez.value.mode.{mode_key}"));
            game.message = Some((
                format!("{side_label}: {mode_label}"),
                std::time::Instant::now(),
            ));
        }
    } else if rules::tier(&held.item) > 0 && game.native_sneaking() {
        *canceled = true;
        if !game.native_authoritative() {
            return;
        }
        let mut old = None;
        let mut accepted = false;
        game.native_change_mod_block_entity(
            pos,
            &mut |e| {
                if modes(e)[side] != 1 {
                    return;
                }
                e.items.resize(6, None);
                old = e.items[side].take();
                let mut one = held.clone();
                one.count = 1;
                e.items[side] = Some(one);
                accepted = true;
                update_data(e);
            },
        );
        if accepted {
            if !game.creative {
                let hand = game.held_mut();
                if let Some(s) = hand {
                    s.count -= 1;
                    if s.count == 0 {
                        *hand = None;
                    }
                }
            }
            if let Some(old) = old {
                if let Some(left) = game.give(old) {
                    if let Some(eye) = game.eye_position() {
                        game.throw_from_eyes(left, eye, false);
                    }
                }
            }
            game.native_block_entity_sync(pos);
        }
    }
}
pub(in crate::game) fn screen(block: &str, out: &mut Option<NativeScreen>) {
    if rules::pipe(block) {
        *out = Some(NativeScreen {
            width: 260.,
            height: 284.,
            slots: (0..6).map(|i| Some([8. + i as f32 * 20., 38.])).collect(),
            inventory_y: 202.,
            inventory_x: 8.,
            own_labels: true,
            own_background: false,
        });
    }
}
fn tr(k: &str) -> String {
    lang::get(&format!("pipez.ui.{k}"))
}
pub(in crate::game) fn draw(
    game: &Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut HudBatch,
    origin: [f32; 2],
) {
    if !rules::pipe(block) {
        return;
    }
    let [x, y] = origin;
    let get = |i: usize| data.get(i).copied().unwrap_or(0).max(0) as usize;
    let label = |batch: &mut HudBatch, s: &str, px: f32, py: f32| {
        game.native_text(batch, s, [x + px, y + py], 0x404040, false);
    };
    label(batch, &tr("title"), 8., 6.);
    label(batch, &tr("upgrade"), 8., 24.);
    let entries = [
        ("side", SIDES[get(0).min(5)]),
        ("channel", CHANNELS[get(1).min(3)]),
        ("mode", ["insert", "extract", "disabled"][get(2).min(2)]),
        (
            "redstone",
            [
                "ignored",
                "off_when_powered",
                "on_when_powered",
                "always_off",
            ][get(3).min(3)],
        ),
        (
            "distribution",
            ["nearest", "furthest", "round_robin", "random"][get(4).min(3)],
        ),
        ("filter", ["whitelist", "blacklist"][get(5).min(1)]),
    ];
    for (i, (key, value)) in entries.iter().enumerate() {
        let py = 62. + i as f32 * 16.;
        game.native_fill(batch, [x + 8., y + py, 244., 14.], 0xffa0a0a0);
        label(
            batch,
            &format!(
                "{}: {}", tr(key), lang::get(& format!("pipez.value.{key}.{value}"))
            ),
            12.,
            py + 3.,
        );
    }
    label(batch, &format!("NBT: {}", get(8)), 198., 165.);
    label(batch, &tr("copy"), 198., 182.);
    label(batch, &tr("clear"), 198., 198.);
    for i in 0..18 {
        let px = 8. + (i % 9) as f32 * 20.;
        let py = 160. + (i / 9) as f32 * 18.;
        game.native_fill(
            batch,
            [x + px, y + py, 18., 16.],
            if get(10 + i) > 0 { 0xff70a070 } else { 0xff707070 },
        );
        label(batch, &(i + 1).to_string(), px + 3., py + 4.);
    }
}
pub(in crate::game) fn screen_click(
    block: &str,
    at: [f32; 2],
    right: bool,
    button: &mut Option<i32>,
) {
    if !rules::pipe(block) {
        return;
    }
    let [x, y] = at;
    if (196.0..258.).contains(&x) && (160.0..212.).contains(&y) {
        *button = Some(if y < 178. { 6 } else if y < 194. { 7 } else { 8 });
    } else if (8.0..252.).contains(&x) && (62.0..158.).contains(&y) {
        *button = Some((y as i32 - 62) / 16);
    } else if (8.0..188.).contains(&x) && (160.0..196.).contains(&y) {
        let col = ((x - 8.) / 20.) as i32;
        let row = ((y - 160.) / 18.) as i32;
        *button = Some(100 + row * 9 + col + if right { 100 } else { 0 });
    }
}
pub(in crate::game) fn button(
    game: &mut Game<'_>,
    _pos: Pos,
    e: &mut ModBlockEntity,
    id: i32,
    carried: &mut Option<ItemStack>,
) {
    if e.id != "pipez:pipe" {
        return;
    }
    let side = int(&e.nbt, "selected_side", 0).clamp(0, 5) as usize;
    let ch = int(&e.nbt, "selected_channel", 0).clamp(0, 3) as usize;
    let mut c = settings(e, side, ch);
    let tier = upgrade(e, side);
    match id {
        0 => put(&mut e.nbt, "selected_side", ((side + 1) % 6) as i64),
        1 => put(&mut e.nbt, "selected_channel", ((ch + 1) % 4) as i64),
        2 => {
            let m = modes(e)[side];
            put(&mut e.nbt, &format!("side{side}"), ((m + 1) % 3) as i64);
            changed();
        }
        3 if tier >= 1 => {
            let v = (int(&c, "redstone", 0) + 1) % 4;
            put(&mut c, "redstone", v);
        }
        4 if tier >= 2 => {
            let v = (int(&c, "distribution", 0) + 1) % 4;
            put(&mut c, "distribution", v);
        }
        5 if tier >= 3 => {
            let v = 1 - int(&c, "blacklist", 0);
            put(&mut c, "blacklist", v);
        }
        6 if tier >= 3 => {
            let v = (int(&c, "components_mode", 0) + 1) % 3;
            put(&mut c, "components_mode", v);
        }
        7 => {
            if let Some(target) = carried
                .as_mut()
                .filter(|s| rules::tier(&s.item) > 0)
            {
                copy_upgrade(e,side,target);
            }
        }
        8 => {
            if let Some(target) = carried
                .as_mut()
                .filter(|s| rules::tier(&s.item) > 0)
            {
                clear_upgrade(target);
            } else {
                for channel in 0..4 {
                    save(e, side, channel, Compound::default());
                }
                c = Compound::default();
            }
        }
        100..=117 | 200..=217 if tier >= 3 => {
            let index = (id % 100) as usize;
            let mut filters = list(&c, "filters");
            filters.resize_with(18, || Tag::Compound(Compound::default()));
            filters[index] = if id >= 200 || carried.is_none() {
                Tag::Compound(Compound::default())
            } else {
                let s = carried.as_ref().unwrap();
                if s.item.as_ref() == "pipez:filter_destination_tool" {
                    let mut f = match &filters[index] {
                        Tag::Compound(c) => c.clone(),
                        _ => Compound::default(),
                    };
                    if let Some(Tag::Compound(target)) = s
                        .custom_data()
                        .and_then(|c| c.get(&"pipez_destination".into()))
                    {
                        for (k, v) in target {
                            f.insert(k.clone(), v.clone());
                        }
                    }
                    Tag::Compound(f)
                } else {
                    let mut f = Compound::default();
                    let identifier = if ch == 1 {
                        match s.item.as_ref() {
                            "minecraft:water_bucket" => "minecraft:water",
                            "minecraft:lava_bucket" => "minecraft:lava",
                            id => id,
                        }
                    } else {
                        s.item.as_ref()
                    };
                    f.insert("id".into(), Tag::String(identifier.into()));
                    let mode = int(&c, "components_mode", 0);
                    if mode > 0 {
                        if let Some(v) = horizon_client::item_data::encode_components(
                            s,
                        ) {
                            f.insert("components".into(), v);
                            put(&mut f, "exact", i64::from(mode == 2));
                        }
                    }
                    Tag::Compound(f)
                }
            };
            c.insert("filters".into(), Tag::List(filters));
        }
        _ => {}
    }
    put(&mut c, "next", 0);
    save(e, side, ch, c);
    remember_upgrade(e, side);
    update_data(e);
    let _ = game;
}
pub(in crate::game) fn tooltip(
    game: &Game<'_>,
    block: &str,
    data: &[i32],
    at: [f32; 2],
    lines: &mut Vec<String>,
) {
    if !rules::pipe(block) {
        return;
    }
    let [x, y] = at;
    if (8.0..188.).contains(&x) && (160.0..196.).contains(&y) {
        let index = ((y - 160.) / 18.) as usize * 9 + ((x - 8.) / 20.) as usize;
        let p = std::array::from_fn(|i| data.get(30 + i).copied().unwrap_or(0));
        let e = game.native_mod_block_entity(p);
        let side = data.first().copied().unwrap_or(0).clamp(0, 5) as usize;
        let ch = data.get(1).copied().unwrap_or(0).clamp(0, 3) as usize;
        if let Some(e) = e {
            if let Some(Tag::Compound(f)) = list(&settings(&e, side, ch), "filters")
                .get(index)
            {
                let id = text(f, "id");
                if !id.is_empty() {
                    lines.push(id);
                    if int(f, "target", 0) != 0 {
                        lines
                            .push(
                                format!(
                                    "{}: {}, {}, {} ({})", tr("destination"), int(f, "tx", 0),
                                    int(f, "ty", 0), int(f, "tz", 0), int(f, "tf",- 1)
                                ),
                            );
                    }
                    return;
                }
            }
        }
        lines.push(tr("hint"));
    }
}
fn json_tag(v: &serde_json::Value) -> Tag {
    match v {
        serde_json::Value::String(s) => Tag::String(s.as_str().into()),
        serde_json::Value::Bool(v) => Tag::Byte(i8::from(*v)),
        serde_json::Value::Number(n) => {
            n.as_i64()
                .map(|n| i32::try_from(n).map(Tag::Int).unwrap_or(Tag::Long(n)))
                .unwrap_or_else(|| Tag::Double(n.as_f64().unwrap_or(0.).to_bits()))
        }
        serde_json::Value::Array(v) => Tag::List(v.iter().map(json_tag).collect()),
        serde_json::Value::Object(v) => {
            Tag::Compound(
                v.iter().map(|(k, v)| (k.as_str().into(), json_tag(v))).collect(),
            )
        }
        _ => Tag::Compound(Compound::default()),
    }
}
pub(in crate::game) fn perform(
    game: &mut Game<'_>,
    player: &horizon_client::mod_api::NativeServerPlayer,
    line: &str,
    ran: &mut bool,
) {
    if *ran || !line.starts_with("pipez settings ") {
        return;
    }
    *ran = true;
    let result = (|| -> Option<()> {
        let w: Vec<_> = line.splitn(8, ' ').collect();
        if w.len() != 8 || w[7].len() > 16384 {
            return None;
        }
        let pos = [w[2].parse().ok()?, w[3].parse().ok()?, w[4].parse().ok()?];
        if pos
            .iter()
            .zip(player.position)
            .map(|(a, b)| (*a as f64 + 0.5 - b).powi(2))
            .sum::<f64>() > 64. || player.dimension != game.dim.id()
        {
            return None;
        }
        let side = SIDES
            .iter()
            .position(|s| *s == w[5])
            .or_else(|| w[5].parse::<usize>().ok().filter(|i| *i < 6))?;
        let ch = CHANNELS
            .iter()
            .position(|s| *s == w[6])
            .or_else(|| w[6].parse::<usize>().ok().filter(|i| *i < 4))?;
        let v: serde_json::Value = serde_json::from_str(w[7]).ok()?;
        let Tag::Compound(c) = json_tag(&v) else {
            return None;
        };
        if !rules::pipe(&game.native_block(pos)) {
            return None;
        }
        let mut changed = false;
        game.native_change_mod_block_entity(
            pos,
            &mut |e| {
                let tier = upgrade(e, side);
                if tier < 3 {
                    return;
                }
                let mut current = settings(e, side, ch);
                for (k, v) in &c {
                    let key = k.to_string_lossy();
                    if ["filters", "blacklist", "distribution", "redstone"]
                        .contains(&key.as_str())
                    {
                        current.insert(k.clone(), v.clone());
                    }
                }
                put(&mut current, "next", 0);
                save(e, side, ch, current);
                remember_upgrade(e, side);
                update_data(e);
                changed = true;
            },
        );
        if !changed {
            return None;
        }
        game.native_block_entity_sync(pos);
        Some(())
    })();
    game.native_action_bar(
        player.uuid,
        horizon_client::text::Text::tr(
            if result.is_some() {
                "pipez.ui.saved"
            } else {
                "pipez.ui.command_error"
            },
            &[],
        ),
    );
}

pub(in crate::game) fn submit(
    game: &Game<'_>,
    block: &str,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    if !rules::pipe(block) {
        return;
    }
    let m = modes(entity);
    if !m.contains(&1) {
        return;
    }
    let tex = block.strip_prefix("pipez:").unwrap_or(block);
    let sprite = format!("pipez:block/{tex}");
    let face_uv = |uv: [f32; 4]| Some(CubeFace { sprite: &sprite, uv: Some(uv) });
    let cap = face_uv([8.0, 6.0, 16.0, 14.0]);
    let rim_h = face_uv([8.0, 6.0, 16.0, 7.0]);
    let rim_v = face_uv([0.0, 6.0, 1.0, 14.0]);
    for side in 0..6 {
        if m[side] != 1 {
            continue;
        }
        let (from, to, faces) = match side {
            0 => ([4.0, 0.0, 4.0], [12.0, 1.0, 12.0], [cap, cap, rim_h, rim_h, rim_h, rim_h]),
            1 => ([4.0, 15.0, 4.0], [12.0, 16.0, 12.0], [cap, cap, rim_h, rim_h, rim_h, rim_h]),
            2 => ([4.0, 4.0, 0.0], [12.0, 12.0, 1.0], [rim_h, rim_h, cap, cap, rim_v, rim_v]),
            3 => ([4.0, 4.0, 15.0], [12.0, 12.0, 16.0], [rim_h, rim_h, cap, cap, rim_v, rim_v]),
            4 => ([0.0, 4.0, 4.0], [1.0, 12.0, 12.0], [rim_v, rim_v, rim_v, rim_v, cap, cap]),
            _ => ([15.0, 4.0, 4.0], [16.0, 12.0, 12.0], [rim_v, rim_v, rim_v, rim_v, cap, cap]),
        };
        game.native_draw_cube(draw, from, to, faces, 0xFF_FFFF);
    }
    draw.keep();
}
