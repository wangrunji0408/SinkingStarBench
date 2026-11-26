use super::{format::*, *};
type M = [f32; 16];
pub struct Anim {
    pub frames: usize,
    pub fps: usize,
    pub parents: Vec<i32>,
    pub order: Vec<usize>,
    pub inv: Vec<M>,
    pub samples: Vec<M>,
}
// Historical blend limits for these graphs; independent of root motion.
fn short_blends(m: &str) -> bool {
    matches!(
        m.trim_end_matches("_alt"),
        "wizard" | "thief" | "castaway" | "moonman"
    )
}
pub fn alias(m: &str) -> &str {
    match m {
        "soko1_dragon" => "sokodragon",
        "soko1_goblin" => "goblin",
        "CHAR_Dragon_Adult_A" => "promAdultDragon",
        "CHAR_Dragon_Baby_A" => "promBabyDragon",
        _ => m,
    }
}
pub fn rig(pkg: &Package, m: &str) -> Res<J> {
    let mut choices = json!({});
    for a in [m, alias(m), m.trim_end_matches("_alt")] {
        if let Some(k) = pkg.by_filename(&format!("{a}.animation_names")) {
            for line in String::from_utf8_lossy(&pkg.get(k)?).lines() {
                let line = line.split('#').next().unwrap().trim();
                let Some((tag, rest)) = line.split_once(char::is_whitespace) else {
                    continue;
                };
                let rest = rest.trim();
                let mut opts = vec![];
                if rest.starts_with('[') {
                    let clean = rest.replace(['[', ']', ',', '='], " ");
                    let items: Vec<_> = clean.split_whitespace().collect();
                    for pair in items.chunks_exact(2) {
                        if let Ok(n) = pair[1].parse::<f64>() {
                            if n > 0. && pkg.by_filename(&format!("{}.anim", pair[0])).is_some() {
                                opts.push(json!([pair[0], n]));
                            }
                        }
                    }
                } else {
                    let c = rest.split_whitespace().next().unwrap();
                    if pkg.by_filename(&format!("{c}.anim")).is_some() {
                        opts.push(json!([c, 1]));
                    }
                }
                if !opts.is_empty() && choices.get(tag).is_none() {
                    choices[tag] = opts.into();
                }
            }
            break;
        }
    }
    fn blends(pkg: &Package, m: &str, seen: &mut BTreeSet<String>, out: &mut J) -> Res<()> {
        if !seen.insert(m.into()) {
            return Ok(());
        }
        if let Some(k) = pkg.by_filename(&format!("{m}.animation_graph")) {
            let txt = pkg.get(k)?;
            let mut node = "";
            for line in std::str::from_utf8(&txt)?.lines() {
                let line = line.split('#').next().unwrap().trim();
                if let Some(n) = line.strip_prefix(':') {
                    node = n.trim()
                }
                let t: Vec<_> = line.split_whitespace().collect();
                if t.first() == Some(&"based_on") && t.len() > 1 {
                    blends(pkg, t[1], seen, out)?
                }
                let target = match t.first().copied() {
                    Some("go" | "when_done") => t.get(1),
                    Some("message") => t.get(2),
                    _ => None,
                };
                if let (Some(dest), Some(i)) = (target, t.iter().position(|x| *x == "blend")) {
                    if let Some(val) = t.get(i + 1).and_then(|v| v.parse::<f64>().ok()) {
                        out[format!("{node}>{}", dest.trim_start_matches('*'))] = val.into();
                    }
                }
            }
        }
        Ok(())
    }
    let mut b = json!({});
    blends(pkg, m, &mut BTreeSet::new(), &mut b)?;
    Ok(json!({"choices":choices,"blends":b,"shortBlends":short_blends(m)}))
}
pub fn read(pkg: &Package, game: &Path, m: &str, clip: &str) -> Res<Anim> {
    let name = format!("{clip}.anim");
    let raw = pkg.get(pkg.by_filename(&name).ok_or("missing anim")?)?;
    let (mut anim, names) = decode_tracks(&raw)?;
    let (_, bones) = mesh_info(&asset(game, m, "compressed_mesh")?)?;
    for (name, mat) in bones {
        anim.order.push(
            names
                .iter()
                .position(|s| s == &name)
                .ok_or("joint missing from mesh")?,
        );
        anim.inv.push(mat);
    }
    Ok(anim)
}
const TRAJECTORY: &str = "rig_TrajectorySHJnt";
fn decode_tracks(raw: &[u8]) -> Res<(Anim, Vec<String>)> {
    let (mut anim, names) = authored_tracks(raw)?;
    if let Some(root) = names.iter().position(|name| name == TRAJECTORY) {
        if anim.parents[root] != -1 {
            return Err("trajectory joint must be a skeleton root".into());
        }
        // Replay positions own horizontal travel. Remove even constant export
        // offsets before blending; retain height and the entire body pose.
        for frame in anim.samples.chunks_exact_mut(names.len()) {
            frame[root][..2].fill(0.);
        }
    }
    Ok((anim, names))
}

