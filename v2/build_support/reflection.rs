// r68307 Jai Type_Info layouts. Schema information is recovered from .rdata,
// not from a separately exported schema or a constructor-memory snapshot.
use serde_json::{Map, Value as J, json};
use std::collections::{BTreeMap, BTreeSet};
struct R<'a>(&'a [u8], usize);
impl R<'_> {
    fn q(&self, a: usize) -> usize {
        usize::from_le_bytes(self.0[a..a + 8].try_into().unwrap())
    }
    fn ptr(&self, a: usize) -> usize {
        self.q(a) - self.1
    }
    fn opt_string(&self, a: usize) -> Option<&str> {
        let n = self.q(a);
        if !(1..=256).contains(&n) {
            return None;
        }
        let p = self.q(a + 8).checked_sub(self.1)?;
        let s = std::str::from_utf8(self.0.get(p..p.checked_add(n)?)?).ok()?;
        s.chars().all(|c| !c.is_control()).then_some(s)
    }
    fn name(&self, a: usize) -> &str {
        self.opt_string(a + 16).unwrap_or("")
    }
    fn tag(&self, a: usize) -> usize {
        self.q(a) & 0xffffffff
    }
    fn members(&self, a: usize) -> impl Iterator<Item = usize> {
        let n = self.q(a + 64);
        assert!(n <= 2048);
        let p = if n == 0 { 0 } else { self.ptr(a + 72) };
        (0..n).map(move |i| p + i * 64)
    }
    fn skip(&self, m: usize) -> bool {
        let n = self.q(m + 40);
        (0..n).any(|i| self.opt_string(self.ptr(m + 48) + i * 16) == Some("NoSerialize"))
    }
    fn flat(
        &self,
        a: usize,
        off: usize,
        prefix: &str,
        vis: &mut Vec<usize>,
        out: &mut Map<String, J>,
    ) {
        if vis.contains(&a) {
            return;
        }
        vis.push(a);
        for m in self.members(a) {
            let t = self.ptr(m + 16);
            let size = self.q(t + 8);
            let d = self.q(m + 24);
            if d == usize::MAX || size == 0 {
                continue;
            }
            let name = self.opt_string(m).unwrap();
            let k = format!("{prefix}{name}");
            let pos = off + d;
            let tag = self.tag(t);
            if tag == 7 {
                let next = if matches!(name, "entity" | "undoable_data") {
                    prefix.into()
                } else {
                    format!("{k}.")
                };
                self.flat(t, pos, &next, vis, out);
            } else if let Some(ty) = match (tag, size) {
                (4, 8) => Some("pointer"),
                (3, 16) => Some("string"),
                (1, 1 | 2 | 4 | 8) => Some("float"),
                (0 | 2 | 11 | 18, 1 | 2 | 4 | 8) => Some("int"),
                _ => None,
            } {
                out.insert(k, json!([pos, size, ty]));
            }
        }
        vis.pop();
    }
    fn desc(&self, a: usize, types: &mut Map<String, J>) {
        let key = (a + self.1).to_string();
        if types.contains_key(&key) {
            return;
        }
        let t = self.tag(a);
        let mut d = json!({"tag":t,"size":self.q(a+8),"addr":format!("{:#x}",self.1+a)});
        types.insert(key.clone(), J::Null);
        match t {
            0 => d["signed"] = json!(self.q(a + 16) != 0),
            11 | 18 => {
                let under = self.ptr(a + 32);
                d["underlying_addr"] = json!(under + self.1);
                self.desc(under, types);
            }
            8 => {
                let el = self.ptr(a + 16);
                d["element_addr"] = json!(self.1 + el);
                d["arrayKind"] = json!(self.q(a + 24));
                d["count"] = json!(self.q(a + 32));
                self.desc(el, types);
            }
            7 => {
                let name = self.name(a);
                d["name"] = json!(name);
                let mut fields = vec![];
                for m in self.members(a) {
                    let t = self.ptr(m + 16);
                    let offset = self.q(m + 24);
                    let k = self.opt_string(m).unwrap();
                    if offset == usize::MAX || self.q(t + 8) == 0 || self.skip(m) {
                        continue;
                    }
                    if matches!(
                        name,
                        "Vector2"
                            | "Vector3"
                            | "Vector4"
                            | "Quaternion"
                            | "Vector2i"
                            | "Vector3i"
                            | "Vector4i"
                    ) && !matches!(k, "x" | "y" | "z" | "w")
                    {
                        continue;
                    }
                    self.desc(t, types);
                    fields.push(
                        json!({"name":k,"typeaddr":format!("{:#x}",self.1+t),"offset":offset}),
                    );
                }
                d["members"] = json!(fields);
            }
            _ => {}
        }
        types.insert(key, d);
    }
    fn serial(
        &self,
        a: usize,
        prefix: &str,
        offset: usize,
        types: &mut Map<String, J>,
        out: &mut Vec<J>,
    ) {
        for m in self.members(a) {
            let k = self.opt_string(m).unwrap();
            let off = self.q(m + 24);
            let t = self.ptr(m + 16);
            if k == "entity" || off == usize::MAX || self.q(t + 8) == 0 || self.skip(m) {
                continue;
            }
            if matches!(
                self.name(a),
                "Vector2"
                    | "Vector3"
                    | "Vector4"
                    | "Quaternion"
                    | "Vector2i"
                    | "Vector3i"
                    | "Vector4i"
            ) && !matches!(k, "x" | "y" | "z" | "w")
            {
                continue;
            }
            let name = format!("{prefix}{k}");
            if self.tag(t) == 7 {
                if self.name(t) == "Allocator" {
                    continue;
                }
                self.serial(t, &format!("{name}."), offset + off, types, out);
            } else {
                self.desc(t, types);
                out.push(json!({"name":name,"offset":offset+off,"type":{"addr":format!("{:#x}",self.1+t)}}));
            }
        }
    }
}
fn index(r: &R, pe: &super::pe::Pe, wanted: impl Fn(&str) -> bool) -> BTreeMap<String, usize> {
    let mut named = BTreeMap::new();
    for a in (pe.rdata.start..pe.rdata.end - 128).step_by(8) {
        if r.tag(a) != 7
            || !(1..10000).contains(&(r.q(a) >> 32))
            || !(1..1024 * 1024).contains(&r.q(a + 8))
        {
            continue;
        }
        if let Some(s) = r.opt_string(a + 16) {
            if wanted(s) {
                named.insert(s.to_owned(), a);
            }
        }
    }
    named
}
pub fn mesh_fields(pe: &super::pe::Pe) -> [u16; 5] {
    let r = R(&pe.image, pe.base);
    let types = index(&r, pe, |s| matches!(s, "Entity" | "Group" | "Spawner"));
    let common = r.ptr(r.members(types["Entity"]).next().unwrap() + 16);
    let field = |ty, name, base| {
        let mut fields = vec![];
        r.serial(ty, "", 0, &mut Map::new(), &mut fields);
        let i = fields
            .iter()
            .position(|f| f["name"] == name)
            .unwrap_or_else(|| panic!("missing serialized field {name}"));
        u16::try_from(base + i).unwrap()
    };
    [
        field(common, "mesh", 0),
        field(common, "group_id", 0),
        field(types["Group"], "is_existence_group", 4096),
        field(types["Group"], "clone_existence", 4096),
        field(types["Spawner"], "spawned_entity_id", 4096),
    ]
}
pub fn derive(
    pe: &super::pe::Pe,
    kinds: &BTreeSet<String>,
    out: &std::path::Path,
    bindings: &BTreeMap<&str, usize>,
) -> (J, J, J) {
    let r = R(&pe.image, pe.base);
    let named = index(&r, pe, |s| {
        kinds.contains(s)
            || matches!(
                s,
                "Triangle_Mesh"
                    | "Entity"
                    | "Animation_Player"
                    | "Entity_Manager"
                    | "Undoable_Entity_Data"
                    | "Move_Transaction"
            )
    });
    let mut generated = format!(
        "pub const IMAGE_BASE:usize={};\npub const DATA_BASE:usize={};\n",
        pe.base,
        pe.base + pe.rdata.start
    );
    // Name the fields the host accesses directly. Validate the width of each
    // typed load/store; an equal name with an incompatible type is not enough.
    for &(constant, kind, path, width) in HOST_FIELDS {
        let mut ty = *named
            .get(kind)
            .unwrap_or_else(|| panic!("missing reflected type {kind}"));
        let mut offset = 0;
        if !path.is_empty() {
            for part in path.split('.') {
                let m = r
                    .members(ty)
                    .find(|&m| r.opt_string(m) == Some(part))
                    .unwrap_or_else(|| panic!("missing reflected member {kind}.{path} ({part})"));
                let at = r.q(m + 24);
                ty = r.ptr(m + 16);
                offset += at;
            }
            assert_eq!(r.q(ty + 8), width, "incompatible width: {kind}.{path}");
            assert!(
                offset + width <= r.q(named[kind] + 8),
                "out-of-bounds field: {kind}.{path}"
            );
        } else {
            offset = r.q(ty + 8);
        }
        generated.push_str(&format!("pub const {constant}:usize={offset};\n"));
    }
    for (name, _, kind, width, _) in super::anchors::offsets() {
        let offset = bindings[name];
        assert!(
            offset + width <= r.q(named[kind] + 8),
            "decoded {name} outside reflected {kind}"
        );
        if name != "POSE_BUFFER" {
            generated.push_str(&format!("pub const {name}:usize={offset};\n"));
        }
    }
    std::fs::write(out.join("layout.rs"), generated).unwrap();
    let base_fields = r.ptr(r.members(named["Entity"]).next().unwrap() + 16);
    let (mut schema, mut serial, mut types) = (Map::new(), Map::new(), Map::new());
    let mut common = vec![];
    r.serial(base_fields, "", 0, &mut types, &mut common);
    for k in kinds.iter().map(String::as_str).chain(["Triangle_Mesh"]) {
        let a = named[k];
        let mut f = Map::new();
        r.flat(a, 0, "", &mut vec![], &mut f);
        schema.insert(
            k.into(),
            json!({"address":pe.base+a,"size":r.q(a+8),"fields":f}),
        );
        if k == "Triangle_Mesh" {
            continue;
        }
        let mut ext = vec![];
        r.serial(a, "", 0, &mut types, &mut ext);
        let fields: Map<_, _> = common
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, f)| (i.to_string(), f))
            .chain(
                ext.into_iter()
                    .enumerate()
                    .map(|(i, f)| ((4096 + i).to_string(), f)),
            )
            .collect();
        serial.insert(k.into(), json!({"fields":fields}));
    }
    (J::Object(schema), J::Object(serial), J::Object(types))
}

