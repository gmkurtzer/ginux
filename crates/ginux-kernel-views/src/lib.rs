#![no_std]

use ginux_devices::{DeviceKind, DeviceView};
use ginux_process::{TaskState, TaskView};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewError {
    NotFound,
    PermissionDenied,
    ReadOnly,
    BufferTooSmall,
    InvalidPath,
}

#[derive(Clone, Copy)]
pub struct MemoryView {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Clone, Copy)]
pub struct IdentityView<'a> {
    pub hostname: &'a [u8],
    pub boot_id: &'a [u8],
}

#[derive(Clone, Copy)]
pub struct MountView<'a> {
    pub source: &'a [u8],
    pub target: &'a [u8],
    pub filesystem: &'a [u8],
    pub options: &'a [u8],
}

pub struct ProcFs<'a> {
    tasks: &'a [TaskView],
    self_pid: u32,
    uptime_ns: u64,
    memory: MemoryView,
    identity: IdentityView<'a>,
    mounts: &'a [MountView<'a>],
}

pub struct SysFs<'a> {
    devices: &'a [DeviceView],
    cpu_count: u32,
}

impl<'a> SysFs<'a> {
    pub const fn new(devices: &'a [DeviceView], cpu_count: u32) -> Self {
        Self { devices, cpu_count }
    }

    pub fn list_devices(&self, output: &mut [DeviceView]) -> usize {
        let count = core::cmp::min(output.len(), self.devices.len());
        output[..count].copy_from_slice(&self.devices[..count]);
        count
    }

    pub fn read(&self, path: &[u8], output: &mut [u8]) -> Result<usize, ViewError> {
        let mut writer = Writer::new(output);
        if path == b"/sys/devices/system/cpu/online" {
            if self.cpu_count == 0 {
                return Err(ViewError::NotFound);
            }
            writer.byte(b'0')?;
            if self.cpu_count > 1 {
                writer.byte(b'-')?;
                writer.decimal(u64::from(self.cpu_count - 1))?;
            }
            writer.byte(b'\n')?;
            return Ok(writer.len);
        }
        let rest = path
            .strip_prefix(b"/sys/class/")
            .ok_or(ViewError::InvalidPath)?;
        let first = rest
            .iter()
            .position(|byte| *byte == b'/')
            .ok_or(ViewError::InvalidPath)?;
        let class = &rest[..first];
        let remaining = &rest[first + 1..];
        let second = remaining
            .iter()
            .position(|byte| *byte == b'/')
            .ok_or(ViewError::InvalidPath)?;
        let name = &remaining[..second];
        let leaf = &remaining[second + 1..];
        let device = self
            .devices
            .iter()
            .find(|device| device.name.as_bytes() == name && device_class(device.kind) == class)
            .ok_or(ViewError::NotFound)?;
        match leaf {
            b"dev" if device.kind != DeviceKind::Network => {
                writer.decimal(u64::from(device.major))?;
                writer.byte(b':')?;
                writer.decimal(u64::from(device.minor))?;
                writer.byte(b'\n')?;
            }
            b"type" if device.kind == DeviceKind::Network => {
                writer.bytes(b"1\n")?;
            }
            b"uevent" => {
                writer.bytes(b"DEVNAME=")?;
                writer.bytes(device.name.as_bytes())?;
                writer.bytes(b"\nDEVTYPE=")?;
                writer.bytes(device_type(device.kind))?;
                writer.byte(b'\n')?;
            }
            _ => return Err(ViewError::NotFound),
        }
        Ok(writer.len)
    }

    pub fn write(&self, _path: &[u8], _input: &[u8]) -> Result<usize, ViewError> {
        Err(ViewError::ReadOnly)
    }
}

fn device_class(kind: DeviceKind) -> &'static [u8] {
    match kind {
        DeviceKind::Block => b"block",
        DeviceKind::Network => b"net",
        DeviceKind::Console => b"tty",
        DeviceKind::Null | DeviceKind::Zero | DeviceKind::Random => b"misc",
    }
}

fn device_type(kind: DeviceKind) -> &'static [u8] {
    match kind {
        DeviceKind::Block => b"disk",
        DeviceKind::Network => b"net",
        DeviceKind::Console => b"console",
        DeviceKind::Null => b"null",
        DeviceKind::Zero => b"zero",
        DeviceKind::Random => b"random",
    }
}

