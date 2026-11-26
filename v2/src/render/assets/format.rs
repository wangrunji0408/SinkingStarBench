// On-disk formats in the Steam installation. All offsets are bounded by their
// containing package member; no decoded game assets are bundled.
use super::super::io::Res;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
pub struct R<'a>(pub &'a [u8]);
impl<'a> R<'a> {
    pub fn take(&mut self, n: usize) -> Res<&'a [u8]> {
        if n > self.0.len() {
            return Err("truncated game resource".into());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    pub fn byte(&mut self) -> Res<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn h(&mut self) -> Res<usize> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?) as usize)
    }
    pub fn w(&mut self) -> Res<usize> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?) as usize)
    }
    pub fn q(&mut self) -> Res<usize> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into()?) as usize)
    }
    pub fn f(&mut self) -> Res<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into()?))
    }
    pub fn str(&mut self, n: usize) -> Res<String> {
        Ok(std::str::from_utf8(self.take(n)?)?.into())
    }
    pub fn short(&mut self) -> Res<String> {
        let n = self.h()?;
        self.str(n)
    }
    pub fn long(&mut self) -> Res<String> {
        let n = self.q()?;
        self.str(n)
    }
}
pub struct Package {
    pub path: PathBuf,
    names: BTreeMap<String, String>,
    pub entries: BTreeMap<String, (usize, usize)>,
}
impl Package {
    pub fn new(path: PathBuf) -> Res<Self> {
        let mut f = File::open(&path)?;
        let mut h = [0; 64];
        f.read_exact(&mut h)?;
        if &h[..8] != b"simp\x01\0\0\0" {
            return Err("bad package magic".into());
        }
        let toc = R(&h[16..]).q()?;
        if toc as u64 > f.metadata()?.len() {
            return Err("bad package directory".into());
        }
        f.seek(SeekFrom::Start(toc as u64))?;
        let mut b = vec![];
        f.read_to_end(&mut b)?;
        let mut r = R(&b);
        let h = r.take(64)?;
        if &h[..4] != b"toc!" {
            return Err("bad package index".into());
        }
        let mut entries = BTreeMap::new();
        for _ in 0..R(&h[8..]).q()? {
            let n = r.w()?;
            let name = r.str(n)?;
            let nul = r.byte()?;
            let len = r.q()?;
            let off = r.q()?;
            if nul != 0 || off < 64 || off.checked_add(len).is_none_or(|end| end > toc) {
                return Err("invalid package extent".into());
            }
            entries.insert(name, (off, len));
        }
        let names = entries
            .keys()
            .map(|k| (k.rsplit('/').next().unwrap().to_owned(), k.clone()))
            .collect();
        Ok(Self {
            path,
            entries,
            names,
        })
    }
    pub fn get(&self, k: &str) -> Res<Vec<u8>> {
        let &(off, len) = self
            .entries
            .get(k)
            .ok_or_else(|| format!("missing {k} in {}", self.path.display()))?;
        let mut f = File::open(&self.path)?;
        f.seek(SeekFrom::Start(off as u64))?;
        let mut b = vec![0; len];
        f.read_exact(&mut b)?;
        Ok(b)
    }
    pub fn by_filename(&self, name: &str) -> Option<&String> {
        self.names.get(name)
    }
}
#[link(name = "compression")]
unsafe extern "C" {
    fn compression_decode_buffer(
        dst: *mut u8,
        n: usize,
        src: *const u8,
        s: usize,
        scratch: *mut std::ffi::c_void,
        algo: u32,
    ) -> usize;
}
pub fn decode(b: &[u8]) -> Res<Vec<u8>> {
    let mut r = R(b);
    r.take(4)?;
    let codec = r.h()?;
    let hdr = r.h()?;
    let size = r.w()?;
    if ![12, 24, 40].contains(&hdr) || size < hdr || size > 512 << 20 || b.len() < hdr {
        return Err("invalid asset envelope".into());
    }
    let n = size - hdr;
    let raw = &b[hdr..];
    match codec {
        0 if raw.len() == n => Ok(raw.to_vec()),
        3 => {
            let mut bytes = vec![0; n];
            if unsafe {
                compression_decode_buffer(
                    bytes.as_mut_ptr(),
                    n,
                    raw.as_ptr(),
                    raw.len(),
                    std::ptr::null_mut(),
                    0x101,
                )
            } != n
            {
                return Err("LZ4 decode failed".into());
            }
            Ok(bytes)
        }
        _ => Err("unsupported asset encoding".into()),
    }
}
pub fn asset(root: &Path, name: &str, ext: &str) -> Res<Vec<u8>> {
    if name.contains(['/', '\\', '\0']) {
        return Err("invalid asset name".into());
    }
    decode(&fs::read(
        root.join("data-pc").join(format!("{name}.{ext}")),
    )?)
}
pub fn mesh_info(raw: &[u8]) -> Res<(Vec<String>, Vec<(String, [f32; 16])>)> {
    let mut r = R(raw);
    let lod = r.w()?;
    let v = r.w()?;
    r.take(6)?;
    let skin = r.byte()? != 0;
    r.take(37)?;
    let mats = r.w()?;
    if lod > 3 || v > 2000000 || mats > 10000 {
        return Err("bad mesh header".into());
    }
    r.take(v * (if skin { 44 } else { 36 }))?;
    for _ in 0..lod {
        r.take(mats * 12 + 4)?;
        let n = r.w()?;
        r.take(n + 2)?;
    }
    let mut names = vec![];
    for _ in 0..mats {
        names.push(r.short()?)
    }
    let mut bones = vec![];
    if skin {
        for _ in 0..r.w()? {
            let name = r.short()?;
            let mut inv = [0.; 16];
            for i in 0..16 {
                inv[i % 4 * 4 + i / 4] = r.f()?
            }
            inv[15] = 1.;
            bones.push((name, inv));
        }
    }
    if !r.0.is_empty() {
        return Err("trailing mesh bytes".into());
    }
    Ok((names, bones))
}
