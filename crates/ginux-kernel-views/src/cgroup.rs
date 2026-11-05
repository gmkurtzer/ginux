use crate::ViewError;

const MAX_PATH: usize = 96;
const MAX_MEMBERS: usize = 16;
const CPU: u8 = 1;
const MEMORY: u8 = 2;
const PIDS: u8 = 4;

#[derive(Clone, Copy)]
struct Group {
    active: bool,
    path: [u8; MAX_PATH],
    path_len: u8,
    members: [u32; MAX_MEMBERS],
    member_count: u8,
    memory_max: u64,
    pids_max: u32,
    controllers: u8,
}

impl Group {
    const fn empty() -> Self {
        Self {
            active: false,
            path: [0; MAX_PATH],
            path_len: 0,
            members: [0; MAX_MEMBERS],
            member_count: 0,
            memory_max: u64::MAX,
            pids_max: u32::MAX,
            controllers: 0,
        }
    }

    fn path(&self) -> &[u8] {
        &self.path[..self.path_len as usize]
    }
}

pub struct CgroupFs<const GROUPS: usize> {
    groups: [Group; GROUPS],
}

impl<const GROUPS: usize> Default for CgroupFs<GROUPS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const GROUPS: usize> CgroupFs<GROUPS> {
    pub const fn new() -> Self {
        let mut groups = [Group::empty(); GROUPS];
        if GROUPS > 0 {
            groups[0].active = true;
            groups[0].path[0] = b'/';
            groups[0].path_len = 1;
        }
        Self { groups }
    }

    pub fn create(&mut self, path: &[u8], actor_uid: u32) -> Result<(), ViewError> {
        require_root(actor_uid)?;
        validate_group_path(path)?;
        if self.find(path).is_some() {
            return Err(ViewError::AlreadyExists);
        }
        let parent = parent_path(path);
        if self.find(parent).is_none() {
            return Err(ViewError::NotFound);
        }
        let group = self
            .groups
            .iter_mut()
            .find(|group| !group.active)
            .ok_or(ViewError::LimitExceeded)?;
        group.active = true;
        group.path[..path.len()].copy_from_slice(path);
        group.path_len = path.len() as u8;
        Ok(())
    }

    pub fn remove(&mut self, path: &[u8], actor_uid: u32) -> Result<(), ViewError> {
        require_root(actor_uid)?;
        if path == b"/" {
            return Err(ViewError::PermissionDenied);
        }
        let index = self.find(path).ok_or(ViewError::NotFound)?;
        if self.groups[index].member_count != 0
            || self.groups.iter().any(|group| {
                group.active && group.path() != path && parent_path(group.path()) == path
            })
        {
            return Err(ViewError::Busy);
        }
        self.groups[index] = Group::empty();
        Ok(())
    }

    pub fn attach(&mut self, path: &[u8], pid: u32, actor_uid: u32) -> Result<(), ViewError> {
        require_root(actor_uid)?;
        let target = self.find(path).ok_or(ViewError::NotFound)?;
        if pid == 0 {
            return Err(ViewError::InvalidPath);
        }
        if self.groups[target].member_count as u32 >= self.groups[target].pids_max
            || self.groups[target].member_count as usize == MAX_MEMBERS
        {
            return Err(ViewError::LimitExceeded);
        }
        for group in &mut self.groups {
            if let Some(position) = group.members[..group.member_count as usize]
                .iter()
                .position(|member| *member == pid)
            {
                let count = group.member_count as usize;
                let mut index = position;
                while index + 1 < count {
                    group.members[index] = group.members[index + 1];
                    index += 1;
                }
                group.member_count -= 1;
            }
        }
        let group = &mut self.groups[target];
        group.members[group.member_count as usize] = pid;
        group.member_count += 1;
        Ok(())
    }

