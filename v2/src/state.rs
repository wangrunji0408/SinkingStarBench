//! Read-only snapshot of the headless simulation at the end of a replay.
use super::*;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

unsafe fn string(len: usize, p: usize) -> Value {
    if len == 0 {
        return json!("");
    }
    if len > 1 << 20 || p == 0 {
        return Value::Null;
    }
    json!(String::from_utf8_lossy(std::slice::from_raw_parts(
        p as *const u8,
        len
    )))
}

unsafe fn reflected(p: usize, specs: &Map<String, Value>, limit: usize) -> Map<String, Value> {
    specs
        .iter()
        .filter_map(|(key, spec)| {
            let offset = spec[0].as_u64().unwrap() as usize;
            let size = spec[1].as_u64().unwrap() as usize;
            if offset + size > limit
                || key.to_ascii_lowercase().contains("xxx_deprecated")
                || key.ends_with(".ptr")
                || ["paint_data.", "index_view.", "drawable_locator."]
                    .iter()
                    .any(|prefix| key.starts_with(prefix))
            {
                return None;
            }
            let a = p + offset;
            let v = match (spec[2].as_str().unwrap(), size) {
                ("int", 1) => get::<u8>(a).into(),
                ("int", 2) => get::<u16>(a).into(),
                ("int", 4) => get::<u32>(a).into(),
                ("int", 8) => get::<u64>(a).into(),
                ("float", 4) => get::<f32>(a).into(),
                ("float", 8) => get::<f64>(a).into(),
                ("string", 16) => {
                    let [len, data] = get::<[usize; 2]>(a);
                    string(len, data)
                }
                _ => return None, // Host addresses are not portable state.
            };
            Some((key.clone(), v))
        })
        .collect()
}

// Factor out fields that are identical on every entity of a given type.
// Keep small, frequently queried gameplay fields directly on each entity.
fn compact_fields(entities: &mut [Value]) -> Map<String, Value> {
    let mut common = Map::new();
    for e in entities.iter() {
        let Some(kind) = e["type"].as_str() else {
            continue;
        };
        let fields = e["fields"].as_object().unwrap();
        if let Some(Value::Object(values)) = common.get_mut(kind) {
            values.retain(|key, value| fields.get(key) == Some(value));
        } else {
            let values = fields
                .iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.as_str(),
                        "position"
                            | "orientation"
                            | "entity_name"
                            | "mesh"
                            | "group_id"
                            | "theta_current"
                            | "theta_target"
                            | "open"
                            | "pressed"
                            | "entity_flags"
                            | "status_flags"
                            | "level_name"
                            | "enabled_t_target"
                            | "solved_t_target"
                    )
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            common.insert(kind.to_owned(), Value::Object(values));
        }
    }
    for e in entities {
        if let Some(values) = e["type"]
            .as_str()
            .and_then(|kind| common.get(kind))
            .and_then(Value::as_object)
        {
            e["fields"]
                .as_object_mut()
                .unwrap()
                .retain(|key, _| !values.contains_key(key));
        }
    }
    common
}

pub unsafe fn snapshot(level: &str, frames: u32) -> Value {
    let schema = init::json("meta/layout");
    let extra: Value = serde_json::from_reader(
        zstd::stream::Decoder::new(
            &include_bytes!(concat!(env!("OUT_DIR"), "/visual-types.zst"))[..],
        )
        .unwrap(),
    )
    .unwrap();
    let decoder = scene::Decoder::load();
    let types: HashMap<_, _> = schema
        .as_object()
        .unwrap()
        .iter()
        .chain(extra.as_object().unwrap())
        .map(|(name, spec)| (spec["address"].as_u64().unwrap() as usize, (name, spec)))
        .collect();
    let common = schema["Guy"]["fields"].as_object().unwrap();
    let table = get::<usize>(W + 0x378);
    let mut entities = (0..get::<usize>(W + 0x370))
        .map(|i| {
            let p = get::<usize>(table + 8 * i);
            let ty = get::<usize>(p + layout::TYPE);
            let (kind, mut fields) = if let Some(&(kind, spec)) = types.get(&ty) {
                let mut fields = reflected(p, spec["fields"].as_object().unwrap(), usize::MAX);
                if schema.get(kind).is_some() {
                    // Serialized descriptors supply typed booleans, signed values
                    // and arrays (including Ocean_Map and Wall_Set).
                    let typed = decoder.defaults(kind, p);
                    fields.retain(|key, _| {
                        !key.split_once('.')
                            .is_some_and(|(root, _)| typed.contains_key(root))
                    });
                    fields.extend(typed);
                }
                (Some(kind), fields)
            } else {
                (None, reflected(p, common, layout::UNDO_DATA_SIZE))
            };
            let mesh = get::<usize>(p + layout::MESH);
            if mesh != 0 {
                let [len, data] = get::<[usize; 2]>(mesh);
                fields.insert("mesh".into(), string(len, data));
            }
            let flags = get::<u32>(p + 0x28);
            let dead = get::<u16>(p + 0x5c) & 1 != 0;
            let exists = flags & (1 << 25) == 0 || flags & (1 << 26) != 0;
            let active = exists && !dead && flags & ((1 << 21) | (1 << 29)) == 0;
            json!({
                "id": get::<u32>(p + layout::UID), "type": kind,
                "position": get::<[f32; 3]>(p),
                "grid_index": get::<i32>(p + layout::GRID_INDEX),
                "grid_z": get::<i32>(p + layout::GRID_Z),
                "active": active, "dead": dead, "exists": exists, "fields": fields
            })
        })
        .collect::<Vec<_>>();
    entities.sort_by_key(|entity| entity["id"].as_u64().unwrap());
    let entity_defaults = compact_fields(&mut entities);

    let families = get::<usize>(W + layout::FAMILY_IDS);
    let families = (0..get::<usize>(W + layout::FAMILY_COUNT))
        .map(|i| get::<u32>(families + i * 4))
        .collect::<Vec<_>>();
    let active_family = families.get(get::<usize>(W + layout::ACTIVE_FAMILY));
    let queue = W + layout::INPUT_QUEUE;
    let data = get::<usize>(queue + 8);
    let input_queue = (0..get::<usize>(queue))
        .map(|i| {
            let p = data + i * 28;
            json!({
                "family_id": get::<u32>(p), "action": get::<u8>(p + 4),
                "direction": get::<[f32; 2]>(p + 8)
            })
        })
        .collect::<Vec<_>>();
    let moves = W + layout::MOVE_TRANSACTIONS;
    let data = get::<usize>(moves + 8);
    let moves = (0..get::<usize>(moves))
        .map(|i| {
            let p = get::<usize>(data + i * 8);
            let single = get::<usize>(p + 48);
            let entities = (0..get::<usize>(p + 40))
                .map(|j| get::<u32>(single + j * 208))
                .collect::<Vec<_>>();
            json!({"flags": get::<u32>(p + layout::MOVE_TRANSACTION_FLAGS), "entity_ids": entities})
        })
        .collect::<Vec<_>>();
    json!({
        "version": 1, "level": level, "frames": frames, "view": view(),
        "game_seconds": get::<f64>(W + 0x67d8),
        "won": won(), "finished": finished(), "target_level": target_level(),
        "families": families, "active_family": active_family,
        "run_held": get::<u8>(layout::RUN_KEY) != 0,
        "slow_held": get::<u8>(layout::SLOW_KEY) != 0,
        "input_queue": input_queue, "moves": moves,
        "entity_defaults": entity_defaults, "entities": entities
    })
}
