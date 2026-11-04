//! Native execution of Windows x86-64 programs.
//!
//! A PE image is loaded into a private region of memory, that region is made
//! user-accessible in the live page tables, and the program runs in ring 3.
//! Win32 imports are bound to tiny thunks (`mov eax, id; int 0x80; ret`) that
//! trap into `syscall_dispatch`, which implements a subset of kernel32.
//!
//! Limits (see README): one process at a time, no preemption (a runaway
//! program hangs the machine), and the process shares the kernel address space
//! with the region's pages opened to ring 3.

use aeris_pe::Pe;
use alloc::alloc::{alloc_zeroed, dealloc, Layout};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::arch::{asm, global_asm};
use x86_64::instructions::segmentation::{Segment, CS, DS, ES, SS};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::{PrivilegeLevel, VirtAddr};

const STACK_SIZE: usize = 64 * 1024;
const PAGE: usize = 4096;

static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();
static mut TSS: TaskStateSegment = TaskStateSegment::new();
static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();
static mut KSTACK: [u8; 64 * 1024] = [0; 64 * 1024];
static mut PHYS_OFFSET: u64 = 0;

#[no_mangle]
static mut IN_USER: u8 = 0;
#[no_mangle]
static mut KSTACK_SAVE: u64 = 0;
#[no_mangle]
static mut EXIT_CODE: u64 = 0;

// Process state shared with the syscall handler.
static mut OUT: Vec<u8> = Vec::new();
static mut PROC_BASE: u64 = 0;
static mut PROC_LEN: u64 = 0;
#[no_mangle]
static mut LAST_FAULT: bool = false;
#[no_mangle]
static mut LAST_TIMEOUT: bool = false;
#[no_mangle]
static mut TICKS: u64 = 0;
static mut CMDLINE_PTR: u64 = 0;

global_asm!(
    r#"
    .global aeris_enter_user
    aeris_enter_user:
        push rbx
        push rbp
        push r12
        push r13
        push r14
        push r15
        mov [rip + KSTACK_SAVE], rsp
        mov byte ptr [rip + IN_USER], 1
        push 0x1B
        push rsi
        push 0x202
        push 0x23
        push rdi
        xor eax, eax
        xor ebx, ebx
        xor ebp, ebp
        xor r12d, r12d
        xor r13d, r13d
        xor r14d, r14d
        xor r15d, r15d
        iretq

    .global aeris_int80
    aeris_int80:
        push rax
        push rcx
        push rdx
        push rbx
        push rbp
        push rsi
        push rdi
        push r8
        push r9
        push r10
        push r11
        push r12
        push r13
        push r14
        push r15
        mov rdi, rsp
        cld
        call aeris_syscall
        test rax, rax
        jnz 2f
        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8
        pop rdi
        pop rsi
        pop rbp
        pop rbx
        pop rdx
        pop rcx
        pop rax
        iretq
    2:
        mov byte ptr [rip + IN_USER], 0
        mov rsp, [rip + KSTACK_SAVE]
        mov rax, [rip + EXIT_CODE]
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rbx
        ret

    .global aeris_timer
    aeris_timer:
        push rax
        mov al, 0x20
        out 0x20, al
        cmp byte ptr [rip + IN_USER], 0
        je 5f
        inc qword ptr [rip + TICKS]
        cmp qword ptr [rip + TICKS], 1500
        jb 5f
        pop rax
        mov byte ptr [rip + IN_USER], 0
        mov eax, 0xC000013A
        mov [rip + EXIT_CODE], rax
        mov byte ptr [rip + LAST_TIMEOUT], 1
        mov rsp, [rip + KSTACK_SAVE]
        mov rax, [rip + EXIT_CODE]
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rbx
        ret
    5:
        pop rax
        iretq

    .global aeris_fault
    aeris_fault:
        cmp byte ptr [rip + IN_USER], 0
        je 3f
        mov byte ptr [rip + IN_USER], 0
        mov eax, 0xC0000005
        mov [rip + EXIT_CODE], rax
        mov byte ptr [rip + LAST_FAULT], 1
        mov rsp, [rip + KSTACK_SAVE]
        mov rax, [rip + EXIT_CODE]
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rbx
        ret
    3:
        cld
        call aeris_kernel_fault
    4:
        hlt
        jmp 4b
    "#
);