impl<'a> ProcFs<'a> {
    pub const fn new(
        tasks: &'a [TaskView],
        self_pid: u32,
        uptime_ns: u64,
        memory: MemoryView,
        identity: IdentityView<'a>,
        mounts: &'a [MountView<'a>],
    ) -> Self {
        Self {
            tasks,
            self_pid,
            uptime_ns,
            memory,
            identity,
            mounts,
        }
    }

    pub fn list_pids(&self, output: &mut [u32]) -> usize {
        let count = core::cmp::min(output.len(), self.tasks.len());
        for (target, task) in output[..count].iter_mut().zip(self.tasks) {
            *target = task.id.pid;
        }
        count
    }

    pub fn read(&self, path: &[u8], output: &mut [u8]) -> Result<usize, ViewError> {
        let mut writer = Writer::new(output);
        match path {
            b"/proc/uptime" => {
                writer.decimal(self.uptime_ns / 1_000_000_000)?;
                writer.byte(b'.')?;
                writer.two_digits((self.uptime_ns / 10_000_000) % 100)?;
                writer.bytes(b" 0.00\n")?;
            }
            b"/proc/meminfo" => {
                writer.bytes(b"MemTotal:       ")?;
                writer.decimal(self.memory.total_bytes / 1024)?;
                writer.bytes(b" kB\nMemAvailable:   ")?;
                writer.decimal(self.memory.available_bytes / 1024)?;
                writer.bytes(b" kB\n")?;
            }
            b"/proc/mounts" | b"/proc/self/mounts" => {
                for mount in self.mounts {
                    writer.bytes(mount.source)?;
                    writer.byte(b' ')?;
                    writer.bytes(mount.target)?;
                    writer.byte(b' ')?;
                    writer.bytes(mount.filesystem)?;
                    writer.byte(b' ')?;
                    writer.bytes(mount.options)?;
                    writer.bytes(b" 0 0\n")?;
                }
            }
            b"/proc/sys/kernel/hostname" => {
                writer.bytes(self.identity.hostname)?;
                writer.byte(b'\n')?;
            }
            b"/proc/sys/kernel/random/boot_id" => {
                writer.bytes(self.identity.boot_id)?;
                writer.byte(b'\n')?;
            }
            _ => {
                let (pid, leaf) = parse_task_path(path, self.self_pid)?;
                let task = self
                    .tasks
                    .iter()
                    .find(|task| task.id.pid == pid)
                    .ok_or(ViewError::NotFound)?;
                match leaf {
                    b"status" => write_status(*task, &mut writer)?,
                    b"stat" => write_stat(*task, &mut writer)?,
                    _ => return Err(ViewError::NotFound),
                }
            }
        }
        Ok(writer.len)
    }

    pub fn write(&self, _path: &[u8], _input: &[u8]) -> Result<usize, ViewError> {
        Err(ViewError::ReadOnly)
    }
}

fn parse_task_path(path: &[u8], self_pid: u32) -> Result<(u32, &[u8]), ViewError> {
    let rest = path.strip_prefix(b"/proc/").ok_or(ViewError::InvalidPath)?;
    let slash = rest
        .iter()
        .position(|byte| *byte == b'/')
        .ok_or(ViewError::InvalidPath)?;
    let pid = if &rest[..slash] == b"self" {
        self_pid
    } else {
        parse_u32(&rest[..slash])?
    };
    Ok((pid, &rest[slash + 1..]))
}

fn parse_u32(input: &[u8]) -> Result<u32, ViewError> {
    if input.is_empty() {
        return Err(ViewError::InvalidPath);
    }
    let mut value = 0u32;
    for byte in input {
        if !byte.is_ascii_digit() {
            return Err(ViewError::InvalidPath);
        }
        value = value
            .checked_mul(10)
            .and_then(|current| current.checked_add(u32::from(*byte - b'0')))
            .ok_or(ViewError::InvalidPath)?;
    }
    Ok(value)
}