fn authored_tracks(raw: &[u8]) -> Res<(Anim, Vec<String>)> {
    let mut r = R(raw);
    if r.take(4)? != b"Anim" || r.h()? != 12 {
        return Err("unsupported animation".into());
    }
    let fps = r.w()?;
    let n = r.w()?;
    let frames = r.w()?;
    let initial = r.w()?;
    let stride = r.w()?;
    if n == 0 || n > 4096 || frames == 0 || frames.checked_mul(n).is_none_or(|v| v > 16_000_000) {
        return Err("invalid animation dimensions".into());
    }
    r.take(48)?;
    let mut parents = vec![];
    for i in 0..n {
        let parent = r.h()? as i16 as i32;
        if parent < -1 || parent >= i as i32 {
            return Err("bad bone parent".into());
        }
        parents.push(parent);
    }
    r.take((4 - (raw.len() - r.0.len()) % 4) % 4)?;
    let mut desc = vec![];
    for _ in 0..n {
        let flags = r.w()?;
        let first = r.w()?;
        let dynoff = r.w()?;
        let mut bounds = [0.; 8];
        for f in &mut bounds {
            *f = r.f()?
        }
        desc.push((flags, first, dynoff, bounds));
    }
    let mut names = vec![];
    for _ in 0..n {
        let len = r.byte()? as usize;
        names.push(r.str(len)?);
        if r.byte()? != 0 {
            return Err("invalid joint name".into());
        }
    }
    r.take((4 - (raw.len() - r.0.len()) % 4) % 4)?;
    if r.0.len() != initial + (frames - 1) * stride {
        return Err("bad animation stride".into());
    }
    let mut samples = vec![[0.; 16]; frames * n];
    for f in 0..frames {
        for (i, &(flags, start, off, b)) in desc.iter().enumerate() {
            let pos = if f == 0 {
                start
            } else {
                initial + (f - 1) * stride + off
            };
            let mut v = R(r.0.get(pos..).ok_or("invalid animation track offset")?);
            let mut o = if f > 0 {
                samples[i]
            } else {
                let mut a = [0.; 16];
                for j in [6, 7, 11, 15] {
                    a[j] = 1.;
                }
                a
            };
            for j in 0..3 {
                o[j] = b[3 + j]
                    + if flags & 1 == 0 {
                        (b[j] - b[3 + j]) * v.h()? as f32 / 65535.
                    } else {
                        0.
                    }
            }
            if flags & 16 == 0 && (f == 0 || flags & 2 == 0) {
                for j in 3..7 {
                    o[j] = v.h()? as i16 as f32 / 32767.
                }
                let norm = o[3..7].iter().map(|q| q * q).sum::<f32>().sqrt();
                for j in 3..7 {
                    o[j] /= norm
                }
            }
            if flags & 32 == 0 && (f == 0 || flags & 4 == 0) {
                for j in 0..9 {
                    o[7 + j % 3 * 3 + j / 3] = b[7] + (b[6] - b[7]) * v.h()? as f32 / 65535.;
                }
            }
            samples[f * n + i] = o;
        }
    }
    Ok((
        Anim {
            frames,
            fps,
            parents,
            order: vec![],
            inv: vec![],
            samples,
        },
        names,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_replay_root_motion() -> Res<()> {
        let pkg = Package::new(crate::render::game()?.join("data/animation.package"))?;
        let mut clips = BTreeSet::new();
        for entry in pkg
            .entries
            .keys()
            .filter(|s| s.ends_with(".animation_names"))
        {
            let rig_name = Path::new(entry).file_stem().unwrap().to_str().unwrap();
            let mapping = rig(&pkg, rig_name)?;
            for options in mapping["choices"].as_object().unwrap().values() {
                for choice in options.as_array().unwrap() {
                    clips.insert(choice[0].as_str().unwrap().to_owned());
                }
            }
        }
        assert!(!clips.is_empty(), "no mapped animations found");
        let mut affected = 0;
        for clip in &clips {
            let bytes = pkg.get(pkg.by_filename(&format!("{clip}.anim")).unwrap())?;
            let (source, names) = authored_tracks(&bytes)?;
            let (replay, _) = decode_tracks(&bytes).map_err(|e| format!("{clip}: {e}"))?;
            let mut moved = false;
            for (i, (orig, sample)) in source.samples.iter().zip(&replay.samples).enumerate() {
                let joint = i % names.len();
                let mut expected = *orig;
                if names[joint] == TRAJECTORY {
                    moved |= orig[0].hypot(orig[1]) >= 1.; // centimetres in asset space
                    expected[..2].fill(0.);
                } else if source.parents[joint] == -1 {
                    assert!(
                        orig[0].hypot(orig[1]) < 1.,
                        "{clip}: review horizontal motion on unclassified root {}",
                        names[joint]
                    );
                }
                assert_eq!(
                    &expected,
                    sample,
                    "{clip}: frame {} joint {}",
                    i / names.len(),
                    names[joint]
                );
            }
            affected += usize::from(moved);
        }
        eprintln!(
            "Audited {} mapped clips; {affected} have at least 1 cm of horizontal trajectory offset.",
            clips.len()
        );
        Ok(())
    }
}
