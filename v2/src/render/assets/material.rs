use super::{format::*, *};
#[derive(Clone, Default)]
struct Def {
    shader: String,
    parent: String,
    flags: u32,
    props: BTreeMap<String, String>,
}
pub struct Materials {
    pub always_resident: BTreeSet<String>,
    pub textures: BTreeMap<String, BTreeSet<String>>,
}
fn meta(shader: &[u8]) -> Res<(BTreeMap<String, usize>, BTreeSet<String>)> {
    let mut r = R(shader);
    r.take(57)?;
    let mut kinds = BTreeMap::new();
    for _ in 0..r.h()? {
        let hdr = r.take(64)?;
        let kind = R(&hdr[16..]).q()?;
        r.short()?;
        let name = r.short()?;
        kinds.insert(name, kind);
        for _ in 0..5 {
            r.short()?;
        }
    }
    let n = r.byte()?;
    r.take(5)?;
    let mut def = BTreeSet::new();
    for _ in 0..n {
        let t = r.short()?;
        if !t.is_empty() {
            def.insert(t);
        }
    }
    Ok((kinds, def))
}
pub fn build(game: &Path, folder: &Path) -> Res<Materials> {
    let shaders = Package::new(game.join("data/shaders.package"))?;
    let mut toc = String::new();
    for (k, (off, len)) in &shaders.entries {
        if k.ends_with(".shader") {
            use std::fmt::Write;
            writeln!(toc, "{k}\t{off}\t{len}")?
        }
    }
    fs::write(folder.join("shaders.tsv"), toc)?;
    let pkg = Package::new(game.join("data/materials.package"))?;
    let bits: J = serde_json::from_str(include_str!("../../../gpu/material-flags.json"))?;
    let mut defs = BTreeMap::new();
    for k in pkg.entries.keys().filter(|k| k.ends_with(".material")) {
        let mut d = Def {
            shader: "standard".into(),
            ..Default::default()
        };
        let name = Path::new(k).file_stem().unwrap().to_str().unwrap();
        for l in String::from_utf8_lossy(&pkg.get(k)?).lines() {
            let l = l.split('#').next().unwrap().trim();
            let Some((key, val)) = l.split_once(char::is_whitespace) else {
                continue;
            };
            let v = val.trim().trim_matches('"');
            match key {
                "parent" => d.parent = v.into(),
                "shader" => d.shader = v.into(),
                "flag" => d.flags |= num(&bits[v]) as u32,
                "outline_depth_modifier" | "mask" => {}
                _ => {
                    let val = if v.contains('\u{fffd}') && v.ends_with("~promesst2.5_deeper") {
                        if key == "tint_map_A" {
                            "Middle_FloorTint_B~promesst2.5_deeper"
                        } else {
                            "FloorTint_A~promesst2.5_deeper"
                        }
                    } else {
                        v
                    };
                    d.props.insert(key.into(), val.into());
                }
            }
        }
        defs.insert(name.to_owned(), d);
    }
    let mut cache = BTreeMap::new();
    let mut b = vec![];
    word(&mut b, 0x3254414d); // MAT2: length-delimited records for demand loading.
    word(&mut b, defs.len());
    let mut textures = BTreeMap::new();
    let mut always_resident = BTreeSet::new();
    for (name, own) in &defs {
        let d = if shaders
            .entries
            .contains_key(&format!("data-pc/{}--100352.shader", own.shader))
        {
            own
        } else {
            defs.get("default").ok_or("missing default material")?
        };
        if !cache.contains_key(&d.shader) {
            let bytes = shaders.get(&format!("data-pc/{}--100352.shader", d.shader))?;
            cache.insert(d.shader.clone(), meta(&decode(&bytes)?)?);
        }
        let (params, defaults) = &cache[&d.shader];
        let mut tex = defaults.clone();
        let mut props = BTreeMap::new();
        let mut parent = d;
        let mut seen = BTreeSet::new();
        loop {
            for (k, v) in &parent.props {
                props.entry(k).or_insert(v);
            }
            if parent.parent.is_empty() {
                break;
            }
            if !seen.insert(&parent.parent) {
                return Err("material inheritance cycle".into());
            }
            parent = defs.get(&parent.parent).ok_or("missing parent")?;
        }
        blob(&mut b, name.as_bytes());
        let size_offset = b.len();
        word(&mut b, 0);
        blob(&mut b, d.shader.as_bytes());
        word(&mut b, d.flags as usize);
        let mut records = vec![];
        let mut count = 0;
        for (k, v) in props {
            let Some(&kind) = params.get(k) else { continue };
            if !(1..16).contains(&kind) {
                continue;
            }
            let mut bytes = [0u8; 16];
            let mut text = "";
            match kind {
                1 => bytes[0] = u8::from(v == "true"),
                2 => bytes[..4].copy_from_slice(&(v.parse::<f64>()? as i32).to_le_bytes()),
                3 => bytes[..4].copy_from_slice(&v.parse::<f32>()?.to_le_bytes()),
                4..=6 => {
                    for (i, n) in v
                        .trim_matches(['(', ')'])
                        .split_whitespace()
                        .take(4)
                        .enumerate()
                    {
                        bytes[i * 4..i * 4 + 4].copy_from_slice(&n.parse::<f32>()?.to_le_bytes());
                    }
                }
                _ => {
                    text = v;
                    if !v.is_empty() {
                        tex.insert(v.clone());
                    }
                }
            }
            blob(&mut records, k.as_bytes());
            word(&mut records, kind);
            blob(&mut records, &bytes);
            blob(&mut records, text.as_bytes());
            count += 1;
        }
        word(&mut b, count);
        b.extend(records);
        let size = u32::try_from(b.len() - size_offset - 4)?;
        b[size_offset..size_offset + 4].copy_from_slice(&size.to_le_bytes());
        // Only these mesh shaders bind their texture overrides through the
        // native resource transition. Decals and other special passes also
        // sample descriptor indices directly. Any such use wins over deferral.
        if matches!(
            d.shader.as_str(),
            "basic"
                | "basic_masked"
                | "rocks_general"
                | "blended2"
                | "blended2_dissolve"
                | "ground_general"
                | "ground_general_dissolve"
                | "foliage"
                | "foliage_masked"
        ) {
            always_resident.extend(defaults.iter().cloned());
        } else {
            always_resident.extend(tex.iter().cloned());
        }
        textures.insert(name.clone(), tex);
    }
    fs::write(folder.join("materials.bin"), b)?;
    Ok(Materials {
        textures,
        always_resident,
    })
}
