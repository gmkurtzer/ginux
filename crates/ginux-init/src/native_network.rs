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
    pub netlink_sequence: u32,
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
            netlink_sequence: 0,
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

    /// Snapshot the one host-routed netlink operation currently required by
    /// systemd: adding the IPv4 loopback address. The immutable replay keeps
    /// mutable tracee memory out of the host call, and the validator rejects
    /// every other netlink family, message, interface, and address shape.
    pub fn snapshot_loopback_netlink(
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
        pending.message_count = 1;
        match regs.orig_rax {
            SYS_SENDTO => {
                let length = regs.rdx as usize;
                if !(24..=256).contains(&length) {
                    return Err(-7);
                }
                let payload = buffer.copy_from_tracee(pid, regs.rsi, length, 8)?;
                pending.netlink_sequence =
                    validate_loopback_netlink(&buffer.bytes[payload..payload + length])?;
                let name = snapshot_netlink_name(pid, regs.r8, regs.r9, buffer)?;
                pending.replay[0] = [
                    regs.rdi,
                    pending.address + payload as u64,
                    regs.rdx,
                    regs.r10,
                    offset_address(pending.address, name, regs.r9 as usize),
                    regs.r9,
                ];
            }
            SYS_SENDMSG => {
                let (header, sequence) =
                    snapshot_loopback_netlink_msghdr(pid, regs.rsi, buffer, pending.address)?;
                pending.netlink_sequence = sequence;
                pending.replay[0] = [regs.rdi, pending.address + header as u64, regs.rdx, 0, 0, 0];
            }
            _ => return Err(-38),
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
    if control_length != 0
        && ancillary_contains_scm_rights(&buffer.bytes[control..control + control_length])?
    {
        return Err(-1);
    }
    let header = buffer.reserve(56, 8)?;
    let mut source_header = [0u8; 56];
    source_header.copy_from_slice(&buffer.bytes[source..source + 56]);
    buffer.bytes[header..header + 56].copy_from_slice(&source_header);
    buffer.write_u64(header, offset_address(base, name, name_length as usize));
    buffer.write_u64(header + 16, offset_address(base, iov_copy, iov_count * 16));
    buffer.write_u64(header + 32, offset_address(base, control, control_length));
    Ok((header, destination))
}

pub fn sendmsg_has_scm_rights(pid: usize, address: u64) -> Result<bool, i64> {
    let mut buffer = Buffer::empty();
    let header = buffer.copy_from_tracee(pid, address, 56, 8)?;
    let control_pointer = buffer.read_u64(header + 32);
    let control_length = buffer.read_u64(header + 40) as usize;
    if control_length == 0 {
        return Ok(false);
    }
    if control_length > MAX_CONTROL {
        return Err(-7);
    }
    let control = buffer.copy_from_tracee(pid, control_pointer, control_length, 8)?;
    ancillary_contains_scm_rights(&buffer.bytes[control..control + control_length])
}

pub fn sendmmsg_has_scm_rights(pid: usize, address: u64, count: u64) -> Result<bool, i64> {
    if count > MAX_MESSAGES as u64 {
        return Err(-EINVAL);
    }
    for index in 0..count {
        let offset = index.checked_mul(64).ok_or(-EINVAL)?;
        let message = address.checked_add(offset).ok_or(-EINVAL)?;
        if sendmsg_has_scm_rights(pid, message)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ancillary_contains_scm_rights(control: &[u8]) -> Result<bool, i64> {
    let mut offset = 0usize;
    while offset < control.len() {
        if control.len() - offset < 16 {
            return Err(-EINVAL);
        }
        let length = u64::from_ne_bytes(
            control[offset..offset + 8]
                .try_into()
                .map_err(|_| -EINVAL)?,
        ) as usize;
        if length < 16 || length > control.len() - offset {
            return Err(-EINVAL);
        }
        let level = i32::from_ne_bytes(
            control[offset + 8..offset + 12]
                .try_into()
                .map_err(|_| -EINVAL)?,
        );
        let kind = i32::from_ne_bytes(
            control[offset + 12..offset + 16]
                .try_into()
                .map_err(|_| -EINVAL)?,
        );
        if level == 1 && kind == 1 {
            return Ok(true);
        }
        offset = offset.checked_add((length + 7) & !7).ok_or(-EINVAL)?;
    }
    Ok(false)
}

fn snapshot_loopback_netlink_msghdr(
    pid: usize,
    address: u64,
    buffer: &mut Buffer,
    base: u64,
) -> Result<(usize, u32), i64> {
    let source = buffer.copy_from_tracee(pid, address, 56, 8)?;
    let name_pointer = buffer.read_u64(source);
    let name_length = u64::from(buffer.read_u32(source + 8));
    let iov_pointer = buffer.read_u64(source + 16);
    let iov_count = buffer.read_u64(source + 24) as usize;
    let control_pointer = buffer.read_u64(source + 32);
    let control_length = buffer.read_u64(source + 40) as usize;
    if iov_count != 1 || control_pointer != 0 || control_length != 0 {
        return Err(-EINVAL);
    }
    let name = snapshot_netlink_name(pid, name_pointer, name_length, buffer)?;
    let iov_source = buffer.copy_from_tracee(pid, iov_pointer, 16, 8)?;
    let payload_pointer = buffer.read_u64(iov_source);
    let payload_length = buffer.read_u64(iov_source + 8) as usize;
    if !(24..=256).contains(&payload_length) {
        return Err(-EINVAL);
    }
    let payload = buffer.copy_from_tracee(pid, payload_pointer, payload_length, 8)?;
    let sequence = validate_loopback_netlink(&buffer.bytes[payload..payload + payload_length])?;
    let iov_copy = buffer.reserve(16, 8)?;
    buffer.write_u64(iov_copy, base + payload as u64);
    buffer.write_u64(iov_copy + 8, payload_length as u64);
    let header = buffer.reserve(56, 8)?;
    let mut source_header = [0u8; 56];
    source_header.copy_from_slice(&buffer.bytes[source..source + 56]);
    buffer.bytes[header..header + 56].copy_from_slice(&source_header);
    buffer.write_u64(
        header,
        offset_address(base, name, if name == 0 { 0 } else { 12 }),
    );
    buffer.write_u64(header + 16, base + iov_copy as u64);
    buffer.write_u64(header + 32, 0);
    Ok((header, sequence))
}

fn snapshot_netlink_name(
    pid: usize,
    address: u64,
    length: u64,
    buffer: &mut Buffer,
) -> Result<usize, i64> {
    if length == 0 {
        return Ok(0);
    }
    if address == 0 || !(12..=128).contains(&length) {
        return Err(-EINVAL);
    }
    let offset = buffer.copy_from_tracee(pid, address, length as usize, 4)?;
    if u16::from_ne_bytes(buffer.bytes[offset..offset + 2].try_into().unwrap()) != 16
        || buffer.bytes[offset + 2..offset + length as usize]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(-EINVAL);
    }
    Ok(offset)
}

fn validate_loopback_netlink(payload: &[u8]) -> Result<u32, i64> {
    let message_length = u32::from_ne_bytes(payload[0..4].try_into().unwrap()) as usize;
    let message_type = u16::from_ne_bytes(payload[4..6].try_into().unwrap());
    let flags = u16::from_ne_bytes(payload[6..8].try_into().unwrap());
    if message_length < 24
        || message_length > payload.len()
        || payload[message_length..].iter().any(|byte| *byte != 0)
        || flags & 1 == 0
    {
        return Err(-EINVAL);
    }
    if matches!(message_type, 16 | 19) {
        if message_length < 32
            || payload[16] != 0
            || i32::from_ne_bytes(payload[20..24].try_into().unwrap()) != 1
            || u32::from_ne_bytes(payload[24..28].try_into().unwrap()) & 1 == 0
            || u32::from_ne_bytes(payload[28..32].try_into().unwrap()) != 1
        {
            return Err(-EINVAL);
        }
        let mut offset = 32;
        while offset < message_length {
            if message_length - offset < 4 {
                return Err(-EINVAL);
            }
            let length =
                u16::from_ne_bytes(payload[offset..offset + 2].try_into().unwrap()) as usize;
            let kind = u16::from_ne_bytes(payload[offset + 2..offset + 4].try_into().unwrap());
            if length != 7 || kind != 3 || &payload[offset + 4..offset + 7] != b"lo\0" {
                return Err(-EINVAL);
            }
            offset = (offset + length + 3) & !3;
        }
        return if offset == (message_length + 3) & !3 {
            Ok(u32::from_ne_bytes(payload[8..12].try_into().unwrap()))
        } else {
            Err(-EINVAL)
        };
    }
    // RTM_NEWADDR + ifaddrmsg: IPv4 127.0.0.1/8 or IPv6 ::1/128,
    // host scope, and the loopback interface index only.
    let family = payload[16];
    if message_type != 20 {
        return Err(-EINVAL);
    }
    if !matches!((family, payload[17]), (2, 8) | (10, 128))
        || payload[19] != 254
        || u32::from_ne_bytes(payload[20..24].try_into().unwrap()) != 1
    {
        return Err(-EINVAL);
    }
    let mut offset = 24;
    let mut saw_address = false;
    while offset < message_length {
        if message_length - offset < 4 {
            return Err(-EINVAL);
        }
        let length = u16::from_ne_bytes(payload[offset..offset + 2].try_into().unwrap()) as usize;
        let kind = u16::from_ne_bytes(payload[offset + 2..offset + 4].try_into().unwrap());
        if length < 4
            || offset
                .checked_add(length)
                .is_none_or(|end| end > message_length)
        {
            return Err(-EINVAL);
        }
        if matches!(kind, 1 | 2) {
            let address_is_loopback = if family == 2 {
                length == 8 && payload[offset + 4..offset + 8] == [127, 0, 0, 1]
            } else {
                length == 20
                    && payload[offset + 4..offset + 19]
                        .iter()
                        .all(|byte| *byte == 0)
                    && payload[offset + 19] == 1
            };
            if !address_is_loopback {
                return Err(-EINVAL);
            }
            saw_address = true;
        } else {
            match kind {
                3 if length == 7 && &payload[offset + 4..offset + 7] == b"lo\0" => {}
                4 if family == 2
                    && length == 8
                    && payload[offset + 4..offset + 8] == [127, 255, 255, 255] => {}
                6 if length == 20 => {}
                8 if length == 8 => {}
                11 if length == 5 => {}
                _ => return Err(-EINVAL),
            }
        }
        offset = (offset + length + 3) & !3;
    }
    if !saw_address || offset != (message_length + 3) & !3 {
        return Err(-EINVAL);
    }
    Ok(u32::from_ne_bytes(payload[8..12].try_into().unwrap()))
}

pub fn accept_loopback_ack(
    pid: usize,
    header_address: usize,
    sequences: &[u32],
) -> Result<Option<usize>, i64> {
    let mut buffer = Buffer::empty();
    let header = buffer.copy_from_tracee(pid, header_address as u64, 56, 8)?;
    let iov_pointer = buffer.read_u64(header + 16);
    let iov_count = buffer.read_u64(header + 24);
    if iov_count == 0 || iov_pointer == 0 {
        return Err(-EINVAL);
    }
    let iov = buffer.copy_from_tracee(pid, iov_pointer, 16, 8)?;
    let payload_pointer = buffer.read_u64(iov);
    let payload_length = buffer.read_u64(iov + 8);
    if payload_pointer == 0 || payload_length < 16 {
        return Err(-EINVAL);
    }
    let copied_length = core::cmp::min(payload_length as usize, MAX_CONTROL);
    let payload = buffer.copy_from_tracee(pid, payload_pointer, copied_length, 4)?;
    ginux_core::loopback_netlink_ack_index(
        &buffer.bytes[payload..payload + copied_length],
        sequences,
    )
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
