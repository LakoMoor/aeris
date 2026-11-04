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
    bootloader::BiosBoot::new(&kernel).create_disk_image(&bios).expect("bios image");
    bootloader::UefiBoot::new(&kernel).create_disk_image(&uefi).expect("uefi image");
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
