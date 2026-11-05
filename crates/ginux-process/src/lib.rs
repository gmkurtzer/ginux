#![no_std]

pub const MAX_GROUPS: usize = 8;
pub const MAX_SIGNAL: u8 = 64;
pub const CAP_SYS_ADMIN: u64 = 1 << 21;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessError {
    TableFull,
    DescriptorFull,
    PipeFull,
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
    /// Terminated by an unhandled signal (number, no core dump tracking).
    Signaled(u8),
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

/// `wait4` option bits the managed wait path understands. `WNOHANG`,
/// `WUNTRACED`, and `WCONTINUED` match Linux; the `__W*` thread-selection
/// bits are accepted and have no effect because thread-group members are
/// joined over a futex and never reaped by `wait`. Any other bit is an
/// explicit caller error. Job-control stops are not mediated, so no stopped
/// or continued child is ever produced; accepting `WUNTRACED`/`WCONTINUED`
/// only keeps libc callers that pass them from failing.
pub const WNOHANG: u32 = 1;
pub const WUNTRACED: u32 = 2;
pub const WCONTINUED: u32 = 8;
pub const WAIT_THREAD_FLAGS: u32 = 0x2000_0000 | 0x4000_0000 | 0x8000_0000;
pub const WAIT_ALLOWED_OPTIONS: u32 = WNOHANG | WUNTRACED | WCONTINUED | WAIT_THREAD_FLAGS;

/// Which children a `wait` call selects. Matches `wait4` pid semantics:
/// any child, the caller's process group (`wait4` pid 0, resolved by the
/// caller), one process group, or one specific child pid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitTarget {
    Any,
    Group(u32),
    Specific(u32),
}

/// How a reaped child terminated: an exit code or a fatal signal number.
/// Encodes to the Linux wait status word with [`ChildStatus::word`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildStatus {
    Exited(i32),
    Signaled(u8),
}

impl ChildStatus {
    /// Linux wait status word: exit code in bits 8-15, signal number in
    /// bits 0-6 with no core-dump bit.
    pub const fn word(self) -> u32 {
        match self {
            ChildStatus::Exited(code) => ((code as u32) & 0xff) << 8,
            ChildStatus::Signaled(signal) => (signal as u32) & 0x7f,
        }
    }

