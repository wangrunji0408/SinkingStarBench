use serde_json::Value as J;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Seek, SeekFrom},
};
use std::{fmt::Write, fs, path::Path};
fn take<'a>(r: &mut &'a [u8], n: usize) -> &'a [u8] {
    let (a, b) = r.split_at(n);
    *r = b;
    a
}
fn u32r(r: &mut &[u8]) -> usize {
    u32::from_le_bytes(take(r, 4).try_into().unwrap()) as usize
}
fn u64r(r: &mut &[u8]) -> usize {
    u64::from_le_bytes(take(r, 8).try_into().unwrap()) as usize
}
#[derive(Default)]
struct MeshNames {
    startup: BTreeSet<String>,
    respawn: BTreeSet<String>,
}
// Scalar fields contain zero defaults, the authored value, and a delimiter.
fn authored<const N: usize>(payload: &[u8], field: u16) -> Option<[u8; N]> {
    payload.windows(3 + N * 2).find_map(|b| {
        (b[..2] == field.to_le_bytes() && b[2..2 + N] == [0; N] && b[2 + N * 2] == 253)
            .then(|| b[2 + N..2 + N * 2].try_into().unwrap())
    })
}
// Copy retained payloads verbatim, preserving type indices, order and entity IDs.
fn filter(
    b: &[u8],
    fx: bool,
    visuals: &mut BTreeSet<String>,
    kinds: &mut BTreeSet<String>,
    meshes: &mut MeshNames,
    actors: &mut BTreeSet<String>,
    [mesh_field, group_field, existence_field, clone_field, spawned_field]: [u16; 5],
) -> Vec<u8> {
    let mut r = b;
    assert_eq!(take(&mut r, 4), b"enty");
    take(&mut r, 10);
    let mut keep = Vec::new();
    let mut types = Vec::new();
    for _ in 0..u64r(&mut r) {
        let n = u64r(&mut r);
        let k = std::str::from_utf8(take(&mut r, n)).unwrap();
        take(&mut r, 8);
        visuals.insert(k.to_string());
        types.push(k);
        keep.push(
            matches!(k, "Camera_Control" | "Camera_Marker")
                || super::scene_filter::gameplay(k)
                || (fx && k == "Global_Effects_Settings"),
        );
    }
    let at = b.len() - r.len();
    let mut out = b[..at].to_vec();
    out.extend([0; 4]);
    let mut count = 0u32;
    let mut clone_groups = BTreeSet::new();
    let mut spawned = BTreeSet::new();
    let mut deferred = Vec::new();
    for _ in 0..u32r(&mut r) {
        let rec = r;
        let id = u32r(&mut r) as u32;
        let kind = u16::from_le_bytes(take(&mut r, 2).try_into().unwrap()) as usize;
        let n = u32r(&mut r);
        take(&mut r, n);
        if keep[kind] {
            let kind = types[kind];
            let payload = &rec[10..10 + n];
            kinds.insert(kind.to_owned());
            if kind == "Group"
                && authored(payload, existence_field) == Some([1])
                && authored(payload, clone_field) == Some([1])
            {
                clone_groups.insert(id);
            }
            if kind == "Spawner" {
                if let Some(uid) = authored(payload, spawned_field) {
                    spawned.insert(u32::from_le_bytes(uid));
                }
            }
            let startup = matches!(
                kind,
                "Rock"
                    | "Mirror"
                    | "Tree"
                    | "Door"
                    | "Gem"
                    | "Fence"
                    | "Guy"
                    | "Monster"
                    | "Friendly_Dragon"
                    | "Baby_Dragon"
            );
            if kind != "Camera_Control" {
                // Serialized mesh field: default and actual Jai strings, then field delimiter.
                for (i, head) in payload.windows(10).enumerate() {
                    if head[..2] != mesh_field.to_le_bytes() || head[2..] != [0; 8] {
                        continue;
                    }
                    let Some(size) = payload.get(i + 10..i + 18) else {
                        continue;
                    };
                    let len = u64::from_le_bytes(size.try_into().unwrap());
                    if !(1..=256).contains(&len) {
                        continue;
                    }
                    let end = i + 18 + len as usize;
                    if payload.get(end) != Some(&253) {
                        continue;
                    }
                    if let Ok(name) = std::str::from_utf8(&payload[i + 18..end]) {
                        if name
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
                        {
                            if startup {
                                meshes.startup.insert(name.into());
                            } else {
                                let group = authored(payload, group_field).map(u32::from_le_bytes);
                                deferred.push((id, group, name));
                            }
                            if kind == "Guy" {
                                actors.insert(name.into());
                            }
                        }
                    }
                }
            }
            out.extend_from_slice(&rec[..10 + n]);

            count += 1;
        }
    }
    meshes.respawn.extend(
        deferred
            .into_iter()
            .filter(|(id, group, _)| {
                spawned.contains(id) || group.is_some_and(|id| clone_groups.contains(&id))
            })
            .map(|(_, _, name)| name.to_owned()),
    );
    assert!(r.is_empty());
    out[at..at + 4].copy_from_slice(&count.to_le_bytes());
    out
}
fn package(p: &Path, ext: &str, mut read: impl FnMut(&str, Vec<u8>)) {
    println!("cargo:rerun-if-changed={}", p.display());
    let mut f = fs::File::open(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let mut header = [0; 64];
    f.read_exact(&mut header).unwrap();
    assert_eq!(&header[..8], b"simp\x01\0\0\0");
    let toc = u64::from_le_bytes(header[16..24].try_into().unwrap());
    f.seek(SeekFrom::Start(toc)).unwrap();
    let mut bytes = vec![];
    f.read_to_end(&mut bytes).unwrap();
    let mut r = &bytes[..];
    let header = take(&mut r, 64);
    assert_eq!(&header[..4], b"toc!");
    let total = u64::from_le_bytes(header[8..16].try_into().unwrap());
    for _ in 0..total {
        let n = u32r(&mut r);
        let name = std::str::from_utf8(take(&mut r, n)).unwrap();
        assert_eq!(take(&mut r, 1), [0]);
        let len = u64r(&mut r);
        let start = u64r(&mut r);
        assert!(start >= 64 && start + len <= toc as usize);
        if let Some(key) = name.strip_suffix(ext) {
            let key = key.rsplit('/').next().unwrap();
            f.seek(SeekFrom::Start(start as u64)).unwrap();
            let mut data = vec![0; len];
            f.read_exact(&mut data).unwrap();
            read(key, data);
        }
    }
    assert!(r.is_empty());
}
pub fn build(game: &Path, out: &Path, pe: &super::pe::Pe, anchors: &BTreeMap<&str, usize>) -> J {
    let mut entries = BTreeMap::new();
    let mut visuals = BTreeSet::new();
    let mut kinds = BTreeSet::from(["Level_State".to_string()]);
    let mut meshes = MeshNames::default();
    let mesh_fields = super::reflection::mesh_fields(pe);
    let mut actors: BTreeSet<String> = super::scene_filter::CREATURE_GRAPHS
        .into_iter()
        .map(str::to_owned)
        .collect();
    package(&game.join("data/levels.package"), ".entities", |k, b| {
        assert!(entries
            .insert(
                format!("scene/{k}"),
                filter(
                    &b,
                    k == "ingame_particles",
                    &mut visuals,
                    &mut kinds,
                    &mut meshes,
                    &mut actors,
                    mesh_fields
                )
            )
            .is_none());
    });
    assert!(
        entries.contains_key("scene/ingame_particles"),
        "required effects scene missing"
    );
    let scenes = entries.len();
    package(
        &game.join("data/animation.package"),
        ".animation_graph",
        |k, b| {
            assert!(entries
                .insert(format!("graph/{k}.animation_graph"), b)
                .is_none());
        },
    );
    assert!(
        entries.len() > scenes,
        "animation package contains no graphs"
    );
    for n in &actors {
        assert!(
            entries.contains_key(&format!("graph/{n}.animation_graph")),
            "missing actor graph {n}"
        );
    }
    fs::write(
        out.join("actor-graphs.rs"),
        format!(
            "pub const ACTOR_GRAPHS:&[&str]=&{:?};",
            actors.into_iter().collect::<Vec<_>>()
        ),
    )
    .unwrap();
    let (schema, serial, types) = super::reflection::derive(pe, &kinds, out, anchors);
    super::reflection::visual_index(pe, &visuals, out);
    let mut movie_kinds = kinds.clone();
    for k in [
        "Flat_Decoration",
        "Temporary_Splash",
        "Particle_Source",
        "Turbulence",
    ] {
        movie_kinds.insert(k.to_owned());
    }
    let (visual, _, _) = super::reflection::derive(pe, &movie_kinds, out, anchors);
    let extra: serde_json::Map<_, _> = visual
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| !schema.as_object().unwrap().contains_key(*k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    fs::write(
        out.join("visual-types.zst"),
        zstd::bulk::compress(&serde_json::to_vec(&extra).unwrap(), 9).unwrap(),
    )
    .unwrap();
    let db = super::meshes::derive(
        game,
        pe,
        meshes.startup,
        meshes.respawn,
        &schema,
        anchors["MESH_RETAIN"],
    );
    for (key, j) in [
        ("meta/layout", &schema),
        ("meta/fields", &serial),
        ("meta/types", &types),
        ("meta/meshes", &db),
    ] {
        entries.insert(key.into(), serde_json::to_vec(j).unwrap());
    }
    let mut pack = Vec::new();
    let mut index = String::from("pub static INDEX:&[(&str,usize,usize,usize)]=&[\n");
    let mut z = zstd::bulk::Compressor::new(9).unwrap();
    for (k, b) in entries {
        let c = z.compress(&b).unwrap();
        writeln!(index, "({k:?},{},{},{}),", pack.len(), c.len(), b.len()).unwrap();
        pack.extend(c);
    }
    fs::write(out.join("assets.pack"), pack).unwrap();
    index += "];";
    fs::write(out.join("assets-index.rs"), index).unwrap();
    schema
}
