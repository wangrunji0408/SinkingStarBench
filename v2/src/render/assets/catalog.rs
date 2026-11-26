use super::*;
use std::io::Read;
pub fn external(b: &mut Vec<u8>, p: &Path) {
    word(b, u32::MAX as usize);
    blob(b, p.as_os_str().as_encoded_bytes());
}
pub fn textures(
    root: &Path,
    dir: &Path,
    mut names: BTreeSet<String>,
    eager: Option<&BTreeSet<String>>,
) -> Res<()> {
    names.extend([
        "default_lightmap_array_texture".into(),
        "black_texture".into(),
    ]);
    let mut b = vec![];
    word(&mut b, 0x31545844);
    word(&mut b, names.len());
    for n in names {
        blob(&mut b, n.as_bytes());
        let special = n == "default_lightmap_array_texture" || n == "black_texture";
        let mut p = root.join("data-pc").join(format!("{n}.texture"));
        let (w, h, s, m, f, flags, block, stride) = if special {
            (
                1,
                1,
                if n == "black_texture" { 1 } else { 3 },
                1,
                28,
                if n == "black_texture" { 0 } else { 4 },
                1,
                4,
            )
        } else {
            if !p.is_file() {
                p = root.join("data-pc/black.texture")
            }
            let mut header = [0; 40];
            fs::File::open(&p)?.read_exact(&mut header)?;
            let mut r = format::R(&header[12..]);
            let (w, h, l, m, t, flags) = (r.h()?, r.h()?, r.h()?, r.h()?, r.h()?, r.h()?);
            let (f, b, s) = match t {
                1 => (61, 1, 1),
                2 => (49, 1, 2),
                4 => (28 + (flags & 1), 1, 4),
                5 => (if flags & 1 == 0 { 87 } else { 91 }, 1, 4),
                18 => (10, 1, 8),
                256 => (71 + (flags & 1), 4, 8),
                257 => (77 + (flags & 1), 4, 16),
                258 => (80, 4, 8),
                259 => (83, 4, 16),
                260 => (95, 4, 16),
                261 => (98 + (flags & 1), 4, 16),
                _ => return Err(format!("unknown texture format {t}: {n}").into()),
            };
            (
                w,
                h,
                l * if flags & 2 != 0 { 6 } else { 1 },
                m,
                f,
                flags,
                b,
                s,
            )
        };
        for v in [f, w, h, s, m, flags, s * m] {
            word(&mut b, v)
        }
        if special {
            let mut bytes = vec![0; if s == 1 { 256 } else { 1536 }];
            for i in 0..s {
                bytes[i * 512 + 3] = 255
            }
            blob(&mut b, &bytes)
        } else {
            if eager.is_some_and(|direct| !direct.contains(&n)) {
                word(&mut b, (u32::MAX - 1) as usize);
                blob(&mut b, p.as_os_str().as_encoded_bytes());
            } else {
                external(&mut b, &p);
            }
        }
        let mut off = 0;
        for mip in 0..m {
            for slice in 0..s {
                let (w, h) = ((w >> mip).max(1), (h >> mip).max(1));
                let row = w.div_ceil(block) * stride;
                let pitch = row.next_multiple_of(256);
                let rows = h.div_ceil(block);
                for v in [slice * m + mip, off, w, h, pitch, row, rows] {
                    word(&mut b, v)
                }
                off += (pitch * rows).next_multiple_of(512);
            }
        }
    }
    fs::write(dir.join("textures.bin"), b)?;
    Ok(())
}
