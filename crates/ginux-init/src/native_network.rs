//! Immutable native network-send arguments.
//!
//! The supervisor copies every userspace object consumed by sendto/sendmsg/
//! sendmmsg into a private read-only tracee mapping, then makes the host
//! syscall consume only that snapshot.  sendmmsg is replayed as bounded
//! sendmsg calls because Linux writes `msg_len` into the caller's array.

use crate::supervisor::{poke_tracee_bytes, syscall6, UserRegs, EFAULT, EINVAL};

const BASE: u64 = 0x6100_0000_0000;
const SNAPSHOT_SIZE: usize = 16 * 1024;
const STRIDE: u64 = SNAPSHOT_SIZE as u64;
const END: u64 = BASE + 128 * STRIDE;
const MAX_MESSAGES: usize = 16;
const MAX_IOV: usize = 16;
const MAX_CONTROL: usize = 4096;
const MAX_SOCKADDR: usize = 128;

const SYS_SENDTO: u64 = 44;
const SYS_SENDMSG: u64 = 46;
const SYS_SENDMMSG: u64 = 307;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Map,
    Call,
    Unmap,
}

#[derive(Clone, Copy)]
pub struct Destination {
    pub port: u16,
}

#[derive(Clone, Copy)]
pub struct Pending {
    pub phase: Phase,
    original_syscall: u64,
    original: [u64; 6],
    replay: [[u64; 6]; MAX_MESSAGES],
    message_output: [u64; MAX_MESSAGES],
    destinations: [Option<Destination>; MAX_MESSAGES],
    message_count: usize,
    current: usize,
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

    fn reserve(&mut self, size: usize, alignment: usize) -> Result<usize, i64> {
        let aligned = self
            .used
            .checked_add(alignment - 1)
            .map(|value| value & !(alignment - 1))
            .ok_or(-7)?;
        let end = aligned.checked_add(size).ok_or(-7)?;
        if end > self.bytes.len() {
            return Err(-7);
        }
        self.used = end;
        Ok(aligned)
    }

    fn copy_from_tracee(
        &mut self,
        pid: usize,
        address: u64,
        size: usize,
        alignment: usize,
    ) -> Result<usize, i64> {
        if size == 0 {
            return Ok(0);
        }
        if address == 0 {
            return Err(-EFAULT);
        }
        let offset = self.reserve(size, alignment)?;
        let local = [self.bytes[offset..].as_mut_ptr() as usize, size];
        let remote = [address as usize, size];
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
        Ok(offset)
    }

    fn read_u32(&self, offset: usize) -> u32 {
        u32::from_ne_bytes(self.bytes[offset..offset + 4].try_into().unwrap())
    }

    fn read_u64(&self, offset: usize) -> u64 {
        u64::from_ne_bytes(self.bytes[offset..offset + 8].try_into().unwrap())
    }

    fn write_u64(&mut self, offset: usize, value: u64) {
        self.bytes[offset..offset + 8].copy_from_slice(&value.to_ne_bytes());
    }
}

