#[link(name = "compression")]
unsafe extern "C" {
    fn compression_decode_buffer(
        dst: *mut u8,
        n: usize,
        src: *const u8,
        s: usize,
        scratch: *mut std::ffi::c_void,
        algorithm: u32,
    ) -> usize;
}
fn lz4(b: &[u8], size: usize) -> Vec<u8> {
    let mut d = vec![0; size];
    let n = unsafe {
        compression_decode_buffer(
            d.as_mut_ptr(),
            size,
            b.as_ptr(),
            b.len(),
            std::ptr::null_mut(),
            0x101,
        )
    };
    assert_eq!(n, size);
    d
}
use serde_json::{json, Map, Value as J};
use std::{collections::BTreeSet, fs, path::Path};
fn u32r(b: &[u8], a: usize) -> usize {
    u32::from_le_bytes(b[a..a + 4].try_into().unwrap()) as usize
}
fn rel(b: &[u8], d: usize) -> usize {
    (d as i64 + 4 + u32r(b, d) as i32 as i64) as usize
}
fn skeleton(b: &[u8]) -> Vec<J> {
    // A footer is a count followed by length-prefixed node names and 16-float matrices.
    // Accept only a complete, finite footer ending at EOF.
    for i in 56..b.len() - 6 {
        let n = u32r(b, i);
        let remaining = b.len() - i - 4;
        if !(1..=4096).contains(&n) || remaining < n * 67 || remaining > n * 322 {
            continue;
        }
        let mut p = i + 4;
        let mut nodes = vec![];
        for _ in 0..n {
            if p + 2 > b.len() {
                break;
            }
            let len = u16::from_le_bytes(b[p..p + 2].try_into().unwrap()) as usize;
            p += 2;
            if !(1..=256).contains(&len) || p + len + 64 > b.len() {
                break;
            }
            let Ok(name) = std::str::from_utf8(&b[p..p + len]) else {
                break;
            };
            if !name.chars().all(|c| !c.is_control()) {
                break;
            }
            p += len;
            let matrix = (0..16)
                .map(|j| f32::from_bits(u32r(b, p + j * 4) as u32) as f64)
                .collect::<Vec<_>>();
            if !matrix.iter().all(|x| x.is_finite()) {
                break;
            }
            p += 64;
            nodes.push(json!({"name":name,"matrix":matrix}));
        }
        if nodes.len() == n && p == b.len() {
            return nodes;
        }
    }
    panic!("invalid r68307 skeleton footer")
}
pub fn derive(
    game: &Path,
    pe: &super::pe::Pe,
    mut names: BTreeSet<String>,
    mut respawn: BTreeSet<String>,
    schema: &J,
    retain: usize,
) -> J {
    let image = &pe.image;
    let mut bindings = Map::new();
    let dir = game.join("data-pc");
    // r68307 startup block: mesh lookup followed by retain and RIP-relative global store.
    let mut lookup_target = None;
    for i in pe.text.start..pe.text.end - 12 {
        if image[i] != 0xe8
            || rel(image, i + 1) != retain
            || image[i + 5] != 0x48
            || image[i + 6] != 0x89
            || image[i + 7] & 0xc7 != 5
        {
            continue;
        }
        let slot = rel(image, i + 8);
        assert!(
            pe.data.contains(&slot),
            "mesh global outside .data at {i:#x}"
        );
        let start = pe
            .function(i)
            .expect("mesh retain outside an unwind function");
        let lookup = (start.max(i.saturating_sub(160))..i - 4)
            .rev()
            .find(|&j| {
                image[j] == 0xe8 && image[j + 5..j + 7] == [0x48, 0x8b] && image[j + 8] == 0x24
            })
            .expect("mesh lookup");
        let target = rel(image, lookup + 1);
        assert!(
            pe.text.contains(&target) && *lookup_target.get_or_insert(target) == target,
            "inconsistent mesh lookup at {lookup:#x}"
        );
        let arg = (start.max(lookup.saturating_sub(64))..lookup)
            .rev()
            .find(|&j| image[j..j + 4] == [0x48, 0x8d, 0x94, 0x24])
            .expect("mesh string argument");
        let stack_slot = u32r(image, arg + 4);
        let def = (start..arg)
            .rev()
            .find(|&j| {
                image[j..j + 4] == [0x48, 0xc7, 0x84, 0x24]
                    && u32r(image, j + 4) == stack_slot
                    && image[j + 12..j + 15] == [0x48, 0x8d, 0x05]
            })
            .expect("mesh string initializer");
        let len = u32r(image, def + 8);
        let addr = rel(image, def + 15);
        let name = std::str::from_utf8(&image[addr..addr + len]).unwrap();
        names.insert(name.into());
        bindings.insert(format!("{:#x}", pe.base + slot), json!({"name":name}));
    }
    assert!(!bindings.is_empty(), "no startup mesh bindings discovered");
    names.insert("GAME_EX_CamControl_A".into());
    respawn.retain(|name| !names.contains(name));
    let headers = read_headers(&dir, names);
    let respawn_headers = read_headers(&dir, respawn);
    let valid = &schema["Triangle_Mesh"]["fields"]["_data.valid"][0];
    assert!(valid.is_u64());
    json!({
        "layout": {"valid": valid},
        "bindings": bindings,
        "headers": headers,
        "respawn_headers": respawn_headers,
    })
}

fn read_headers(dir: &Path, names: BTreeSet<String>) -> Map<String, J> {
    let mut headers = Map::new();
    for n in names {
        let path = dir.join(format!("{n}.compressed_mesh"));
        println!("cargo:rerun-if-changed={}", path.display());
        let b = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let codec = u16::from_le_bytes(b[4..6].try_into().unwrap());
        let h = u16::from_le_bytes(b[6..8].try_into().unwrap()) as usize;
        assert_eq!(h, 12);
        let len = u32r(&b, 8) - h;
        let d = match codec {
            0 => b[h..].to_vec(),
            3 => lz4(&b[h..], len),
            _ => panic!("mesh codec"),
        };
        assert_eq!(d.len(), len);
        assert_eq!(u32r(&d, 0), 3);
        let floats = |p: usize, n: usize| {
            (0..n)
                .map(|i| f32::from_bits(u32r(&d, p + 4 * i) as u32) as f64)
                .collect::<Vec<_>>()
        };
        headers.insert(n,json!({"bounds":floats(16,6),"lightmap_uv_extents":floats(40,2),"checksum":u32r(&d,8),"flags":u32r(&d,48),"skeleton":if d[14]!=0{skeleton(&d)}else{vec![]}}));
    }
    headers
}