    /// Shell-style summary (`128 + signal` for signaled children) for
    /// diagnostics that previously only carried exit codes.
    pub const fn shell_code(self) -> i32 {
        match self {
            ChildStatus::Exited(code) => code,
            ChildStatus::Signaled(signal) => 128 + signal as i32,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WaitResult {
    pub child: TaskId,
    pub status: ChildStatus,
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

    /// Latest view for a virtual pid in any occupied state, or `None` once
    /// reaped. Selector validation uses this; pids are never reused within
    /// a table, so no generation check applies.
    pub fn lookup(&self, pid: u32) -> Option<TaskView> {
        self.tasks
            .iter()
            .find(|task| task.occupied && task.id.pid == pid)
            .map(|task| TaskView {
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
            parent.tgid,
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

    /// Retire a dead thread immediately; only process leaders are waitable.
    pub fn reap_thread(&mut self, id: TaskId) -> Result<(), ProcessError> {
        let index = self.index(id)?;
        let task = self.tasks[index];
        if task.id.pid == task.tgid || task.state == TaskState::Running {
            return Err(ProcessError::InvalidState);
        }
        self.tasks[index] = Task::empty();
        Ok(())
    }

    /// Linux exec collapses a thread group into its leader's identity.
    pub fn exec_group(&mut self, caller: TaskId) -> Result<(TaskId, u32), ProcessError> {
        let caller_index = self.index(caller)?;
        let mut replacement = self.tasks[caller_index];
        let leader_index = self
            .tasks
            .iter()
            .position(|task| task.occupied && task.id.pid == replacement.tgid)
            .ok_or(ProcessError::InvalidTask)?;
        let leader = self.tasks[leader_index];
        replacement.id = leader.id;
        replacement.parent = leader.parent;
        replacement.state = TaskState::Running;
        replacement.exec_epoch = leader.exec_epoch;
        for task in &mut self.tasks {
            if task.occupied && task.tgid == leader.id.pid {
                *task = Task::empty();
            }
        }
        self.tasks[leader_index] = replacement;
        let epoch = self.exec(leader.id)?;
        Ok((leader.id, epoch))
    }

    /// Record termination by an unhandled signal. The task must be running;
    /// zombies keep whatever outcome they already reported.
    pub fn signaled(&mut self, id: TaskId, signal: u8) -> Result<(), ProcessError> {
        if signal == 0 || signal > MAX_SIGNAL {
            return Err(ProcessError::InvalidSignal);
        }
        let index = self.index(id)?;
        if self.tasks[index].state != TaskState::Running {
            return Err(ProcessError::InvalidState);
        }
        self.tasks[index].state = TaskState::Signaled(signal);
        Ok(())
    }

    fn is_waitable_child(&self, task: &Task, parent_pid: u32, target: WaitTarget) -> bool {
        // Thread-group members are joined over a futex, never reaped: only
        // leaders (tgid == pid) are waitable children.
        task.occupied
            && task.parent == parent_pid
            && task.id.pid == task.tgid
            && match target {
                WaitTarget::Any => true,
                WaitTarget::Group(group) => task.process_group == group,
                WaitTarget::Specific(pid) => task.id.pid == pid,
            }
    }

    /// Whether the caller has any unwaited-for child matching the target,
    /// running or already reportable. The supervision loop maps "none" to
    /// `ECHILD` even for blocking waits, matching the kernel.
    pub fn has_waitable_child(
        &self,
        parent: TaskId,
        target: WaitTarget,
    ) -> Result<bool, ProcessError> {
        let parent_index = self.index(parent)?;
        if self.tasks[parent_index].state != TaskState::Running {
            return Err(ProcessError::InvalidState);
        }
        let parent_pid = self.tasks[parent_index].tgid;
        Ok(self
            .tasks
            .iter()
            .any(|task| self.is_waitable_child(task, parent_pid, target)))
    }

    /// Reap one reportable child: exited or signaled. Threads are never
    /// reportable (`NotChild` for a specific thread pid). A specific pid
    /// that names no child is `NotChild`; anything else with nothing
    /// reportable yet is `NoExitedChild`, which the supervisor turns into a
    /// parked blocking wait or a zero `WNOHANG` return. `options` is
    /// accepted for interface symmetry and currently only selects which
    /// outcomes exist; no stopped or continued state is produced because
    /// job-control stops are not mediated.
    pub fn wait(
        &mut self,
        parent: TaskId,
        target: WaitTarget,
        _options: u32,
    ) -> Result<WaitResult, ProcessError> {
        let parent_index = self.index(parent)?;
        if self.tasks[parent_index].state != TaskState::Running {
            return Err(ProcessError::InvalidState);
        }
        let parent_pid = self.tasks[parent_index].tgid;
        let child = self.tasks.iter().position(|task| {
            self.is_waitable_child(task, parent_pid, target)
                && matches!(task.state, TaskState::Exited(_) | TaskState::Signaled(_))
        });
        if let Some(index) = child {
            let task = self.tasks[index];
            let status = match task.state {
                TaskState::Exited(code) => ChildStatus::Exited(code),
                TaskState::Signaled(signal) => ChildStatus::Signaled(signal),
                _ => return Err(ProcessError::InvalidState),
            };
            self.tasks[index] = Task::empty();
            return Ok(WaitResult {
                child: task.id,
                status,
            });
        }
        if let WaitTarget::Specific(pid) = target {
            if !self.tasks.iter().any(|task| {
                task.occupied
                    && task.parent == parent_pid
                    && task.id.pid == pid
                    && task.id.pid == task.tgid
            }) {
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
            if task.occupied && matches!(task.state, TaskState::Exited(_) | TaskState::Signaled(_))
            {
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

/// G05a descriptor tracking: pipe pairs, dups, and close-on-exec ends as
/// supervisor-owned domain objects.
///
/// The supervision loop lets descriptor syscalls execute on the host: pipe
/// pairs, dups, and closes are real kernel objects, so data flows, offsets
/// stay shared, and close-on-exec closes happen in the kernel. The loop
/// mirrors every outcome here so it can gate reads/writes to domain objects,
/// drop close-on-exec ends at exec, and inherit ends across fork/clone. No
/// supervisor descriptor is ever handed to a tracee: every tracked fd is the
/// tracee's own host fd, created by its own syscalls under supervision.
///
/// Inheritance copies the parent's entries and retains each pipe end; the
/// host duplicated the same table with the address space, so the mirror
/// agrees. CLONE_FILES shares one table and exec first unshares it.
/// Task death releases entries only when the last table owner exits,
/// matching the host's descriptor lifetime.
///
/// LIMIT (merge with the G07 lineage): this mirror tracks pipe and dup ends
/// only. The G07 file-descriptor table lives on its own branch; at merge
/// the two converge into one descriptor table (open-file kinds join here,
/// dup must cross both kinds, and a single close path must release both).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FdKind {
    PipeRead,
    PipeWrite,
    External,
}

/// One tracked descriptor: the tracee-visible fd number, what it names, the
/// pipe object id (`0` for [`FdKind::External` aliases of pre-supervision
/// stdio), and whether exec closes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FdEntry {
    pub fd: u32,
    pub kind: FdKind,
    pub pipe: u32,
    pub cloexec: bool,
}

#[derive(Clone, Copy)]
struct PipeObject {
    live: bool,
    ends: u32,
}

impl PipeObject {
    const fn empty() -> Self {
        Self {
            live: false,
            ends: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct TaskDescriptors<const FDS: usize, const MEMBERS: usize> {
    used: bool,
    members: [u32; MEMBERS],
    entries: [Option<FdEntry>; FDS],
}

impl<const FDS: usize, const MEMBERS: usize> TaskDescriptors<FDS, MEMBERS> {
    const fn empty() -> Self {
        Self {
            used: false,
            members: [0; MEMBERS],
            entries: [None; FDS],
        }
    }
}

/// Supervisor-owned descriptor mirror: per-task fd entries plus the pipe
/// objects they name. Keyed by virtual pid; sized to the supervision loop's
/// task table. Every mutation follows a host outcome the loop observed, so
/// the mirror never invents a descriptor the kernel does not have.
pub struct PipeSpace<const TASKS: usize, const FDS: usize, const PIPES: usize> {
    tasks: [TaskDescriptors<FDS, TASKS>; TASKS],
    pipes: [PipeObject; PIPES],
}

impl<const TASKS: usize, const FDS: usize, const PIPES: usize> Default
    for PipeSpace<TASKS, FDS, PIPES>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const TASKS: usize, const FDS: usize, const PIPES: usize> PipeSpace<TASKS, FDS, PIPES> {
    pub const fn new() -> Self {
        Self {
            tasks: [TaskDescriptors::empty(); TASKS],
            pipes: [PipeObject::empty(); PIPES],
        }
    }

    fn task_index(&self, task: u32) -> Option<usize> {
        self.tasks
            .iter()
            .position(|slot| slot.used && slot.members.contains(&task))
    }

    fn task_or_alloc(&mut self, task: u32) -> Result<usize, ProcessError> {
        if let Some(index) = self.task_index(task) {
            return Ok(index);
        }
        let index = self
            .tasks
            .iter()
            .position(|slot| !slot.used)
            .ok_or(ProcessError::TableFull)?;
        self.tasks[index].used = true;
        self.tasks[index].members[0] = task;
        Ok(index)
    }

    fn pipe_alloc(&mut self) -> Result<usize, ProcessError> {
        self.pipes
            .iter()
            .position(|pipe| !pipe.live)
            .ok_or(ProcessError::PipeFull)
    }

    fn retain(&mut self, pipe: u32) {
        if pipe == 0 {
            return;
        }
        let index = pipe as usize - 1;
        if index < PIPES && self.pipes[index].live {
            self.pipes[index].ends += 1;
        }
    }

    fn release(&mut self, entry: FdEntry) {
        if entry.kind == FdKind::External || entry.pipe == 0 {
            return;
        }
        let index = entry.pipe as usize - 1;
        if index < PIPES && self.pipes[index].live && self.pipes[index].ends > 0 {
            self.pipes[index].ends -= 1;
            if self.pipes[index].ends == 0 {
                self.pipes[index] = PipeObject::empty();
            }
        }
    }

    fn evict(&mut self, slot: usize, fd: u32) {
        if let Some(position) = self.tasks[slot]
            .entries
            .iter()
            .position(|entry| entry.is_some_and(|entry| entry.fd == fd))
        {
            if let Some(previous) = self.tasks[slot].entries[position].take() {
                self.release(previous);
            }
        }
    }

    /// What a task's descriptor names, or `None` for untracked numbers
    /// (stdio the tracee held before supervision, or nothing at all). The
    /// supervision loop gates reads/writes on this: tracked ends and stdio
    /// execute on the host, anything else fails closed.
    pub fn lookup(&self, task: u32, fd: u32) -> Option<FdEntry> {
        let index = self.task_index(task)?;
        self.tasks[index].entries.iter().find_map(|entry| {
            entry.and_then(|entry| if entry.fd == fd { Some(entry) } else { None })
        })
    }

    /// Record a pipe pair the host just created. Both numbers must be fresh:
    /// the kernel returns the lowest free descriptors, so a tracked number
    /// here means the mirror is corrupt and the loop must fail honestly.
    pub fn create_pipe(
        &mut self,
        task: u32,
        read_fd: u32,
        write_fd: u32,
        cloexec: bool,
    ) -> Result<u32, ProcessError> {
        if self.lookup(task, read_fd).is_some() || self.lookup(task, write_fd).is_some() {
            return Err(ProcessError::InvalidState);
        }
        let slot = self.task_or_alloc(task)?;
        let free = self.tasks[slot]
            .entries
            .iter()
            .filter(|entry| entry.is_none())
            .count();
        if free < 2 {
            return Err(ProcessError::DescriptorFull);
        }
        let pipe = self.pipe_alloc()?;
        let id = pipe as u32 + 1;
        self.pipes[pipe] = PipeObject {
            live: true,
            ends: 2,
        };
        let mut placed = 0;
        for entry in self.tasks[slot].entries.iter_mut() {
            if entry.is_none() {
                *entry = Some(if placed == 0 {
                    FdEntry {
                        fd: read_fd,
                        kind: FdKind::PipeRead,
                        pipe: id,
                        cloexec,
                    }
                } else {
                    FdEntry {
                        fd: write_fd,
                        kind: FdKind::PipeWrite,
                        pipe: id,
                        cloexec,
                    }
                });
                placed += 1;
                if placed == 2 {
                    break;
                }
            }
        }
        Ok(id)
    }

    /// Record a duplication the host just performed: `dup`, `dup2`, `dup3`,
    /// or `fcntl` `F_DUPFD`/`F_DUPFD_CLOEXEC`. The source is either a tracked
    /// end (same object, retained) or pre-supervision stdio (an
    /// [`FdKind::External`] alias). A previous occupant at the target number
    /// is closed first, exactly like the kernel did. `old == new` is a
    /// no-op: host success already proved the number valid.
    pub fn duplicate(
        &mut self,
        task: u32,
        old_fd: u32,
        new_fd: u32,
        cloexec: bool,
    ) -> Result<(), ProcessError> {
        if old_fd == new_fd {
            return Ok(());
        }
        let source = match self.lookup(task, old_fd) {
            Some(entry) => FdEntry {
                fd: new_fd,
                kind: entry.kind,
                pipe: entry.pipe,
                cloexec,
            },
            None if old_fd <= 2 => FdEntry {
                fd: new_fd,
                kind: FdKind::External,
                pipe: 0,
                cloexec,
            },
            None => return Err(ProcessError::InvalidState),
        };
        let slot = self.task_or_alloc(task)?;
        let occupied = self.tasks[slot]
            .entries
            .iter()
            .any(|entry| entry.is_some_and(|entry| entry.fd == new_fd));
        if !occupied && self.tasks[slot].entries.iter().all(|entry| entry.is_some()) {
            return Err(ProcessError::DescriptorFull);
        }
        self.evict(slot, new_fd);
        if source.pipe != 0 {
            self.retain(source.pipe);
        }
        let position = self.tasks[slot]
            .entries
            .iter()
            .position(|entry| entry.is_none())
            .ok_or(ProcessError::DescriptorFull)?;
        self.tasks[slot].entries[position] = Some(source);
        Ok(())
    }

    /// Drop one descriptor after the host closed it. Returns whether a
    /// tracked end was released; untracked numbers (plain stdio closes the
    /// host also performed) are a silent no-op.
    pub fn close(&mut self, task: u32, fd: u32) -> Result<bool, ProcessError> {
        let Some(slot) = self.task_index(task) else {
            return Ok(false);
        };
        let Some(position) = self.tasks[slot]
            .entries
            .iter()
            .position(|entry| entry.is_some_and(|entry| entry.fd == fd))
        else {
            return Ok(false);
        };
        if let Some(entry) = self.tasks[slot].entries[position].take() {
            self.release(entry);
            return Ok(true);
        }
        Ok(false)
    }

    /// Follow an `F_SETFD` the host performed. Untracked numbers have no
    /// mirror entry and need none: the kernel holds their real flag.
    pub fn set_cloexec(&mut self, task: u32, fd: u32, cloexec: bool) -> Result<(), ProcessError> {
        let Some(slot) = self.task_index(task) else {
            return Ok(());
        };
        for entry in self.tasks[slot].entries.iter_mut().flatten() {
            if entry.fd == fd {
                entry.cloexec = cloexec;
                break;
            }
        }
        Ok(())
    }

    /// Copy the host descriptor table on fork (without CLONE_FILES).
    pub fn inherit(&mut self, child: u32, parent: u32) -> Result<(), ProcessError> {
        let Some(parent_slot) = self.task_index(parent) else {
            return Ok(());
        };
        let entries = self.tasks[parent_slot].entries;
        self.release_task(child);
        let child_slot = self.task_or_alloc(child)?;
        self.tasks[child_slot].entries = entries;
        for entry in entries.iter().flatten() {
            self.retain(entry.pipe);
        }
        Ok(())
    }

    /// CLONE_FILES shares one table, including mutations made after clone.
    pub fn share(&mut self, child: u32, parent: u32) -> Result<(), ProcessError> {
        let slot = self.task_or_alloc(parent)?;
        let member = self.tasks[slot]
            .members
            .iter_mut()
            .find(|member| **member == 0)
            .ok_or(ProcessError::TableFull)?;
        *member = child;
        Ok(())
    }

    /// Exec unshares files before closing CLOEXEC descriptors.
    pub fn unshare(&mut self, task: u32) -> Result<(), ProcessError> {
        let Some(slot) = self.task_index(task) else {
            return Ok(());
        };
        if self.tasks[slot]
            .members
            .iter()
            .filter(|member| **member != 0)
            .count()
            > 1
        {
            self.inherit(task, task)?;
        }
        Ok(())
    }

    /// Follow the host tid change when a non-leader thread executes.
    pub fn rename_task(&mut self, old: u32, new: u32) {
        if let Some(slot) = self.task_index(old) {
            for member in &mut self.tasks[slot].members {
                if *member == old {
                    *member = new;
                }
            }
        }
    }

    /// Drop every close-on-exec end after the host closed them during
    /// executable replacement, after unsharing the exec'ing task's table.
    /// Returns the count dropped for the audit line.
    pub fn exec_scrub(&mut self, task: u32) -> u32 {
        let Some(slot) = self.task_index(task) else {
            return 0;
        };
        let mut closed = [None; FDS];
        let mut count = 0;
        for entry in self.tasks[slot].entries.iter_mut() {
            if entry.is_some_and(|entry| entry.cloexec) {
                closed[count] = entry.take();
                count += 1;
            }
        }
        for entry in closed.iter().take(count).flatten() {
            self.release(*entry);
        }
        count as u32
    }

    /// Release one table owner; descriptors survive while another owner lives.
    pub fn release_task(&mut self, task: u32) {
        let Some(slot) = self.task_index(task) else {
            return;
        };
        for member in &mut self.tasks[slot].members {
            if *member == task {
                *member = 0;
            }
        }
        if self.tasks[slot].members.iter().any(|member| *member != 0) {
            return;
        }
        let entries = self.tasks[slot].entries;
        for entry in entries.iter().flatten() {
            self.release(*entry);
        }
        self.tasks[slot] = TaskDescriptors::empty();
    }

    /// Ends still open on a pipe object: the last-close check. Unknown or
    /// reaped pipes report zero.
    pub fn pipe_ends(&self, pipe: u32) -> u32 {
        if pipe == 0 {
            return 0;
        }
        let index = pipe as usize - 1;
        if index < PIPES && self.pipes[index].live {
            self.pipes[index].ends
        } else {
            0
        }
    }

    /// Live pipe objects, for leak-check diagnostics.
    pub fn active_pipes(&self) -> usize {
        self.pipes.iter().filter(|pipe| pipe.live).count()
    }

    /// Tracked descriptors held by one task, for leak-check diagnostics.
    pub fn task_fds(&self, task: u32) -> usize {
        let Some(slot) = self.task_index(task) else {
            return 0;
        };
        self.tasks[slot]
            .entries
            .iter()
            .filter(|entry| entry.is_some())
            .count()
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
        assert_eq!(
            table
                .wait(init, WaitTarget::Specific(child.pid), 0)
                .unwrap()
                .status,
            ChildStatus::Exited(17)
        );
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
            table.wait(child, WaitTarget::Specific(init.pid), 0),
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
