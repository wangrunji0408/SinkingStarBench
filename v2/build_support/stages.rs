// Relocate just the overwritten instruction(s) when execution falls through a
// stage boundary. The signature identifies the opcode; operands come from PE.
use super::{pe::Pe, u32at};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
pub fn install(
    pe: &Pe,
    a: &BTreeMap<&str, usize>,
    code: &mut String,
    patch: &mut BTreeMap<usize, (usize, String)>,
    labels: &mut BTreeSet<usize>,
) {
    let mut globals = BTreeSet::new();
    for (name, _) in super::anchors::ends() {
        let end = a[name];
        let start = a[name.strip_prefix("END_").unwrap()];
        let rel = |disp: usize, len: usize| {
            (end + len)
                .checked_add_signed(u32at(&pe.image, end + disp) as i32 as isize)
                .unwrap()
        };
        let data = |g: usize| {
            assert!(
                (pe.rdata.start..pe.image.len()).contains(&g),
                "bad data operand at {end:#x}"
            );
            g
        };
        let (n, ins) = match name {
            "END_START_SCENE" => {
                let target = rel(4, 8);
                assert!(pe.text.contains(&target));
                labels.insert(target);
                (8, format!("movq %r15,%rcx\ncall Lpc{target:x}"))
            }
            "END_BUILD_GRIDS_A" | "END_BUILD_GRIDS_B" | "END_BUILD_GRIDS_C" | "END_SCHEDULE" => {
                // The scheduler includes the native timers for deferred entity
                // events, including Judge-triggered freezer and reset queues.
                // Stop before the next stage, after the timer handlers.
                let n = match name {
                    "END_BUILD_GRIDS_A" => 7,
                    "END_BUILD_GRIDS_B" => 8,
                    _ => 6,
                };
                (
                    n,
                    format!(
                        ".byte {}",
                        pe.image[end..end + n]
                            .iter()
                            .map(u8::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                )
            }
            _ => {
                let (n, disp, op, reg) = match name {
                    "END_REGISTER_DEFAULTS" => (7, 3, "leaq", "%rax"),
                    "END_ROVER_SETUP" => (8, 3, "cmpq", ""),
                    "END_CLEAR_TRANSITION" => (7, 3, "movq", "%rsi"),
                    "END_STEP_PHASE" => (7, 2, "cmpb", ""),
                    _ => panic!("unknown stage {name}"),
                };
                let g = data(rel(disp, n));
                globals.insert(g);
                (
                    n,
                    if reg.is_empty() {
                        format!("{op} $0,Lg{g:x}(%rip)")
                    } else {
                        format!("{op} Lg{g:x}(%rip),{reg}")
                    },
                )
            }
        };
        assert!(patch
            .insert(end, (n, format!("jmp Lexit{end:x}")))
            .is_none());
        labels.insert(end + n);
        writeln!(code,"Lexit{end:x}:\npushfq\ncmpl ${start}, Lstage(%rip)\njne Lpass{end:x}\npopfq\njmp Lreturn\nLpass{end:x}:\npopfq\n{ins}\njmp Lpc{:x}",end+n).unwrap();
    }
    for g in globals {
        writeln!(code, ".set Lg{g:x}, _game_data+{}", g - pe.rdata.start).unwrap();
    }
}
