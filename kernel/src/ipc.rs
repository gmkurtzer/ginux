use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

const CAPACITY: usize = 4;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut CONTROL_CHANNEL: MaybeUninit<Duplex> = MaybeUninit::uninit();

struct Duplex {
    requests: Channel,
    replies: Channel,
}

impl Duplex {
    const fn new() -> Self {
        Self {
            requests: Channel::new(),
            replies: Channel::new(),
        }
    }

    fn send(&mut self, from_service: bool, message: u64) -> bool {
        if from_service {
            self.replies.send(message)
        } else {
            self.requests.send(message)
        }
    }

    fn receive(&mut self, for_service: bool) -> Option<u64> {
        if for_service {
            self.requests.receive()
        } else {
            self.replies.receive()
        }
    }

    const fn is_empty(&self) -> bool {
        self.requests.len == 0 && self.replies.len == 0
    }
}

struct Channel {
    messages: [u64; CAPACITY],
    head: usize,
    len: usize,
}

impl Channel {
    const fn new() -> Self {
        Self {
            messages: [0; CAPACITY],
            head: 0,
            len: 0,
        }
    }

    fn send(&mut self, message: u64) -> bool {
        if self.len == CAPACITY {
            return false;
        }
        let tail = (self.head + self.len) % CAPACITY;
        self.messages[tail] = message;
        self.len += 1;
        true
    }

    fn receive(&mut self) -> Option<u64> {
        if self.len == 0 {
            return None;
        }
        let message = self.messages[self.head];
        self.messages[self.head] = 0;
        self.head = (self.head + 1) % CAPACITY;
        self.len -= 1;
        Some(message)
    }
}

pub(crate) fn initialize_control_channel() -> bool {
    if INITIALIZED.load(Ordering::Acquire) {
        return false;
    }
    unsafe {
        core::ptr::addr_of_mut!(CONTROL_CHANNEL).write(MaybeUninit::new(Duplex::new()));
    }
    INITIALIZED.store(true, Ordering::Release);
    true
}

pub(crate) fn send(from_service: bool, message: u64) -> bool {
    channel_mut().is_some_and(|channel| channel.send(from_service, message))
}

pub(crate) fn receive(for_service: bool) -> Option<u64> {
    channel_mut()?.receive(for_service)
}

pub(crate) fn is_empty() -> bool {
    channel_mut().is_some_and(|channel| channel.is_empty())
}

pub(crate) fn verify_channel_invariants() -> bool {
    let mut channel = Duplex::new();
    channel.receive(false).is_none()
        && channel.receive(true).is_none()
        && channel.send(false, 10)
        && channel.send(false, 20)
        && channel.send(false, 30)
        && channel.send(false, 40)
        && !channel.send(false, 50)
        && channel.receive(false).is_none()
        && channel.receive(true) == Some(10)
        && channel.receive(true) == Some(20)
        && channel.send(false, 50)
        && channel.receive(true) == Some(30)
        && channel.receive(true) == Some(40)
        && channel.receive(true) == Some(50)
        && channel.send(true, 60)
        && channel.receive(true).is_none()
        && channel.receive(false) == Some(60)
        && channel.is_empty()
}

fn channel_mut() -> Option<&'static mut Duplex> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &mut *core::ptr::addr_of_mut!(CONTROL_CHANNEL).cast::<Duplex>() })
}
