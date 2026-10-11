use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

const CAPACITY: usize = 4;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut CONTROL_CHANNEL: MaybeUninit<Duplex> = MaybeUninit::uninit();

struct Duplex {
    control_domain: u64,
    service_domain: Option<u64>,
    requests: Channel,
    replies: Channel,
}

impl Duplex {
    fn new(control_domain: u64, service_domain: u64) -> Option<Self> {
        if control_domain == 0 || service_domain == 0 || control_domain == service_domain {
            return None;
        }
        Some(Self {
            control_domain,
            service_domain: Some(service_domain),
            requests: Channel::new(),
            replies: Channel::new(),
        })
    }

    fn send(&mut self, domain: u64, message: u64) -> bool {
        if self.service_domain == Some(domain) {
            self.replies.send(message)
        } else if self.control_domain == domain && self.service_domain.is_some() {
            self.requests.send(message)
        } else {
            false
        }
    }

    fn receive(&mut self, domain: u64) -> Option<u64> {
        if self.service_domain == Some(domain) {
            self.requests.receive()
        } else if self.control_domain == domain {
            self.replies.receive()
        } else {
            None
        }
    }

    fn retire_service(&mut self, domain: u64) -> bool {
        if self.service_domain != Some(domain) {
            return false;
        }
        self.requests.clear();
        self.service_domain = None;
        true
    }

    fn bind_service(&mut self, domain: u64) -> bool {
        if domain == 0 || domain == self.control_domain || self.service_domain.is_some() {
            return false;
        }
        self.service_domain = Some(domain);
        true
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

    fn clear(&mut self) {
        self.messages.fill(0);
        self.head = 0;
        self.len = 0;
    }
}

pub(crate) fn initialize_control_channel(control_domain: u64, service_domain: u64) -> bool {
    if INITIALIZED.load(Ordering::Acquire) {
        return false;
    }
    let Some(channel) = Duplex::new(control_domain, service_domain) else {
        return false;
    };
    unsafe {
        core::ptr::addr_of_mut!(CONTROL_CHANNEL).write(MaybeUninit::new(channel));
    }
    INITIALIZED.store(true, Ordering::Release);
    true
}

pub(crate) fn send(domain: u64, message: u64) -> bool {
    channel_mut().is_some_and(|channel| channel.send(domain, message))
}

pub(crate) fn receive(domain: u64) -> Option<u64> {
    channel_mut()?.receive(domain)
}

pub(crate) fn retire_service(domain: u64) -> bool {
    channel_mut().is_some_and(|channel| channel.retire_service(domain))
}

pub(crate) fn bind_service(domain: u64) -> bool {
    channel_mut().is_some_and(|channel| channel.bind_service(domain))
}

pub(crate) fn is_empty() -> bool {
    channel_mut().is_some_and(|channel| channel.is_empty())
}

pub(crate) fn verify_channel_invariants() -> bool {
    let (control, service, replacement) = (1, 2, 3);
    let Some(mut channel) = Duplex::new(control, service) else {
        return false;
    };
    channel.receive(control).is_none()
        && channel.receive(service).is_none()
        && channel.send(control, 10)
        && channel.send(control, 20)
        && channel.send(control, 30)
        && channel.send(control, 40)
        && !channel.send(control, 50)
        && channel.receive(control).is_none()
        && channel.receive(service) == Some(10)
        && channel.receive(service) == Some(20)
        && channel.send(control, 50)
        && channel.receive(service) == Some(30)
        && channel.receive(service) == Some(40)
        && channel.receive(service) == Some(50)
        && channel.send(service, 60)
        && channel.receive(service).is_none()
        && channel.receive(control) == Some(60)
        && channel.send(control, 70)
        && channel.send(service, 80)
        && channel.retire_service(service)
        && !channel.retire_service(service)
        && !channel.send(service, 90)
        && channel.receive(service).is_none()
        && !channel.send(control, 90)
        && channel.receive(control) == Some(80)
        && channel.bind_service(replacement)
        && !channel.bind_service(service)
        && channel.receive(replacement).is_none()
        && channel.send(control, 100)
        && channel.receive(replacement) == Some(100)
        && channel.is_empty()
}

fn channel_mut() -> Option<&'static mut Duplex> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &mut *core::ptr::addr_of_mut!(CONTROL_CHANNEL).cast::<Duplex>() })
}
