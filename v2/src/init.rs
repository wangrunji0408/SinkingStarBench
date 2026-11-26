use super::*;
use serde_json::Value as J;
use std::collections::{HashMap, HashSet};
const CTX: usize = 0x100000;
const TMP: usize = 0x130000;
const GRID: usize = 0x120000;
use layout::IMAGE_BASE as BASE;
const STACK: usize = 0x2e0008;
pub fn json(key: &str) -> J {
    serde_json::from_slice(&crate::assets::read(key)).unwrap()
}
pub unsafe fn alloc(n: usize) -> usize {
    let n = n.max(16).checked_add(15).unwrap() & !15;
    let p = *guest_cursor;
    assert!(n <= 0x40000000 - p);
    *guest_cursor += n;
    std::ptr::write_bytes(p as *mut u8, 0, n);
    p
}
pub unsafe fn copy(p: usize, b: &[u8]) {
    std::ptr::copy_nonoverlapping(b.as_ptr(), p as *mut u8, b.len())
}
unsafe fn string(p: usize, s: &str) {
    let dest = alloc(s.len());
    copy(dest, s.as_bytes());
    put(p, [s.len(), dest]);
}
// Drive one statically linked r68307 routine using the same guest stack as a frame.
unsafe fn call(pc: usize, args: &[usize], stack: &[usize]) {
    for (i, &v) in [2, 3, 7, 8].iter().zip(args) {
        reg(*i, v as u64)
    }
    for (i, &v) in stack.iter().enumerate() {
        put(STACK + 0x28 + i * 8, v)
    }
    original(pc);
}
fn u(j: &J) -> usize {
    j.as_u64().unwrap() as usize
}
fn num(j: &J) -> u64 {
    j.as_u64()
        .or_else(|| j.as_i64().map(|x| x as u64))
        .or_else(|| j.as_bool().map(u64::from))
        .unwrap()
}
unsafe fn scalar(p: usize, desc: &J, v: &J) {
    let a = p + u(&desc[0]);
    let n = u(&desc[1]);
    match desc[2].as_str().unwrap() {
        "string" => {
            if let Some(s) = v.as_str() {
                string(a, s)
            }
        }
        "int" => {
            let bits = num(v);
            copy(a, &bits.to_le_bytes()[..n]);
        }
        "float" => {
            let f = v.as_f64().unwrap();
            if n == 4 { put(a, f as f32) } else { put(a, f) }
        }
        "pointer" => {}
        t => panic!("unsupported scalar {t}"),
    }
}
unsafe fn ctor(spec: &J, p: usize) {
    let addr = u(&spec["address"]);
    let c = get::<usize>(addr + 0x78);
    if c != 0 {
        call(c - BASE, &[p], &[])
    }
}
struct Mesh<'a> {
    spec: &'a J,
    db: J,
    known: HashMap<String, usize>,
    respawn: Vec<&'a str>,
}
impl Mesh<'_> {
    unsafe fn load(&mut self, name: &str) -> usize {
        if let Some(&p) = self.known.get(name) {
            return p;
        }
        let p = alloc(u(&self.spec["size"]));
        ctor(self.spec, p);
        put(p + layout::MESH_DATA, p + 0x40);
        string(p, name);
        self.known.insert(name.into(), p);
        let h = if self.respawn.contains(&name) {
            &self.db["respawn_headers"][name]
        } else {
            &self.db["headers"][name]
        };
        if !h.is_null() {
            for (off, key) in [(0x40, "bounds"), (0x58, "lightmap_uv_extents")] {
                floats(p + off, &h[key]);
            }
            put(p + 0x60, u(&h["checksum"]) as u32);
            put(p + 0x64, u(&h["flags"]) as u32);
            put(p + u(&self.db["layout"]["valid"]), 1u8);
            put(p + layout::MESH_LOADED, 1u8);
            if let Some(nodes) = h["skeleton"].as_array() {
                if !nodes.is_empty() {
                    let a = alloc(nodes.len() * 80);
                    put(p + 0x68, [nodes.len(), a]);
                    for (i, n) in nodes.iter().enumerate() {
                        string(a + i * 80, n["name"].as_str().unwrap());
                        floats(a + i * 80 + 16, &n["matrix"]);
                    }
                }
            }
        }
        p
    }
}
unsafe fn floats(a: usize, v: &J) {
    for (i, f) in v.as_array().unwrap().iter().enumerate() {
        put(a + 4 * i, f.as_f64().unwrap() as f32)
    }
}
unsafe fn graphs() -> HashMap<String, usize> {
    let m = layout::GRAPH_CATALOG;
    for n in services::assets() {
        string(TMP, n.strip_suffix(".animation_graph").unwrap());
        string(TMP + 16, &n);
        put(TMP + 32, [0usize; 2]);
        call(
            layout::GRAPH_REGISTER,
            &[CTX, m, TMP, TMP + 16],
            &[TMP + 32, 0],
        );
    }
    let mut graphs = HashMap::new();
    for &n in layout::ACTOR_GRAPHS {
        string(TMP, n);
        call(layout::GRAPH_FIND, &[CTX, m, TMP, TMP + 64], &[TMP + 80]);
        let p = get::<usize>(TMP + 64);
        assert!(p != 0, "missing {n}");
        call(layout::GRAPH_LOAD, &[CTX, p], &[]);
        graphs.insert(n.into(), p);
    }
    graphs
}
pub type WorldLayout = (usize, Vec<(usize, usize)>, u8);
// Set up the original context, register types and graphs, then enter the chosen scene.
pub unsafe fn world(key: &str, capture_ranges: bool) -> WorldLayout {
    assert!(
        key.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
            && !key.is_empty()
    );
    let (mut source, fx, decoder, view) = scene::read(key);
    // Retail load (0x140437bc7..0x140437d0e) removes demo-phase signs.
    // The headless loader does not run that presentation/campaign setup pass.
    if key == "overworld" {
        source.retain(|e| !(e[1] == "Sign" && e[2]["demo_phase"].as_u64().unwrap_or(0) != 0));
    }
    let schema = json("meta/layout");
    let guy = &schema["Guy"];
    PLAYER_LAYOUT
        .set((u(&guy["address"]), u(&guy["fields"]["turn_order_id"][0])))
        .unwrap();

    for a in [CTX, layout::GAME_CONTEXT] {
        call(layout::CTX_INIT, &[a], &[]);
    }
    call(layout::WORLD_INIT, &[W], &[]);
    for a in [CTX, layout::GAME_CONTEXT] {
        for (off, v) in [
            (0x90, W),
            (0x10, layout::GAME_ALLOC),
            (0x50, 0x2fe000),
            (0x20, 0x2fe100),
        ] {
            put(a + off, v);
        }
    }
    put(W + 0x50, GRID);
    put(W + 0x3ef0, W);
    put(layout::CURRENT_WORLD, W);
    put(layout::WORLD_SCRATCH, alloc(0x2000));
    call(layout::REGISTER_DEFAULTS, &[], &[]);
    call(layout::INIT_CATALOGS, &[], &[]);
    let graphs = graphs();
    put(
        layout::MESH_ALLOCATOR,
        get::<[u8; 16]>(layout::GAME_CONTEXT + 0x10),
    );
    if !source.iter().any(|e| e[1] == "Level_State") {
        source.push(serde_json::json!([4294967294u64, "Level_State", {}]));
    }
    // Native entity cloning looks up the prototype's mesh by name.
    let respawn_ids: HashSet<_> = source
        .iter()
        .filter(|e| e[1] == "Spawner")
        .filter_map(|e| e[2]["spawned_entity_id"].as_u64())
        .filter(|&id| id != 0)
        .collect();
    let clone_groups: HashSet<_> = source
        .iter()
        .filter(|e| {
            e[1] == "Group" && e[2]["is_existence_group"] == true && e[2]["clone_existence"] == true
        })
        .filter_map(|e| e[0].as_u64())
        .collect();
    let mesh_db = json("meta/meshes");
    let respawn = source
        .iter()
        .filter(|e| {
            e[0].as_u64().is_some_and(|id| respawn_ids.contains(&id))
                || e[2]["group_id"]
                    .as_u64()
                    .is_some_and(|id| clone_groups.contains(&id))
        })
        .filter_map(|e| e[2]["mesh"].as_str())
        .filter(|name| !mesh_db["respawn_headers"][*name].is_null())
        .collect();
    let mut meshes = Mesh {
        spec: &schema["Triangle_Mesh"],
        db: mesh_db,
        known: HashMap::new(),
        respawn,
    };
    let mut ents = vec![];
    let mut types = HashMap::new();
    for e in &source {
        let id = u(&e[0]);
        let k = e[1].as_str().unwrap();
        let s = &schema[k];
        let addr = u(&s["address"]);
        call(layout::ENTITY_CREATE, &[CTX, W, addr, id], &[TMP]);
        let p = get::<usize>(TMP);
        let handle = get::<usize>(p + layout::LOCATOR);
        ctor(s, p);
        put(p + layout::LOCATOR, handle);
        let mut f = decoder.defaults(k, p);
        for (name, v) in e[2].as_object().unwrap() {
            if f.get(name) != Some(v) {
                f.insert(name.clone(), v.clone());
            }
        }
        for (name, v) in &f {
            if let Some(desc) = s["fields"].get(name) {
                scalar(p, desc, v);
            }
        }
        for (name, d) in s["fields"].as_object().unwrap() {
            if d[2] == "pointer" && name.ends_with("animation_state.entity") {
                put(p + u(&d[0]), p);
            }
        }
        put(p + layout::MANAGER, W);
        put(p + layout::TYPE, addr);
        put(p + layout::UID, id as u32);
        put(p + layout::GRID_INDEX, -1i32);
        put(p + layout::GRID_Z, -1i32);
        if k == "Wall_Set" {
            let words = f["wall_data.bits.slots"].as_array().unwrap();
            let a = alloc(words.len() * 8);
            for (i, n) in words.iter().enumerate() {
                let v = n
                    .as_str()
                    .map(|s| {
                        if let Some(hex) = s.strip_prefix("-0x") {
                            u64::from_str_radix(hex, 16).unwrap().wrapping_neg()
                        } else {
                            u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
                        }
                    })
                    .unwrap_or_else(|| num(n));
                put(a + i * 8, v);
            }
            put(p + layout::WALL_SLOTS, [words.len(), a]);
            put(p + layout::WALL_COUNT, u(&f["wall_data.bits.count"]));
        }
        if k == "Ocean_Map" {
            let v = f
                .get("ocean_values")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .map(|v| u(v) as u8)
                .collect::<Vec<_>>();
            let a = alloc(v.len());
            copy(a, &v);
            put(p + layout::OCEAN_VALUES, [v.len(), a]);
        }
        if let Some(n) = f
            .get("mesh")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
        {
            let m = meshes.load(n);
            call(layout::MESH_RETAIN, &[CTX, 0, m], &[]);
            put(p + layout::MESH, m);
        }
        if matches!(k, "Guy" | "Monster" | "Friendly_Dragon" | "Baby_Dragon") {
            let n = match k {
                "Guy" => f["mesh"].as_str().unwrap(),
                "Friendly_Dragon" => scene_filter::CREATURE_GRAPHS[0],
                "Baby_Dragon" => scene_filter::CREATURE_GRAPHS[1],
                _ => {
                    if num(&f["constant_flags"]) & 1 != 0 {
                        scene_filter::CREATURE_GRAPHS[2]
                    } else {
                        scene_filter::CREATURE_GRAPHS[3]
                    }
                }
            };
            put(p + layout::GRAPH, graphs[n]);
        }
        let player = alloc(layout::PLAYER_SIZE);
        put(player + 0x128, 1u8);
        put(player + 0x158, p);
        put(p + layout::PLAYER, player);
        ents.push((k, p));
    }
    let bindings = meshes.db["bindings"].as_object().unwrap().clone();
    for (a, b) in bindings {
        let m = meshes.load(b["name"].as_str().unwrap());
        call(layout::MESH_RETAIN, &[CTX, 0, m], &[]);
        put(
            usize::from_str_radix(a.trim_start_matches("0x"), 16).unwrap(),
            m,
        );
    }
    let mut names = meshes.db["headers"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    names.extend(meshes.respawn.iter().map(|s| (*s).to_owned()));
    names.sort();
    names.dedup();
    for n in &names {
        meshes.load(n);
    }
    let cap = (names.len() * 2).next_power_of_two().max(16);
    let table = alloc(cap * 32);
    put(layout::MESH_CATALOG, [names.len(), cap]);
    put(layout::MESH_HASH_TABLE, [cap, table]);
    for n in names {
        let mesh = meshes.known[&n];
        let mut h = n
            .bytes()
            .fold(0x1505u32, |h, b| (h ^ b as u32).wrapping_mul(0x1b3));
        if h < 2 {
            h |= 2
        }
        let mut slot = h as usize & (cap - 1);
        let mut step = 1;
        while get::<u32>(table + slot * 32) != 0 {
            slot = (slot + step) & (cap - 1);
            step += 1
        }
        let a = table + slot * 32;
        put(a, h);
        put(a + 8, [n.len(), get::<usize>(mesh + 8), mesh]);
    }
    let settings = ents.iter().find(|e| e.0 == "Settings").unwrap().1;
    string(W, key);
    call(layout::START_SCENE, &[], &[]);
    effects(&schema, &fx);
    for &(k, p) in &ents {
        call(layout::REGISTER_ENTITY, &[CTX, W, p], &[]);
        types.insert(k, 0usize);
    }
    for (k, idx) in &mut types {
        call(
            layout::TYPE_INDEX,
            &[CTX, u(&schema[*k]["address"]), TMP],
            &[],
        );
        *idx = get(TMP);
    }
    call(layout::CONFIGURE_LEVEL, &[CTX, settings, TMP], &[]);
    call(layout::GRID_INIT, &[CTX, W, 1], &[]);
    let nx = get::<usize>(settings + layout::GRID_X);
    let ny = get::<usize>(settings + layout::GRID_Y);
    put(GRID, [nx, ny, nx * ny]);
    put(GRID + 0x1c, get::<[u8; 12]>(settings + layout::GRID_CORNER));
    put(GRID + 0x30, alloc(nx * ny * 40));
    for &(_, p) in &ents {
        call(layout::GRID_ADD, &[CTX, GRID, p], &[]);
    }
    if types.contains_key("Gem_Slot") {
        call(layout::CONNECT_SLOTS, &[CTX, W], &[]);
    }
    for a in [
        layout::BUILD_GRIDS_A,
        layout::BUILD_GRIDS_B,
        layout::BUILD_GRIDS_C,
    ] {
        put(STACK + 0x58, CTX);
        put(STACK + 0xa0, W);
        reg(12, CTX as u64);
        reg(13, W as u64);
        call(a, &[], &[]);
    }
    if types.contains_key("Spawner") {
        init_spawner_prototypes(&schema, &ents);
    }
    for a in [
        layout::PREPARE,
        layout::WORLD_FINALIZE_A,
        layout::WORLD_FINALIZE_B,
    ] {
        call(a, &[CTX, W], &[]);
    }
    put(TMP, [0u8; 0x400]);
    call(layout::START_WORLD, &[CTX, W, TMP], &[]);
    call(layout::WORLD_REFRESH, &[CTX, W], &[]);
    // The shipping loader saves prototypes for cloneable existence groups.
    // Returning from a trial toggles these groups to restore the original props and actors.
    let group = &schema["Group"]["fields"];
    let is_existence_group = u(&group["is_existence_group"][0]);
    let clone_existence = u(&group["clone_existence"][0]);
    for &(kind, p) in &ents {
        if kind == "Group"
            && get::<u8>(p + is_existence_group) != 0
            && get::<u8>(p + clone_existence) != 0
        {
            call(layout::SNAPSHOT_EXISTENCE_GROUP, &[CTX, p], &[]);
        }
    }
    call(layout::LEVEL_STATE, &[CTX, W, TMP], &[]);
    let level = get::<usize>(TMP);
    let rover = types.contains_key("Rover");
    if rover {
        reg(6, CTX as u64);
        reg(14, W as u64);
        call(layout::ROVER_SETUP, &[], &[]);
    }
    if key == "overworld" {
        overworld::init(&ents, &schema);
    }
    let mut ranges = vec![];
    if capture_ranges {
        for (k, p) in ents {
            let end = get::<u32>(BASE + 0xb5dfa8 + 240 * types[k] + 0xec) as usize;
            ranges.extend([
                (p, layout::UNDO_DATA_SIZE),
                (
                    p + layout::ENTITY_SIZE,
                    end.saturating_sub(layout::ENTITY_SIZE),
                ),
            ]);
        }
    }
    (level, ranges, view)
}

// The original level loader gives each reset spawner a detached copy of its
// target as loaded. Copying the live entity at activation would preserve the
// very changes that a reset is supposed to undo.
unsafe fn init_spawner_prototypes(schema: &J, entities: &[(&str, usize)]) {
    let fields = &schema["Spawner"]["fields"];
    let target_uid = u(&fields["spawned_entity_id"][0]);
    let saved = u(&fields["prototype_entity_to_clone"][0]);
    for &(kind, spawner) in entities {
        if kind != "Spawner" {
            continue;
        }
        let uid = get::<u32>(spawner + target_uid);
        if uid == 0 {
            continue;
        }
        let Some(&(kind, source)) = entities
            .iter()
            .find(|&&(_, p)| get::<u32>(p + layout::UID) == uid)
        else {
            continue;
        };
        call(
            layout::ENTITY_CREATE,
            &[CTX, W, u(&schema[kind]["address"]), 0],
            &[TMP],
        );
        let prototype = get::<usize>(TMP);
        call(layout::SET_ENTITY_DEFAULTS, &[CTX, prototype], &[]);
        call(layout::COPY_ENTITY, &[CTX, source, prototype, 0], &[]);
        // The game's loader also initializes the derived render state on the
        // detached copy. These fields are not serialized in the level.
        for (dst, src, size) in [
            (0x1b0, 0x64, 32), // output colors from tint colors
            (0x1d0, 0xa0, 12), // output position from visual position
            (0x1dc, 0x18, 16), // output orientation
            (0x1ec, 0x0c, 12), // output scale
        ] {
            copy(
                prototype + dst,
                std::slice::from_raw_parts((prototype + src) as *const u8, size),
            );
        }
        put(spawner + saved, prototype);
    }
}

unsafe fn effects(schema: &J, e: &J) {
    let kind = "Global_Effects_Settings";
    let s = &schema[kind];
    // Keep the historical reservation so trace pointer values remain reproducible.
    let manager = alloc(layout::MANAGER_SIZE.max(0x9000));
    call(layout::WORLD_INIT, &[manager], &[]);
    call(
        layout::ENTITY_CREATE,
        &[CTX, manager, u(&s["address"]), u(&e[0])],
        &[TMP],
    );
    let p = get::<usize>(TMP);
    ctor(s, p);
    for (k, v) in e[2].as_object().unwrap() {
        if let Some(d) = s["fields"].get(k) {
            if d[2] == "int" || d[2] == "float" {
                scalar(p, d, v);
            }
        }
    }
    let arr = alloc(8);
    put(arr, p);
    put(manager + 0x8a8, [1, arr, 1]);
    put(layout::EFFECTS_WORLD, manager);
}