extern "C" {
    fn aeris_enter_user(entry: u64, rsp: u64) -> u64;
    fn aeris_int80();
    fn aeris_fault();
    fn aeris_timer();
}

#[no_mangle]
extern "C" fn aeris_kernel_fault() {
    use core::fmt::Write;
    struct S;
    impl Write for S {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            for b in s.bytes() {
                crate::sys::outb(0x3F8, b);
            }
            Ok(())
        }
    }
    let _ = writeln!(S, "KERNEL CPU EXCEPTION (not in a Windows process)");
}

#[repr(C)]
pub struct Regs {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rdi: u64,
    rsi: u64,
    rbp: u64,
    rbx: u64,
    rdx: u64,
    rcx: u64,
    rax: u64,
}

/// Win32 functions we implement. The thunk number is the index here.
const API: &[&str] = &[
    "GetStdHandle",
    "WriteFile",
    "WriteConsoleA",
    "ExitProcess",
    "GetLastError",
    "GetModuleHandleA",
    "GetCurrentProcessId",
    "GetTickCount",
    "SetConsoleTitleA",
    "SetLastError",
    "Sleep",
    "GetCommandLineA",
];

#[no_mangle]
extern "C" fn aeris_syscall(r: &mut Regs) -> u64 {
    // SAFETY: single-threaded; PROC_* describe the live process.
    let (base, len) = unsafe { (PROC_BASE, PROC_LEN) };
    let in_proc = |p: u64, n: u64| p >= base && p.checked_add(n).map_or(false, |e| e <= base + len);
    match (r.rax & 0xFFFF) as usize {
        0 => r.rax = match r.rcx as u32 {
            0xFFFF_FFF6 => 0x0C, // stdin
            0xFFFF_FFF5 => 0x10, // stdout
            _ => 0x14,           // stderr
        },
        1 | 2 => {
            let (buf, n, written) = (r.rdx, r.r8 & 0xFFFF_FFFF, r.r9);
            if in_proc(buf, n) {
                // SAFETY: range checked to lie inside the process region.
                let s = unsafe { core::slice::from_raw_parts(buf as *const u8, n as usize) };
                unsafe { (*core::ptr::addr_of_mut!(OUT)).extend_from_slice(s) };
                if written != 0 && in_proc(written, 4) {
                    // SAFETY: range checked.
                    unsafe { (written as *mut u32).write_unaligned(n as u32) };
                }
                r.rax = 1;
            } else {
                r.rax = 0;
            }
        }
        3 => {
            // SAFETY: handler runs with interrupts off.
            unsafe { EXIT_CODE = r.rcx & 0xFFFF_FFFF };
            return 1;
        }
        4 | 9 => r.rax = 0,
        5 => r.rax = base,
        6 => r.rax = 4242,
        7 => r.rax = (crate::sys::ms() & 0xFFFF_FFFF) as u64,
        8 | 10 => r.rax = 1,
        // SAFETY: set before the process starts.
        11 => r.rax = unsafe { CMDLINE_PTR },
        _ => r.rax = 0,
    }
    0
}

