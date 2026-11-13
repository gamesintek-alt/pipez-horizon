use std::collections::{HashSet, VecDeque};
pub type Pos = [i32; 3];
pub const OFFSETS: [Pos; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];
pub fn adjacent(p: Pos, side: usize) -> Pos {
    std::array::from_fn(|i| p[i] + OFFSETS[side][i])
}
pub fn opposite(side: usize) -> usize {
    side ^ 1
}
pub fn pipe(name: &str) -> bool {
    name.starts_with("pipez:")
        && ["item_pipe", "fluid_pipe", "energy_pipe", "gas_pipe", "universal_pipe"]
            .iter()
            .any(|k| name.ends_with(k))
}
pub fn channels(name: &str) -> Vec<usize> {
    if name.ends_with("universal_pipe") {
        vec![0, 1, 2, 3]
    } else if name.ends_with("item_pipe") {
        vec![0]
    } else if name.ends_with("fluid_pipe") {
        vec![1]
    } else if name.ends_with("energy_pipe") {
        vec![2]
    } else if name.ends_with("gas_pipe") {
        vec![3]
    } else {
        vec![]
    }
}
pub fn tier(id: &str) -> usize {
    match id.strip_prefix("pipez:").unwrap_or("") {
        "basic_upgrade" => 1,
        "improved_upgrade" => 2,
        "advanced_upgrade" => 3,
        "ultimate_upgrade" => 4,
        "omega_upgrade" => 5,
        "singularity_upgrade" => 6,
        "infinity_upgrade" => 7,
        _ => 0,
    }
}
pub fn rate(tier: usize, channel: usize) -> u32 {
    const R: [[u32; 4]; 8] = [
        [4, 50, 256, 200],
        [8, 100, 1024, 400],
        [16, 500, 8192, 2000],
        [32, 2000, 32768, 8000],
        [64, 10000, 131072, 40000],
        [640, 100000, 1310720, 400000],
        [64000, 10000000, 131072000, 40000000],
        [2147483647; 4],
    ];
    R[tier.min(7)][channel.min(3)]
}
pub fn interval(tier: usize, channel: usize) -> i64 {
    if channel == 0 { [20, 15, 10, 5, 1, 1, 1, 1][tier.min(7)] } else { 1 }
}
pub fn powered_allowed(tier: usize, mode: i32, signal: bool) -> bool {
    match if tier > 0 { mode } else { 0 } {
        1 => !signal,
        2 => signal,
        3 => false,
        _ => true,
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Backoff {
    pub delay: i64,
    pub next: i64,
}
impl Backoff {
    pub fn result(&mut self, now: i64, success: bool, base: i64, max: i64) {
        self.delay = if success {
            self.delay >> 1
        } else {
            if self.delay == 0 { base } else { self.delay.saturating_mul(2) }
                .min(max)
        };
        self.next = now.saturating_add(self.delay).saturating_add(1);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Endpoint {
    pub pos: Pos,
    pub side: usize,
    pub distance: usize,
}
/// Only identically named pipes connect. Disabled edges are checked at BOTH ends.
/// Output endpoints are not extracting faces. Loaded-world reads are supplied by the adapter.
pub fn network(
    start: Pos,
    name: &str,
    limit: usize,
    mut read: impl FnMut(Pos) -> Option<(String, [u8; 6])>,
) -> Vec<Endpoint> {
    let mut seen = HashSet::from([start]);
    let mut targets = HashSet::new();
    let mut queue = VecDeque::from([(start, 0)]);
    let mut out = Vec::new();
    while let Some((p, distance)) = queue.pop_front() {
        let Some((here, modes)) = read(p) else { continue };
        if here != name {
            continue;
        }
        for side in 0..6 {
            if modes[side] == 2 {
                continue;
            }
            let q = adjacent(p, side);
            let Some((other, other_modes)) = read(q) else {
                continue;
            };
            if other == name {
                if other_modes[opposite(side)] != 2 && seen.len() < limit
                    && seen.insert(q)
                {
                    queue.push_back((q, distance + 1));
                }
            } else if !pipe(&other) && modes[side] == 0
                && targets.insert((q, opposite(side)))
            {
                out.push(Endpoint {
                    pos: q,
                    side: opposite(side),
                    distance: distance + 1,
                });
            }
        }
    }
    out.sort_by_key(|e| (e.distance, e.pos, e.side));
    out
}
pub fn ordered(
    mut endpoints: Vec<Endpoint>,
    mode: i32,
    cursor: usize,
    random: u64,
) -> Vec<Endpoint> {
    match mode {
        1 => endpoints.reverse(),
        2 => {
            let n = endpoints.len();
            if n > 0 {
                endpoints.rotate_left(cursor % n);
            }
        }
        3 => {
            let mut seed = random;
            for n in (1..endpoints.len()).rev() {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                endpoints.swap(n, seed as usize % (n + 1));
            }
        }
        _ => {}
    }
    endpoints
}
pub fn distribute(
    targets: &[Endpoint],
    mut budget: u32,
    round_items: bool,
    equal: bool,
    mut send: impl FnMut(&Endpoint, u32) -> u32,
) -> (u32, Option<usize>) {
    let mut total = 0;
    let mut last = None;
    let mut visit = 0;
    let mut blocked = vec![false; targets.len()];
    while !targets.is_empty() && budget > 0 && (round_items || visit < targets.len())
    {
        if blocked.iter().all(|v| *v) {
            break;
        }
        let i = visit % targets.len();
        visit += 1;
        if blocked[i] {
            continue;
        }
        let allowance = if round_items {
            1
        } else if equal {
            budget.div_ceil((targets.len() - i) as u32)
        } else {
            budget
        };
        let moved = send(&targets[i], allowance).min(allowance);
        budget -= moved;
        total += moved;
        if moved > 0 {
            last = Some(i);
        } else {
            blocked[i] = true;
        }
    }
    (total, last)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[test]
    fn backoff_matches_addon_and_recovers() {
        let mut b = Backoff::default();
        let mut now = 0;
        for expected in [2, 4, 8, 16, 32, 32] {
            b.result(now, false, 2, 32);
            assert_eq!(b.delay, expected);
            assert_eq!(b.next, now + expected + 1);
            now = b.next;
        }
        b.result(now, true, 2, 32);
        assert_eq!(b.delay, 16);
        b.result(b.next, true, 2, 32);
        assert_eq!(b.delay, 8);
    }
    #[test]
    fn levels_preserve_original_rates() {
        assert_eq!((rate(0, 0), interval(0, 0)), (4, 20));
        assert_eq!((rate(3, 0), interval(3, 0)), (32, 5));
        assert_eq!((rate(5, 0), rate(6, 2)), (640, 131072000));
        assert_eq!(rate(7, 0), 2147483647);
    }
    #[test]
    fn color_and_type_separate_networks() {
        let world = HashMap::from([
            ([0, 0, 0], ("pipez:red_item_pipe".into(), [0; 6])),
            ([1, 0, 0], ("pipez:blue_item_pipe".into(), [0; 6])),
            ([2, 0, 0], ("minecraft:chest".into(), [0; 6])),
            ([0, 1, 0], ("pipez:red_item_pipe".into(), [0; 6])),
            ([0, 2, 0], ("minecraft:barrel".into(), [0; 6])),
        ]);
        let ends = network(
            [0, 0, 0],
            "pipez:red_item_pipe",
            100,
            |p| world.get(&p).cloned(),
        );
        assert_eq!(ends, vec![Endpoint { pos : [0, 2, 0], side : 0, distance : 2 }]);
    }
    #[test]
    fn cycles_disabled_edges_and_extraction_do_not_leak() {
        let mut w = HashMap::from([
            ([0, 0, 0], ("pipez:item_pipe".into(), [0; 6])),
            ([1, 0, 0], ("pipez:item_pipe".into(), [0; 6])),
            ([0, 1, 0], ("pipez:item_pipe".into(), [0; 6])),
            ([1, 1, 0], ("pipez:item_pipe".into(), [0; 6])),
            ([2, 0, 0], ("minecraft:chest".into(), [0; 6])),
        ]);
        w.get_mut(&[1, 0, 0]).unwrap().1[5] = 1;
        assert!(
            network([0, 0, 0], "pipez:item_pipe", 100,| p | w.get(& p).cloned())
            .is_empty()
        );
        w.get_mut(&[1, 0, 0]).unwrap().1[5] = 0;
        assert_eq!(
            network([0, 0, 0], "pipez:item_pipe", 100,| p | w.get(& p).cloned())
            .len(), 1
        );
        w.get_mut(&[1, 0, 0]).unwrap().1[5] = 2;
        assert!(
            network([0, 0, 0], "pipez:item_pipe", 100,| p | w.get(& p).cloned())
            .is_empty()
        );
    }
    #[test]
    fn round_robin_rotates_and_redstone_gates() {
        let a = Endpoint {
            pos: [0, 0, 0],
            side: 0,
            distance: 1,
        };
        let b = Endpoint {
            pos: [1, 0, 0],
            side: 0,
            distance: 2,
        };
        assert_eq!(ordered(vec![a, b], 2, 1, 1), vec![b, a]);
        assert!(powered_allowed(0, 3, true));
        assert!(! powered_allowed(1, 3, false));
        assert!(powered_allowed(1, 2, true));
        assert!(! powered_allowed(1, 2, false));
    }
    #[test]
    fn round_robin_resumes_after_last_item() {
        let ends: Vec<_> = (0..3)
            .map(|i| Endpoint {
                pos: [i, 0, 0],
                side: 0,
                distance: 1,
            })
            .collect();
        let mut contents = [0; 3];
        let (total, last) = distribute(
            &ends,
            4,
            true,
            true,
            |e, n| {
                contents[e.pos[0] as usize] += n;
                n
            },
        );
        assert_eq!((total, last, contents), (4, Some(0), [2, 1, 1]));
        let rotated = ordered(ends, 2, last.unwrap() + 1, 1);
        distribute(
            &rotated,
            4,
            true,
            true,
            |e, n| {
                contents[e.pos[0] as usize] += n;
                n
            },
        );
        assert_eq!(contents, [3, 3, 2]);
    }
    #[test]
    fn full_containers_leave_resources_at_source() {
        let ends: Vec<_> = (0..3)
            .map(|i| Endpoint {
                pos: [i, 0, 0],
                side: 0,
                distance: 1,
            })
            .collect();
        let mut source = 20;
        let mut room = [0, 2, 3];
        let (total, _) = distribute(
            &ends,
            64,
            true,
            true,
            |e, n| {
                let i = e.pos[0] as usize;
                let moved = n.min(room[i]).min(source);
                room[i] -= moved;
                source -= moved;
                moved
            },
        );
        assert_eq!(total, 5);
        assert_eq!(source, 15);
        assert_eq!(source + total, 20);
    }
}
