use horizon_client::{containers::ModBlockEntity,items::ItemStack};
use horizon_world::nbt::{Compound,Tag};
use crate::game::pipez_rules as rules;
pub(super) fn copy_upgrade(e:&ModBlockEntity,side:usize,target:&mut ItemStack){
 let all=(0..4).map(|ch|{let mut c=settings(e,side,ch);for key in ["next","delay","cursor","moved"]{c.remove(&key.into());}Tag::Compound(c)}).collect();
 let mut custom=target.custom_data().cloned().unwrap_or_default();custom.insert("pipez_settings".into(),Tag::List(all));target.data_mut().set_other("minecraft:custom_data",Some(Tag::Compound(custom)));
}
pub(super) fn clear_upgrade(target:&mut ItemStack){let mut custom=target.custom_data().cloned().unwrap_or_default();custom.remove(&"pipez_settings".into());target.data_mut().set_other("minecraft:custom_data",if custom.is_empty(){None}else{Some(Tag::Compound(custom))});}
pub(super) fn int(n: &Compound, k: &str, d: i64) -> i64 {
    match n.get(&k.into()) {
        Some(Tag::Int(v)) => *v as i64,
        Some(Tag::Long(v)) => *v,
        Some(Tag::Byte(v)) => *v as i64,
        _ => d,
    }
}
pub(super) fn put(n: &mut Compound, k: &str, v: i64) {
    n.insert(k.into(), Tag::Long(v));
}
pub(super) fn text(n: &Compound, k: &str) -> String {
    match n.get(&k.into()) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    }
}
pub(super) fn list(n: &Compound, k: &str) -> Vec<Tag> {
    match n.get(&k.into()) {
        Some(Tag::List(v)) => v.clone(),
        _ => vec![],
    }
}
pub(super) fn settings(e: &ModBlockEntity, side: usize, ch: usize) -> Compound {
    let key = format!("p{side}_{ch}");
    match e.nbt.get(&key.as_str().into()) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Compound::default(),
    }
}
pub(super) fn save(e: &mut ModBlockEntity, side: usize, ch: usize, c: Compound) {
    e.nbt.insert(format!("p{side}_{ch}").as_str().into(), Tag::Compound(c));
}
pub(super) fn modes(e: &ModBlockEntity) -> [u8; 6] {
    std::array::from_fn(|i| int(&e.nbt, &format!("side{i}"), 0).clamp(0, 2) as u8)
}
pub(super) fn upgrade(e: &ModBlockEntity, side: usize) -> usize {
    e.items.get(side).and_then(Option::as_ref).map_or(0, |s| rules::tier(&s.item))
}
pub(super) fn stack_tag(s: &ItemStack) -> Tag {
    let mut c = Compound::default();
    c.insert("id".into(), Tag::String(s.item.as_ref().into()));
    if let Some(v) = horizon_client::item_data::encode_components(s) {
        c.insert("components".into(), v);
    }
    Tag::Compound(c)
}
pub(super) fn subset(needle: &Tag, value: &Tag) -> bool {
    match (needle, value) {
        (Tag::Compound(a), Tag::Compound(b)) => {
            a.iter().all(|(k, v)| b.get(k).is_some_and(|x| subset(v, x)))
        }
        (Tag::List(a), Tag::List(b)) => {
            a.iter().all(|v| b.iter().any(|x| subset(v, x)))
        }
        _ => needle == value,
    }
}
pub(super) fn remember_upgrade(e: &mut ModBlockEntity, side: usize) {
    let data: Vec<Tag> = (0..4)
        .map(|ch| {
            let mut c = settings(e, side, ch);
            for key in ["next", "delay", "cursor", "moved"] {
                c.remove(&key.into());
            }
            Tag::Compound(c)
        })
        .collect();
    if let Some(Some(s)) = e.items.get_mut(side) {
        let mut custom = s.custom_data().cloned().unwrap_or_default();
        custom.insert("pipez_settings".into(), Tag::List(data));
        s.data_mut().set_other("minecraft:custom_data", Some(Tag::Compound(custom)));
        let identity = stack_tag(s);
        e.nbt.insert(format!("seen{side}").as_str().into(), identity);
    }
}
pub(super) fn load_upgrades(e: &mut ModBlockEntity) {
    for side in 0..6 {
        let identity = e.items.get(side).and_then(Option::as_ref).map(stack_tag);
        let key = format!("seen{side}");
        if identity.as_ref() == e.nbt.get(&key.as_str().into()) {
            continue;
        }
        let data = e
            .items
            .get(side)
            .and_then(Option::as_ref)
            .and_then(|s| s.custom_data())
            .map(|c| list(c, "pipez_settings"))
            .unwrap_or_default();
        for ch in 0..4 {
            let c = match data.get(ch) {
                Some(Tag::Compound(c)) => c.clone(),
                _ => Compound::default(),
            };
            save(e, side, ch, c);
        }
        match identity {
            Some(v) => {
                e.nbt.insert(key.as_str().into(), v);
            }
            None => {
                e.nbt.remove(&key.as_str().into());
            }
        }
    }
}