    pub fn write(&mut self, path: &[u8], input: &[u8], actor_uid: u32) -> Result<usize, ViewError> {
        require_root(actor_uid)?;
        let (group_path, file) = split_file(path)?;
        let index = self.find(group_path).ok_or(ViewError::NotFound)?;
        let value = trim_newline(input);
        match file {
            b"cgroup.procs" => self.attach(group_path, parse_u32(value)?, actor_uid)?,
            b"memory.max" => {
                self.groups[index].memory_max = parse_limit_u64(value)?;
            }
            b"pids.max" => {
                let limit = parse_limit_u32(value)?;
                if limit < self.groups[index].member_count as u32 {
                    return Err(ViewError::LimitExceeded);
                }
                self.groups[index].pids_max = limit;
            }
            b"cgroup.subtree_control" => {
                self.groups[index].controllers = parse_controllers(value)?;
            }
            _ => return Err(ViewError::ReadOnly),
        }
        Ok(input.len())
    }

    pub fn read(&self, path: &[u8], output: &mut [u8]) -> Result<usize, ViewError> {
        let (group_path, file) = split_file(path)?;
        let group = &self.groups[self.find(group_path).ok_or(ViewError::NotFound)?];
        let mut writer = ByteWriter::new(output);
        match file {
            b"cgroup.controllers" => writer.bytes(b"cpu memory pids\n")?,
            b"cgroup.subtree_control" => {
                if group.controllers & CPU != 0 {
                    writer.bytes(b"cpu ")?;
                }
                if group.controllers & MEMORY != 0 {
                    writer.bytes(b"memory ")?;
                }
                if group.controllers & PIDS != 0 {
                    writer.bytes(b"pids ")?;
                }
                writer.byte(b'\n')?;
            }
            b"cgroup.procs" => {
                for pid in &group.members[..group.member_count as usize] {
                    writer.decimal(u64::from(*pid))?;
                    writer.byte(b'\n')?;
                }
            }
            b"memory.max" => writer.limit(group.memory_max)?,
            b"pids.max" => writer.limit(u64::from(group.pids_max))?,
            b"cgroup.events" => {
                writer.bytes(b"populated ")?;
                writer.byte(if group.member_count == 0 { b'0' } else { b'1' })?;
                writer.bytes(b"\nfrozen 0\n")?;
            }
            _ => return Err(ViewError::NotFound),
        }
        Ok(writer.len)
    }

    fn find(&self, path: &[u8]) -> Option<usize> {
        self.groups
            .iter()
            .position(|group| group.active && group.path() == path)
    }
}

fn require_root(uid: u32) -> Result<(), ViewError> {
    if uid == 0 {
        Ok(())
    } else {
        Err(ViewError::PermissionDenied)
    }
}

fn validate_group_path(path: &[u8]) -> Result<(), ViewError> {
    if path.len() < 2
        || path.len() > MAX_PATH
        || path[0] != b'/'
        || path.ends_with(b"/")
        || path.windows(2).any(|window| window == b"//")
        || path.split(|byte| *byte == b'/').any(|part| part == b"..")
        || path.iter().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.'))
        })
    {
        return Err(ViewError::InvalidPath);
    }
    Ok(())
}

fn parent_path(path: &[u8]) -> &[u8] {
    let split = path.iter().rposition(|byte| *byte == b'/').unwrap_or(0);
    if split == 0 {
        b"/"
    } else {
        &path[..split]
    }
}

fn split_file(path: &[u8]) -> Result<(&[u8], &[u8]), ViewError> {
    let split = path
        .iter()
        .rposition(|byte| *byte == b'/')
        .ok_or(ViewError::InvalidPath)?;
    if split + 1 == path.len() {
        return Err(ViewError::InvalidPath);
    }
    let group = if split == 0 { b"/" } else { &path[..split] };
    Ok((group, &path[split + 1..]))
}

fn trim_newline(input: &[u8]) -> &[u8] {
    input.strip_suffix(b"\n").unwrap_or(input)
}

fn parse_u32(input: &[u8]) -> Result<u32, ViewError> {
    let value = parse_u64(input)?;
    u32::try_from(value).map_err(|_| ViewError::InvalidPath)
}

