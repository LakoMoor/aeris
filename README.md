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

## Status

Preview. Settings and files persist on a second (data) disk via an ATA PIO driver: `target/images/aeris-data.img`
is created by the builder and attached to QEMU; delete it to run Setup again. Without a data disk
everything lives in RAM. Missing: USB, network, audio, SMP, user programs, NVMe/AHCI. Looks follow the Windows 11 and macOS design language with original artwork;
the project is not affiliated with Microsoft or Apple.