const HOST_FIELDS: &[(&str, &str, &str, usize)] = &[
    ("ENTITY_SIZE", "Entity", "", 0),
    ("MANAGER_SIZE", "Entity_Manager", "", 0),
    ("PLAYER_SIZE", "Animation_Player", "", 0),
    ("UNDO_DATA_SIZE", "Undoable_Entity_Data", "", 0),
    ("MANAGER", "Entity", "entity_manager", 8),
    ("LOCATOR", "Entity", "locator", 8),
    ("UID", "Entity", "entity_id", 4),
    ("TYPE", "Entity", "entity_type", 8),
    ("GRID_INDEX", "Entity", "cached_proximity_grid_index", 4),
    ("GRID_Z", "Entity", "cached_proximity_grid_z_index", 4),
    ("PLAYER", "Entity", "animation_player", 8),
    ("GRAPH", "Entity", "animation_graph", 8),
    ("MESH", "Entity", "undoable_data.mesh", 8),
    ("PHASE_FLAGS", "Level_State", "misc_flags", 4),
    ("MOVE_TRANSACTION_FLAGS", "Move_Transaction", "flags", 4),
    ("WALL_SLOTS", "Wall_Set", "wall_data.bits.slots", 16),
    ("WALL_COUNT", "Wall_Set", "wall_data.bits.count", 8),
    ("OCEAN_VALUES", "Ocean_Map", "ocean_values", 16),
    ("GRID_X", "Settings", "grid.squares_x", 8),
    ("GRID_Y", "Settings", "grid.squares_y", 8),
    ("GRID_CORNER", "Settings", "grid.corner", 12),
    ("MESH_LOADED", "Triangle_Mesh", "resource.loaded", 1),
    ("MESH_DATA", "Triangle_Mesh", "data", 8),
];

pub fn visual_index(pe: &super::pe::Pe, kinds: &BTreeSet<String>, out: &std::path::Path) {
    let r = R(&pe.image, pe.base);
    let names = index(&r, pe, |s| kinds.contains(s));
    let map: BTreeMap<_, _> = names.into_iter().map(|(s, a)| (s, pe.base + a)).collect();
    std::fs::write(
        out.join("render-types.json"),
        serde_json::to_vec(&map).unwrap(),
    )
    .unwrap();
}