impl Pending {
    pub const fn empty() -> Self {
        Self {
            phase: Phase::Idle,
            original_syscall: 0,
            original: [0; 6],
            replay: [[0; 6]; MAX_MESSAGES],
            message_output: [0; MAX_MESSAGES],
            destinations: [None; MAX_MESSAGES],
            message_count: 0,
            current: 0,
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
        buffer.reset();
        let mut pending = Self {
            phase: Phase::Map,
            original_syscall: regs.orig_rax,
            original: arguments(regs),
            rip: regs.rip,
            address: BASE + slot as u64 * STRIDE,
            ..Self::empty()
        };
        match regs.orig_rax {
            SYS_SENDTO => pending.snapshot_sendto(pid, regs, buffer)?,
            SYS_SENDMSG => pending.snapshot_sendmsg(pid, regs, buffer)?,
            SYS_SENDMMSG => pending.snapshot_sendmmsg(pid, regs, buffer)?,
            _ => return Err(-EINVAL),
        }
        Ok(pending)
    }

    pub fn destinations(&self) -> &[Option<Destination>] {
        &self.destinations[..self.message_count]
    }

    fn snapshot_sendto(
        &mut self,
        pid: usize,
        regs: &UserRegs,
        buffer: &mut Buffer,
    ) -> Result<(), i64> {
        let payload = buffer.copy_from_tracee(pid, regs.rsi, regs.rdx as usize, 8)?;
        let (name, destination) = snapshot_destination(pid, regs.r8, regs.r9, buffer)?;
        self.message_count = 1;
        self.destinations[0] = destination;
        self.replay[0] = [
            regs.rdi,
            offset_address(self.address, payload, regs.rdx as usize),
            regs.rdx,
            regs.r10,
            offset_address(self.address, name, regs.r9 as usize),
            regs.r9,
        ];
        Ok(())
    }

    fn snapshot_sendmsg(
        &mut self,
        pid: usize,
        regs: &UserRegs,
        buffer: &mut Buffer,
    ) -> Result<(), i64> {
        let (header, destination) = snapshot_msghdr(pid, regs.rsi, buffer, self.address)?;
        self.message_count = 1;
        self.destinations[0] = destination;
        self.replay[0] = [regs.rdi, self.address + header as u64, regs.rdx, 0, 0, 0];
        Ok(())
    }

    fn snapshot_sendmmsg(
        &mut self,
        pid: usize,
        regs: &UserRegs,
        buffer: &mut Buffer,
    ) -> Result<(), i64> {
        let count = regs.rdx as usize;
        if count == 0 || count > MAX_MESSAGES || regs.rsi == 0 {
            return Err(-EINVAL);
        }
        self.message_count = count;
        for index in 0..count {
            let original = regs.rsi.checked_add((index * 64) as u64).ok_or(-EFAULT)?;
            let (header, destination) = snapshot_msghdr(pid, original, buffer, self.address)?;
            self.destinations[index] = destination;
            self.replay[index] = [regs.rdi, self.address + header as u64, regs.r10, 0, 0, 0];
            self.message_output[index] = original + 56;
        }
        Ok(())
    }

    pub fn map(&self, regs: &mut UserRegs) {
        set_call(
            regs,
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
        self.replay_current(regs);
        Ok(())
    }

    pub fn completed_call(&mut self, pid: usize, regs: &mut UserRegs) -> Result<bool, i64> {
        let result = regs.rax as i64;
        if self.original_syscall == SYS_SENDMMSG && result >= 0 {
            let length = u32::try_from(result).map_err(|_| -EINVAL)?;
            poke_tracee_bytes(
                pid,
                self.message_output[self.current] as usize,
                &length.to_ne_bytes(),
            )?;
            self.current += 1;
            if self.current < self.message_count {
                self.replay_current(regs);
                return Ok(false);
            }
            self.unmap(regs, self.message_count as i64);
            return Ok(true);
        }
        let final_result = if self.original_syscall == SYS_SENDMMSG && self.current > 0 {
            self.current as i64
        } else {
            result
        };
        self.unmap(regs, final_result);
        Ok(true)
    }

    fn unmap(&mut self, regs: &mut UserRegs, result: i64) {
        self.result = result;
        self.phase = Phase::Unmap;
        self.replay(regs, 11, [self.address, SNAPSHOT_SIZE as u64, 0, 0, 0, 0]);
    }

    pub fn matches_entry(&self, regs: &UserRegs) -> bool {
        regs.rip == self.rip
            && match self.phase {
                Phase::Call => {
                    let syscall = if self.original_syscall == SYS_SENDMMSG {
                        SYS_SENDMSG
                    } else {
                        self.original_syscall
                    };
                    regs.orig_rax == syscall && regs.rsi == self.replay[self.current][1]
                }
                Phase::Unmap => {
                    regs.orig_rax == 11
                        && regs.rdi == self.address
                        && regs.rsi == SNAPSHOT_SIZE as u64
                }
                _ => false,
            }
    }

    pub fn restore(&mut self, regs: &mut UserRegs) -> bool {
        set_call(regs, self.original_syscall, self.original);
        self.phase = Phase::Idle;
        if matches!(self.result, -512 | -513 | -514 | -516) {
            regs.rip = self.rip - 2;
            regs.rax = self.original_syscall;
            return true;
        }
        regs.rip = self.rip;
        regs.rax = self.result as u64;
        false
    }

    fn replay_current(&self, regs: &mut UserRegs) {
        let syscall = if self.original_syscall == SYS_SENDMMSG {
            SYS_SENDMSG
        } else {
            self.original_syscall
        };
        self.replay(regs, syscall, self.replay[self.current]);
    }

    fn replay(&self, regs: &mut UserRegs, syscall: u64, args: [u64; 6]) {
        set_call(regs, syscall, args);
        regs.rax = syscall;
        regs.rip = self.rip - 2;
    }
}

fn snapshot_msghdr(
    pid: usize,
    address: u64,
    buffer: &mut Buffer,
    base: u64,
) -> Result<(usize, Option<Destination>), i64> {
    let source = buffer.copy_from_tracee(pid, address, 56, 8)?;
    let name_pointer = buffer.read_u64(source);
    let name_length = u64::from(buffer.read_u32(source + 8));
    let iov_pointer = buffer.read_u64(source + 16);
    let iov_count = buffer.read_u64(source + 24) as usize;
    let control_pointer = buffer.read_u64(source + 32);
    let control_length = buffer.read_u64(source + 40) as usize;
    if iov_count > MAX_IOV || control_length > MAX_CONTROL {
        return Err(-7);
    }
    let (name, destination) = snapshot_destination(pid, name_pointer, name_length, buffer)?;
    let iov_source = if iov_count == 0 {
        0
    } else {
        buffer.copy_from_tracee(pid, iov_pointer, iov_count * 16, 8)?
    };
    let iov_copy = if iov_count == 0 {
        0
    } else {
        buffer.reserve(iov_count * 16, 8)?
    };
    for index in 0..iov_count {
        let pointer = buffer.read_u64(iov_source + index * 16);
        let length = buffer.read_u64(iov_source + index * 16 + 8) as usize;
        let payload = buffer.copy_from_tracee(pid, pointer, length, 8)?;
        buffer.write_u64(iov_copy + index * 16, offset_address(base, payload, length));
        buffer.write_u64(iov_copy + index * 16 + 8, length as u64);
    }
    let control = if control_length == 0 {
        0
    } else {
        buffer.copy_from_tracee(pid, control_pointer, control_length, 8)?
    };
    let header = buffer.reserve(56, 8)?;
    let mut source_header = [0u8; 56];
    source_header.copy_from_slice(&buffer.bytes[source..source + 56]);
    buffer.bytes[header..header + 56].copy_from_slice(&source_header);
    buffer.write_u64(header, offset_address(base, name, name_length as usize));
    buffer.write_u64(header + 16, offset_address(base, iov_copy, iov_count * 16));
    buffer.write_u64(header + 32, offset_address(base, control, control_length));
    Ok((header, destination))
}

fn snapshot_destination(
    pid: usize,
    address: u64,
    length: u64,
    buffer: &mut Buffer,
) -> Result<(usize, Option<Destination>), i64> {
    if address == 0 && length == 0 {
        return Ok((0, None));
    }
    if address == 0 || length < 4 || length as usize > MAX_SOCKADDR {
        return Err(-EINVAL);
    }
    let offset = buffer.copy_from_tracee(pid, address, length as usize, 8)?;
    let word = buffer.read_u64(offset);
    let family = word & 0xffff;
    let required = match family {
        2 => 16,
        10 => 28,
        _ => return Err(-EINVAL),
    };
    if length < required {
        return Err(-EINVAL);
    }
    let port = ((((word >> 16) & 0xff) << 8) | ((word >> 24) & 0xff)) as u16;
    Ok((offset, Some(Destination { port })))
}

fn offset_address(base: u64, offset: usize, length: usize) -> u64 {
    if length == 0 {
        0
    } else {
        base + offset as u64
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

pub fn changes_snapshot(syscall: u64, regs: &UserRegs) -> bool {
    match syscall {
        9 => overlaps(regs.rdi, regs.rsi),
        10 | 11 | 28 | 329 => overlaps(regs.rdi, regs.rsi),
        25 => overlaps(regs.rdi, regs.rsi) || (regs.r10 & 2 != 0 && overlaps(regs.r8, regs.rdx)),
        _ => false,
    }
}
