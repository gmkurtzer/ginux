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

    fn create(&mut self, domain: u64) -> Option<ThreadId> {
        if domain == 0 {
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
        });
        Some(id)
    }

    fn schedule_next(&mut self) -> Option<ThreadId> {
        if self.current.is_some() {
            return self.current;
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
                return self.current;
            }
        }
        None
    }

    fn yield_current(&mut self, id: ThreadId) -> Option<ThreadId> {
        if self.current != Some(id) {
            return None;
        }
        let thread = self.thread_mut(id)?;
        if thread.state != State::Running {
            return None;
        }
        thread.state = State::Ready;
        self.current = None;
        self.schedule_next()
    }

    fn exit_current(&mut self, id: ThreadId) -> Option<ThreadId> {
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
    let mut scheduler = Scheduler::new();
    let first = scheduler.create(1)?;
    let second = scheduler.create(1)?;
    if scheduler.schedule_next() != Some(first)
        || scheduler.thread(first)?.domain != 1
        || scheduler.yield_current(second).is_some()
        || scheduler.yield_current(first) != Some(second)
        || scheduler.thread(first)?.state != State::Ready
        || scheduler.thread(second)?.state != State::Running
        || scheduler.exit_current(second) != Some(first)
        || scheduler.thread(second).is_some()
        || scheduler.exit_current(first).is_some()
        || scheduler.schedule_next().is_some()
    {
        return Some(false);
    }

    let replacement = scheduler.create(2)?;
    Some(
        replacement != first
            && replacement.slot == first.slot
            && scheduler.thread(first).is_none()
            && scheduler.thread(replacement)?.domain == 2,
    )
}
