//! Read the game's final collision grid and water masks without advancing it.
use super::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

// r68307: the collision query reads these Entity_Manager fields. Each mask
// header is [word count, data pointer, bit count] with x changing fastest.
const TERRAIN_BOUNDS: usize = W + 0x3e80;
const SOLID_MASK: usize = W + 0x3e98;
const ENABLED_COLUMN_MASK: usize = W + 0x3ec0;
const OCEAN_MAPS: usize = W + 0xb78;

unsafe fn bit_reader(header: usize) -> impl Fn(usize) -> bool {
    let [words, data, bits] = get::<[usize; 3]>(header);
    move |i| {
        data != 0
            && i < bits
            && i / 64 < words
            && unsafe { get::<u64>(data + 8 * (i / 64)) } & (1 << (i % 64)) != 0
    }
}

fn x_runs(
    width: usize,
    mut set: impl FnMut(usize) -> bool,
) -> impl Iterator<Item = (usize, usize)> {
    let mut x = 0;
    std::iter::from_fn(move || {
        while x < width && !set(x) {
            x += 1;
        }
        if x == width {
            return None;
        }
        let start = x;
        while x < width && set(x) {
            x += 1;
        }
        Some((start, x - 1))
    })
}

fn xy_transform(
    position: [f32; 3],
    [x, y, z, w]: [f32; 4],
) -> Option<impl Fn(i32, i32) -> [i32; 2]> {
    if (x * x + y * y + z * z + w * w - 1.).abs() > 0.002 {
        return None;
    }
    let integer =
        |a: f32| (a.is_finite() && (a - a.round()).abs() < 0.001).then(|| a.round() as i32);
    let px = integer(position[0])?;
    let py = integer(position[1])?;
    let c = integer(1. - 2. * z * z)?;
    let s = integer(2. * z * w)?;
    if x.abs() > 0.001 || y.abs() > 0.001 || c * c + s * s != 1 {
        return None;
    }
    Some(move |x, y| [px + c * x - s * y, py + s * x + c * y])
}

unsafe fn ocean_map(p: usize) -> Value {
    // Ocean_Map.grid and kind, read by the r68307 water lookup.
    let [width, height] = get::<[usize; 2]>(p + 0x330);
    let [ox, oy, _] = get::<[i32; 3]>(p + 0x34c);
    let [count, data] = get::<[usize; 2]>(p + layout::OCEAN_VALUES);
    let position = get::<[f32; 3]>(p);
    let orientation = get::<[f32; 4]>(p + 0x18);
    let flags = get::<u32>(p + 0x28);
    let active =
        flags & ((1 << 21) | (1 << 29)) == 0 && (flags & (1 << 25) == 0 || flags & (1 << 26) != 0);
    let kind = get::<u8>(p + 0x358);
    let mut local = Vec::new();
    if data != 0 && width != 0 && width.checked_mul(height) == Some(count) {
        for y in 0..height {
            for (a, b) in x_runs(width, |x| get::<u8>(data + y * width + x) & 1 != 0) {
                local.push([oy + y as i32, ox + a as i32, ox + b as i32]);
            }
        }
    }
    let world = xy_transform(position, orientation).map(|transform| {
        let mut cells = BTreeSet::new();
        for &[y, a, b] in &local {
            let [xa, ya] = transform(a, y);
            let [xb, yb] = transform(b, y);
            if ya == yb {
                cells.extend((xa.min(xb)..=xa.max(xb)).map(|x| (ya, x)));
            } else {
                cells.extend((ya.min(yb)..=ya.max(yb)).map(|y| (y, xa)));
            }
        }
        let mut runs: Vec<[i32; 3]> = Vec::new();
        for (y, x) in cells {
            if let Some(last) = runs.last_mut()
                && last[0] == y
                && last[2] + 1 == x
            {
                last[2] = x;
            } else {
                runs.push([y, x, x]);
            }
        }
        runs
    });
    json!({
        "id": get::<u32>(p + layout::UID), "active": active, "kind": kind,
        "position": position, "orientation": orientation, "origin_xy": [ox, oy],
        "size_xy": [width, height], "local_runs": local, "world_runs": world,
    })
}

pub unsafe fn snapshot() -> Value {
    let [ox, oy, oz, width, height, depth] = get::<[i32; 6]>(TERRAIN_BOUNDS);
    let solid = bit_reader(SOLID_MASK);
    let enabled = bit_reader(ENABLED_COLUMN_MASK);
    let (nx, ny, nz) = (
        width.max(0) as usize,
        height.max(0) as usize,
        depth.max(0) as usize,
    );
    let mut solid_runs = Vec::new();
    for z in 0..nz {
        for y in 0..ny {
            let row = (z * ny + y) * nx;
            for (a, b) in x_runs(nx, |x| solid(row + x)) {
                solid_runs.push([oz + z as i32, oy + y as i32, ox + a as i32, ox + b as i32]);
            }
        }
    }
    let mut enabled_column_runs = Vec::new();
    for y in 0..ny {
        for (a, b) in x_runs(nx, |x| enabled(y * nx + x)) {
            enabled_column_runs.push([oy + y as i32, ox + a as i32, ox + b as i32]);
        }
    }
    let [count, data] = get::<[usize; 2]>(OCEAN_MAPS);
    let mut oceans = (0..if data == 0 { 0 } else { count })
        .filter_map(|i| {
            let p = get::<usize>(data + 8 * i);
            (p != 0).then(|| ocean_map(p))
        })
        .collect::<Vec<_>>();
    oceans.sort_by_key(|map| map["id"].as_u64());
    json!({
        "origin": [ox, oy, oz], "size": [width, height, depth],
        "solid_runs": solid_runs, "enabled_column_runs": enabled_column_runs,
        "ocean_maps": oceans,
        "input_directions": {
            "R": movement_direction(0), "U": movement_direction(1),
            "L": movement_direction(2), "D": movement_direction(3)
        }
    })
}