fn write_status(task: TaskView, writer: &mut Writer<'_>) -> Result<(), ViewError> {
    writer.bytes(b"Name:\t")?;
    writer.bytes(if task.id.pid == 1 {
        b"init"
    } else {
        b"managed"
    })?;
    writer.bytes(b"\nState:\t")?;
    writer.bytes(match task.state {
        TaskState::Running => b"R (running)",
        TaskState::Exited(_) => b"Z (zombie)",
    })?;
    writer.bytes(b"\nTgid:\t")?;
    writer.decimal(u64::from(task.tgid))?;
    writer.bytes(b"\nPid:\t")?;
    writer.decimal(u64::from(task.id.pid))?;
    writer.bytes(b"\nPPid:\t")?;
    writer.decimal(u64::from(task.parent))?;
    writer.bytes(b"\nUid:\t")?;
    for _ in 0..4 {
        writer.decimal(u64::from(task.credentials.uid))?;
        writer.byte(b'\t')?;
    }
    writer.bytes(b"\nGid:\t")?;
    for _ in 0..4 {
        writer.decimal(u64::from(task.credentials.gid))?;
        writer.byte(b'\t')?;
    }
    writer.byte(b'\n')
}

fn write_stat(task: TaskView, writer: &mut Writer<'_>) -> Result<(), ViewError> {
    writer.decimal(u64::from(task.id.pid))?;
    writer.bytes(if task.id.pid == 1 {
        b" (init) "
    } else {
        b" (managed) "
    })?;
    writer.byte(match task.state {
        TaskState::Running => b'R',
        TaskState::Exited(_) => b'Z',
    })?;
    writer.byte(b' ')?;
    writer.decimal(u64::from(task.parent))?;
    writer.byte(b' ')?;
    writer.decimal(u64::from(task.process_group))?;
    writer.byte(b' ')?;
    writer.decimal(u64::from(task.session))?;
    writer.byte(b'\n')
}

