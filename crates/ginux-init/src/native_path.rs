//! Immutable pathname arguments for the native filesystem lane.
//!
//! A stopped tracee's path strings are copied once into supervisor memory,
//! installed in a private read-only tracee mapping, and replayed from there.
//! Policy and the host kernel therefore consume the same bounded bytes even
//! when another task shares the caller's address space.

use crate::supervisor::{poke_tracee_bytes, ptrace, UserRegs, EFAULT, EINVAL, PTRACE_PEEKDATA};

const BASE: u64 = 0x6200_0000_0000;
const SNAPSHOT_SIZE: usize = 16 * 1024;
const STRIDE: u64 = SNAPSHOT_SIZE as u64;
const END: u64 = BASE + 128 * STRIDE;
const MAX_STRING: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Map,
    Call,
    Unmap,
}

#[derive(Clone, Copy)]
pub struct Pending {
    pub phase: Phase,
    syscall: u64,
    original: [u64; 6],
    replay: [u64; 6],
    rip: u64,
    address: u64,
    pub result: i64,
}

pub struct Buffer {
    bytes: [u8; SNAPSHOT_SIZE],
    used: usize,
}

impl Buffer {
    pub const fn empty() -> Self {
        Self {
            bytes: [0; SNAPSHOT_SIZE],
            used: 0,
        }
    }

    fn reset(&mut self) {
        self.bytes.fill(0);
        self.used = 0;
    }

    fn snapshot_c_string(&mut self, pid: usize, address: u64) -> Result<usize, i64> {
        if address == 0 {
            return Err(-EFAULT);
        }
        let start = self.used;
        for offset in (0..MAX_STRING).step_by(8) {
            let mut word = 0u64;
            if ptrace(
                PTRACE_PEEKDATA,
                pid,
                address.checked_add(offset as u64).ok_or(-EFAULT)? as usize,
                &mut word as *mut _ as usize,
            ) < 0
            {
                return Err(-EFAULT);
            }
            for byte in word.to_ne_bytes() {
                if self.used == self.bytes.len() {
                    return Err(-7);
                }
                self.bytes[self.used] = byte;
                self.used += 1;
                if byte == 0 {
                    return Ok(start);
                }
                if self.used - start == MAX_STRING {
                    return Err(-36);
                }
            }
        }
        Err(-36)
    }
}

impl Pending {
    pub const fn empty() -> Self {
        Self {
            phase: Phase::Idle,
            syscall: 0,
            original: [0; 6],
            replay: [0; 6],
            rip: 0,
            address: 0,
            result: 0,
        }
    }

    pub fn snapshot(
        pid: usize,
        slot: usize,
        regs: &UserRegs,
        buffer: &mut Buffer,
    ) -> Result<Self, i64> {
        let mask = ginux_fs_policy::path_snapshot_arguments(regs.orig_rax);
        if mask == 0 {
            return Err(-EINVAL);
        }
        buffer.reset();
        let address = BASE + slot as u64 * STRIDE;
        let original = arguments(regs);
        let mut replay = original;
        for index in 0..6 {
            if mask & (1 << index) != 0 {
                let offset = buffer.snapshot_c_string(pid, original[index])?;
                replay[index] = address + offset as u64;
            }
        }
        Ok(Self {
            phase: Phase::Map,
            syscall: regs.orig_rax,
            original,
            replay,
            rip: regs.rip,
            address,
            result: 0,
        })
    }

    pub fn map(&self, regs: &mut UserRegs) {
        set_replay(
            regs,
            self.rip,
            9,
            [self.address, SNAPSHOT_SIZE as u64, 1, 0x100022, u64::MAX, 0],
        );
    }

    pub fn mapped(&mut self, pid: usize, buffer: &Buffer, regs: &mut UserRegs) -> Result<(), i64> {
        if regs.rax != self.address {
            return Err(regs.rax as i64);
        }
        poke_tracee_bytes(pid, self.address as usize, &buffer.bytes[..buffer.used])?;
        self.phase = Phase::Call;
        set_replay(regs, self.rip, self.syscall, self.replay);
        Ok(())
    }

    pub fn matches_entry(&self, regs: &UserRegs) -> bool {
        regs.rip == self.rip
            && match self.phase {
                Phase::Call => regs.orig_rax == self.syscall && arguments(regs) == self.replay,
                Phase::Unmap => {
                    regs.orig_rax == 11
                        && regs.rdi == self.address
                        && regs.rsi == SNAPSHOT_SIZE as u64
                }
                _ => false,
            }
    }

    pub fn unmap(&mut self, regs: &mut UserRegs, result: i64) {
        self.result = result;
        self.phase = Phase::Unmap;
        set_replay(
            regs,
            self.rip,
            11,
            [self.address, SNAPSHOT_SIZE as u64, 0, 0, 0, 0],
        );
    }

    pub fn restore(&mut self, regs: &mut UserRegs) -> bool {
        set_call(regs, self.syscall, self.original);
        self.phase = Phase::Idle;
        regs.rip = self.rip;
        regs.rax = self.result as u64;
        matches!(self.result, -512 | -513 | -514 | -516)
    }
}

pub const fn has_snapshot(syscall: u64) -> bool {
    ginux_fs_policy::path_snapshot_arguments(syscall) != 0
}

pub fn changes_snapshot(syscall: u64, regs: &UserRegs) -> bool {
    let overlaps =
        |address: u64, length: u64| address < END && address.saturating_add(length.max(1)) > BASE;
    match syscall {
        9 => overlaps(regs.rdi, regs.rsi),
        10 | 11 | 28 | 329 => overlaps(regs.rdi, regs.rsi),
        25 => overlaps(regs.rdi, regs.rsi) || (regs.r10 & 2 != 0 && overlaps(regs.r8, regs.rdx)),
        _ => false,
    }
}

fn arguments(regs: &UserRegs) -> [u64; 6] {
    [regs.rdi, regs.rsi, regs.rdx, regs.r10, regs.r8, regs.r9]
}

fn set_call(regs: &mut UserRegs, syscall: u64, args: [u64; 6]) {
    regs.orig_rax = syscall;
    [regs.rdi, regs.rsi, regs.rdx, regs.r10, regs.r8, regs.r9] = args;
}

fn set_replay(regs: &mut UserRegs, rip: u64, syscall: u64, args: [u64; 6]) {
    set_call(regs, syscall, args);
    regs.rax = syscall;
    regs.rip = rip - 2;
}
