// Read-only virtual file handles for the game's animation-graph catalog.
use super::*;
use std::{cell::RefCell, collections::HashMap};
thread_local! {static FILES:RefCell<HashMap<String,Vec<u8>>>=RefCell::default();static HANDLES:RefCell<Vec<(String,usize)>>=RefCell::default();}
pub fn assets() -> Vec<String> {
    let mut names = vec![];
    for &(key, _, _, _) in crate::assets::INDEX {
        if let Some(name) = key.strip_prefix("graph/") {
            FILES.with_borrow_mut(|f| f.insert(name.to_owned(), crate::assets::read(key)));
            names.push(name.to_owned());
        }
    }
    names.sort();
    names
}
fn handle<T>(h: usize, f: impl FnOnce(&[u8], &mut usize) -> T) -> T {
    HANDLES.with_borrow_mut(|hs| {
        let (n, pos) = &mut hs[h - 100];
        FILES.with_borrow(|files| f(&files[n], pos))
    })
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn star_CreateFileW(p: *const u16) -> usize {
    let mut n = 0;
    while n < 4096 && p.add(n).read() != 0 {
        n += 1
    }
    let s = String::from_utf16(std::slice::from_raw_parts(p, n)).unwrap();
    let name = s.rsplit(['\\', '/']).next().unwrap();
    assert!(
        FILES.with_borrow(|f| f.contains_key(name)),
        "unknown animation {name}"
    );
    HANDLES.with_borrow_mut(|hs| {
        hs.push((name.into(), 0));
        99 + hs.len()
    })
}
#[unsafe(no_mangle)]
extern "win64" fn star_CreateFileMappingW(h: usize) -> usize {
    h
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn star_MapViewOfFile(h: usize) -> usize {
    handle(h, |b, _| {
        let p = alloc(b.len());
        copy(p, b);
        p
    })
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn star_GetFileSizeEx(h: usize, p: usize) -> usize {
    handle(h, |b, _| put(p, b.len()));
    1
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn star_ReadFile(h: usize, p: usize, n: usize, out: usize) -> usize {
    handle(h, |b, pos| {
        let n = n.min(b.len() - *pos);
        copy(p, &b[*pos..*pos + n]);
        put(out, n as u32);
        *pos += n;
    });
    1
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn star_MultiByteToWideChar(
    _: usize,
    _: usize,
    p: usize,
    n: u32,
    d: usize,
    cap: u32,
) -> usize {
    let v: String =
        String::from_utf8_lossy(std::slice::from_raw_parts(p as *const u8, n as usize)).into();
    let a: Vec<_> = v.encode_utf16().collect();
    if d != 0 {
        assert!(a.len() <= cap as usize);
        std::ptr::copy_nonoverlapping(a.as_ptr(), d as *mut u16, a.len());
    }
    a.len()
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn star_WideCharToMultiByte(
    _: usize,
    _: usize,
    p: usize,
    n: u32,
    d: usize,
    cap: u32,
) -> usize {
    let a = String::from_utf16_lossy(std::slice::from_raw_parts(p as *const u16, n as usize));
    if d != 0 {
        assert!(a.len() <= cap as usize);
        copy(d, a.as_bytes())
    }
    a.len()
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn star_malloc(n: usize) -> usize {
    let p = alloc(n + 16);
    put(p, n);
    p + 16
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn star_calloc(n: usize, s: usize) -> usize {
    star_malloc(n.checked_mul(s).unwrap())
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn star_realloc(old: usize, n: usize) -> usize {
    if old != 0 && n == 0 {
        return 0;
    }
    let p = star_malloc(n);
    if old != 0 {
        std::ptr::copy_nonoverlapping(
            old as *const u8,
            p as *mut u8,
            n.min(get::<usize>(old - 16)),
        );
    }
    p
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn star_QueryPerformanceCounter(p: usize) -> usize {
    put(p, (get::<f64>(W + 0x67d8) * 1e6).round_ties_even() as u64);
    1
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn star_QueryPerformanceFrequency(p: usize) -> usize {
    put(p, 1000000u64);
    1
}
