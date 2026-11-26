// Static linker for one fingerprinted build of Sinking Star; no runtime code loading.
#[path = "build_support/anchors.rs"]
mod anchors;
mod asset_pack;
#[path = "build_support/meshes.rs"]
mod meshes;
#[path = "build_support/pe.rs"]
mod pe;
#[path = "build_support/profile.rs"]
mod profile;
#[path = "build_support/reflection.rs"]
mod reflection;
mod scene_filter;
#[path = "build_support/stages.rs"]
mod stages;
use pe::{u32at, wide};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fmt::Write,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
};
fn main() {
    assert_eq!(
        env::var("TARGET").unwrap(),
        "x86_64-apple-darwin",
        "use --target x86_64-apple-darwin"
    );
    println!("cargo:rerun-if-env-changed=GAME_ROOT");
    let game = env::var_os("GAME_ROOT").map(PathBuf::from).unwrap_or_else(|| {
        let home = PathBuf::from(env::var_os("HOME").unwrap());
        let suffix = "steamapps/common/Order of the Sinking Star";
        ["Library/Application Support/Steam", "Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam"]
            .iter().map(|base| home.join(base).join(suffix)).find(|dir| dir.join("sinking_star.exe").is_file())
            .expect("Steam installation not found. Set GAME_ROOT to the directory containing sinking_star.exe (Windows Steam build r68307).")
    });
    println!("cargo:rerun-if-changed=gpu");
    let gpu_out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("gpu-worker");
    assert!(
        Command::new("clang++")
            .args([
                "-arch",
                "x86_64",
                "-std=c++17",
                "-O2",
                "-Igpu/DirectX-Headers/include/wsl/stubs",
                "-Igpu/DirectX-Headers/include",
                "gpu/original_global_check.cpp",
                "-lcompression",
                "-o"
            ])
            .arg(gpu_out)
            .status()
            .unwrap()
            .success(),
        "D3DMetal bridge build failed"
    );
    let video = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("video.o");
    assert!(
        Command::new("clang")
            .args([
                "-arch",
                "x86_64",
                "-fobjc-arc",
                "-fblocks",
                "-Wno-deprecated-declarations",
                "-O2",
                "-c",
                "gpu/video.m",
                "-o"
            ])
            .arg(&video)
            .status()
            .unwrap()
            .success()
    );
    println!("cargo:rustc-link-arg-bin=sinking-star={}", video.display());
    for name in [
        "AVFoundation",
        "AppKit",
        "Accelerate",
        "CoreVideo",
        "CoreMedia",
        "CoreFoundation",
        "CoreGraphics",
        "ImageIO",
        "Foundation",
    ] {
        println!("cargo:rustc-link-arg-bin=sinking-star=-Wl,-framework,{name}");
    }
    println!("cargo:rustc-link-arg-bin=sinking-star=-lobjc");
    println!("cargo:rustc-link-arg-bin=sinking-star=-Wl,-rpath,/usr/lib/swift");
    let exe = game.join("sinking_star.exe");
    let plat = Path::new("platform.s").canonicalize().unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let replay_key_path = out.join("challenge-replay-key.bin");
    match fs::metadata(&replay_key_path) {
        Ok(metadata) => assert_eq!(metadata.len(), 32),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut key = [0; 32];
            fs::File::open("/dev/urandom")
                .unwrap()
                .read_exact(&mut key)
                .unwrap();
            fs::write(&replay_key_path, key).unwrap();
        }
        Err(error) => panic!("{}: {error}", replay_key_path.display()),
    }
    // Solution files and optional victory ticks (VERIFIED_JSON) are read at runtime;
    // they are deliberately not embedded here, so
    // adding a solution never requires rebuilding the binary.
    for p in [&exe, &plat] {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    println!("cargo:rerun-if-changed=build.rs");
    for f in [
        "asset_pack.rs",
        "scene_filter.rs",
        "build_support/reflection.rs",
        "build_support/pe.rs",
        "build_support/profile.rs",
        "build_support/anchors.rs",
        "build_support/routines.txt",
        "build_support/globals.txt",
        "build_support/offsets.txt",
        "build_support/stages.rs",
        "build_support/movie.s",
        "build_support/stage_ends.txt",
        "build_support/meshes.rs",
    ] {
        println!("cargo:rerun-if-changed={f}");
    }
    let b = fs::read(&exe).unwrap_or_else(|e| {
        panic!(
            "{}: {e}. Set GAME_ROOT to the Windows Steam game directory.",
            exe.display()
        )
    });
    let source_hash = format!("{:x}", Sha256::digest(&b));
    let pe = pe::Pe::parse(&b);
    let image = &pe.image;
    let base = pe.base;
    let text = pe.text.start;
    let data = pe.rdata.start;
    let image_hash = format!("{:x}", Sha256::digest(image));
    // Private routines and the host bridge still require a reviewed ABI profile.
    // An overlay, checksum or signature outside the mapped sections does not change it.
    let (a, errors) = anchors::discover(&pe);
    let report = out.join("abi-report.json");
    let inventory = serde_json::json!({
        "executable_sha256":source_hash, "mapped_sha256":image_hash, "base":base,
        "text_rva":pe.text, "rdata_rva":pe.rdata, "data_rva":pe.data,
        "imports_rva":pe.imports, "exception_directory_rva":pe.unwind,
        "anchors":a, "unresolved":errors
    });
    fs::write(&report, serde_json::to_vec_pretty(&inventory).unwrap()).unwrap();
    let report = report.display();
    assert!(
        profile::supported(&pe, &image_hash),
        "unsupported game ABI: executable SHA-256 {source_hash}; see {report} and PORTING.md"
    );
    println!("cargo:rustc-env=STAR_GAME_BUILD={}", profile::BUILD);
    assert!(
        errors.is_empty(),
        "game instruction bindings: {}; see {report}",
        errors.join("; ")
    );
    let mut globals = format!("pub const GAME_ALLOC:usize={};\n", base + profile::ALLOC);
    for name in ["GAME_DT", "FRAME_DT", "GAME_CONTEXT", "PHASE_MODE"]
        .into_iter()
        .chain(anchors::globals().map(|g| g.0))
    {
        writeln!(globals, "pub const {name}:usize={};", base + a[name]).unwrap();
    }
    for (name, _) in anchors::routines() {
        writeln!(
            globals,
            "#[allow(dead_code)] pub const {name}:usize={};",
            a[name]
        )
        .unwrap();
    }
    fs::write(out.join("globals.rs"), globals).unwrap();
    println!("cargo:rustc-env=STAR_SOURCE_SHA256={source_hash}");
    fs::write(out.join("text.bin"), &image[text..data]).unwrap();
    fs::write(out.join("data.bin"), &image[data..]).unwrap();
    let mut code = String::from(".section __GLUE,__text,regular,pure_instructions\n");
    code += &fs::read_to_string(plat).unwrap();
    code += "\nLreturn:\n";
    let regs = [
        "rax", "rbx", "rcx", "rdx", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13",
        "r14", "r15",
    ];
    for (i, r) in regs.iter().enumerate() {
        writeln!(code, "movq %{r}, Lregs+{}(%rip)", i * 8).unwrap();
    }
    code += "pushfq\npopq Lregs+120(%rip)\n";
    for i in 0..16 {
        writeln!(code, "movups %xmm{i}, Lregs+{}(%rip)", 128 + i * 16).unwrap();
    }
    code += "movq Lhostsp(%rip),%rsp\ncld\npopq %r15\npopq %r14\npopq %r13\npopq %r12\npopq %rbp\npopq %rbx\nret\n";
    let mut starts = anchors::routines()
        .map(|(n, _)| a[n])
        .collect::<BTreeSet<_>>();
    let spec = asset_pack::build(&game, &out, &pe, &a);
    for s in spec.as_object().unwrap().values() {
        let a = s["address"].as_u64().unwrap() as usize - base;
        let ctor = wide(&image, a + 0x78) as usize;
        if ctor.checked_sub(base).is_some_and(|r| pe.text.contains(&r)) {
            starts.insert(ctor - base);
        }
    }
    let mut labels = starts.clone();
    let mut patch = BTreeMap::new();
    stages::install(&pe, &a, &mut code, &mut patch, &mut labels);
    // The accepted named transition receives two Jai strings. Save the target
    // header before the original scene lookup; unnamed puzzle exits skip this site.
    let switch = a["NAMED_TRANSITION"];
    labels.insert(switch + 6);
    patch.insert(switch, (6, "jmp Lcapture_target".into()));
    writeln!(code, "Lcapture_target:
movq (%r8),%rbx
movq %rbx,_star_transition_target(%rip)
movq 8(%r8),%rbx
movq %rbx,_star_transition_target+8(%rip)
movq %r9,%rbx
movq %r8,%r14
jmp Lpc{:x}
.data
.p2align 3
.globl _star_transition_target
_star_transition_target: .quad 0,0
.section __GLUE,__text,regular,pure_instructions", switch + 6).unwrap();
    let observer = a["PARTICLE_REQUEST"];
    assert_eq!(
        &image[observer..observer + 6],
        &[0x41, 0x57, 0x41, 0x56, 0x41, 0x55]
    );
    labels.insert(observer + 6);
    patch.insert(observer, (6, "jmp Lparticle_observer".into()));
    code += &include_str!("build_support/movie.s")
        .replace("Lparticle_resume", &format!("Lpc{:x}", observer + 6));
    // The EXE has both packed and lossless history writers. A wrap can move a lily
    // by the board width; packed_lilies.jai can encode only -1, 0 or +1 on each axis.
    // Keep the original move records and select the regular writer in that case.
    let history = a["HISTORY_GATE"];
    let regular = a["HISTORY_REGULAR"];
    let packed = history + 11;
    patch.insert(history, (11, "jmp Lhistory_format".into()));
    labels.extend([regular, packed]);
    code += &format!(
        r#"
Lhistory_format:
    testb $0x40,0x1c(%r13)
    je Lpc{regular:x}
    pushq %rax
    pushq %rcx
    pushq %rdx
    movq 0x28(%r13),%rcx
    movq 0x30(%r13),%rdx
Lhistory_next:
    testq %rcx,%rcx
    jz Lhistory_packed
    cmpb $9,8(%rdx)
    je Lhistory_axes
    cmpb $19,8(%rdx)
    jne Lhistory_advance
Lhistory_axes:
    movl 12(%rdx),%eax
    andl $0x7fffffff,%eax
    jz Lhistory_y
    cmpl $0x3f800000,%eax
    jne Lhistory_regular
Lhistory_y:
    movl 16(%rdx),%eax
    andl $0x7fffffff,%eax
    jz Lhistory_advance
    cmpl $0x3f800000,%eax
    jne Lhistory_regular
Lhistory_advance:
    addq $208,%rdx
    decq %rcx
    jmp Lhistory_next
Lhistory_regular:
    popq %rdx
    popq %rcx
    popq %rax
    jmp Lpc{regular:x}
Lhistory_packed:
    popq %rdx
    popq %rcx
    popq %rax
    jmp Lpc{packed:x}
"#
    );
    // Without a sampled skeleton, use the EXE's existing entity-position fallback.
    let pose = a["POSE_CHECK"];
    let pose_buffer = a["POSE_BUFFER"];
    let missing = a["POSE_MISSING"];
    let present = pose + 9;
    patch.insert(pose, (9, "jmp Lpose_available".into()));
    labels.extend([missing, present]);
    code += &format!(
        r#"
Lpose_available:
    testq %rax,%rax
    jz Lpc{missing:x}
    cmpq $0,{pose_buffer}(%rax)
    je Lpc{missing:x}
    jmp Lpc{present:x}
"#
    );
    let mut dispatch = String::from("unsafe fn original(pc:usize){match pc{\n");
    for &pc in &starts {
        writeln!(code,"Llaunch{pc:x}:\npushq %rbx\npushq %rbp\npushq %r12\npushq %r13\npushq %r14\npushq %r15\nmovq %rsp,Lhostsp(%rip)\nmovl ${pc},Lstage(%rip)\nmovl $0x2e0008,%esp\nmovq $0x2ff000,(%rsp)\npushq Lregs+120(%rip)\npopfq").unwrap();
        for (i, r) in regs.iter().enumerate() {
            writeln!(code, "movq Lregs+{}(%rip),%{r}", i * 8).unwrap();
        }
        for i in 0..16 {
            writeln!(code, "movups Lregs+{}(%rip),%xmm{i}", 128 + i * 16).unwrap();
        }
        writeln!(code, "jmp Lpc{pc:x}").unwrap();
        writeln!(
            dispatch,
            "{pc}=>{{unsafe extern \"C\"{{static invoke_{pc:x}:Stage;}}invoke_{pc:x}()}},"
        )
        .unwrap();
    }
    dispatch += "_=>panic!(\"unknown game routine {pc:x}\"),}}";
    fs::write(out.join("dispatch.rs"), dispatch).unwrap();
    profile::host_services(&mut patch);
    // Resolve only the shipping game's DLL import thunks. Unimplemented imports trap.
    let mut names = BTreeMap::new();
    let mut d = pe.imports;
    while u32at(&image, d + 12) != 0 {
        let mut i = u32at(&image, d);
        let mut slot = u32at(&image, d + 16);
        while wide(&image, i) != 0 {
            let a = wide(&image, i);
            if a >> 63 == 0 {
                let v = &image[a as usize + 2..];
                let s = std::str::from_utf8(&v[..v.iter().position(|&b| b == 0).unwrap()]).unwrap();
                names.insert(slot, s);
            }
            i += 8;
            slot += 8;
        }
        d += 20;
    }
    for i in text..pe.text.end - 5 {
        if image[i..i + 2] != [0xff, 0x25] {
            continue;
        }
        let dst = (i as i64 + 6 + u32at(&image, i + 2) as i32 as i64) as usize;
        if let Some(&name) = names.get(&dst) {
            let txt = match name {
                "memcpy" | "memset" | "memcmp" | "memmove" => format!("jmp L{name}"),
                "GetCurrentThreadId"
                | "EnterCriticalSection"
                | "LeaveCriticalSection"
                | "DeleteCriticalSection"
                | "InitializeCriticalSection"
                | "TlsSetValue"
                | "TlsFree"
                | "Sleep"
                | "UnmapViewOfFile"
                | "CloseHandle" => "movl $1,%eax\nret".into(),
                "TlsAlloc" | "TlsGetValue" | "free" => "movl $0,%eax\nret".into(),
                "CreateFileW"
                | "CreateFileMappingW"
                | "MapViewOfFile"
                | "GetFileSizeEx"
                | "ReadFile"
                | "MultiByteToWideChar"
                | "WideCharToMultiByte"
                | "malloc"
                | "realloc"
                | "calloc"
                | "QueryPerformanceCounter"
                | "QueryPerformanceFrequency" => {
                    writeln!(
                        code,
                        "Lsvc{i:x}:\njmpq *Lhost{i:x}(%rip)\nLhost{i:x}:\n.quad _star_{name}"
                    )
                    .unwrap();
                    format!("jmp Lsvc{i:x}")
                }
                _ => "ud2".into(),
            };
            patch.insert(i, (6, txt));
        }
    }
    writeln!(
        code,
        ".section __GAME,__text,regular,pure_instructions\n.org {text},0xcc"
    )
    .unwrap();
    let mut pos = text;
    labels.extend(patch.keys());
    for i in labels {
        assert!(i >= pos);
        if i > pos {
            writeln!(code, ".incbin \"text.bin\",{},{ }", pos - text, i - pos).unwrap();
        }
        writeln!(code, "Lpc{i:x}:").unwrap();
        pos = i;
        if let Some((n, s)) = patch.get(&i) {
            writeln!(code, "{s}\n.org {},0xcc", i + n).unwrap();
            pos += n;
        }
    }
    writeln!(code,".incbin \"text.bin\",{}\n.section __STOP,__text,regular,pure_instructions\n.org 0x2000\nud2\n.org 0x2100\nret\n.org 0x3000\njmpq *0(%rip)\n.quad Lreturn",pos-text).unwrap();
    code += ".section __BRIDGE,__data\n.p2align 4\nLregs:\n.space 120\n.quad 2\n.space 256\nLhostsp:\n.quad 0\nLstage:\n.quad 0\nLcursor:\n.quad 0x2000000\n.data\n";
    for (s, label) in [
        ("guest_cursor", "Lcursor"),
        ("guest_registers", "Lregs"),
        ("base_world", "_game_world"),
        ("base_heap", "_game_heap"),
        ("base_data", "_game_data"),
    ] {
        writeln!(code, ".globl _{s}\n_{s}:\n.quad {label}").unwrap();
    }
    for &pc in &starts {
        writeln!(
            code,
            ".globl _invoke_{pc:x}\n_invoke_{pc:x}:\n.quad Llaunch{pc:x}"
        )
        .unwrap();
    }
    for (name, pc) in [
        ("prepare", a["PREPARE"]),
        ("schedule", a["SCHEDULE"]),
        ("select", a["SELECT_FAMILY"]),
        ("phase1", a["CLEAR_TRANSITION"]),
        ("phase2", a["STEP_PHASE"]),
    ] {
        writeln!(
            code,
            ".globl _run_{name}\n_run_{name}:\n.quad Llaunch{pc:x}"
        )
        .unwrap();
    }
    writeln!(code,".zerofill __WORLD,__bss,_game_world,0x1fc000,12\n.zerofill __HEAP,__bss,_game_heap,0x3e000000,12\n.section __GDATA,__data\n_game_data:\n.incbin \"data.bin\",0").unwrap();
    fs::write(out.join("game.s"), code).unwrap();
    assert!(
        Command::new("clang")
            .current_dir(&out)
            .args(["-arch", "x86_64", "-c", "game.s", "-o", "game.o"])
            .status()
            .unwrap()
            .success()
    );
    for bin in ["sinking-star", "sinking-star-challenge"] {
        println!(
            "cargo:rustc-link-arg-bin={bin}={}",
            out.join("game.o").display()
        );
        println!(
            "cargo:rustc-link-arg-bin={bin}=-Wl,-no_compact_unwind,-pagezero_size,0x1000,-no_pie,-segaddr,__TEXT,0x100000000,-segaddr,__WORLD,0x100000,-segaddr,__STOP,0x2fc000,-segaddr,__HEAP,0x2000000,-segaddr,__GAME,{base:#x},-segaddr,__GDATA,{:#x},-segaddr,__BRIDGE,0x141200000,-segaddr,__GLUE,0x141300000,-segprot,__GAME,rx,rx,-segprot,__GLUE,rx,rx,-segprot,__STOP,rx,rx",
            base + data
        );
    }
}