fn parse_u64(input: &[u8]) -> Result<u64, ViewError> {
    if input.is_empty() {
        return Err(ViewError::InvalidPath);
    }
    let mut value = 0u64;
    for byte in input {
        if !byte.is_ascii_digit() {
            return Err(ViewError::InvalidPath);
        }
        value = value
            .checked_mul(10)
            .and_then(|current| current.checked_add(u64::from(*byte - b'0')))
            .ok_or(ViewError::InvalidPath)?;
    }
    Ok(value)
}

fn parse_limit_u64(input: &[u8]) -> Result<u64, ViewError> {
    if input == b"max" {
        Ok(u64::MAX)
    } else {
        parse_u64(input)
    }
}

fn parse_limit_u32(input: &[u8]) -> Result<u32, ViewError> {
    if input == b"max" {
        Ok(u32::MAX)
    } else {
        parse_u32(input)
    }
}

fn parse_controllers(input: &[u8]) -> Result<u8, ViewError> {
    let mut controllers = 0u8;
    for token in input.split(|byte| *byte == b' ') {
        controllers |= match token {
            b"+cpu" => CPU,
            b"+memory" => MEMORY,
            b"+pids" => PIDS,
            b"" => continue,
            _ => return Err(ViewError::PermissionDenied),
        };
    }
    Ok(controllers)
}

struct ByteWriter<'a> {
    output: &'a mut [u8],
    len: usize,
}

impl<'a> ByteWriter<'a> {
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

    fn bytes(&mut self, input: &[u8]) -> Result<(), ViewError> {
        for byte in input {
            self.byte(*byte)?;
        }
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

    fn limit(&mut self, value: u64) -> Result<(), ViewError> {
        if value == u64::MAX || value == u64::from(u32::MAX) {
            self.bytes(b"max\n")
        } else {
            self.decimal(value)?;
            self.byte(b'\n')
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_hierarchy_supports_systemd_style_operations() {
        let mut cgroups: CgroupFs<4> = CgroupFs::new();
        cgroups.create(b"/system.slice", 0).unwrap();
        cgroups
            .write(b"/cgroup.subtree_control", b"+cpu +memory +pids\n", 0)
            .unwrap();
        cgroups
            .write(b"/system.slice/memory.max", b"1048576\n", 0)
            .unwrap();
        cgroups.write(b"/system.slice/pids.max", b"4\n", 0).unwrap();
        cgroups
            .write(b"/system.slice/cgroup.procs", b"2\n", 0)
            .unwrap();

        let mut output = [0u8; 128];
        let len = cgroups
            .read(b"/system.slice/cgroup.procs", &mut output)
            .unwrap();
        assert_eq!(&output[..len], b"2\n");
        let len = cgroups
            .read(b"/system.slice/memory.max", &mut output)
            .unwrap();
        assert_eq!(&output[..len], b"1048576\n");
        let len = cgroups.read(b"/cgroup.events", &mut output).unwrap();
        assert_eq!(&output[..len], b"populated 0\nfrozen 0\n");
    }

    #[test]
    fn cgroup_mutation_is_virtual_bounded_and_privileged() {
        let mut cgroups: CgroupFs<2> = CgroupFs::new();
        assert_eq!(
            cgroups.create(b"/host.slice", 1000),
            Err(ViewError::PermissionDenied)
        );
        cgroups.create(b"/domain.slice", 0).unwrap();
        assert_eq!(
            cgroups.create(b"/another.slice", 0),
            Err(ViewError::LimitExceeded)
        );
        assert_eq!(
            cgroups.write(b"/domain.slice/cgroup.freeze", b"1\n", 0),
            Err(ViewError::ReadOnly)
        );
        cgroups.attach(b"/domain.slice", 7, 0).unwrap();
        assert_eq!(cgroups.remove(b"/domain.slice", 0), Err(ViewError::Busy));
        assert_eq!(cgroups.create(b"/../host", 0), Err(ViewError::InvalidPath));
    }
}
