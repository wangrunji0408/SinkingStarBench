// A moved or ambiguous instruction must never silently bind to its old RVA.
#[path = "../build_support/anchors.rs"]
mod anchors;
#[path = "../build_support/pe.rs"]
mod pe;

fn fixture(site: usize, target: usize, duplicate: bool) -> pe::Pe {
    let mut b = vec![0u8; 0x900];
    let u16at = |b: &mut [u8], i: usize, v: u16| b[i..i + 2].copy_from_slice(&v.to_le_bytes());
    let u32at = |b: &mut [u8], i: usize, v: u32| b[i..i + 4].copy_from_slice(&v.to_le_bytes());
    b[..2].copy_from_slice(b"MZ");
    u32at(&mut b, 60, 0x80);
    b[0x80..0x84].copy_from_slice(b"PE\0\0");
    u16at(&mut b, 0x84, 0x8664);
    u16at(&mut b, 0x86, 3);
    u16at(&mut b, 0x94, 240);
    let opt = 0x98;
    u16at(&mut b, opt, 0x20b);
    b[opt + 24..opt + 32].copy_from_slice(&0x140000000u64.to_le_bytes());
    u32at(&mut b, opt + 56, 0x4000);
    u32at(&mut b, opt + 136, 0x2000);
    u32at(&mut b, opt + 140, 12);
    for (i, (name, rva, raw)) in [
        (".text", 0x1000, 0x300),
        (".rdata", 0x2000, 0x500),
        (".data", 0x3000, 0x700),
    ]
    .into_iter()
    .enumerate()
    {
        let h = opt + 240 + 40 * i;
        b[h..h + name.len()].copy_from_slice(name.as_bytes());
        for (o, v) in [(8, 0x200), (12, rva), (16, 0x200), (20, raw)] {
            u32at(&mut b, h + o, v)
        }
    }
    u32at(&mut b, 0x500, 0x1000);
    u32at(&mut b, 0x504, 0x11f0);
    for p in [Some(site), duplicate.then_some(site + 32)]
        .into_iter()
        .flatten()
    {
        let sig = [
            0x0f, 0x57, 0xc0, 0xf3, 0x0f, 0x11, 0x05, 0, 0, 0, 0, 0xf3, 0x0f, 0x5a, 0xc0, 0xf2,
            0x0f, 0x58, 0x05,
        ];
        b[0x300 + p..0x300 + p + sig.len()].copy_from_slice(&sig);
        u32at(
            &mut b,
            0x300 + p + 7,
            (target as i32 - (0x1000 + p + 11) as i32) as u32,
        );
    }
    pe::Pe::parse(&b)
}
#[test]
fn resolves_relocated_instruction_and_operand() {
    for (p, d) in [(4, 0x3010), (60, 0x30c0)] {
        let pe = fixture(p, d, false);
        assert_eq!(pe.base, 0x140000000);
        assert_eq!(pe.rdata, 0x2000..0x2200);
        assert_eq!(pe.imports, 0);
        assert_eq!(pe.function(0x1000 + p), Ok(0x1000));
        let (found, _) = anchors::discover(&pe);
        assert_eq!(found.get("GAME_DT"), Some(&d));
    }
}
#[test]
fn rejects_ambiguous_or_wrong_section_without_guessing() {
    for (dest, twice, why) in [(0x3004, true, "ambiguous"), (0x2014, false, "outside")] {
        let (found, errors) = anchors::discover(&fixture(12, dest, twice));
        assert!(!found.contains_key("GAME_DT"));
        assert!(errors
            .iter()
            .any(|e| e.starts_with("GAME_DT:") && e.contains(why)));
    }
}
