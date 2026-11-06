#![no_std]

pub const MAX_GROUPS: usize = 8;
pub const MAX_SIGNAL: u8 = 64;
pub const CAP_SYS_ADMIN: u64 = 1 << 21;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessError {
    TableFull,
    InvalidTask,
    StaleTask,
    InvalidParent,
    InvalidState,
    NotChild,
    NoExitedChild,
    InvalidSignal,
    SignalBlocked,
    PermissionDenied,
    InvalidCredential,
    GroupLimit,
    CounterOverflow,
    WouldBlock,
    InvalidFutex,
    FutexFull,
    TimerFull,
    InvalidTimer,
    EventLimit,
    InvalidEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalAction {
    Default,
    Ignore,
    Handler(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalDelivery {
    Default(u8),
    Ignored(u8),
    Handler { signal: u8, address: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskId {
    pub pid: u32,
    generation: u32,
}

impl TaskId {
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskState {
    Running,
    Exited(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Credentials {
    pub uid: u32,
    pub gid: u32,
    groups: [u32; MAX_GROUPS],
    group_count: u8,
    pub capabilities: u64,
}

impl Credentials {
    pub const fn root() -> Self {
        Self {
            uid: 0,
            gid: 0,
            groups: [0; MAX_GROUPS],
            group_count: 0,
            capabilities: u64::MAX,
        }
    }

    pub const fn unprivileged(uid: u32, gid: u32) -> Self {
        Self {
            uid,
            gid,
            groups: [0; MAX_GROUPS],
            group_count: 0,
            capabilities: 0,
        }
    }

    pub fn groups(&self) -> &[u32] {
        &self.groups[..self.group_count as usize]
    }

    pub fn set_groups(&mut self, groups: &[u32]) -> Result<(), ProcessError> {
        if self.uid != 0 || groups.len() > MAX_GROUPS {
            return Err(if groups.len() > MAX_GROUPS {
                ProcessError::GroupLimit
            } else {
                ProcessError::PermissionDenied
            });
        }
        self.groups.fill(0);
        self.groups[..groups.len()].copy_from_slice(groups);
        self.group_count = groups.len() as u8;
        Ok(())
    }

    pub fn set_uid(&mut self, uid: u32) -> Result<(), ProcessError> {
        if self.uid != 0 && uid != self.uid {
            return Err(ProcessError::PermissionDenied);
        }
        self.uid = uid;
        if uid != 0 {
            self.capabilities = 0;
        }
        Ok(())
    }

    pub fn set_gid(&mut self, gid: u32) -> Result<(), ProcessError> {
        if self.uid != 0 && gid != self.gid {
            return Err(ProcessError::PermissionDenied);
        }
        self.gid = gid;
        Ok(())
    }

    pub fn drop_capabilities(&mut self, keep: u64) {
        self.capabilities &= keep;
    }
}

#[derive(Clone, Copy)]
struct Task {
    occupied: bool,
    id: TaskId,
    tgid: u32,
    parent: u32,
    process_group: u32,
    session: u32,
    state: TaskState,
    credentials: Credentials,
    signal_mask: u64,
    pending_signals: u64,
    exec_epoch: u32,
    signal_actions: [SignalAction; MAX_SIGNAL as usize],
    signal_frame_active: bool,
    saved_signal_mask: u64,
}

impl Task {
    const fn empty() -> Self {
        Self {
            occupied: false,
            id: TaskId {
                pid: 0,
                generation: 0,
            },
            tgid: 0,
            parent: 0,
            process_group: 0,
            session: 0,
            state: TaskState::Exited(0),
            credentials: Credentials::unprivileged(0, 0),
            signal_mask: 0,
            pending_signals: 0,
            exec_epoch: 0,
            signal_actions: [SignalAction::Default; MAX_SIGNAL as usize],
            signal_frame_active: false,
            saved_signal_mask: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskView {
    pub id: TaskId,
    pub tgid: u32,
    pub parent: u32,
    pub process_group: u32,
    pub session: u32,
    pub state: TaskState,
    pub credentials: Credentials,
    pub exec_epoch: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WaitResult {
    pub child: TaskId,
    pub status: i32,
}

pub struct ProcessTable<const TASKS: usize> {
    tasks: [Task; TASKS],
    next_pid: u32,
    next_generation: u32,
}

impl<const TASKS: usize> Default for ProcessTable<TASKS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const TASKS: usize> ProcessTable<TASKS> {
    pub const fn new() -> Self {
        Self {
            tasks: [Task::empty(); TASKS],
            next_pid: 1,
            next_generation: 1,
        }
    }

    pub fn create_init(&mut self, credentials: Credentials) -> Result<TaskId, ProcessError> {
        if self.tasks.iter().any(|task| task.occupied) {
            return Err(ProcessError::InvalidState);
        }
        self.allocate(0, 1, 1, 1, credentials)
    }

    fn allocate(
        &mut self,
        parent: u32,
        tgid: u32,
        process_group: u32,
        session: u32,
        credentials: Credentials,
    ) -> Result<TaskId, ProcessError> {
        let slot = self
            .tasks
            .iter()
            .position(|task| !task.occupied)
            .ok_or(ProcessError::TableFull)?;
        let id = TaskId {
            pid: self.next_pid,
            generation: self.next_generation,
        };
        self.next_pid = self
            .next_pid
            .checked_add(1)
            .ok_or(ProcessError::TableFull)?;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(ProcessError::TableFull)?;
        self.tasks[slot] = Task {
            occupied: true,
            id,
            tgid: if tgid == 0 { id.pid } else { tgid },
            parent,
            process_group: if process_group == 0 {
                id.pid
            } else {
                process_group
            },
            session: if session == 0 { id.pid } else { session },
            state: TaskState::Running,
            credentials,
            signal_mask: 0,
            pending_signals: 0,
            exec_epoch: 0,
            signal_actions: [SignalAction::Default; MAX_SIGNAL as usize],
            signal_frame_active: false,
            saved_signal_mask: 0,
        };
        Ok(id)
    }

    fn index(&self, id: TaskId) -> Result<usize, ProcessError> {
        match self
            .tasks
            .iter()
            .position(|task| task.occupied && task.id.pid == id.pid)
        {
            Some(index) if self.tasks[index].id == id => Ok(index),
            Some(_) => Err(ProcessError::StaleTask),
            None => Err(ProcessError::InvalidTask),
        }
    }

    pub fn view(&self, id: TaskId) -> Result<TaskView, ProcessError> {
        let task = self.tasks[self.index(id)?];
        Ok(TaskView {
            id: task.id,
            tgid: task.tgid,
            parent: task.parent,
            process_group: task.process_group,
            session: task.session,
            state: task.state,
            credentials: task.credentials,
            exec_epoch: task.exec_epoch,
        })
    }

    pub fn fork(&mut self, parent: TaskId) -> Result<TaskId, ProcessError> {
        let parent = self.tasks[self.index(parent)?];
        if parent.state != TaskState::Running {
            return Err(ProcessError::InvalidParent);
        }
        self.allocate(
            parent.id.pid,
            0,
            parent.process_group,
            parent.session,
            parent.credentials,
        )
    }

    pub fn clone_thread(&mut self, leader: TaskId) -> Result<TaskId, ProcessError> {
        let leader = self.tasks[self.index(leader)?];
        if leader.state != TaskState::Running {
            return Err(ProcessError::InvalidParent);
        }
        self.allocate(
            leader.parent,
            leader.tgid,
            leader.process_group,
            leader.session,
            leader.credentials,
        )
    }

    pub fn exec(&mut self, id: TaskId) -> Result<u32, ProcessError> {
        let index = self.index(id)?;
        let task = &mut self.tasks[index];
        if task.state != TaskState::Running {
            return Err(ProcessError::InvalidState);
        }
        task.pending_signals = 0;
        task.signal_mask = 0;
        task.signal_frame_active = false;
        task.saved_signal_mask = 0;
        for action in &mut task.signal_actions {
            if *action != SignalAction::Ignore {
                *action = SignalAction::Default;
            }
        }
        task.exec_epoch = task
            .exec_epoch
            .checked_add(1)
            .ok_or(ProcessError::InvalidState)?;
        Ok(task.exec_epoch)
    }

    pub fn exit(&mut self, id: TaskId, status: i32) -> Result<(), ProcessError> {
        let index = self.index(id)?;
        if self.tasks[index].state != TaskState::Running {
            return Err(ProcessError::InvalidState);
        }
        self.tasks[index].state = TaskState::Exited(status);
        Ok(())
    }

    pub fn wait(
        &mut self,
        parent: TaskId,
        selected: Option<u32>,
    ) -> Result<WaitResult, ProcessError> {
        let parent_index = self.index(parent)?;
        if self.tasks[parent_index].state != TaskState::Running {
            return Err(ProcessError::InvalidState);
        }
        let parent_pid = parent.pid;
        let child = self.tasks.iter().position(|task| {
            task.occupied
                && task.parent == parent_pid
                && selected.is_none_or(|pid| task.id.pid == pid)
                && matches!(task.state, TaskState::Exited(_))
        });
        if let Some(index) = child {
            let task = self.tasks[index];
            let TaskState::Exited(status) = task.state else {
                return Err(ProcessError::InvalidState);
            };
            self.tasks[index] = Task::empty();
            return Ok(WaitResult {
                child: task.id,
                status,
            });
        }
        if let Some(pid) = selected {
            if !self
                .tasks
                .iter()
                .any(|task| task.occupied && task.parent == parent_pid && task.id.pid == pid)
            {
                return Err(ProcessError::NotChild);
            }
        }
        Err(ProcessError::NoExitedChild)
    }

    pub fn set_process_group(&mut self, id: TaskId, group: u32) -> Result<(), ProcessError> {
        let index = self.index(id)?;
        if group == 0 {
            return Err(ProcessError::InvalidTask);
        }
        self.tasks[index].process_group = group;
        Ok(())
    }

    pub fn set_process_group_for(
        &mut self,
        caller: TaskId,
        target_pid: u32,
        group: u32,
    ) -> Result<TaskId, ProcessError> {
        let caller_index = self.index(caller)?;
        let caller_task = self.tasks[caller_index];
        let target_pid = if target_pid == 0 {
            caller_task.id.pid
        } else {
            target_pid
        };
        let target_index = self
            .tasks
            .iter()
            .position(|task| task.occupied && task.id.pid == target_pid)
            .ok_or(ProcessError::InvalidTask)?;
        let target = self.tasks[target_index];
        if target.id != caller && target.parent != caller_task.id.pid {
            return Err(ProcessError::PermissionDenied);
        }
        if target.session != caller_task.session || target.session == target.id.pid {
            return Err(ProcessError::PermissionDenied);
        }
        let group = if group == 0 { target.id.pid } else { group };
        let group_exists = group == target.id.pid
            || self.tasks.iter().any(|task| {
                task.occupied && task.session == target.session && task.process_group == group
            });
        if !group_exists {
            return Err(ProcessError::PermissionDenied);
        }
        self.tasks[target_index].process_group = group;
        Ok(target.id)
    }

    pub fn create_session(&mut self, id: TaskId) -> Result<u32, ProcessError> {
        let index = self.index(id)?;
        let pid = self.tasks[index].id.pid;
        if self.tasks[index].process_group == pid {
            return Err(ProcessError::PermissionDenied);
        }
        self.tasks[index].session = pid;
        self.tasks[index].process_group = pid;
        Ok(pid)
    }

    pub fn credentials_mut(&mut self, id: TaskId) -> Result<&mut Credentials, ProcessError> {
        let index = self.index(id)?;
        Ok(&mut self.tasks[index].credentials)
    }

    pub fn set_signal_mask(&mut self, id: TaskId, mask: u64) -> Result<(), ProcessError> {
        let index = self.index(id)?;
        self.tasks[index].signal_mask = mask;
        Ok(())
    }

    pub fn signal(&mut self, target: TaskId, signal: u8) -> Result<(), ProcessError> {
        if signal == 0 || signal > MAX_SIGNAL {
            return Err(ProcessError::InvalidSignal);
        }
        let index = self.index(target)?;
        if self.tasks[index].state != TaskState::Running {
            return Err(ProcessError::InvalidState);
        }
        self.tasks[index].pending_signals |= 1u64 << (signal - 1);
        Ok(())
    }

    pub fn set_signal_action(
        &mut self,
        id: TaskId,
        signal: u8,
        action: SignalAction,
    ) -> Result<(), ProcessError> {
        if signal == 0 || signal > MAX_SIGNAL || matches!(signal, 9 | 19) {
            return Err(ProcessError::InvalidSignal);
        }
        let index = self.index(id)?;
        self.tasks[index].signal_actions[signal as usize - 1] = action;
        Ok(())
    }

    pub fn signal_action(&self, id: TaskId, signal: u8) -> Result<SignalAction, ProcessError> {
        if signal == 0 || signal > MAX_SIGNAL {
            return Err(ProcessError::InvalidSignal);
        }
        let index = self.index(id)?;
        Ok(self.tasks[index].signal_actions[signal as usize - 1])
    }

    pub fn next_signal(&mut self, id: TaskId) -> Result<Option<u8>, ProcessError> {
        let index = self.index(id)?;
        let deliverable = self.tasks[index].pending_signals & !self.tasks[index].signal_mask;
        if deliverable == 0 {
            return Ok(None);
        }
        let signal = deliverable.trailing_zeros() as u8 + 1;
        self.tasks[index].pending_signals &= !(1u64 << (signal - 1));
        Ok(Some(signal))
    }

    pub fn begin_signal(&mut self, id: TaskId) -> Result<Option<SignalDelivery>, ProcessError> {
        let Some(signal) = self.next_signal(id)? else {
            return Ok(None);
        };
        let index = self.index(id)?;
        let task = &mut self.tasks[index];
        let delivery = match task.signal_actions[signal as usize - 1] {
            SignalAction::Default => SignalDelivery::Default(signal),
            SignalAction::Ignore => SignalDelivery::Ignored(signal),
            SignalAction::Handler(address) => {
                if task.signal_frame_active {
                    task.pending_signals |= 1u64 << (signal - 1);
                    return Err(ProcessError::InvalidState);
                }
                task.signal_frame_active = true;
                task.saved_signal_mask = task.signal_mask;
                task.signal_mask |= 1u64 << (signal - 1);
                SignalDelivery::Handler { signal, address }
            }
        };
        Ok(Some(delivery))
    }

    pub fn signal_return(&mut self, id: TaskId) -> Result<(), ProcessError> {
        let index = self.index(id)?;
        let task = &mut self.tasks[index];
        if !task.signal_frame_active {
            return Err(ProcessError::InvalidState);
        }
        task.signal_mask = task.saved_signal_mask;
        task.saved_signal_mask = 0;
        task.signal_frame_active = false;
        Ok(())
    }

    pub fn blocking_interrupted(&self, id: TaskId) -> Result<bool, ProcessError> {
        let task = self.tasks[self.index(id)?];
        Ok(task.pending_signals & !task.signal_mask != 0)
    }

    pub fn shutdown(&mut self, status: i32) -> usize {
        let mut stopped = 0;
        for task in &mut self.tasks {
            if task.occupied && task.state == TaskState::Running {
                task.state = TaskState::Exited(status);
                stopped += 1;
            }
        }
        stopped
    }

    pub fn reap_all(&mut self) -> usize {
        let mut reaped = 0;
        for task in &mut self.tasks {
            if task.occupied && matches!(task.state, TaskState::Exited(_)) {
                *task = Task::empty();
                reaped += 1;
            }
        }
        reaped
    }

    pub fn active(&self) -> usize {
        self.tasks.iter().filter(|task| task.occupied).count()
    }

    pub fn views(&self, output: &mut [TaskView]) -> usize {
        let mut count = 0usize;
        for task in self.tasks.iter().filter(|task| task.occupied) {
            if count == output.len() {
                break;
            }
            output[count] = TaskView {
                id: task.id,
                tgid: task.tgid,
                parent: task.parent,
                process_group: task.process_group,
                session: task.session,
                state: task.state,
                credentials: task.credentials,
                exec_epoch: task.exec_epoch,
            };
            count += 1;
        }
        count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualClock {
    monotonic_ns: u64,
    realtime_offset_ns: i64,
}

impl Default for VirtualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualClock {
    pub const fn new() -> Self {
        Self {
            monotonic_ns: 0,
            realtime_offset_ns: 0,
        }
    }

    pub const fn monotonic_ns(&self) -> u64 {
        self.monotonic_ns
    }

    pub fn realtime_ns(&self) -> Result<u64, ProcessError> {
        if self.realtime_offset_ns >= 0 {
            self.monotonic_ns
                .checked_add(self.realtime_offset_ns as u64)
                .ok_or(ProcessError::CounterOverflow)
        } else {
            self.monotonic_ns
                .checked_sub(self.realtime_offset_ns.unsigned_abs())
                .ok_or(ProcessError::InvalidTimer)
        }
    }

    pub fn set_realtime_ns(&mut self, value: u64) -> Result<(), ProcessError> {
        let difference = value as i128 - self.monotonic_ns as i128;
        self.realtime_offset_ns = difference
            .try_into()
            .map_err(|_| ProcessError::InvalidTimer)?;
        Ok(())
    }

    pub fn advance(&mut self, nanoseconds: u64) -> Result<u64, ProcessError> {
        self.monotonic_ns = self
            .monotonic_ns
            .checked_add(nanoseconds)
            .ok_or(ProcessError::CounterOverflow)?;
        Ok(self.monotonic_ns)
    }
}

#[derive(Clone, Copy)]
struct Timer {
    active: bool,
    id: u32,
    deadline_ns: u64,
    interval_ns: u64,
    expirations: u64,
}

impl Timer {
    const fn empty() -> Self {
        Self {
            active: false,
            id: 0,
            deadline_ns: 0,
            interval_ns: 0,
            expirations: 0,
        }
    }
}

pub struct TimerTable<const TIMERS: usize> {
    timers: [Timer; TIMERS],
    next_id: u32,
}

impl<const TIMERS: usize> Default for TimerTable<TIMERS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const TIMERS: usize> TimerTable<TIMERS> {
    pub const fn new() -> Self {
        Self {
            timers: [Timer::empty(); TIMERS],
            next_id: 1,
        }
    }

    pub fn create(&mut self) -> Result<u32, ProcessError> {
        let timer = self
            .timers
            .iter_mut()
            .find(|timer| !timer.active)
            .ok_or(ProcessError::TimerFull)?;
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or(ProcessError::TimerFull)?;
        *timer = Timer {
            active: true,
            id,
            deadline_ns: 0,
            interval_ns: 0,
            expirations: 0,
        };
        Ok(id)
    }

    fn timer_mut(&mut self, id: u32) -> Result<&mut Timer, ProcessError> {
        self.timers
            .iter_mut()
            .find(|timer| timer.active && timer.id == id)
            .ok_or(ProcessError::InvalidTimer)
    }

    pub fn arm(
        &mut self,
        id: u32,
        now_ns: u64,
        initial_ns: u64,
        interval_ns: u64,
    ) -> Result<(), ProcessError> {
        if initial_ns == 0 {
            return Err(ProcessError::InvalidTimer);
        }
        let timer = self.timer_mut(id)?;
        timer.deadline_ns = now_ns
            .checked_add(initial_ns)
            .ok_or(ProcessError::InvalidTimer)?;
        timer.interval_ns = interval_ns;
        timer.expirations = 0;
        Ok(())
    }

    pub fn update(&mut self, now_ns: u64) {
        for timer in &mut self.timers {
            if !timer.active || timer.deadline_ns == 0 || now_ns < timer.deadline_ns {
                continue;
            }
            match (now_ns - timer.deadline_ns).checked_div(timer.interval_ns) {
                None => {
                    timer.expirations = timer.expirations.saturating_add(1);
                    timer.deadline_ns = 0;
                }
                Some(periods) => {
                    let count = 1 + periods;
                    timer.expirations = timer.expirations.saturating_add(count);
                    timer.deadline_ns = timer
                        .deadline_ns
                        .saturating_add(count.saturating_mul(timer.interval_ns));
                }
            }
        }
    }

    pub fn read(&mut self, id: u32) -> Result<u64, ProcessError> {
        let timer = self.timer_mut(id)?;
        if timer.expirations == 0 {
            return Err(ProcessError::WouldBlock);
        }
        let count = timer.expirations;
        timer.expirations = 0;
        Ok(count)
    }

    pub fn close(&mut self, id: u32) -> Result<(), ProcessError> {
        *self.timer_mut(id)? = Timer::empty();
        Ok(())
    }
}

pub const EVENT_READ: u32 = 1;
pub const EVENT_WRITE: u32 = 2;

#[derive(Clone, Copy)]
struct EventInterest {
    active: bool,
    descriptor: u32,
    events: u32,
}

impl EventInterest {
    const fn empty() -> Self {
        Self {
            active: false,
            descriptor: 0,
            events: 0,
        }
    }
}

pub struct EventPoll<const ENTRIES: usize> {
    entries: [EventInterest; ENTRIES],
}

impl<const ENTRIES: usize> Default for EventPoll<ENTRIES> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const ENTRIES: usize> EventPoll<ENTRIES> {
    pub const fn new() -> Self {
        Self {
            entries: [EventInterest::empty(); ENTRIES],
        }
    }

    pub fn add(&mut self, descriptor: u32, events: u32) -> Result<(), ProcessError> {
        if descriptor == 0 || events == 0 || events & !(EVENT_READ | EVENT_WRITE) != 0 {
            return Err(ProcessError::InvalidEvent);
        }
        if self
            .entries
            .iter()
            .any(|entry| entry.active && entry.descriptor == descriptor)
        {
            return Err(ProcessError::InvalidEvent);
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| !entry.active)
            .ok_or(ProcessError::EventLimit)?;
        *entry = EventInterest {
            active: true,
            descriptor,
            events,
        };
        Ok(())
    }

    pub fn ready(&self, descriptor: u32, available: u32) -> Result<u32, ProcessError> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.active && entry.descriptor == descriptor)
            .ok_or(ProcessError::InvalidEvent)?;
        Ok(entry.events & available)
    }

    pub fn remove(&mut self, descriptor: u32) -> Result<(), ProcessError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.active && entry.descriptor == descriptor)
            .ok_or(ProcessError::InvalidEvent)?;
        *entry = EventInterest::empty();
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventFd {
    counter: u64,
    semaphore: bool,
}

impl EventFd {
    pub const fn new(initial: u64, semaphore: bool) -> Self {
        Self {
            counter: initial,
            semaphore,
        }
    }

    pub fn write(&mut self, value: u64) -> Result<(), ProcessError> {
        if value == u64::MAX {
            return Err(ProcessError::InvalidState);
        }
        self.counter = self
            .counter
            .checked_add(value)
            .ok_or(ProcessError::CounterOverflow)?;
        Ok(())
    }

    pub fn read(&mut self) -> Result<u64, ProcessError> {
        if self.counter == 0 {
            return Err(ProcessError::WouldBlock);
        }
        if self.semaphore {
            self.counter -= 1;
            Ok(1)
        } else {
            let value = self.counter;
            self.counter = 0;
            Ok(value)
        }
    }
}

#[derive(Clone, Copy)]
struct FutexWaiter {
    active: bool,
    task: TaskId,
    address: u64,
}

impl FutexWaiter {
    const fn empty() -> Self {
        Self {
            active: false,
            task: TaskId {
                pid: 0,
                generation: 0,
            },
            address: 0,
        }
    }
}

pub struct FutexTable<const WAITERS: usize> {
    waiters: [FutexWaiter; WAITERS],
}

impl<const WAITERS: usize> Default for FutexTable<WAITERS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const WAITERS: usize> FutexTable<WAITERS> {
    pub const fn new() -> Self {
        Self {
            waiters: [FutexWaiter::empty(); WAITERS],
        }
    }

    pub fn wait(
        &mut self,
        task: TaskId,
        address: u64,
        observed: u32,
        expected: u32,
    ) -> Result<(), ProcessError> {
        if address & 3 != 0 {
            return Err(ProcessError::InvalidFutex);
        }
        if observed != expected {
            return Err(ProcessError::WouldBlock);
        }
        let slot = self
            .waiters
            .iter_mut()
            .find(|waiter| !waiter.active)
            .ok_or(ProcessError::FutexFull)?;
        *slot = FutexWaiter {
            active: true,
            task,
            address,
        };
        Ok(())
    }

    pub fn wake(
        &mut self,
        address: u64,
        maximum: usize,
        output: &mut [TaskId],
    ) -> Result<usize, ProcessError> {
        if address & 3 != 0 {
            return Err(ProcessError::InvalidFutex);
        }
        let limit = core::cmp::min(maximum, output.len());
        let mut count = 0;
        for waiter in &mut self.waiters {
            if count == limit {
                break;
            }
            if waiter.active && waiter.address == address {
                output[count] = waiter.task;
                *waiter = FutexWaiter::empty();
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn remove_task(&mut self, task: TaskId) {
        for waiter in &mut self.waiters {
            if waiter.active && waiter.task == task {
                *waiter = FutexWaiter::empty();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_lifecycle_uses_virtual_generation_safe_identity() {
        let mut table: ProcessTable<4> = ProcessTable::new();
        let init = table.create_init(Credentials::root()).unwrap();
        assert_eq!(init.pid, 1);
        let child = table.fork(init).unwrap();
        assert_eq!(table.view(child).unwrap().parent, 1);
        assert_eq!(table.exec(child), Ok(1));
        table.exit(child, 17).unwrap();
        assert_eq!(table.wait(init, Some(child.pid)).unwrap().status, 17);
        assert_eq!(table.view(child), Err(ProcessError::InvalidTask));
        let replacement = table.fork(init).unwrap();
        assert_ne!(replacement, child);
        assert!(replacement.pid > child.pid);
    }

    #[test]
    fn threads_groups_and_sessions_remain_virtual() {
        let mut table: ProcessTable<8> = ProcessTable::new();
        let init = table.create_init(Credentials::root()).unwrap();
        let child = table.fork(init).unwrap();
        let thread = table.clone_thread(child).unwrap();
        assert_eq!(table.view(thread).unwrap().tgid, child.pid);
        table.set_process_group(child, 77).unwrap();
        assert_eq!(table.create_session(child), Ok(child.pid));
        assert_eq!(table.view(child).unwrap().session, child.pid);
    }

    #[test]
    fn parent_establishes_child_job_control_group() {
        let mut table: ProcessTable<8> = ProcessTable::new();
        let init = table.create_init(Credentials::root()).unwrap();
        let shell = table.fork(init).unwrap();
        let job = table.fork(shell).unwrap();
        assert_eq!(
            table.set_process_group_for(shell, job.pid, job.pid),
            Ok(job)
        );
        assert_eq!(table.view(job).unwrap().process_group, job.pid);
        assert_eq!(table.set_process_group_for(job, 0, 0), Ok(job));
        assert_eq!(
            table.set_process_group_for(init, job.pid, job.pid),
            Err(ProcessError::PermissionDenied)
        );
    }

    #[test]
    fn virtual_credentials_never_require_host_privilege() {
        let mut table: ProcessTable<4> = ProcessTable::new();
        let init = table.create_init(Credentials::root()).unwrap();
        let child = table.fork(init).unwrap();
        let credentials = table.credentials_mut(child).unwrap();
        credentials.set_groups(&[10, 20]).unwrap();
        credentials.drop_capabilities(CAP_SYS_ADMIN);
        credentials.set_gid(1000).unwrap();
        credentials.set_uid(1000).unwrap();
        assert_eq!(credentials.groups(), &[10, 20]);
        assert_eq!(credentials.capabilities, 0);
        assert_eq!(credentials.set_uid(0), Err(ProcessError::PermissionDenied));
    }

    #[test]
    fn signals_are_masked_queued_and_delivered_deterministically() {
        let mut table: ProcessTable<2> = ProcessTable::new();
        let init = table.create_init(Credentials::root()).unwrap();
        table.set_signal_mask(init, 1 << (15 - 1)).unwrap();
        table.signal(init, 15).unwrap();
        table.signal(init, 2).unwrap();
        assert_eq!(table.next_signal(init), Ok(Some(2)));
        assert_eq!(table.next_signal(init), Ok(None));
        table.set_signal_mask(init, 0).unwrap();
        assert_eq!(table.next_signal(init), Ok(Some(15)));
        assert_eq!(table.signal(init, 65), Err(ProcessError::InvalidSignal));
        table
            .set_signal_action(init, 10, SignalAction::Handler(0x4000))
            .unwrap();
        assert_eq!(
            table.signal_action(init, 10),
            Ok(SignalAction::Handler(0x4000))
        );
        table.signal(init, 10).unwrap();
        assert_eq!(table.blocking_interrupted(init), Ok(true));
        assert_eq!(
            table.begin_signal(init),
            Ok(Some(SignalDelivery::Handler {
                signal: 10,
                address: 0x4000,
            }))
        );
        table.signal_return(init).unwrap();
        assert_eq!(table.signal_return(init), Err(ProcessError::InvalidState));
        table.exec(init).unwrap();
        assert_eq!(table.signal_action(init, 10), Ok(SignalAction::Default));
        assert_eq!(
            table.set_signal_action(init, 9, SignalAction::Ignore),
            Err(ProcessError::InvalidSignal)
        );
    }

    #[test]
    fn eventfd_and_futex_have_bounded_linux_style_results() {
        let mut event = EventFd::new(0, false);
        assert_eq!(event.read(), Err(ProcessError::WouldBlock));
        event.write(7).unwrap();
        assert_eq!(event.read(), Ok(7));

        let mut table: ProcessTable<3> = ProcessTable::new();
        let init = table.create_init(Credentials::root()).unwrap();
        let child = table.fork(init).unwrap();
        let mut futex: FutexTable<2> = FutexTable::new();
        futex.wait(child, 0x1000, 4, 4).unwrap();
        let mut woken = [init; 2];
        assert_eq!(futex.wake(0x1000, 1, &mut woken), Ok(1));
        assert_eq!(woken[0], child);
        assert_eq!(futex.wait(child, 3, 1, 1), Err(ProcessError::InvalidFutex));
    }

    #[test]
    fn exhaustion_invalid_wait_and_shutdown_cleanup_fail_closed() {
        let mut table: ProcessTable<2> = ProcessTable::new();
        let init = table.create_init(Credentials::root()).unwrap();
        let child = table.fork(init).unwrap();
        assert_eq!(table.fork(init), Err(ProcessError::TableFull));
        assert_eq!(
            table.wait(child, Some(init.pid)),
            Err(ProcessError::NotChild)
        );
        assert_eq!(table.shutdown(137), 2);
        assert_eq!(table.reap_all(), 2);
        assert_eq!(table.active(), 0);
        assert_eq!(table.view(child), Err(ProcessError::InvalidTask));
    }

    #[test]
    fn virtual_clocks_timers_and_epoll_are_deterministic() {
        let mut clock = VirtualClock::new();
        clock.set_realtime_ns(1_000_000).unwrap();
        assert_eq!(clock.advance(100), Ok(100));
        assert_eq!(clock.realtime_ns(), Ok(1_000_100));

        let mut timers: TimerTable<2> = TimerTable::new();
        let timer = timers.create().unwrap();
        timers.arm(timer, clock.monotonic_ns(), 50, 25).unwrap();
        timers.update(clock.advance(125).unwrap());
        assert_eq!(timers.read(timer), Ok(4));
        assert_eq!(timers.read(timer), Err(ProcessError::WouldBlock));
        timers.close(timer).unwrap();

        let mut poll: EventPoll<2> = EventPoll::new();
        poll.add(7, EVENT_READ | EVENT_WRITE).unwrap();
        assert_eq!(poll.ready(7, EVENT_READ), Ok(EVENT_READ));
        assert_eq!(poll.ready(8, EVENT_READ), Err(ProcessError::InvalidEvent));
        poll.remove(7).unwrap();
    }
}
