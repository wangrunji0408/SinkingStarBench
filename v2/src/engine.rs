// r68307 replay only. Game instructions are linked read/execute, never mapped by the runner.
mod assets;

mod init;
mod layout {
    include!(concat!(::std::env!("OUT_DIR"), "/layout.rs"));
    include!(concat!(::std::env!("OUT_DIR"), "/globals.rs"));
    include!(concat!(::std::env!("OUT_DIR"), "/actor-graphs.rs"));
}
mod routes;
mod scene;
#[path = "../scene_filter.rs"]
mod scene_filter;
mod services;
mod state;
use init::{alloc, copy};
include!(concat!(::std::env!("OUT_DIR"), "/dispatch.rs"));
static PLAYER_LAYOUT: std::sync::OnceLock<(usize, usize)> = std::sync::OnceLock::new();
static VIEW: std::sync::OnceLock<u8> = std::sync::OnceLock::new();

fn view() -> u8 {
    *VIEW.get().unwrap()
}

fn has_scene(level: &str) -> bool {
    assets::INDEX
        .binary_search_by_key(&format!("scene/{level}").as_str(), |e| e.0)
        .is_ok()
}

unsafe fn init_world(key: &str, capture_ranges: bool) -> (usize, Vec<(usize, usize)>) {
    let (level, ranges, view) = init::world(key, capture_ranges);
    assert!(VIEW.set(view).is_ok());
    (level, ranges)
}

unsafe fn check_mapping() {
    for (p, addr) in [
        (base_world, 0x100000),
        (base_heap, 0x2000000),
        (base_data, layout::DATA_BASE),
    ] {
        assert_eq!(p as usize, addr);
    }
}

type Stage = unsafe extern "C" fn();
unsafe extern "C" {
    static base_world: *mut u8;
    static base_heap: *mut u8;
    static base_data: *mut u8;
    static guest_cursor: *mut usize;
    static guest_registers: *mut u64;
    static run_prepare: Stage;
    static run_schedule: Stage;
    static run_select: Stage;
    static run_phase1: Stage;
    static run_phase2: Stage;
}
const W: usize = 0x110000;
unsafe fn get<T: Copy>(a: usize) -> T {
    (a as *const T).read_unaligned()
}
unsafe fn put<T>(a: usize, v: T) {
    (a as *mut T).write_unaligned(v)
}
unsafe fn reg(i: usize, v: u64) {
    guest_registers.add(i).write(v);
}
unsafe fn phase() -> bool {
    // Group resets can replace the Level_State entity. Follow the same
    // manager slot as the original LEVEL_STATE routine on each call.
    let level = get::<usize>(get::<usize>(W + layout::LEVEL_STATES));
    let f = get::<u32>(level + layout::PHASE_FLAGS);
    f & 16 == 0 && f & 12 != 0
}
unsafe extern "C" {
    static mut star_transition_target: [usize; 2];
}
unsafe fn target_level() -> Option<String> {
    let [len, ptr] = (&raw const star_transition_target).read();
    (len != 0 && finished()).then(|| {
        // Destination string storage belongs to the fixed game heap or asset image.
        std::str::from_utf8(std::slice::from_raw_parts(ptr as *const u8, len))
            .unwrap()
            .to_owned()
    })
}
unsafe fn won() -> bool {
    finished() && star_transition_target[0] == 0
}
unsafe fn finished() -> bool {
    get::<u8>(W + layout::TRANSITION) == 2 && !phase()
}
unsafe fn advance(ctrl: bool) {
    let dt = if ctrl { 0.25 } else { 1. } / 60.;
    put(layout::GAME_DT, dt as f32);
    put(layout::FRAME_DT, 1f32 / 60.);
    if phase() && matches!(get::<u8>(layout::PHASE_MODE), 3 | 5 | 6) {
        reg(5, W as u64);
        run_phase1();
        put(0x2e0070, W);
        run_phase2();
    }
    reg(2, 0x100000);
    reg(3, W as u64);
    run_prepare();
    reg(5, W as u64);
    run_schedule();
    overworld::advance(dt);
}

// Only uncommitted moves involving a controllable Guy block the route.
// Committed transactions may stay in the list while their animation plays;
// ambient actors and platforms can keep producing transactions indefinitely.
unsafe fn player_move_pending() -> bool {
    const COMMITTED: u32 = 2;
    const SINGLE_MOVE_SIZE: usize = 208;
    let moves = W + layout::MOVE_TRANSACTIONS;
    let count = get::<usize>(moves);
    if count == 0 {
        return false;
    }
    let &(guy, family_offset) = PLAYER_LAYOUT.get().unwrap();
    let families = get::<usize>(W + layout::FAMILY_IDS);
    let family_count = get::<usize>(W + layout::FAMILY_COUNT);
    let entities = get::<usize>(W + 0x378);
    let mut controlled = Vec::with_capacity(family_count);
    for i in 0..get::<usize>(W + 0x370) {
        let p = get::<usize>(entities + 8 * i);
        if get::<usize>(p + layout::TYPE) == guy {
            let family = get::<u32>(p + family_offset);
            if (0..family_count).any(|f| get::<u32>(families + 4 * f) == family) {
                controlled.push(get::<u32>(p + layout::UID));
            }
        }
    }
    let data = get::<usize>(moves + 8);
    (0..count).any(|i| {
        let tx = get::<usize>(data + i * 8);
        if get::<u32>(tx + layout::MOVE_TRANSACTION_FLAGS) & COMMITTED != 0 {
            return false;
        }
        let single = get::<usize>(tx + 48);
        (0..get::<usize>(tx + 40))
            .any(|j| controlled.contains(&get::<u32>(single + SINGLE_MOVE_SIZE * j)))
    })
}
unsafe fn pending_input() -> usize {
    get::<usize>(W + layout::INPUT_QUEUE)
}
unsafe fn needs_settle() -> bool {
    !finished() && (pending_input() != 0 || player_move_pending())
}
enum ReplayStep {
    Input(usize, routes::Key),
    Frame(u32),
    Boundary,
}

