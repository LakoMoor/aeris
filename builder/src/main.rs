//! `cargo run -p builder -- [build|run|uefi] [extra qemu args]`
//! Builds the Aeris kernel and bootable BIOS/UEFI disk images, optionally launching QEMU.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("run");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let kernel = build_kernel(&root);
    let out = root.join("target").join("images");
    std::fs::create_dir_all(&out).unwrap();
    let bios = out.join("aeris-bios.img");
    let uefi = out.join("aeris-uefi.img");
    let progs = pack_programs(&root, &out);
    let mut bb = bootloader::BiosBoot::new(&kernel);
    let mut ub = bootloader::UefiBoot::new(&kernel);
    if let Some(p) = &progs {
        bb.set_ramdisk(p);
        ub.set_ramdisk(p);
    }
    bb.create_disk_image(&bios).expect("bios image");
    ub.create_disk_image(&uefi).expect("uefi image");
    println!("BIOS image: {}\nUEFI image: {}", bios.display(), uefi.display());
    let data = out.join("aeris-data.img");
    if !data.exists() {
        // 32 MiB blank data disk: holds your settings and files between boots.
        std::fs::File::create(&data).unwrap().set_len(32 * 1024 * 1024).unwrap();
    }
    let extra = if args.len() > 1 { &args[1..] } else { &[] };
    match cmd {
        "build" => {}
        "run" => qemu(&bios, &data, false, extra),
        "uefi" => qemu(&uefi, &data, true, extra),
        other => eprintln!("unknown command {other}; use build | run | uefi"),
    }
}

/// Pack `programs/*.exe` into a small archive; the kernel installs it into C:\Applications at boot.
fn pack_programs(root: &Path, out: &Path) -> Option<PathBuf> {
    let dir = root.join("programs");
    let mut blob: Vec<u8> = Vec::new();
    let mut names = Vec::new();
    for e in std::fs::read_dir(&dir).ok()?.flatten() {
        let p = e.path();
        if p.extension().map_or(false, |x| x.eq_ignore_ascii_case("exe")) {
            let name = p.file_name()?.to_string_lossy().to_string();
            let data = std::fs::read(&p).ok()?;
            blob.extend_from_slice(&(name.len() as u16).to_le_bytes());
            blob.extend_from_slice(name.as_bytes());
            blob.extend_from_slice(&(data.len() as u32).to_le_bytes());
            blob.extend_from_slice(&data);
            names.push(name);
        }
    }
    if names.is_empty() {
        return None;
    }
    println!("Programs: {}", names.join(", "));
    let f = out.join("programs.bin");
    std::fs::write(&f, blob).ok()?;
    Some(f)
}

fn build_kernel(root: &Path) -> PathBuf {
    let st = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(root)
        .args(["build", "--release", "-p", "aeris-kernel", "--target", "x86_64-unknown-none"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("cargo");
    assert!(st.success(), "kernel build failed");
    root.join("target/x86_64-unknown-none/release/aeris-kernel")
}

fn qemu(img: &Path, data: &Path, uefi: bool, extra: &[String]) {
    let win = Path::new(r"C:\Program Files\qemu\qemu-system-x86_64.exe");
    let exe = if win.exists() { win.to_string_lossy().into_owned() } else { "qemu-system-x86_64".to_string() };
    let mut c = Command::new(exe);
    c.args(["-m", "1024", "-smp", "2", "-vga", "std", "-name", "Aeris OS"]);
    c.arg("-drive").arg(format!("format=raw,file={},if=ide,index=0", img.display()));
    c.arg("-drive").arg(format!("format=raw,file={},if=ide,index=1", data.display()));
    if uefi {
        c.args(["-drive", r"if=pflash,format=raw,readonly=on,file=C:\Program Files\qemu\share\edk2-x86_64-code.fd"]);
    }
    c.args(extra);
    c.status().expect("qemu");
}
