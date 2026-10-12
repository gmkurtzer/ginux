const PROCESS_LIMIT: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcessHandle {
    slot: u8,
    generation: u32,
    pid: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcessState {
    Preparing,
    Running,
    Zombie { status: u8 },
}

#[derive(Clone, Copy)]
struct Process {
    handle: ProcessHandle,
    parent: Option<ProcessHandle>,
    address_space_root: u64,
    state: ProcessState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WaitResult {
    pid: u32,
    status: u8,
}

struct ProcessTable {
    processes: [Option<Process>; PROCESS_LIMIT],
    generations: [u32; PROCESS_LIMIT],
    next_pid: u32,
    init: Option<ProcessHandle>,
}

impl ProcessTable {
    const fn new() -> Self {
        Self {
            processes: [None; PROCESS_LIMIT],
            generations: [0; PROCESS_LIMIT],
            next_pid: 1,
            init: None,
        }
    }

    fn create_init(&mut self, address_space_root: u64) -> Option<ProcessHandle> {
        if self.init.is_some() || address_space_root == 0 || self.next_pid != 1 {
            return None;
        }
        let handle = self.allocate(None, address_space_root, ProcessState::Running)?;
        if handle.pid != 1 {
            return None;
        }
        self.init = Some(handle);
        Some(handle)
    }

    fn prepare_fork(&mut self, parent: ProcessHandle) -> Option<ProcessHandle> {
        if self.process(parent)?.state != ProcessState::Running {
            return None;
        }
        self.allocate(Some(parent), 0, ProcessState::Preparing)
    }

    fn commit_fork(&mut self, handle: ProcessHandle, address_space_root: u64) -> bool {
        if address_space_root == 0
            || self
                .processes
                .iter()
                .flatten()
                .any(|process| process.address_space_root == address_space_root)
        {
            return false;
        }
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if process.state != ProcessState::Preparing || process.address_space_root != 0 {
            return false;
        }
        process.address_space_root = address_space_root;
        process.state = ProcessState::Running;
        true
    }

    fn abort_fork(&mut self, handle: ProcessHandle) -> bool {
        let Some(process) = self.process(handle) else {
            return false;
        };
        if process.state != ProcessState::Preparing || process.address_space_root != 0 {
            return false;
        }
        self.processes[usize::from(handle.slot)] = None;
        true
    }

    fn release_address_space(&mut self, handle: ProcessHandle, expected_root: u64) -> bool {
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if process.state != ProcessState::Running
            || expected_root == 0
            || process.address_space_root != expected_root
        {
            return false;
        }
        process.address_space_root = 0;
        true
    }

    fn exit(&mut self, handle: ProcessHandle, status: u8) -> bool {
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if process.state != ProcessState::Running || process.address_space_root != 0 {
            return false;
        }
        process.state = ProcessState::Zombie { status };
        true
    }

    fn wait(&mut self, parent: ProcessHandle, pid: u32) -> Option<WaitResult> {
        if self.process(parent)?.state != ProcessState::Running {
            return None;
        }
        let slot = self.processes.iter().position(|candidate| {
            candidate.is_some_and(|process| {
                process.handle.pid == pid
                    && process.parent == Some(parent)
                    && matches!(process.state, ProcessState::Zombie { .. })
            })
        })?;
        let process = self.processes[slot].take()?;
        let ProcessState::Zombie { status } = process.state else {
            return None;
        };
        Some(WaitResult { pid, status })
    }

    fn allocate(
        &mut self,
        parent: Option<ProcessHandle>,
        address_space_root: u64,
        state: ProcessState,
    ) -> Option<ProcessHandle> {
        let slot = self.processes.iter().position(Option::is_none)?;
        let generation = self.generations[slot].checked_add(1)?;
        let pid = self.next_pid;
        self.next_pid = self.next_pid.checked_add(1)?;
        let handle = ProcessHandle {
            slot: u8::try_from(slot).ok()?,
            generation,
            pid,
        };
        self.generations[slot] = generation;
        self.processes[slot] = Some(Process {
            handle,
            parent,
            address_space_root,
            state,
        });
        Some(handle)
    }

    fn process(&self, handle: ProcessHandle) -> Option<&Process> {
        self.processes
            .get(usize::from(handle.slot))?
            .as_ref()
            .filter(|process| process.handle == handle)
    }

    fn process_mut(&mut self, handle: ProcessHandle) -> Option<&mut Process> {
        self.processes
            .get_mut(usize::from(handle.slot))?
            .as_mut()
            .filter(|process| process.handle == handle)
    }
}

pub(crate) fn verify_lifecycle_contract() -> bool {
    let mut table = ProcessTable::new();
    let Some(init) = table.create_init(0x1000) else {
        return false;
    };
    let Some(rejected_child) = table.prepare_fork(init) else {
        return false;
    };
    if init.pid != 1
        || rejected_child.pid != 2
        || table.create_init(0x2000).is_some()
        || table.commit_fork(rejected_child, 0x1000)
        || table.commit_fork(rejected_child, 0)
        || table.wait(init, rejected_child.pid).is_some()
        || table.exit(rejected_child, 1)
        || !table.abort_fork(rejected_child)
        || table.commit_fork(rejected_child, 0x2000)
        || table.abort_fork(rejected_child)
    {
        return false;
    }
    let Some(first_child) = table.prepare_fork(init) else {
        return false;
    };
    if first_child.pid != 3
        || !table.commit_fork(first_child, 0x2000)
        || table.commit_fork(first_child, 0x3000)
        || table.wait(init, first_child.pid).is_some()
        || table.exit(first_child, 42)
        || table.release_address_space(first_child, 0x3000)
        || !table.release_address_space(first_child, 0x2000)
        || !table.exit(first_child, 42)
        || table.wait(first_child, first_child.pid).is_some()
        || table.wait(init, first_child.pid) != Some(WaitResult { pid: 3, status: 42 })
        || table.exit(first_child, 42)
    {
        return false;
    }
    let Some(second_child) = table.prepare_fork(init) else {
        return false;
    };
    second_child.slot == first_child.slot
        && second_child.generation > first_child.generation
        && second_child.pid == 4
        && !table.release_address_space(first_child, 0x3000)
        && table.commit_fork(second_child, 0x3000)
        && table.release_address_space(second_child, 0x3000)
        && table.exit(second_child, 7)
        && table.wait(init, second_child.pid) == Some(WaitResult { pid: 4, status: 7 })
}