// Keep replay, scoring and rendering at the same input boundaries. Contiguous
// keys can fill the game's queue; T and end of input wait for it to drain.
unsafe fn play_route(
    route: &[routes::Key],
    mut step: impl FnMut(ReplayStep) -> std::io::Result<()>,
) -> std::io::Result<()> {
    use ReplayStep::*;
    use routes::Key;
    use std::io::Error;

    fn frame(
        ticks: &mut u32,
        step: &mut impl FnMut(ReplayStep) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        *ticks = ticks
            .checked_add(1)
            .ok_or_else(|| Error::other("frame count overflow"))?;
        step(Frame(*ticks))
    }

    unsafe fn settle(
        ticks: &mut u32,
        step: &mut impl FnMut(ReplayStep) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        // Some inputs, such as moves after all characters die, can leave the
        // original queue permanently blocked.
        let mut stalled = 0;
        while needs_settle() {
            let queued = pending_input();
            if stalled == 600 {
                return Err(Error::other(format!(
                    "replay stalled at frame {ticks} with {queued} queued inputs"
                )));
            }
            frame(ticks, step)?;
            stalled = if queued == pending_input() {
                stalled + 1
            } else {
                0
            };
        }
        Ok(())
    }

    let mut ticks = 0u32;
    for (index, &key) in route.iter().enumerate() {
        if finished() {
            return Ok(());
        }
        match key {
            Key::Frames(n) => {
                settle(&mut ticks, &mut step)?;
                for _ in 0..n {
                    if finished() {
                        break;
                    }
                    frame(&mut ticks, &mut step)?;
                }
            }
            Key::Take if overworld::enabled() => {
                settle(&mut ticks, &mut step)?;
                if !finished() {
                    step(Input(index, key))?;
                }
            }
            _ => step(Input(index, key))?,
        }
        step(Boundary)?;
    }
    settle(&mut ticks, &mut step)
}

fn movement_direction(d: u8) -> [i8; 2] {
    [[1, 0], [0, 1], [-1, 0], [0, -1]][((view() + d) % 4) as usize]
}

// True for queued gameplay actions; immediate actions are displayed at their input tick.
unsafe fn op(key: routes::Key, ctrl: &mut bool) -> bool {
    use routes::Key::*;
    let (kind, x, y): (u8, f32, f32) = match key {
        Frames(_) => unreachable!("frame waits are handled by play_route"),
        Run(down) => {
            put(layout::RUN_KEY, down as u8);
            return false;
        }
        Slow(down) => {
            *ctrl = down;
            put(layout::SLOW_KEY, down as u8);
            return false;
        }
        Swap => {
            let count = get::<usize>(W + layout::FAMILY_COUNT);
            assert!(count > 0, "no selectable family");
            if count > 1 {
                let next = (get::<usize>(W + layout::ACTIVE_FAMILY) + 1) % count;
                let uid = get::<u32>(get::<usize>(W + layout::FAMILY_IDS) + 4 * next);
                reg(2, 0x100000);
                reg(3, W as u64);
                reg(7, uid as u64);
                reg(8, 0x130000);
                run_select();
            }
            return false;
        }
        Direction(d) => {
            let [x, y] = movement_direction(d);
            (0, x.into(), y.into())
        }
        Action => (3, 0., 0.),
        Take if overworld::enter() => return false,
        Take => (5, 0., 0.),
        Dragon => (7, 0., 0.),
        Undo => (1, 0., 0.),
    };
    let fs = get::<usize>(W + layout::FAMILY_IDS);
    let active = get::<usize>(W + layout::ACTIVE_FAMILY);
    assert!(active < get::<usize>(W + layout::FAMILY_COUNT));
    let q = W + layout::INPUT_QUEUE;
    let n = get::<usize>(q);
    let cap = get::<usize>(q + 16);
    let mut p = get::<usize>(q + 8);
    assert!(n <= cap);
    if n == cap {
        let cap = 16.max(cap * 2);
        let dest = *guest_cursor;
        let bytes = (cap * 28 + 15) & !15;
        assert!(dest + bytes <= 0x40000000);
        std::ptr::write_bytes(dest as *mut u8, 0, bytes);
        if n != 0 {
            std::ptr::copy_nonoverlapping(p as *const u8, dest as *mut u8, n * 28);
        }
        *guest_cursor += bytes;
        p = dest;
        put(q + 8, p);
        put(q + 16, cap);
    }
    let a = p + 28 * n;
    put(a, [0u8; 28]);
    put(a, get::<u32>(fs + active * 4));
    put(a + 4, kind);
    put(a + 8, x);
    put(a + 12, y);
    put(q, n + 1);
    true
}
