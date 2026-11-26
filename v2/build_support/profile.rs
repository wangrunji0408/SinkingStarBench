// The instruction resolver identifies locations, not game semantics. Only
// profiles reviewed against gameplay traces may use these private host hooks.
use super::pe::Pe;
use std::collections::BTreeMap;
pub const BUILD: &str = "r68307";
pub fn supported(pe: &Pe, digest: &str) -> bool {
    digest == "589f8f12ab1b5398872be5228a1d55cafd4d510c32d424d6811fa2248fe06790"
        && pe.base == 0x140000000
        && pe.text == (0x1000..0x661ee6)
        && pe.rdata == (0x662000..0x803e70)
        && pe.data == (0x804000..0x10e0100)
        && pe.imports == 0x7c202c
        && pe.unwind == (0x10e1000..0x110824c)
}
// These shims have private calling conventions. Several are byte-identical
// wrappers; a substring match would not reliably identify their meaning.
pub const ALLOC: usize = 0x1000;
pub fn host_services(patch: &mut BTreeMap<usize, (usize, String)>) {
    for p in [0x1a4290, 0x4476e0, 0x1cc630, 0x165620, 0x3ad390] {
        patch.insert(p, (1, "ret".into()));
    }
    for (p, v) in [
        (0x480e40, 0),
        (0x47bb70, 0),
        (0x480460, 0),
        (0x480ac0, 0),
        (0x480d70, 0),
        (0x47d390, 1),
        (0x47d470, 1),
    ] {
        patch.insert(p, (6, format!("movl ${v},%eax\nret")));
    }
    for (p, name) in [
        (ALLOC, "alloc"),
        (0x1840, "alloc"),
        (0xba8c0, "sqrt"),
        (0xefbf0, "timer"),
        (0x219a20, "portrait"),
    ] {
        patch.insert(p, (5, format!("jmp L{name}")));
    }
}
