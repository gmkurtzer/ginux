//! Native clone3 argument ownership. The host reads only a private read-only
//! anonymous snapshot, never a validated-then-reused caller structure.
use crate::supervisor::{poke_tracee_bytes, syscall6, UserRegs, EFAULT, EINVAL};

const BASE: u64 = 0x6000_0000_0000;
const PAGE: u64 = 4096;
const END: u64 = BASE + 128 * PAGE;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Map,
    Call,
    Child,
    Unmap,
}

#[derive(Clone, Copy)]
pub struct Pending {
    pub phase: Phase,
    args: [u8; 88],
    original: [u64; 6],
    rip: u64,
    address: u64,
    pub result: i64,
    pub sigchld: bool,
}

impl Pending {
    pub const fn empty() -> Self {
        Self {
            phase: Phase::Idle,
            args: [0; 88],
            original: [0; 6],
            rip: 0,
            address: 0,
            result: 0,
            sigchld: false,
        }
    }

    pub fn snapshot(pid: usize, slot: usize, regs: &UserRegs) -> Result<Self, i64> {
        // Linux 6.12 copy_struct_from_user: min version 64, maximum page,
        // absent fields zero, unknown extension bytes must all be zero.
        let size = regs.rsi as usize;
        if size > PAGE as usize {
            return Err(-7);
        }
        if size < 64 {
            return Err(-EINVAL);
        }
        let mut bytes = [0u8; PAGE as usize];
        let local = [bytes.as_mut_ptr() as usize, size];
        let remote = [regs.rdi as usize, size];
        let read = unsafe {
            syscall6(
                310,
                pid,
                local.as_ptr() as usize,
                1,
                remote.as_ptr() as usize,
                1,
                0,
            )
        };
        if read < 0 {
            return Err(read as i64);
        }
        if read as usize != size {
            return Err(-EFAULT);
        }
        if bytes[88..size.max(88)].iter().any(|byte| *byte != 0) {
            return Err(-7);
        }
        let mut args = [0u8; 88];
        args.copy_from_slice(&bytes[..88]);
        if u64::from_ne_bytes(args[..8].try_into().unwrap()) & 0x2_0000_0000 != 0 && size < 88 {
            return Err(-EINVAL);
        }
        Ok(Self {
            phase: Phase::Map,
            args,
            original: arguments(regs),
            rip: regs.rip,
            address: BASE + slot as u64 * PAGE,
            result: 0,
            sigchld: false,
        })
    }

    pub fn flags(&self) -> u64 {
        u64::from_ne_bytes(self.args[..8].try_into().unwrap())
    }
    pub fn cgroup(&self) -> u64 {
        u64::from_ne_bytes(self.args[80..88].try_into().unwrap())
    }
    pub fn clear_cgroup(&mut self) {
        let flags = self.flags() & !0x2_0000_0000;
        self.args[..8].copy_from_slice(&flags.to_ne_bytes());
    }
    pub fn map(&self, regs: &mut UserRegs) {
        // Never replace a pre-existing workload mapping. Only this trusted
        // path can map into the reserved range; the mapping is born read-only.
        set_call(regs, 9, [self.address, PAGE, 1, 0x100022, u64::MAX, 0]);
    }
    pub fn mapped(&mut self, pid: usize, regs: &mut UserRegs) -> Result<(), i64> {
        if regs.rax != self.address {
            return Err(regs.rax as i64);
        }
        // ptrace intentionally writes this supervisor-owned, read-only page.
        // No process_vm_writev fallback or caller-memory write is involved.
        poke_tracee_bytes(pid, self.address as usize, &self.args)?;
        self.phase = Phase::Call;
        self.replay(
            regs,
            435,
            [
                self.address,
                88,
                self.original[2],
                self.original[3],
                self.original[4],
                self.original[5],
            ],
        );
        Ok(())
    }
    pub fn unmap(&mut self, regs: &mut UserRegs, result: i64) {
        self.result = result;
        self.phase = Phase::Unmap;
        self.replay(regs, 11, [self.address, PAGE, 0, 0, 0, 0]);
    }
    pub fn child(&self) -> Self {
        Self {
            phase: Phase::Child,
            sigchld: false,
            ..*self
        }
    }
    pub fn matches_entry(&self, regs: &UserRegs) -> bool {
        regs.rip == self.rip
            && match self.phase {
                Phase::Call => regs.orig_rax == 435 && regs.rdi == self.address && regs.rsi == 88,
                Phase::Unmap => regs.orig_rax == 11 && regs.rdi == self.address && regs.rsi == PAGE,
                _ => false,
            }
    }
    fn replay(&self, regs: &mut UserRegs, syscall: u64, args: [u64; 6]) {
        set_call(regs, syscall, args);
        regs.rax = syscall;
        regs.rip = self.rip - 2;
    }
    pub fn restore(&mut self, regs: &mut UserRegs) {
        set_call(regs, 435, self.original);
        regs.rip = self.rip;
        regs.rax = self.result as u64;
        self.phase = Phase::Idle;
    }
}

fn arguments(regs: &UserRegs) -> [u64; 6] {
    [regs.rdi, regs.rsi, regs.rdx, regs.r10, regs.r8, regs.r9]
}
fn set_call(regs: &mut UserRegs, syscall: u64, args: [u64; 6]) {
    regs.orig_rax = syscall;
    [regs.rdi, regs.rsi, regs.rdx, regs.r10, regs.r8, regs.r9] = args;
}
fn overlaps(address: u64, length: u64) -> bool {
    address < END && address.saturating_add(length.max(1)) > BASE
}

/// These calls could overwrite, discard, change permissions, move, or alias
/// the immutable snapshot. The native dispatcher applies this even before a
/// particular slot is allocated. Internal map/unmap stages never use this lane.
pub fn changes_snapshot(syscall: u64, regs: &UserRegs) -> bool {
    match syscall {
        9 => overlaps(regs.rdi, regs.rsi),
        10 | 11 | 28 | 329 => overlaps(regs.rdi, regs.rsi),
        25 => overlaps(regs.rdi, regs.rsi) || (regs.r10 & 2 != 0 && overlaps(regs.r8, regs.rdx)),
        _ => false,
    }
}
