const MAX_THREADS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ThreadId {
    slot: u16,
    generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Ready,
    Running,
}

#[derive(Clone, Copy)]
struct Thread {
    id: ThreadId,
    domain: u64,
    state: State,
    context: UserContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
struct UserContext {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rip: u64,
    rsp: u64,
    rflags: u64,
    address_space_root: u64,
}

impl UserContext {
    fn is_valid(&self) -> bool {
        const LOWER_CANONICAL_END: u64 = (1_u64 << 47) - 1;
        self.rip <= LOWER_CANONICAL_END
            && self.rsp <= LOWER_CANONICAL_END
            && self.rip != 0
            && self.rsp != 0
            && self.rflags & 2 != 0
            && self.address_space_root != 0
            && self.address_space_root.is_multiple_of(4096)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Scheduled {
    id: ThreadId,
    context: UserContext,
}

struct Scheduler {
    threads: [Option<Thread>; MAX_THREADS],
    generations: [u32; MAX_THREADS],
    current: Option<ThreadId>,
    next_slot: usize,
}

impl Scheduler {
    const fn new() -> Self {
        Self {
            threads: [None; MAX_THREADS],
            generations: [0; MAX_THREADS],
            current: None,
            next_slot: 0,
        }
    }

    fn create(&mut self, domain: u64, context: UserContext) -> Option<ThreadId> {
        if domain == 0 || !context.is_valid() {
            return None;
        }
        let slot = self.threads.iter().position(Option::is_none)?;
        let generation = self.generations[slot].checked_add(1)?;
        let id = ThreadId {
            slot: slot as u16,
            generation,
        };
        self.generations[slot] = generation;
        self.threads[slot] = Some(Thread {
            id,
            domain,
            state: State::Ready,
            context,
        });
        Some(id)
    }

    fn schedule_next(&mut self) -> Option<Scheduled> {
        if self.current.is_some() {
            let id = self.current?;
            return Some(Scheduled {
                id,
                context: self.thread(id)?.context,
            });
        }
        for offset in 0..MAX_THREADS {
            let slot = (self.next_slot + offset) % MAX_THREADS;
            let Some(mut thread) = self.threads[slot] else {
                continue;
            };
            if thread.state == State::Ready {
                thread.state = State::Running;
                self.threads[slot] = Some(thread);
                self.current = Some(thread.id);
                self.next_slot = (slot + 1) % MAX_THREADS;
                return Some(Scheduled {
                    id: thread.id,
                    context: thread.context,
                });
            }
        }
        None
    }

    fn yield_current(&mut self, id: ThreadId, saved: UserContext) -> Option<Scheduled> {
        if self.current != Some(id) {
            return None;
        }
        if !saved.is_valid() {
            return None;
        }
        let thread = self.thread_mut(id)?;
        if thread.state != State::Running {
            return None;
        }
        thread.context = saved;
        thread.state = State::Ready;
        self.current = None;
        self.schedule_next()
    }

    fn exit_current(&mut self, id: ThreadId) -> Option<Scheduled> {
        if self.current != Some(id) || self.thread(id)?.state != State::Running {
            return None;
        }
        self.threads[usize::from(id.slot)] = None;
        self.current = None;
        self.schedule_next()
    }

    fn thread(&self, id: ThreadId) -> Option<&Thread> {
        self.threads
            .get(usize::from(id.slot))?
            .as_ref()
            .filter(|thread| thread.id == id)
    }

    fn thread_mut(&mut self, id: ThreadId) -> Option<&mut Thread> {
        self.threads
            .get_mut(usize::from(id.slot))?
            .as_mut()
            .filter(|thread| thread.id == id)
    }
}

pub(crate) fn verify_scheduler_invariants() -> bool {
    verify_scheduler_invariants_inner().unwrap_or(false)
}

fn verify_scheduler_invariants_inner() -> Option<bool> {
    let first_context = test_context(10, 0x1000, 0x3000, 0x100000);
    let second_context = test_context(20, 0x5000, 0x7000, 0x200000);
    let mut scheduler = Scheduler::new();
    if scheduler.create(0, first_context).is_some()
        || scheduler
            .create(
                1,
                UserContext {
                    rip: 0,
                    ..first_context
                },
            )
            .is_some()
    {
        return Some(false);
    }
    let first = scheduler.create(1, first_context)?;
    let second = scheduler.create(1, second_context)?;
    let first_run = scheduler.schedule_next()?;
    let mut first_saved = first_run.context;
    first_saved.rax = 11;
    if first_run.id != first
        || first_run.context != first_context
        || scheduler.thread(first)?.domain != 1
        || scheduler.yield_current(second, second_context).is_some()
        || scheduler.yield_current(first, first_saved)?.id != second
        || scheduler.thread(first)?.state != State::Ready
        || scheduler.thread(second)?.state != State::Running
        || scheduler.exit_current(second)?.id != first
        || scheduler.schedule_next()?.context != first_saved
        || scheduler.thread(second).is_some()
        || scheduler.exit_current(first).is_some()
        || scheduler.schedule_next().is_some()
    {
        return Some(false);
    }

    let replacement = scheduler.create(2, second_context)?;
    Some(
        replacement != first
            && replacement.slot == first.slot
            && scheduler.thread(first).is_none()
            && scheduler.thread(replacement)?.domain == 2,
    )
}

const fn test_context(rax: u64, rip: u64, rsp: u64, root: u64) -> UserContext {
    UserContext {
        rax,
        rbx: 0,
        rcx: 0,
        rdx: 0,
        rsi: 0,
        rdi: 0,
        rbp: 0,
        r8: 0,
        r9: 0,
        r10: 0,
        r11: 0,
        r12: 0,
        r13: 0,
        r14: 0,
        r15: 0,
        rip,
        rsp,
        rflags: 2,
        address_space_root: root,
    }
}