struct Writer<'a> {
    output: &'a mut [u8],
    len: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        Self { output, len: 0 }
    }

    fn byte(&mut self, byte: u8) -> Result<(), ViewError> {
        if self.len == self.output.len() {
            return Err(ViewError::BufferTooSmall);
        }
        self.output[self.len] = byte;
        self.len += 1;
        Ok(())
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), ViewError> {
        let end = self
            .len
            .checked_add(bytes.len())
            .filter(|end| *end <= self.output.len())
            .ok_or(ViewError::BufferTooSmall)?;
        self.output[self.len..end].copy_from_slice(bytes);
        self.len = end;
        Ok(())
    }

    fn decimal(&mut self, mut value: u64) -> Result<(), ViewError> {
        let mut digits = [0u8; 20];
        let mut start = digits.len();
        loop {
            start -= 1;
            digits[start] = b'0' + (value % 10) as u8;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        self.bytes(&digits[start..])
    }

    fn two_digits(&mut self, value: u64) -> Result<(), ViewError> {
        self.byte(b'0' + ((value / 10) % 10) as u8)?;
        self.byte(b'0' + (value % 10) as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ginux_devices::{DeviceName, DeviceRegistry};
    use ginux_process::{Credentials, ProcessTable, TaskView};

    fn fixture() -> ([TaskView; 4], usize) {
        let mut table: ProcessTable<4> = ProcessTable::new();
        let init = table.create_init(Credentials::root()).unwrap();
        let child = table.fork(init).unwrap();
        table.credentials_mut(child).unwrap().set_uid(1000).unwrap();
        let mut views = [table.view(init).unwrap(); 4];
        let count = table.views(&mut views);
        (views, count)
    }

    #[test]
    fn procfs_exposes_only_virtual_tasks_and_identity() {
        let (tasks, count) = fixture();
        let mounts = [
            MountView {
                source: b"ginux-root",
                target: b"/",
                filesystem: b"ginuxfs",
                options: b"ro",
            },
            MountView {
                source: b"proc",
                target: b"/proc",
                filesystem: b"proc",
                options: b"rw,nosuid,nodev,noexec",
            },
        ];
        let procfs = ProcFs::new(
            &tasks[..count],
            2,
            12_340_000_000,
            MemoryView {
                total_bytes: 512 * 1024 * 1024,
                available_bytes: 384 * 1024 * 1024,
            },
            IdentityView {
                hostname: b"domain-a",
                boot_id: b"11111111-2222-3333-4444-555555555555",
            },
            &mounts,
        );
        let mut pids = [0u32; 4];
        assert_eq!(procfs.list_pids(&mut pids), 2);
        assert_eq!(&pids[..2], &[1, 2]);

        let mut output = [0u8; 512];
        let len = procfs.read(b"/proc/2/status", &mut output).unwrap();
        let status = &output[..len];
        assert!(status.windows(7).any(|value| value == b"Pid:\t2\n"));
        assert!(status.windows(8).any(|value| value == b"PPid:\t1\n"));
        assert!(status.windows(9).any(|value| value == b"Uid:\t1000"));
        assert_eq!(
            procfs
                .read(b"/proc/424242/status", &mut output)
                .unwrap_err(),
            ViewError::NotFound
        );
        let len = procfs
            .read(b"/proc/sys/kernel/random/boot_id", &mut output)
            .unwrap();
        assert_eq!(&output[..len], b"11111111-2222-3333-4444-555555555555\n");
    }

    #[test]
    fn procfs_system_views_are_bounded_consistent_and_read_only() {
        let (tasks, count) = fixture();
        let mounts = [MountView {
            source: b"proc",
            target: b"/proc",
            filesystem: b"proc",
            options: b"rw,nosuid,nodev,noexec",
        }];
        let procfs = ProcFs::new(
            &tasks[..count],
            1,
            12_340_000_000,
            MemoryView {
                total_bytes: 512 * 1024 * 1024,
                available_bytes: 384 * 1024 * 1024,
            },
            IdentityView {
                hostname: b"domain-a",
                boot_id: b"11111111-2222-3333-4444-555555555555",
            },
            &mounts,
        );
        let mut output = [0u8; 512];
        let len = procfs.read(b"/proc/uptime", &mut output).unwrap();
        assert_eq!(&output[..len], b"12.34 0.00\n");
        let len = procfs.read(b"/proc/meminfo", &mut output).unwrap();
        assert!(output[..len]
            .windows(19)
            .any(|value| value == b"MemTotal:       524"));
        let len = procfs.read(b"/proc/self/mounts", &mut output).unwrap();
        assert_eq!(
            &output[..len],
            b"proc /proc proc rw,nosuid,nodev,noexec 0 0\n"
        );
        assert_eq!(
            procfs.write(b"/proc/sys/kernel/hostname", b"host"),
            Err(ViewError::ReadOnly)
        );
        assert_eq!(
            procfs.read(b"/proc/not-a-pid/status", &mut output),
            Err(ViewError::InvalidPath)
        );
    }

    #[test]
    fn sysfs_projects_only_assigned_virtual_devices() {
        let mut registry: DeviceRegistry<6, 2> = DeviceRegistry::new();
        registry
            .assign(7, 100, DeviceName::new(b"vda").unwrap(), DeviceKind::Block)
            .unwrap();
        registry
            .assign(
                7,
                101,
                DeviceName::new(b"net0").unwrap(),
                DeviceKind::Network,
            )
            .unwrap();
        registry
            .assign(
                8,
                102,
                DeviceName::new(b"host-only").unwrap(),
                DeviceKind::Block,
            )
            .unwrap();
        let placeholder = DeviceView {
            name: DeviceName::new(b"none").unwrap(),
            kind: DeviceKind::Null,
            major: 0,
            minor: 0,
        };
        let mut projected = [placeholder; 6];
        let count = registry.views(7, &mut projected);
        let sysfs = SysFs::new(&projected[..count], 2);
        let mut output = [0u8; 128];

        let len = sysfs
            .read(b"/sys/class/block/vda/dev", &mut output)
            .unwrap();
        assert_eq!(&output[..len], b"254:0\n");
        let len = sysfs
            .read(b"/sys/class/net/net0/type", &mut output)
            .unwrap();
        assert_eq!(&output[..len], b"1\n");
        let len = sysfs
            .read(b"/sys/devices/system/cpu/online", &mut output)
            .unwrap();
        assert_eq!(&output[..len], b"0-1\n");
        assert_eq!(
            sysfs.read(b"/sys/class/block/host-only/dev", &mut output),
            Err(ViewError::NotFound)
        );
        assert_eq!(
            sysfs.write(b"/sys/class/block/vda/dev", b"8:0"),
            Err(ViewError::ReadOnly)
        );
    }
}
