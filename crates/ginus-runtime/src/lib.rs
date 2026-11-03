#![no_std]

use ginus_core::{decide, Action, SYS_EXIT, SYS_GETPID, SYS_WRITE};

pub const PAGE_SIZE: usize = 4096;
pub const ADDRESS_BASE: u64 = 0x1_0000;
pub const ADDRESS_SIZE: usize = 0x4_0000;
pub const ADDRESS_END: u64 = ADDRESS_BASE + ADDRESS_SIZE as u64;
pub const STACK_SIZE: usize = 0x4000;
pub const STACK_BASE: u64 = ADDRESS_END - STACK_SIZE as u64;
pub const VIRTUAL_PID: u64 = 4242;
pub const MAX_AUDIT: usize = 32;
pub const MAX_OUTPUT: usize = 4096;

const PERM_READ: u8 = 1;
const PERM_WRITE: u8 = 2;
const PERM_EXEC: u8 = 4;
const PAGE_COUNT: usize = ADDRESS_SIZE / PAGE_SIZE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryError {
    OutsideAddressSpace,
    Overflow,
    PermissionDenied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadError {
    Truncated,
    InvalidMagic,
    UnsupportedClass,
    UnsupportedEndian,
    UnsupportedType,
    UnsupportedMachine,
    InvalidProgramHeaders,
    UnsupportedInterpreter,
    InvalidSegment,
    SegmentOverlap,
    WriteExecuteSegment,
    InvalidEntry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackError {
    TooManyValues,
    StringTooLong,
    NoSpace,
    Memory(MemoryError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuError {
    Memory(MemoryError),
    UnsupportedInstruction,
    StepLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunError {
    Cpu(CpuError),
    AuditOverflow,
    OutputOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoadedElf {
    pub entry: u64,
    pub segments: u8,
}

pub struct AddressSpace {
    bytes: [u8; ADDRESS_SIZE],
    permissions: [u8; PAGE_COUNT],
}

impl AddressSpace {
    pub const fn new() -> Self {
        Self {
            bytes: [0; ADDRESS_SIZE],
            permissions: [0; PAGE_COUNT],
        }
    }

    pub fn load_elf(&mut self, elf: &[u8]) -> Result<LoadedElf, LoadError> {
        self.bytes.fill(0);
        self.permissions.fill(0);
        if elf.len() < 64 {
            return Err(LoadError::Truncated);
        }
        if &elf[0..4] != b"\x7fELF" {
            return Err(LoadError::InvalidMagic);
        }
        if elf[4] != 2 {
            return Err(LoadError::UnsupportedClass);
        }
        if elf[5] != 1 {
            return Err(LoadError::UnsupportedEndian);
        }
        if read_u16(elf, 16)? != 2 {
            return Err(LoadError::UnsupportedType);
        }
        if read_u16(elf, 18)? != 62 {
            return Err(LoadError::UnsupportedMachine);
        }
        let entry = read_u64(elf, 24)?;
        let phoff =
            usize::try_from(read_u64(elf, 32)?).map_err(|_| LoadError::InvalidProgramHeaders)?;
        let phentsize = read_u16(elf, 54)? as usize;
        let phnum = read_u16(elf, 56)? as usize;
        if phentsize != 56 || phnum == 0 || phnum > 16 {
            return Err(LoadError::InvalidProgramHeaders);
        }
        let table_size = phentsize
            .checked_mul(phnum)
            .and_then(|size| phoff.checked_add(size))
            .ok_or(LoadError::InvalidProgramHeaders)?;
        if table_size > elf.len() {
            return Err(LoadError::InvalidProgramHeaders);
        }

        let mut ranges = [(0u64, 0u64); 16];
        let mut range_count = 0usize;
        let mut segments = 0u8;
        for index in 0..phnum {
            let offset = phoff + index * phentsize;
            let kind = read_u32(elf, offset)?;
            if kind == 3 {
                return Err(LoadError::UnsupportedInterpreter);
            }
            if kind != 1 {
                continue;
            }
            let flags = read_u32(elf, offset + 4)?;
            let file_offset = read_u64(elf, offset + 8)?;
            let address = read_u64(elf, offset + 16)?;
            let file_size = read_u64(elf, offset + 32)?;
            let memory_size = read_u64(elf, offset + 40)?;
            if memory_size == 0 || file_size > memory_size {
                return Err(LoadError::InvalidSegment);
            }
            let end = address
                .checked_add(memory_size)
                .ok_or(LoadError::InvalidSegment)?;
            if address < ADDRESS_BASE || end > STACK_BASE {
                return Err(LoadError::InvalidSegment);
            }
            let file_end = file_offset
                .checked_add(file_size)
                .ok_or(LoadError::InvalidSegment)?;
            if usize::try_from(file_end).map_or(true, |value| value > elf.len()) {
                return Err(LoadError::InvalidSegment);
            }
            if flags & 2 != 0 && flags & 1 != 0 {
                return Err(LoadError::WriteExecuteSegment);
            }
            for (other_start, other_end) in ranges.iter().take(range_count) {
                if address < *other_end && *other_start < end {
                    return Err(LoadError::SegmentOverlap);
                }
            }
            ranges[range_count] = (address, end);
            range_count += 1;

            let destination = self
                .offset(address, memory_size as usize)
                .map_err(|_| LoadError::InvalidSegment)?;
            let source_start = file_offset as usize;
            let source_end = source_start + file_size as usize;
            self.bytes[destination..destination + file_size as usize]
                .copy_from_slice(&elf[source_start..source_end]);
            self.bytes[destination + file_size as usize..destination + memory_size as usize]
                .fill(0);

            let mut permissions = 0;
            if flags & 4 != 0 {
                permissions |= PERM_READ;
            }
            if flags & 2 != 0 {
                permissions |= PERM_WRITE;
            }
            if flags & 1 != 0 {
                permissions |= PERM_EXEC;
            }
            if self.would_write_execute(address, memory_size as usize, permissions) {
                return Err(LoadError::WriteExecuteSegment);
            }
            self.set_permissions(address, memory_size as usize, permissions)
                .map_err(|_| LoadError::InvalidSegment)?;
            segments = segments.checked_add(1).ok_or(LoadError::InvalidSegment)?;
        }
        if segments == 0 || !self.allowed(entry, 1, PERM_EXEC) {
            return Err(LoadError::InvalidEntry);
        }
        Ok(LoadedElf { entry, segments })
    }

    pub fn build_stack(&mut self, argv: &[&[u8]], env: &[&[u8]]) -> Result<u64, StackError> {
        if argv.len() > 8 || env.len() > 8 {
            return Err(StackError::TooManyValues);
        }
        self.set_permissions(STACK_BASE, STACK_SIZE, PERM_READ | PERM_WRITE)
            .map_err(StackError::Memory)?;
        let mut cursor = ADDRESS_END;
        let mut argv_addresses = [0u64; 8];
        let mut env_addresses = [0u64; 8];
        for (index, value) in env.iter().enumerate().rev() {
            cursor = self.push_string(cursor, value)?;
            env_addresses[index] = cursor;
        }
        for (index, value) in argv.iter().enumerate().rev() {
            cursor = self.push_string(cursor, value)?;
            argv_addresses[index] = cursor;
        }
        cursor &= !15;
        cursor = self.push_u64(cursor, 0)?;
        for address in env_addresses[..env.len()].iter().rev() {
            cursor = self.push_u64(cursor, *address)?;
        }
        cursor = self.push_u64(cursor, 0)?;
        for address in argv_addresses[..argv.len()].iter().rev() {
            cursor = self.push_u64(cursor, *address)?;
        }
        cursor = self.push_u64(cursor, argv.len() as u64)?;
        Ok(cursor)
    }

    pub fn read(&self, address: u64, output: &mut [u8]) -> Result<(), MemoryError> {
        if !self.allowed(address, output.len(), PERM_READ) {
            return Err(MemoryError::PermissionDenied);
        }
        let offset = self.offset(address, output.len())?;
        output.copy_from_slice(&self.bytes[offset..offset + output.len()]);
        Ok(())
    }

    pub fn write(&mut self, address: u64, input: &[u8]) -> Result<(), MemoryError> {
        if !self.allowed(address, input.len(), PERM_WRITE) {
            return Err(MemoryError::PermissionDenied);
        }
        let offset = self.offset(address, input.len())?;
        self.bytes[offset..offset + input.len()].copy_from_slice(input);
        Ok(())
    }

    pub fn read_u64(&self, address: u64) -> Result<u64, MemoryError> {
        let mut bytes = [0u8; 8];
        self.read(address, &mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn fetch(&self, address: u64, output: &mut [u8]) -> Result<(), MemoryError> {
        if !self.allowed(address, output.len(), PERM_EXEC) {
            return Err(MemoryError::PermissionDenied);
        }
        let offset = self.offset(address, output.len())?;
        output.copy_from_slice(&self.bytes[offset..offset + output.len()]);
        Ok(())
    }

    fn push_string(&mut self, cursor: u64, value: &[u8]) -> Result<u64, StackError> {
        if value.len() > 4096 {
            return Err(StackError::StringTooLong);
        }
        let size = value.len().checked_add(1).ok_or(StackError::NoSpace)? as u64;
        let next = cursor.checked_sub(size).ok_or(StackError::NoSpace)?;
        if next < STACK_BASE {
            return Err(StackError::NoSpace);
        }
        self.write(next, value).map_err(StackError::Memory)?;
        self.write(next + value.len() as u64, &[0])
            .map_err(StackError::Memory)?;
        Ok(next)
    }

    fn push_u64(&mut self, cursor: u64, value: u64) -> Result<u64, StackError> {
        let next = cursor.checked_sub(8).ok_or(StackError::NoSpace)?;
        if next < STACK_BASE {
            return Err(StackError::NoSpace);
        }
        self.write(next, &value.to_le_bytes())
            .map_err(StackError::Memory)?;
        Ok(next)
    }

    fn set_permissions(
        &mut self,
        address: u64,
        length: usize,
        permissions: u8,
    ) -> Result<(), MemoryError> {
        let offset = self.offset(address, length)?;
        if length == 0 {
            return Ok(());
        }
        let first = offset / PAGE_SIZE;
        let last = (offset + length - 1) / PAGE_SIZE;
        for page in first..=last {
            self.permissions[page] |= permissions;
        }
        Ok(())
    }

    fn would_write_execute(&self, address: u64, length: usize, permissions: u8) -> bool {
        let Ok(offset) = self.offset(address, length) else {
            return true;
        };
        if length == 0 {
            return false;
        }
        let first = offset / PAGE_SIZE;
        let last = (offset + length - 1) / PAGE_SIZE;
        (first..=last).any(|page| {
            let combined = self.permissions[page] | permissions;
            combined & PERM_WRITE != 0 && combined & PERM_EXEC != 0
        })
    }

    fn allowed(&self, address: u64, length: usize, permission: u8) -> bool {
        let Ok(offset) = self.offset(address, length) else {
            return false;
        };
        if length == 0 {
            return true;
        }
        let first = offset / PAGE_SIZE;
        let last = (offset + length - 1) / PAGE_SIZE;
        (first..=last).all(|page| self.permissions[page] & permission == permission)
    }

    fn offset(&self, address: u64, length: usize) -> Result<usize, MemoryError> {
        if address < ADDRESS_BASE {
            return Err(MemoryError::OutsideAddressSpace);
        }
        let offset = usize::try_from(address - ADDRESS_BASE).map_err(|_| MemoryError::Overflow)?;
        let end = offset.checked_add(length).ok_or(MemoryError::Overflow)?;
        if end > ADDRESS_SIZE {
            return Err(MemoryError::OutsideAddressSpace);
        }
        Ok(offset)
    }
}

impl Default for AddressSpace {
    fn default() -> Self {
        Self::new()
    }
}

const RAX: usize = 0;
const RDX: usize = 2;
const RSP: usize = 4;
const RSI: usize = 6;
const RDI: usize = 7;

pub struct Cpu {
    registers: [u64; 16],
    pub rip: u64,
    zero: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyscallRequest {
    pub number: u64,
    pub args: [u64; 6],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Continue,
    Syscall(SyscallRequest),
}

impl Cpu {
    pub const fn new(entry: u64, stack: u64) -> Self {
        let mut registers = [0; 16];
        registers[RSP] = stack;
        Self {
            registers,
            rip: entry,
            zero: false,
        }
    }

    pub fn set_result(&mut self, result: i64) {
        self.registers[RAX] = result as u64;
    }

    pub fn step(&mut self, memory: &AddressSpace) -> Result<Step, CpuError> {
        let mut bytes = [0u8; 15];
        memory
            .fetch(self.rip, &mut bytes)
            .map_err(CpuError::Memory)?;
        let mut index = 0usize;
        let rex = if bytes[index] & 0xf0 == 0x40 {
            let value = bytes[index];
            index += 1;
            value
        } else {
            0
        };
        let opcode = bytes[index];
        index += 1;
        match opcode {
            0x90 => {}
            0xb8..=0xbf if rex & 8 != 0 => {
                let register = (opcode - 0xb8) as usize + if rex & 1 != 0 { 8 } else { 0 };
                self.registers[register] =
                    u64::from_le_bytes(bytes[index..index + 8].try_into().unwrap());
                index += 8;
            }
            0xc7 if rex & 8 != 0 => {
                let modrm = bytes[index];
                index += 1;
                if modrm & 0xf8 != 0xc0 {
                    return Err(CpuError::UnsupportedInstruction);
                }
                let register = (modrm & 7) as usize + if rex & 1 != 0 { 8 } else { 0 };
                let immediate =
                    i32::from_le_bytes(bytes[index..index + 4].try_into().unwrap()) as i64;
                self.registers[register] = immediate as u64;
                index += 4;
            }
            0x8b if rex & 8 != 0 => {
                let modrm = bytes[index];
                index += 1;
                let destination = ((modrm >> 3) & 7) as usize + if rex & 4 != 0 { 8 } else { 0 };
                let address = self.decode_address(modrm, &bytes, &mut index, rex)?;
                self.registers[destination] = memory.read_u64(address).map_err(CpuError::Memory)?;
            }
            0x31 if rex & 8 != 0 => {
                let modrm = bytes[index];
                index += 1;
                if modrm >> 6 != 3 {
                    return Err(CpuError::UnsupportedInstruction);
                }
                let source = ((modrm >> 3) & 7) as usize + if rex & 4 != 0 { 8 } else { 0 };
                let destination = (modrm & 7) as usize + if rex & 1 != 0 { 8 } else { 0 };
                let value = self.registers[destination] ^ self.registers[source];
                self.registers[destination] = value;
                self.zero = value == 0;
            }
            0x3d if rex & 8 != 0 => {
                let immediate =
                    i32::from_le_bytes(bytes[index..index + 4].try_into().unwrap()) as i64 as u64;
                index += 4;
                self.zero = self.registers[RAX] == immediate;
            }
            0x83 if rex & 8 != 0 => {
                let modrm = bytes[index];
                index += 1;
                if modrm >> 3 != 0x1f {
                    return Err(CpuError::UnsupportedInstruction);
                }
                let register = (modrm & 7) as usize + if rex & 1 != 0 { 8 } else { 0 };
                let immediate = bytes[index] as i8 as i64 as u64;
                index += 1;
                self.zero = self.registers[register] == immediate;
            }
            0x74 | 0x75 => {
                let displacement = bytes[index] as i8 as i64;
                index += 1;
                let take = (opcode == 0x74 && self.zero) || (opcode == 0x75 && !self.zero);
                if take {
                    self.rip = (self.rip as i64 + index as i64 + displacement) as u64;
                    return Ok(Step::Continue);
                }
            }
            0x0f if bytes[index] == 0x05 => {
                index += 1;
                self.rip += index as u64;
                return Ok(Step::Syscall(SyscallRequest {
                    number: self.registers[RAX],
                    args: [
                        self.registers[RDI],
                        self.registers[RSI],
                        self.registers[RDX],
                        self.registers[10],
                        self.registers[8],
                        self.registers[9],
                    ],
                }));
            }
            _ => return Err(CpuError::UnsupportedInstruction),
        }
        self.rip += index as u64;
        Ok(Step::Continue)
    }

    fn decode_address(
        &self,
        modrm: u8,
        bytes: &[u8; 15],
        index: &mut usize,
        rex: u8,
    ) -> Result<u64, CpuError> {
        let mode = modrm >> 6;
        let rm = modrm & 7;
        if mode == 3 {
            return Err(CpuError::UnsupportedInstruction);
        }
        let mut address;
        if rm == 4 {
            let sib = bytes[*index];
            *index += 1;
            if sib & 0x38 != 0x20 {
                return Err(CpuError::UnsupportedInstruction);
            }
            let base = (sib & 7) as usize + if rex & 1 != 0 { 8 } else { 0 };
            address = self.registers[base];
        } else {
            let base = rm as usize + if rex & 1 != 0 { 8 } else { 0 };
            address = self.registers[base];
        }
        match mode {
            0 => {}
            1 => {
                let displacement = bytes[*index] as i8 as i64;
                *index += 1;
                address = (address as i64 + displacement) as u64;
            }
            2 => {
                let displacement =
                    i32::from_le_bytes(bytes[*index..*index + 4].try_into().unwrap()) as i64;
                *index += 4;
                address = (address as i64 + displacement) as u64;
            }
            _ => return Err(CpuError::UnsupportedInstruction),
        }
        Ok(address)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditAction {
    Emulate,
    Mediate,
    Deny,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditRecord {
    pub sequence: u32,
    pub syscall: u64,
    pub action: AuditAction,
    pub result: i64,
}

const EMPTY_AUDIT: AuditRecord = AuditRecord {
    sequence: 0,
    syscall: 0,
    action: AuditAction::Deny,
    result: 0,
};

pub struct ExecutionReport {
    pub exit_code: i32,
    pub output: [u8; MAX_OUTPUT],
    pub output_len: usize,
    pub audit: [AuditRecord; MAX_AUDIT],
    pub audit_len: usize,
    pub steps: u64,
}

pub fn execute(
    memory: &AddressSpace,
    entry: u64,
    stack: u64,
    step_limit: u64,
) -> Result<ExecutionReport, RunError> {
    let mut cpu = Cpu::new(entry, stack);
    let mut report = ExecutionReport {
        exit_code: -1,
        output: [0; MAX_OUTPUT],
        output_len: 0,
        audit: [EMPTY_AUDIT; MAX_AUDIT],
        audit_len: 0,
        steps: 0,
    };
    while report.steps < step_limit {
        report.steps += 1;
        let request = match cpu.step(memory).map_err(RunError::Cpu)? {
            Step::Continue => continue,
            Step::Syscall(request) => request,
        };
        let decision = decide(request.number);
        let (action, result) = match decision.action {
            Action::Emulate if request.number == SYS_GETPID => {
                (AuditAction::Emulate, VIRTUAL_PID as i64)
            }
            Action::Emulate if request.number == SYS_EXIT => {
                let result = request.args[0] as i32 as i64;
                push_audit(&mut report, request.number, AuditAction::Emulate, result)?;
                report.exit_code = result as i32;
                return Ok(report);
            }
            Action::Mediate if request.number == SYS_WRITE => {
                let length = usize::try_from(request.args[2]).unwrap_or(usize::MAX);
                if request.args[0] != 1 || length > MAX_OUTPUT - report.output_len {
                    (AuditAction::Deny, -22)
                } else {
                    let end = report.output_len + length;
                    match memory.read(request.args[1], &mut report.output[report.output_len..end]) {
                        Ok(()) => {
                            report.output_len = end;
                            (AuditAction::Mediate, length as i64)
                        }
                        Err(_) => (AuditAction::Deny, -14),
                    }
                }
            }
            Action::Deny(errno) => (AuditAction::Deny, -(errno as i64)),
            _ => (AuditAction::Deny, -38),
        };
        push_audit(&mut report, request.number, action, result)?;
        cpu.set_result(result);
    }
    Err(RunError::Cpu(CpuError::StepLimit))
}

fn push_audit(
    report: &mut ExecutionReport,
    syscall: u64,
    action: AuditAction,
    result: i64,
) -> Result<(), RunError> {
    if report.audit_len == MAX_AUDIT {
        return Err(RunError::AuditOverflow);
    }
    report.audit[report.audit_len] = AuditRecord {
        sequence: report.audit_len as u32 + 1,
        syscall,
        action,
        result,
    };
    report.audit_len += 1;
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, LoadError> {
    let value = bytes.get(offset..offset + 2).ok_or(LoadError::Truncated)?;
    Ok(u16::from_le_bytes(value.try_into().unwrap()))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, LoadError> {
    let value = bytes.get(offset..offset + 4).ok_or(LoadError::Truncated)?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, LoadError> {
    let value = bytes.get(offset..offset + 8).ok_or(LoadError::Truncated)?;
    Ok(u64::from_le_bytes(value.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elf_with(code: &[u8], flags: u32) -> [u8; 512] {
        let mut elf = [0u8; 512];
        elf[0..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2;
        elf[5] = 1;
        elf[6] = 1;
        elf[16..18].copy_from_slice(&2u16.to_le_bytes());
        elf[18..20].copy_from_slice(&62u16.to_le_bytes());
        elf[20..24].copy_from_slice(&1u32.to_le_bytes());
        elf[24..32].copy_from_slice(&0x2_0000u64.to_le_bytes());
        elf[32..40].copy_from_slice(&64u64.to_le_bytes());
        elf[52..54].copy_from_slice(&64u16.to_le_bytes());
        elf[54..56].copy_from_slice(&56u16.to_le_bytes());
        elf[56..58].copy_from_slice(&1u16.to_le_bytes());
        elf[64..68].copy_from_slice(&1u32.to_le_bytes());
        elf[68..72].copy_from_slice(&flags.to_le_bytes());
        elf[72..80].copy_from_slice(&0x100u64.to_le_bytes());
        elf[80..88].copy_from_slice(&0x2_0000u64.to_le_bytes());
        elf[96..104].copy_from_slice(&(code.len() as u64).to_le_bytes());
        elf[104..112].copy_from_slice(&(code.len() as u64).to_le_bytes());
        elf[112..120].copy_from_slice(&0x1000u64.to_le_bytes());
        elf[0x100..0x100 + code.len()].copy_from_slice(code);
        elf
    }

    #[test]
    fn loads_executes_and_audits_static_elf() {
        let code = [
            0x48, 0xc7, 0xc0, 39, 0, 0, 0, 0x0f, 0x05, 0x48, 0xc7, 0xc0, 60, 0, 0, 0, 0x48, 0x31,
            0xff, 0x0f, 0x05,
        ];
        let elf = elf_with(&code, 5);
        let mut memory = AddressSpace::new();
        let loaded = memory.load_elf(&elf).unwrap();
        let stack = memory
            .build_stack(&[b"fixture", b"arg"], &[b"KEY=value"])
            .unwrap();
        assert_eq!(memory.read_u64(stack).unwrap(), 2);
        let report = execute(&memory, loaded.entry, stack, 100).unwrap();
        assert_eq!(report.exit_code, 0);
        assert_eq!(report.audit[0].syscall, SYS_GETPID);
        assert_eq!(report.audit[0].result, VIRTUAL_PID as i64);
    }

    #[test]
    fn rejects_malformed_and_write_execute_elfs() {
        let mut memory = AddressSpace::new();
        assert_eq!(memory.load_elf(b"bad"), Err(LoadError::Truncated));
        let elf = elf_with(&[0x90], 7);
        assert_eq!(memory.load_elf(&elf), Err(LoadError::WriteExecuteSegment));
    }

    #[test]
    fn enforces_copy_and_mapping_boundaries() {
        let mut memory = AddressSpace::new();
        let elf = elf_with(&[0x90], 5);
        memory.load_elf(&elf).unwrap();
        assert_eq!(
            memory.write(0x2_0000, &[1]),
            Err(MemoryError::PermissionDenied)
        );
        let mut byte = [0];
        assert_eq!(
            memory.read(ADDRESS_END, &mut byte),
            Err(MemoryError::PermissionDenied)
        );
        let stack = memory
            .build_stack(&[b"fixture", b"argument"], &[b"KEY=value"])
            .unwrap();
        assert_eq!(memory.read_u64(stack).unwrap(), 2);
        let argument = memory.read_u64(stack + 16).unwrap();
        let environment = memory.read_u64(stack + 32).unwrap();
        let mut argument_bytes = [0u8; 8];
        let mut environment_bytes = [0u8; 9];
        memory.read(argument, &mut argument_bytes).unwrap();
        memory.read(environment, &mut environment_bytes).unwrap();
        assert_eq!(&argument_bytes, b"argument");
        assert_eq!(&environment_bytes, b"KEY=value");
    }

    #[test]
    fn unknown_syscall_is_denied_without_host_execution() {
        let code = [
            0x48, 0xc7, 0xc0, 0xe7, 3, 0, 0, 0x0f, 0x05, 0x48, 0xc7, 0xc0, 60, 0, 0, 0, 0x48, 0x31,
            0xff, 0x0f, 0x05,
        ];
        let mut memory = AddressSpace::new();
        let loaded = memory.load_elf(&elf_with(&code, 5)).unwrap();
        let stack = memory.build_stack(&[b"fixture"], &[]).unwrap();
        let report = execute(&memory, loaded.entry, stack, 100).unwrap();
        assert_eq!(report.audit[0].syscall, 999);
        assert_eq!(report.audit[0].action, AuditAction::Deny);
        assert_eq!(report.audit[0].result, -38);
    }

    #[test]
    fn invalid_pointer_and_oversized_output_are_denied() {
        let mut code = [0u8; 96];
        let program = [
            0x48, 0xc7, 0xc0, 1, 0, 0, 0, 0x48, 0xc7, 0xc7, 1, 0, 0, 0, 0x48, 0xbe,
        ];
        code[..program.len()].copy_from_slice(&program);
        code[program.len()..program.len() + 8].copy_from_slice(&ADDRESS_END.to_le_bytes());
        let tail = [
            0x48, 0xc7, 0xc2, 1, 0, 0, 0, 0x0f, 0x05, 0x48, 0xc7, 0xc0, 1, 0, 0, 0, 0x48, 0xc7,
            0xc7, 1, 0, 0, 0, 0x48, 0xbe,
        ];
        let tail_start = program.len() + 8;
        code[tail_start..tail_start + tail.len()].copy_from_slice(&tail);
        let address_start = tail_start + tail.len();
        code[address_start..address_start + 8].copy_from_slice(&0x2_0000u64.to_le_bytes());
        let final_code = [
            0x48, 0xc7, 0xc2, 0x88, 0x13, 0, 0, 0x0f, 0x05, 0x48, 0xc7, 0xc0, 60, 0, 0, 0, 0x48,
            0x31, 0xff, 0x0f, 0x05,
        ];
        let final_start = address_start + 8;
        code[final_start..final_start + final_code.len()].copy_from_slice(&final_code);
        let code_len = final_start + final_code.len();

        let mut memory = AddressSpace::new();
        let loaded = memory.load_elf(&elf_with(&code[..code_len], 5)).unwrap();
        let stack = memory.build_stack(&[b"fixture"], &[]).unwrap();
        let report = execute(&memory, loaded.entry, stack, 100).unwrap();
        assert_eq!(report.audit[0].result, -14);
        assert_eq!(report.audit[1].result, -22);
        assert_eq!(report.output_len, 0);
    }

    #[test]
    fn invalid_instruction_fails_deterministically() {
        let mut memory = AddressSpace::new();
        let loaded = memory.load_elf(&elf_with(&[0xcc], 5)).unwrap();
        let stack = memory.build_stack(&[b"fixture"], &[]).unwrap();
        assert!(matches!(
            execute(&memory, loaded.entry, stack, 10),
            Err(RunError::Cpu(CpuError::UnsupportedInstruction))
        ));
    }
}
