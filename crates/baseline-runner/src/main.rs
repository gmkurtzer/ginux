#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

global_asm!(
    ".global _start",
    ".type _start,@function",
    "_start:",
    "xor %rbp, %rbp",
    "andq $-16, %rsp",
    "call rust_main",
    "ud2",
    options(att_syntax)
);

const SYS_WRITE: usize = 1;
const SYS_FORK: usize = 57;
const SYS_EXIT: usize = 60;
const SYS_WAIT4: usize = 61;
const SYS_GETPID: usize = 39;
const SYS_CLOCK_GETTIME: usize = 228;
const SYS_UNSHARE: usize = 272;

const CLOCK_MONOTONIC: usize = 1;
const CLONE_NEWNS: usize = 0x0002_0000;
const CLONE_NEWUTS: usize = 0x0400_0000;
const CLONE_NEWIPC: usize = 0x0800_0000;
const CLONE_NEWPID: usize = 0x2000_0000;
const NAMESPACE_FLAGS: usize = CLONE_NEWNS | CLONE_NEWUTS | CLONE_NEWIPC | CLONE_NEWPID;

const REPETITIONS: usize = 5;

#[repr(C)]
struct Timespec {
    seconds: i64,
    nanoseconds: i64,
}

#[derive(Clone, Copy)]
enum Mode {
    Direct,
    Container,
}

#[derive(Clone, Copy)]
enum Workload {
    Cpu,
    Syscall,
    Process,
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    fail(b"panic")
}

#[no_mangle]
unsafe extern "C" fn memcpy(
    destination: *mut core::ffi::c_void,
    source: *const core::ffi::c_void,
    length: usize,
) -> *mut core::ffi::c_void {
    let destination_bytes = destination.cast::<u8>();
    let source_bytes = source.cast::<u8>();
    let mut index = 0;
    while index < length {
        core::ptr::write_volatile(
            destination_bytes.add(index),
            core::ptr::read_volatile(source_bytes.add(index)),
        );
        index += 1;
    }
    destination
}

#[no_mangle]
extern "C" fn rust_main() -> ! {
    if unsafe { syscall0(SYS_GETPID) } == 1 {
        fail(b"refusing to benchmark as host PID 1");
    }
    line(b"BENCH_META architecture=x86_64 accelerator=tcg repetitions=5 unit=ns");
    line(b"BENCH_META container=linux-namespaces(mount,pid,uts,ipc)");

    for mode in [Mode::Direct, Mode::Container] {
        for workload in [Workload::Cpu, Workload::Syscall, Workload::Process] {
            for repetition in 1..=REPETITIONS {
                run_sample(mode, workload, repetition);
            }
        }
    }

    line(b"BENCH: PASS");
    exit(0)
}

fn run_sample(mode: Mode, workload: Workload, repetition: usize) {
    let child = unsafe { syscall0(SYS_FORK) };
    if child < 0 {
        fail(b"benchmark fork failed");
    }
    if child == 0 {
        if matches!(mode, Mode::Container) {
            if unsafe { syscall1(SYS_UNSHARE, NAMESPACE_FLAGS) } < 0 {
                fail(b"namespace setup failed");
            }
            let namespace_child = unsafe { syscall0(SYS_FORK) };
            if namespace_child < 0 {
                fail(b"PID namespace fork failed");
            }
            if namespace_child > 0 {
                let mut status = 0i32;
                let result = wait(namespace_child as usize, &mut status);
                exit(if result >= 0 && exit_status(status) == 0 {
                    0
                } else {
                    1
                });
            }
        }
        measure(mode, workload, repetition);
    }

    let mut status = 0i32;
    if wait(child as usize, &mut status) < 0 || exit_status(status) != 0 {
        fail(b"benchmark child failed");
    }
}

fn measure(mode: Mode, workload: Workload, repetition: usize) -> ! {
    let start = now();
    match workload {
        Workload::Cpu => cpu_workload(),
        Workload::Syscall => syscall_workload(),
        Workload::Process => process_workload(),
    }
    let elapsed = now().saturating_sub(start);

    write_all(b"BENCH mode=");
    write_all(match mode {
        Mode::Direct => b"direct",
        Mode::Container => b"container",
    });
    write_all(b" workload=");
    write_all(match workload {
        Workload::Cpu => b"cpu",
        Workload::Syscall => b"syscall",
        Workload::Process => b"process",
    });
    write_all(b" rep=");
    write_number(repetition as u64);
    write_all(b" ns=");
    write_number(elapsed);
    write_all(b"\n");
    exit(0)
}

#[inline(never)]
fn cpu_workload() {
    let mut accumulator = 0x9e37_79b9_7f4a_7c15u64;
    for value in 0..5_000_000u64 {
        accumulator = accumulator.rotate_left(7) ^ value.wrapping_mul(0x100_0000_01b3);
        unsafe { core::ptr::write_volatile(&mut accumulator, accumulator) };
    }
    unsafe { core::ptr::read_volatile(&accumulator) };
}

#[inline(never)]
fn syscall_workload() {
    for _ in 0..100_000 {
        unsafe { syscall0(SYS_GETPID) };
    }
}

#[inline(never)]
fn process_workload() {
    for _ in 0..64 {
        let child = unsafe { syscall0(SYS_FORK) };
        if child < 0 {
            exit(1);
        }
        if child == 0 {
            exit(0);
        }
        let mut status = 0i32;
        if wait(child as usize, &mut status) < 0 || exit_status(status) != 0 {
            exit(1);
        }
    }
}

fn now() -> u64 {
    let mut time = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    if unsafe {
        syscall2(
            SYS_CLOCK_GETTIME,
            CLOCK_MONOTONIC,
            &mut time as *mut _ as usize,
        )
    } < 0
    {
        fail(b"clock_gettime failed");
    }
    (time.seconds as u64) * 1_000_000_000 + time.nanoseconds as u64
}

fn wait(pid: usize, status: &mut i32) -> isize {
    unsafe { syscall4(SYS_WAIT4, pid, status as *mut _ as usize, 0, 0) }
}

fn exit_status(status: i32) -> i32 {
    if status & 0x7f == 0 {
        (status >> 8) & 0xff
    } else {
        255
    }
}

fn write_number(mut value: u64) {
    let mut buffer = [0u8; 20];
    let mut cursor = buffer.len();
    if value == 0 {
        write_all(b"0");
        return;
    }
    while value > 0 {
        cursor -= 1;
        buffer[cursor] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    write_all(&buffer[cursor..]);
}

fn line(message: &[u8]) {
    write_all(message);
    write_all(b"\n");
}

fn write_all(bytes: &[u8]) {
    let mut offset = 0;
    while offset < bytes.len() {
        let result = unsafe {
            syscall3(
                SYS_WRITE,
                1,
                bytes.as_ptr().add(offset) as usize,
                bytes.len() - offset,
            )
        };
        if result <= 0 {
            return;
        }
        offset += result as usize;
    }
}

fn fail(reason: &[u8]) -> ! {
    write_all(b"BENCH: FAIL: ");
    line(reason);
    exit(1)
}

fn exit(code: usize) -> ! {
    unsafe { syscall1(SYS_EXIT, code) };
    loop {
        core::hint::spin_loop();
    }
}

#[inline(always)]
unsafe fn syscall0(number: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn syscall1(number: usize, a1: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn syscall2(number: usize, a1: usize, a2: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn syscall3(number: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn syscall4(number: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}
