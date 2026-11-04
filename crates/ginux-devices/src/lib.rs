#![no_std]

pub const MAX_DEVICE_NAME: usize = 16;
pub const MAX_FRAME_SIZE: usize = 2048;
pub const IOCTL_CONSOLE_GET_SIZE: u64 = 0x5413;
pub const IOCTL_BLOCK_GET_SIZE: u64 = 0x8008_1272;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceError {
    InvalidName,
    UnknownDevice,
    NotAssigned,
    AssignmentConflict,
    RegistryFull,
    DescriptorLimit,
    InvalidDescriptor,
    PermissionDenied,
    InvalidPayload,
    UnknownIoctl,
    DirectMappingDenied,
    OutOfRange,
    ReadOnly,
    Backend,
    Closed,
    FrameTooLarge,
    QueueFull,
    QueueEmpty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceKind {
    Console,
    Null,
    Zero,
    Random,
    Block,
    Network,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceName {
    bytes: [u8; MAX_DEVICE_NAME],
    len: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceView {
    pub name: DeviceName,
    pub kind: DeviceKind,
    pub major: u32,
    pub minor: u32,
}

impl DeviceName {
    pub fn new(value: &[u8]) -> Result<Self, DeviceError> {
        if value.is_empty()
            || value.len() > MAX_DEVICE_NAME
            || !value
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(DeviceError::InvalidName);
        }
        let mut name = Self {
            bytes: [0; MAX_DEVICE_NAME],
            len: value.len() as u8,
        };
        name.bytes[..value.len()].copy_from_slice(value);
        Ok(name)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

#[derive(Clone, Copy)]
struct Assignment {
    active: bool,
    domain: u64,
    backend: u64,
    name: DeviceName,
    kind: DeviceKind,
}

impl Assignment {
    const fn empty() -> Self {
        Self {
            active: false,
            domain: 0,
            backend: 0,
            name: DeviceName {
                bytes: [0; MAX_DEVICE_NAME],
                len: 0,
            },
            kind: DeviceKind::Null,
        }
    }
}

#[derive(Clone, Copy)]
struct Descriptor {
    open: bool,
    domain: u64,
    assignment: usize,
}

impl Descriptor {
    const fn empty() -> Self {
        Self {
            open: false,
            domain: 0,
            assignment: 0,
        }
    }
}

pub struct DeviceRegistry<const DEVICES: usize, const FDS: usize> {
    assignments: [Assignment; DEVICES],
    descriptors: [Descriptor; FDS],
    random_state: u64,
}

impl<const DEVICES: usize, const FDS: usize> Default for DeviceRegistry<DEVICES, FDS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const DEVICES: usize, const FDS: usize> DeviceRegistry<DEVICES, FDS> {
    pub const fn new() -> Self {
        Self {
            assignments: [Assignment::empty(); DEVICES],
            descriptors: [Descriptor::empty(); FDS],
            random_state: 0x6a09_e667_f3bc_c909,
        }
    }

    pub fn assign(
        &mut self,
        domain: u64,
        backend: u64,
        name: DeviceName,
        kind: DeviceKind,
    ) -> Result<(), DeviceError> {
        if self.assignments.iter().any(|entry| {
            entry.active
                && (entry.backend == backend
                    || (entry.domain == domain && entry.name.as_bytes() == name.as_bytes()))
        }) {
            return Err(DeviceError::AssignmentConflict);
        }
        let slot = self
            .assignments
            .iter_mut()
            .find(|entry| !entry.active)
            .ok_or(DeviceError::RegistryFull)?;
        *slot = Assignment {
            active: true,
            domain,
            backend,
            name,
            kind,
        };
        Ok(())
    }

    pub fn visible(&self, domain: u64, output: &mut [DeviceName]) -> usize {
        let mut count = 0;
        for entry in self
            .assignments
            .iter()
            .filter(|entry| entry.active && entry.domain == domain)
        {
            if count == output.len() {
                break;
            }
            output[count] = entry.name;
            count += 1;
        }
        count
    }

    pub fn views(&self, domain: u64, output: &mut [DeviceView]) -> usize {
        let mut count = 0usize;
        for (index, entry) in self
            .assignments
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.active && entry.domain == domain)
        {
            if count == output.len() {
                break;
            }
            let (major, minor) = match entry.kind {
                DeviceKind::Console => (5, 1),
                DeviceKind::Null => (1, 3),
                DeviceKind::Zero => (1, 5),
                DeviceKind::Random => (1, 8),
                DeviceKind::Block => (254, index as u32),
                DeviceKind::Network => (0, index as u32 + 1),
            };
            output[count] = DeviceView {
                name: entry.name,
                kind: entry.kind,
                major,
                minor,
            };
            count += 1;
        }
        count
    }

    pub fn open(&mut self, domain: u64, name: &[u8]) -> Result<u32, DeviceError> {
        let assignment = self
            .assignments
            .iter()
            .position(|entry| {
                entry.active && entry.domain == domain && entry.name.as_bytes() == name
            })
            .ok_or(DeviceError::NotAssigned)?;
        let descriptor = self
            .descriptors
            .iter_mut()
            .position(|entry| !entry.open)
            .ok_or(DeviceError::DescriptorLimit)?;
        self.descriptors[descriptor] = Descriptor {
            open: true,
            domain,
            assignment,
        };
        Ok(0x4000_0000 | descriptor as u32)
    }

    fn descriptor(&self, domain: u64, fd: u32) -> Result<&Assignment, DeviceError> {
        if fd & 0x4000_0000 == 0 {
            return Err(DeviceError::InvalidDescriptor);
        }
        let index = (fd & 0x3fff_ffff) as usize;
        let descriptor = self
            .descriptors
            .get(index)
            .filter(|entry| entry.open && entry.domain == domain)
            .ok_or(DeviceError::InvalidDescriptor)?;
        self.assignments
            .get(descriptor.assignment)
            .filter(|entry| entry.active && entry.domain == domain)
            .ok_or(DeviceError::InvalidDescriptor)
    }

    pub fn read(&mut self, domain: u64, fd: u32, output: &mut [u8]) -> Result<usize, DeviceError> {
        let kind = self.descriptor(domain, fd)?.kind;
        match kind {
            DeviceKind::Null | DeviceKind::Console => Ok(0),
            DeviceKind::Zero => {
                output.fill(0);
                Ok(output.len())
            }
            DeviceKind::Random => {
                for byte in output.iter_mut() {
                    self.random_state ^= self.random_state << 13;
                    self.random_state ^= self.random_state >> 7;
                    self.random_state ^= self.random_state << 17;
                    *byte = self.random_state as u8;
                }
                Ok(output.len())
            }
            DeviceKind::Block | DeviceKind::Network => Err(DeviceError::PermissionDenied),
        }
    }

    pub fn write(&self, domain: u64, fd: u32, input: &[u8]) -> Result<usize, DeviceError> {
        match self.descriptor(domain, fd)?.kind {
            DeviceKind::Null | DeviceKind::Console => Ok(input.len()),
            DeviceKind::Zero | DeviceKind::Random | DeviceKind::Block | DeviceKind::Network => {
                Err(DeviceError::PermissionDenied)
            }
        }
    }

    pub fn ioctl(
        &self,
        domain: u64,
        fd: u32,
        request: u64,
        payload: &mut [u8],
    ) -> Result<(), DeviceError> {
        let kind = self.descriptor(domain, fd)?.kind;
        match (kind, request) {
            (DeviceKind::Console, IOCTL_CONSOLE_GET_SIZE) if payload.len() == 8 => {
                payload.copy_from_slice(&[24, 0, 80, 0, 0, 0, 0, 0]);
                Ok(())
            }
            (DeviceKind::Console, IOCTL_CONSOLE_GET_SIZE) => Err(DeviceError::InvalidPayload),
            _ => Err(DeviceError::UnknownIoctl),
        }
    }

    pub fn map(&self, domain: u64, fd: u32) -> Result<(), DeviceError> {
        self.descriptor(domain, fd)?;
        Err(DeviceError::DirectMappingDenied)
    }

    pub fn close(&mut self, domain: u64, fd: u32) -> Result<(), DeviceError> {
        self.descriptor(domain, fd)?;
        let index = (fd & 0x3fff_ffff) as usize;
        self.descriptors[index] = Descriptor::empty();
        Ok(())
    }

    pub fn revoke(&mut self, domain: u64) {
        for descriptor in &mut self.descriptors {
            if descriptor.open && descriptor.domain == domain {
                *descriptor = Descriptor::empty();
            }
        }
        for assignment in &mut self.assignments {
            if assignment.active && assignment.domain == domain {
                *assignment = Assignment::empty();
            }
        }
    }
}

pub trait BlockBackend {
    fn len(&self) -> u64;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), DeviceError>;
    fn write_at(&mut self, offset: u64, input: &[u8]) -> Result<(), DeviceError>;
    fn flush(&mut self) -> Result<(), DeviceError>;
    fn close(&mut self) -> Result<(), DeviceError>;
}

pub struct Partition<B> {
    backend: B,
    start: u64,
    len: u64,
    read_only: bool,
    closed: bool,
}

impl<B: BlockBackend> Partition<B> {
    pub fn new(backend: B, start: u64, len: u64, read_only: bool) -> Result<Self, DeviceError> {
        let end = start.checked_add(len).ok_or(DeviceError::OutOfRange)?;
        if len == 0 || end > backend.len() {
            return Err(DeviceError::OutOfRange);
        }
        Ok(Self {
            backend,
            start,
            len,
            read_only,
            closed: false,
        })
    }

    fn range(&self, offset: u64, size: usize) -> Result<u64, DeviceError> {
        if self.closed {
            return Err(DeviceError::Closed);
        }
        let end = offset
            .checked_add(size as u64)
            .ok_or(DeviceError::OutOfRange)?;
        if end > self.len {
            return Err(DeviceError::OutOfRange);
        }
        self.start
            .checked_add(offset)
            .ok_or(DeviceError::OutOfRange)
    }

    pub fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), DeviceError> {
        let absolute = self.range(offset, output.len())?;
        self.backend.read_at(absolute, output)
    }

    pub fn write_at(&mut self, offset: u64, input: &[u8]) -> Result<(), DeviceError> {
        if self.read_only {
            return Err(DeviceError::ReadOnly);
        }
        let absolute = self.range(offset, input.len())?;
        self.backend.write_at(absolute, input)
    }

    pub fn size(&self) -> u64 {
        self.len
    }

    pub fn flush(&mut self) -> Result<(), DeviceError> {
        if self.closed {
            return Err(DeviceError::Closed);
        }
        self.backend.flush()
    }

    pub fn close(&mut self) -> Result<(), DeviceError> {
        if self.closed {
            return Err(DeviceError::Closed);
        }
        self.backend.flush()?;
        self.backend.close()?;
        self.closed = true;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Frame {
    len: usize,
    bytes: [u8; MAX_FRAME_SIZE],
}

impl Frame {
    const fn empty() -> Self {
        Self {
            len: 0,
            bytes: [0; MAX_FRAME_SIZE],
        }
    }
}

pub struct IsolatedNetwork<const FRAMES: usize> {
    queue: [Frame; FRAMES],
    head: usize,
    len: usize,
    closed: bool,
}

impl<const FRAMES: usize> Default for IsolatedNetwork<FRAMES> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const FRAMES: usize> IsolatedNetwork<FRAMES> {
    pub const fn new() -> Self {
        Self {
            queue: [Frame::empty(); FRAMES],
            head: 0,
            len: 0,
            closed: false,
        }
    }

    pub fn send(&mut self, frame: &[u8]) -> Result<(), DeviceError> {
        if self.closed {
            return Err(DeviceError::Closed);
        }
        if frame.is_empty() || frame.len() > MAX_FRAME_SIZE {
            return Err(DeviceError::FrameTooLarge);
        }
        if self.len == FRAMES {
            return Err(DeviceError::QueueFull);
        }
        let index = (self.head + self.len) % FRAMES;
        self.queue[index].bytes[..frame.len()].copy_from_slice(frame);
        self.queue[index].len = frame.len();
        self.len += 1;
        Ok(())
    }

    pub fn receive(&mut self, output: &mut [u8]) -> Result<usize, DeviceError> {
        if self.closed {
            return Err(DeviceError::Closed);
        }
        if self.len == 0 {
            return Err(DeviceError::QueueEmpty);
        }
        let frame = &self.queue[self.head];
        if output.len() < frame.len {
            return Err(DeviceError::InvalidPayload);
        }
        output[..frame.len].copy_from_slice(&frame.bytes[..frame.len]);
        let size = frame.len;
        self.queue[self.head] = Frame::empty();
        self.head = (self.head + 1) % FRAMES;
        self.len -= 1;
        Ok(size)
    }

    pub fn control(&self, _request: u64) -> Result<(), DeviceError> {
        Err(DeviceError::UnknownIoctl)
    }

    pub fn close(&mut self) {
        self.queue.fill(Frame::empty());
        self.head = 0;
        self.len = 0;
        self.closed = true;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;
    use std::vec::Vec;

    struct MemoryBlock {
        bytes: Vec<u8>,
        flushes: usize,
        closed: bool,
    }

    impl BlockBackend for MemoryBlock {
        fn len(&self) -> u64 {
            self.bytes.len() as u64
        }
        fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), DeviceError> {
            let start = offset as usize;
            let end = start
                .checked_add(output.len())
                .ok_or(DeviceError::OutOfRange)?;
            let source = self.bytes.get(start..end).ok_or(DeviceError::OutOfRange)?;
            output.copy_from_slice(source);
            Ok(())
        }
        fn write_at(&mut self, offset: u64, input: &[u8]) -> Result<(), DeviceError> {
            let start = offset as usize;
            let end = start
                .checked_add(input.len())
                .ok_or(DeviceError::OutOfRange)?;
            let destination = self
                .bytes
                .get_mut(start..end)
                .ok_or(DeviceError::OutOfRange)?;
            destination.copy_from_slice(input);
            Ok(())
        }
        fn flush(&mut self) -> Result<(), DeviceError> {
            self.flushes += 1;
            Ok(())
        }
        fn close(&mut self) -> Result<(), DeviceError> {
            self.closed = true;
            Ok(())
        }
    }

    fn block() -> MemoryBlock {
        MemoryBlock {
            bytes: vec![0; 64],
            flushes: 0,
            closed: false,
        }
    }

    #[test]
    fn registry_projects_only_assigned_devices_and_uses_virtual_handles() {
        let mut registry: DeviceRegistry<8, 8> = DeviceRegistry::new();
        registry
            .assign(1, 10, DeviceName::new(b"null").unwrap(), DeviceKind::Null)
            .unwrap();
        registry
            .assign(2, 11, DeviceName::new(b"zero").unwrap(), DeviceKind::Zero)
            .unwrap();
        let mut visible = [DeviceName::new(b"unused").unwrap(); 4];
        assert_eq!(registry.visible(1, &mut visible), 1);
        assert_eq!(visible[0].as_bytes(), b"null");
        assert_eq!(registry.open(1, b"zero"), Err(DeviceError::NotAssigned));
        let fd = registry.open(1, b"null").unwrap();
        assert_eq!(fd, 0x4000_0000);
        assert_eq!(registry.write(1, fd, b"discard"), Ok(7));
        assert_eq!(
            registry.write(2, fd, b"escape"),
            Err(DeviceError::InvalidDescriptor)
        );
    }

    #[test]
    fn pseudo_devices_and_console_are_ginux_owned() {
        let mut registry: DeviceRegistry<8, 8> = DeviceRegistry::new();
        for (backend, name, kind) in [
            (1, b"console".as_slice(), DeviceKind::Console),
            (2, b"null", DeviceKind::Null),
            (3, b"zero", DeviceKind::Zero),
            (4, b"random", DeviceKind::Random),
        ] {
            registry
                .assign(7, backend, DeviceName::new(name).unwrap(), kind)
                .unwrap();
        }
        let zero = registry.open(7, b"zero").unwrap();
        let mut bytes = [9; 8];
        assert_eq!(registry.read(7, zero, &mut bytes), Ok(8));
        assert_eq!(bytes, [0; 8]);
        let random = registry.open(7, b"random").unwrap();
        registry.read(7, random, &mut bytes).unwrap();
        assert_ne!(bytes, [0; 8]);
        let console = registry.open(7, b"console").unwrap();
        let mut size = [0; 8];
        registry
            .ioctl(7, console, IOCTL_CONSOLE_GET_SIZE, &mut size)
            .unwrap();
        assert_eq!(&size[..4], &[24, 0, 80, 0]);
    }

    #[test]
    fn conflicts_ioctls_mappings_and_revoke_fail_closed() {
        let mut registry: DeviceRegistry<4, 4> = DeviceRegistry::new();
        registry
            .assign(
                1,
                77,
                DeviceName::new(b"console").unwrap(),
                DeviceKind::Console,
            )
            .unwrap();
        assert_eq!(
            registry.assign(
                2,
                77,
                DeviceName::new(b"tty0").unwrap(),
                DeviceKind::Console
            ),
            Err(DeviceError::AssignmentConflict)
        );
        let fd = registry.open(1, b"console").unwrap();
        assert_eq!(
            registry.ioctl(1, fd, IOCTL_CONSOLE_GET_SIZE, &mut [0; 7]),
            Err(DeviceError::InvalidPayload)
        );
        assert_eq!(
            registry.ioctl(1, fd, 0xdead, &mut []),
            Err(DeviceError::UnknownIoctl)
        );
        assert_eq!(registry.map(1, fd), Err(DeviceError::DirectMappingDenied));
        registry.revoke(1);
        assert_eq!(
            registry.write(1, fd, b"x"),
            Err(DeviceError::InvalidDescriptor)
        );
        registry
            .assign(
                2,
                77,
                DeviceName::new(b"tty0").unwrap(),
                DeviceKind::Console,
            )
            .unwrap();
    }

    #[test]
    fn block_partition_confines_reads_and_writes() {
        let mut partition = Partition::new(block(), 16, 16, false).unwrap();
        partition.write_at(0, b"inside").unwrap();
        let mut value = [0; 6];
        partition.read_at(0, &mut value).unwrap();
        assert_eq!(&value, b"inside");
        assert_eq!(partition.write_at(15, b"xx"), Err(DeviceError::OutOfRange));
        assert_eq!(
            partition.read_at(16, &mut [0; 1]),
            Err(DeviceError::OutOfRange)
        );
    }

    #[test]
    fn block_read_only_flush_and_close_policy() {
        let mut read_only = Partition::new(block(), 0, 32, true).unwrap();
        assert_eq!(read_only.write_at(0, b"x"), Err(DeviceError::ReadOnly));
        read_only.flush().unwrap();
        read_only.close().unwrap();
        assert_eq!(read_only.read_at(0, &mut [0; 1]), Err(DeviceError::Closed));
        assert_eq!(read_only.close(), Err(DeviceError::Closed));
    }

    #[test]
    fn network_is_isolated_bounded_and_revocable() {
        let mut network: IsolatedNetwork<2> = IsolatedNetwork::new();
        network.send(b"packet-one").unwrap();
        network.send(b"packet-two").unwrap();
        assert_eq!(network.send(b"packet-three"), Err(DeviceError::QueueFull));
        let mut output = [0; 32];
        let size = network.receive(&mut output).unwrap();
        assert_eq!(&output[..size], b"packet-one");
        assert_eq!(network.control(1), Err(DeviceError::UnknownIoctl));
        network.close();
        assert_eq!(network.send(b"after-close"), Err(DeviceError::Closed));
    }
}
