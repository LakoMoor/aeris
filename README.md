# Aeris OS

A bootable x86_64 operating system written in Rust. At first boot you choose how it looks:
**Windows 11 style** or **macOS 27 style**. The terminal speaks **PowerShell**, **CMD** and **macOS zsh**.

Site: https://lakomoor.github.io/aeris/ (published from `site/` by `.github/workflows/pages.yml`)

## Run

Needs Rust nightly (see `rust-toolchain.toml`) and QEMU.

```sh
cargo run -p builder -- run      # BIOS image in QEMU
cargo run -p builder -- uefi     # UEFI image (QEMU's edk2 firmware)
cargo run -p builder -- build    # only produce target/images/aeris-{bios,uefi}.img
cargo test -p aeris-core         # shell + file-system unit tests (host)
```

Click the QEMU window to capture the mouse, Ctrl+Alt+G releases it.
`python tools/qtest.py IMG PREFIX -- wait:4 key:ret shot:name` drives QEMU headlessly and takes screenshots.

## Layout

| Path | What |
|---|---|
| `crates/aeris-core` | `no_std` file system and the three shell dialects. Tested on the host. |
| `kernel` | Bootloader entry, software compositor, PS/2 input, Setup, both desktops, apps. |
| `builder` | Builds the kernel and the BIOS/UEFI disk images, launches QEMU. |
| `brand` | Logo. |
| `site` | GitHub Pages site. |
| `.claude/skills/stageshow` | The Stageshow skill, for producing product-UI illustrations for the site. |

## Shell features

Prompts, error text and output formats follow each real shell: `dir` / `Get-ChildItem` / `ls`,
`cd`, `type` / `cat` / `Get-Content`, `mkdir`, `rm` / `Remove-Item`, `cp`, `mv`, `echo`, pipes,
`>` / `>>`, variables (`$x`, `%X%`, `export`), `Select-String` / `findstr` / `grep`, `tree`,
`neofetch`, tab completion and history. `cmd` / `powershell` / `pwsh` switch dialect, `exit` returns.

## Running Windows programs

Windows x86-64 (PE32+) console programs run **natively** in ring 3: `crates/aeris-pe` loads and
relocates the image and binds imports; `kernel/src/winproc.rs` builds the GDT/TSS/IDT, opens the
process pages to user mode and implements a kernel32 subset (`GetStdHandle`, `WriteFile`,
`WriteConsoleA`, `ExitProcess`, ...) behind `int 0x80` thunks. Try `hello.exe` in PowerShell or CMD,
then `echo $LASTEXITCODE` / `echo %ERRORLEVEL%`. Rebuild the sample with
`cd tests/win-hello && cargo build --release` (needs the MSVC linker).
Limits: one process at a time, no preemption, programs are built into the kernel (no binary files in
the RAM file system yet), only the kernel32 subset above. macOS arm64 (Mach-O) programs cannot run
natively on an x86-64 CPU; that needs an AArch64 interpreter and is not implemented yet (typing a
`.exe` in the zsh terminal reports `exec format error`).

## Status

Preview. Settings and files persist on a second (data) disk via an ATA PIO driver: `target/images/aeris-data.img`
is created by the builder and attached to QEMU; delete it to run Setup again. Without a data disk
everything lives in RAM. Missing: USB, network, audio, SMP, user programs, NVMe/AHCI. Looks follow the Windows 11 and macOS design language with original artwork;
the project is not affiliated with Microsoft or Apple.