/// One-time CPU setup: our own GDT (with ring-3 segments), TSS and IDT.
pub fn init(phys_offset: u64) {
    // SAFETY: called once at boot, single-threaded, before any process runs.
    unsafe {
        PHYS_OFFSET = phys_offset;
        let tss = &mut *core::ptr::addr_of_mut!(TSS);
        tss.privilege_stack_table[0] = VirtAddr::new(core::ptr::addr_of!(KSTACK) as u64 + (64 * 1024) as u64);
        let gdt: &'static mut GlobalDescriptorTable = &mut *core::ptr::addr_of_mut!(GDT);
        let kc = gdt.append(Descriptor::kernel_code_segment());
        let kd = gdt.append(Descriptor::kernel_data_segment());
        let _ud = gdt.append(Descriptor::user_data_segment());
        let _uc = gdt.append(Descriptor::user_code_segment());
        let ts = gdt.append(Descriptor::tss_segment(&*core::ptr::addr_of!(TSS) as &'static TaskStateSegment));
        debug_assert_eq!(_ud, SegmentSelector::new(3, PrivilegeLevel::Ring3));
        gdt.load();
        CS::set_reg(kc);
        SS::set_reg(kd);
        DS::set_reg(kd);
        ES::set_reg(kd);
        load_tss(ts);

        let idt: &'static mut InterruptDescriptorTable = &mut *core::ptr::addr_of_mut!(IDT);
        let f = VirtAddr::new(aeris_fault as usize as u64);
        idt.divide_error.set_handler_addr(f);
        idt.debug.set_handler_addr(f);
        idt.breakpoint.set_handler_addr(f);
        idt.overflow.set_handler_addr(f);
        idt.bound_range_exceeded.set_handler_addr(f);
        idt.invalid_opcode.set_handler_addr(f);
        idt.device_not_available.set_handler_addr(f);
        idt.double_fault.set_handler_addr(f);
        idt.invalid_tss.set_handler_addr(f);
        idt.segment_not_present.set_handler_addr(f);
        idt.stack_segment_fault.set_handler_addr(f);
        idt.general_protection_fault.set_handler_addr(f);
        idt.page_fault.set_handler_addr(f);
        idt.x87_floating_point.set_handler_addr(f);
        idt.alignment_check.set_handler_addr(f);
        idt.simd_floating_point.set_handler_addr(f);
        idt[0x80].set_handler_addr(VirtAddr::new(aeris_int80 as usize as u64)).set_privilege_level(PrivilegeLevel::Ring3);
        idt[32].set_handler_addr(VirtAddr::new(aeris_timer as usize as u64));
        idt.load();
        // Remap the PICs, let only the timer (IRQ0) through, PIT at 100 Hz (watchdog).
        use crate::sys::outb;
        outb(0x20, 0x11);
        outb(0xA0, 0x11);
        outb(0x21, 0x20);
        outb(0xA1, 0x28);
        outb(0x21, 4);
        outb(0xA1, 2);
        outb(0x21, 1);
        outb(0xA1, 1);
        outb(0x21, 0xFE);
        outb(0xA1, 0xFF);
        outb(0x43, 0x36);
        outb(0x40, (11932u32 & 0xFF) as u8);
        outb(0x40, (11932u32 >> 8) as u8);
    }
}

fn read_cr3() -> u64 {
    let v: u64;
    // SAFETY: reading CR3 has no side effects.
    unsafe { asm!("mov {}, cr3", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v & 0x000F_FFFF_FFFF_F000
}

fn flush_tlb() {
    let v = read_cr3();
    // SAFETY: writing the current CR3 back flushes the (non-global) TLB.
    unsafe { asm!("mov cr3, {}", in(reg) v, options(nostack)) };
}

/// Make every page of `[start, start+len)` accessible and executable from ring 3.
fn open_to_user(start: u64, len: usize) {
    const P: u64 = 1;
    const US: u64 = 1 << 2;
    const PS: u64 = 1 << 7;
    const NX: u64 = 1 << 63;
    // SAFETY: walks the live page tables through the bootloader's physical-memory mapping.
    unsafe {
        let off = PHYS_OFFSET;
        let table = |phys: u64| (phys + off) as *mut u64;
        let mut va = start & !(PAGE as u64 - 1);
        let end = start + len as u64;
        while va < end {
            let idx = [(va >> 39) & 511, (va >> 30) & 511, (va >> 21) & 511, (va >> 12) & 511];
            let mut t = table(read_cr3());
            for (level, i) in idx.iter().enumerate() {
                let e = t.add(*i as usize);
                let v = e.read();
                if v & P == 0 {
                    break;
                }
                e.write((v | US) & !NX);
                if level == 3 || (level > 0 && v & PS != 0) {
                    break;
                }
                t = table(v & 0x000F_FFFF_FFFF_F000);
            }
            va += PAGE as u64;
        }
    }
    flush_tlb();
}

#[derive(Debug)]
pub struct Exit {
    pub code: u32,
    pub output: String,
    pub faulted: bool,
    pub timed_out: bool,
}

/// Run a PE image to completion and return what it printed.
pub fn run(exe: &[u8], cmdline: &str) -> Result<Exit, String> {
    let pe = Pe::parse(exe).map_err(|e| match e {
        aeris_pe::PeError::Machine(m) => alloc::format!("unsupported machine type 0x{m:04x} (only x86-64 images run natively)"),
        aeris_pe::PeError::Not64Bit => "32-bit (PE32) programs are not supported".to_string(),
        _ => "not a valid Windows executable".to_string(),
    })?;
    let img = (pe.size_of_image as usize).next_multiple_of(PAGE);
    let total = img + PAGE + STACK_SIZE;
    let layout = Layout::from_size_align(total, PAGE).map_err(|_| "bad layout".to_string())?;
    // SAFETY: layout is non-zero and page aligned.
    let mem = unsafe { alloc_zeroed(layout) };
    if mem.is_null() {
        return Err("out of memory".to_string());
    }
    let base = mem as u64;
    // SAFETY: `mem` is a live allocation of `total` bytes.
    let region = unsafe { core::slice::from_raw_parts_mut(mem, total) };
    let thunk_base = base + img as u64;
    let mut missing: Vec<String> = Vec::new();
    let entry = {
        let (image, thunks) = region.split_at_mut(img);
        let r = pe.load_into(image, base, |dll, name| {
            let dll = dll.to_lowercase();
            match API.iter().position(|a| *a == name) {
                Some(id) if dll == "kernel32.dll" => Some(thunk_base + id as u64 * 8),
                _ => {
                    missing.push(alloc::format!("{dll}!{name}"));
                    Some(0)
                }
            }
        });
        if !missing.is_empty() {
            // SAFETY: matches the allocation above.
            unsafe { dealloc(mem, layout) };
            return Err(alloc::format!("missing Win32 imports: {}", missing.join(", ")));
        }
        for id in 0..API.len() {
            let t = &mut thunks[id * 8..id * 8 + 8];
            t[0] = 0xB8; // mov eax, imm32
            t[1..5].copy_from_slice(&(id as u32).to_le_bytes());
            t[5] = 0xCD; // int 0x80
            t[6] = 0x80;
            t[7] = 0xC3; // ret
        }
        let cl = cmdline.as_bytes();
        let n = cl.len().min(PAGE - 1024 - 1);
        thunks[1024..1024 + n].copy_from_slice(&cl[..n]);
        match r {
            Ok(e) => e,
            Err(e) => {
                // SAFETY: matches the allocation above.
                unsafe { dealloc(mem, layout) };
                return Err(alloc::format!("load failed: {e:?}"));
            }
        }
    };
    open_to_user(base, total);
    let rsp = (base + total as u64 - 0x28) & !0xF;
    let rsp = rsp - 8; // as if `call`ed: rsp % 16 == 8 at entry
    // SAFETY: single process at a time; state reset before entering ring 3.
    let code = unsafe {
        PROC_BASE = base;
        PROC_LEN = total as u64;
        CMDLINE_PTR = thunk_base + 1024;
        TICKS = 0;
        LAST_TIMEOUT = false;
        (*core::ptr::addr_of_mut!(OUT)).clear();
        LAST_FAULT = false;
        aeris_enter_user(entry, rsp)
    };
    // SAFETY: the process has ended; nothing references its memory any more.
    let (output, faulted, timed_out) = unsafe {
        let o = String::from_utf8_lossy(&*core::ptr::addr_of!(OUT)).to_string();
        let f = LAST_FAULT;
        let t = LAST_TIMEOUT;
        PROC_BASE = 0;
        PROC_LEN = 0;
        dealloc(mem, layout);
        (o, f, t)
    };
    Ok(Exit { code: code as u32, output, faulted, timed_out })
}
