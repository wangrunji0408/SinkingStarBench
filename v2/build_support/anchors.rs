// Match instruction context, masking operands which the game linker relocates.
// Code and data locations are RVAs. Private member locations are byte offsets.
use super::pe::Pe;
use std::collections::BTreeMap;
fn unique(pe: &Pe, pattern: &str) -> Result<usize, String> {
    let bytes: Vec<_> = pattern
        .split_whitespace()
        .map(|p| {
            if p == "?" {
                None
            } else {
                Some(u8::from_str_radix(p, 16).expect("invalid signature"))
            }
        })
        .collect();
    let mut it = pe.image[pe.text.clone()]
        .windows(bytes.len())
        .enumerate()
        .filter(|(_, w)| bytes.iter().zip(*w).all(|(b, x)| b.is_none_or(|v| v == *x)))
        .map(|(i, _)| pe.text.start + i);
    match (it.next(), it.next()) {
        (Some(a), None) => Ok(a),
        (None, _) => Err("no match".into()),
        (Some(a), Some(b)) => Err(format!("ambiguous at {a:#x} and {b:#x}")),
    }
}
pub fn routines() -> impl Iterator<Item = (&'static str, &'static str)> {
    include_str!("routines.txt")
        .lines()
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(|s| s.split_once(' ').expect("routine signature"))
}
pub fn ends() -> impl Iterator<Item = (&'static str, &'static str)> {
    include_str!("stage_ends.txt")
        .lines()
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(|s| s.split_once(' ').expect("routine signature"))
}
pub fn discover(pe: &Pe) -> (BTreeMap<&'static str, usize>, Vec<String>) {
    let mut found = BTreeMap::new();
    let mut errors = vec![];
    // name, instruction context, byte offset, interpretation of that operand
    for (name, pat, at, kind) in [
        (
            "NAMED_TRANSITION",
            "4c 89 cb 4d 89 c6 48 89 d6 48 89 cf c6 05 ? ? ? ? 03 48 c7 05",
            0,
            "site",
        ),
        (
            "PARTICLE_REQUEST",
            "4c 89 f1 45 31 c0 e8 ? ? ? ? 41 08 f4 49 8b 85 f0 00 00 00",
            7,
            "branch",
        ),
        (
            "GAME_DT",
            "0f 57 c0 f3 0f 11 05 ? ? ? ? f3 0f 5a c0 f2 0f 58 05",
            7,
            "data",
        ),
        (
            "FRAME_DT",
            "f3 0f 10 35 ? ? ? ? 48 8d 0d ? ? ? ? 4c 8d 84 24 10 01 00 00",
            4,
            "data",
        ),
        (
            "GAME_CONTEXT",
            "f3 0f 10 35 ? ? ? ? 48 8d 0d ? ? ? ? 4c 8d 84 24 10 01 00 00",
            11,
            "data",
        ),
        (
            "PHASE_MODE",
            "0f b6 05 ? ? ? ? ff c8 83 f8 05 0f 87 ? ? ? ? 48 8d 0d",
            3,
            "data",
        ),
        (
            "HISTORY_GATE",
            "41 f6 45 1c 40 0f 84 ? ? ? ? 0f 29 b4 24 ? ? ? ? 49 8b 4d 28",
            0,
            "site",
        ),
        (
            "HISTORY_REGULAR",
            "41 f6 45 1c 40 0f 84 ? ? ? ? 0f 29 b4 24 ? ? ? ? 49 8b 4d 28",
            7,
            "branch",
        ),
        (
            "POSE_CHECK",
            "48 85 c0 0f 84 ? ? ? ? 4c 8b 80 ? ? ? ? 4d 85 c0 0f 84 ? ? ? ? 41 ff c8 4c 8b 88",
            0,
            "site",
        ),
        (
            "POSE_MISSING",
            "48 85 c0 0f 84 ? ? ? ? 4c 8b 80 ? ? ? ? 4d 85 c0 0f 84 ? ? ? ? 41 ff c8 4c 8b 88",
            5,
            "branch",
        ),
    ] {
        let res = unique(pe, pat).and_then(|p| {
            let at = p + at;
            if kind == "site" {
                return Ok(at);
            }
            let disp = i32::from_le_bytes(pe.image[at..at + 4].try_into().unwrap());
            let dst = (at + 4)
                .checked_add_signed(disp as isize)
                .ok_or("relative operand overflows")?;
            let valid = if kind == "branch" {
                pe.text.contains(&dst)
            } else {
                pe.data.contains(&dst)
            };
            valid
                .then_some(dst)
                .ok_or_else(|| format!("target outside expected section: {dst:#x}"))
        });
        match res {
            Ok(a) => {
                found.insert(name, a);
            }
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    for (name, pattern) in routines().chain(ends()) {
        let result = if let Some(p) = pattern.strip_prefix("@function ") {
            unique(pe, p).and_then(|pc| pe.function(pc))
        } else {
            unique(pe, pattern)
        };
        match result {
            Ok(rva) => {
                found.insert(name, rva);
            }
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    for (name, d, end, pattern) in globals() {
        let result = unique(pe, pattern).and_then(|p| {
            let displacement = i32::from_le_bytes(pe.image[p + d..p + d + 4].try_into().unwrap());
            let rva = (p + end)
                .checked_add_signed(displacement as isize)
                .ok_or("relative address overflow")?;
            pe.data
                .contains(&rva)
                .then_some(rva)
                .ok_or_else(|| format!("target {rva:#x} is not writable game data"))
        });
        match result {
            Ok(rva) => {
                found.insert(name, rva);
            }
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    for (name, at, _, _, pattern) in offsets() {
        match unique(pe, pattern) {
            Ok(pc) => {
                found.insert(name, super::pe::u32at(&pe.image, pc + at));
            }
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    (found, errors)
}

pub fn globals() -> impl Iterator<Item = (&'static str, usize, usize, &'static str)> {
    include_str!("globals.txt")
        .lines()
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(|line| {
            let mut p = line.splitn(4, ' ');
            (
                p.next().unwrap(),
                p.next().unwrap().parse().unwrap(),
                p.next().unwrap().parse().unwrap(),
                p.next().unwrap(),
            )
        })
}

pub fn offsets() -> impl Iterator<Item = (&'static str, usize, &'static str, usize, &'static str)> {
    include_str!("offsets.txt")
        .lines()
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(|line| {
            let mut p = line.splitn(5, ' ');
            (
                p.next().unwrap(),
                p.next().unwrap().parse().unwrap(),
                p.next().unwrap(),
                p.next().unwrap().parse().unwrap(),
                p.next().unwrap(),
            )
        })
}
