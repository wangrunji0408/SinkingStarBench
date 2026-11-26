// Layout shared by the static linker and the build-time metadata readers.
pub fn u32at(b: &[u8], p: usize) -> usize {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap()) as usize
}
pub fn word(b: &[u8], p: usize) -> usize {
    u16::from_le_bytes(b[p..p + 2].try_into().unwrap()) as usize
}
pub fn wide(b: &[u8], p: usize) -> u64 {
    u64::from_le_bytes(b[p..p + 8].try_into().unwrap())
}

use std::ops::Range;
pub struct Pe {
    pub image: Vec<u8>,
    pub base: usize,
    pub text: Range<usize>,
    pub rdata: Range<usize>,
    pub imports: usize,
    pub data: Range<usize>,
    pub unwind: Range<usize>,
}
impl Pe {
    pub fn function(&self, pc: usize) -> Result<usize, String> {
        let mut rows = self.image[self.unwind.clone()]
            .chunks_exact(12)
            .filter(|r| (u32at(r, 0)..u32at(r, 4)).contains(&pc));
        let start = rows
            .next()
            .map(|r| u32at(r, 0))
            .ok_or_else(|| format!("no exception-table entry for {pc:#x}"))?;
        if rows.next().is_some() {
            return Err(format!("ambiguous exception-table entry for {pc:#x}"));
        }
        Ok(start)
    }
    pub fn parse(b: &[u8]) -> Self {
        assert!(b.starts_with(b"MZ"), "expected a Windows PE executable");
        let p = u32at(b, 60);
        let opt = p + 24;
        assert_eq!(
            b.get(p..p + 4),
            Some(&b"PE\0\0"[..]),
            "invalid PE signature"
        );
        assert_eq!(
            (word(b, p + 4), word(b, opt)),
            (0x8664, 0x20b),
            "expected AMD64 PE32+"
        );
        let base = usize::try_from(wide(b, opt + 24)).unwrap();
        let imports = u32at(b, opt + 120);
        let sh = opt + word(b, p + 20);
        let len = u32at(b, opt + 56);
        assert!(
            (4096..=256 * 1024 * 1024).contains(&len),
            "unsupported PE image size {len}"
        );
        let mut image = vec![0; len];
        let mut sections = std::collections::BTreeMap::new();
        for i in 0..word(b, p + 6) {
            let h = sh + 40 * i;
            let r = u32at(b, h + 12);
            let n = u32at(b, h + 16);
            let file = u32at(b, h + 20);
            let v = u32at(b, h + 8);
            assert!(r + v.max(n) <= len, "section exceeds PE image");
            image[r..r + n].copy_from_slice(&b[file..file + n]);
            let name = std::str::from_utf8(&b[h..h + 8])
                .unwrap()
                .trim_end_matches('\0');
            assert!(
                sections.insert(name, r..r + v).is_none(),
                "duplicate PE section {name}"
            );
        }
        let text = sections.remove(".text").expect("missing .text");
        let rdata = sections.remove(".rdata").expect("missing .rdata");
        assert!(
            text.end <= rdata.start && rdata.end <= len && imports < len,
            "unsupported PE section layout"
        );
        let data = sections.remove(".data").expect("missing .data");
        let u = u32at(b, opt + 136);
        let unwind = u..u + u32at(b, opt + 140);
        assert!(
            unwind.end <= len && unwind.len() % 12 == 0,
            "invalid AMD64 exception directory"
        );
        Self {
            data,
            unwind,
            image,
            base,
            text,
            rdata,
            imports,
        }
    }
}
