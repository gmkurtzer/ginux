use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

const CAPACITY: usize = 4;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut CONTROL_CHANNEL: MaybeUninit<Channel> = MaybeUninit::uninit();

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
        core::ptr::addr_of_mut!(CONTROL_CHANNEL).write(MaybeUninit::new(Channel::new()));
    }
    INITIALIZED.store(true, Ordering::Release);
    true
}

pub(crate) fn send(message: u64) -> bool {
    channel_mut().is_some_and(|channel| channel.send(message))
}

pub(crate) fn receive() -> Option<u64> {
    channel_mut()?.receive()
}

pub(crate) fn is_empty() -> bool {
    channel_mut().is_some_and(|channel| channel.len == 0)
}

pub(crate) fn verify_channel_invariants() -> bool {
    let mut channel = Channel::new();
    channel.receive().is_none()
        && channel.send(10)
        && channel.send(20)
        && channel.send(30)
        && channel.send(40)
        && !channel.send(50)
        && channel.receive() == Some(10)
        && channel.receive() == Some(20)
        && channel.send(50)
        && channel.receive() == Some(30)
        && channel.receive() == Some(40)
        && channel.receive() == Some(50)
        && channel.receive().is_none()
}

fn channel_mut() -> Option<&'static mut Channel> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &mut *core::ptr::addr_of_mut!(CONTROL_CHANNEL).cast::<Channel>() })
}
