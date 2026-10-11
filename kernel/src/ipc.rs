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
    service_closures: Channel,
    last_control_message_source: Option<u64>,
    last_service_message_source: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MessageEnvelope {
    source: u64,
    value: u64,
}

const EMPTY_MESSAGE: MessageEnvelope = MessageEnvelope {
    source: 0,
    value: 0,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Receive {
    Message(u64),
    Empty,
    PeerClosed(u64),
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
            service_closures: Channel::new(),
            last_control_message_source: None,
            last_service_message_source: None,
        })
    }

    fn send(&mut self, domain: u64, message: u64) -> bool {
        if self.service_side(domain) == Some(true) {
            self.replies.send(MessageEnvelope {
                source: domain,
                value: message,
            })
        } else if self.service_side(domain) == Some(false) && self.service_domain.is_some() {
            self.requests.send(MessageEnvelope {
                source: domain,
                value: message,
            })
        } else {
            false
        }
    }

    fn receive(&mut self, domain: u64) -> Receive {
        if self.service_side(domain) == Some(true) {
            let Some(message) = self.requests.receive() else {
                return Receive::Empty;
            };
            if message.source != self.control_domain {
                return Receive::Empty;
            }
            self.last_service_message_source = Some(message.source);
            Receive::Message(message.value)
        } else if self.service_side(domain) == Some(false) {
            if let Some(message) = self.replies.receive() {
                if message.source == 0 || message.source == self.control_domain {
                    return Receive::Empty;
                }
                self.last_control_message_source = Some(message.source);
                Receive::Message(message.value)
            } else if let Some(closure) = self.service_closures.receive() {
                if closure.source != closure.value {
                    return Receive::Empty;
                }
                Receive::PeerClosed(closure.value)
            } else {
                Receive::Empty
            }
        } else {
            Receive::Empty
        }
    }

    fn retire_service(&mut self, domain: u64) -> bool {
        if self.service_domain != Some(domain) || self.service_closures.len == CAPACITY {
            return false;
        }
        self.requests.clear();
        self.service_domain = None;
        self.service_closures.send(MessageEnvelope {
            source: domain,
            value: domain,
        })
    }

    fn bind_service(&mut self, domain: u64) -> bool {
        if domain == 0 || domain == self.control_domain || self.service_domain.is_some() {
            return false;
        }
        self.service_domain = Some(domain);
        true
    }

    fn rollback_service_binding(&mut self, domain: u64) -> bool {
        if self.service_domain != Some(domain) || self.requests.len != 0 {
            return false;
        }
        self.service_domain = None;
        true
    }

    fn service_side(&self, domain: u64) -> Option<bool> {
        if self.service_domain == Some(domain) {
            Some(true)
        } else if self.control_domain == domain {
            Some(false)
        } else {
            None
        }
    }

    fn peer_domain(&self, domain: u64) -> Option<u64> {
        if self.service_domain == Some(domain) {
            Some(self.control_domain)
        } else if self.control_domain == domain {
            self.service_domain
        } else {
            None
        }
    }

    const fn is_empty(&self) -> bool {
        self.requests.len == 0 && self.replies.len == 0
    }
}

struct Channel {
    messages: [MessageEnvelope; CAPACITY],
    head: usize,
    len: usize,
}

impl Channel {
    const fn new() -> Self {
        Self {
            messages: [EMPTY_MESSAGE; CAPACITY],
            head: 0,
            len: 0,
        }
    }

    fn send(&mut self, message: MessageEnvelope) -> bool {
        if self.len == CAPACITY {
            return false;
        }
        let tail = (self.head + self.len) % CAPACITY;
        self.messages[tail] = message;
        self.len += 1;
        true
    }

    fn receive(&mut self) -> Option<MessageEnvelope> {
        if self.len == 0 {
            return None;
        }
        let message = self.messages[self.head];
        self.messages[self.head] = EMPTY_MESSAGE;
        self.head = (self.head + 1) % CAPACITY;
        self.len -= 1;
        Some(message)
    }

    fn clear(&mut self) {
        self.messages.fill(EMPTY_MESSAGE);
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

pub(crate) fn receive(domain: u64) -> Receive {
    channel_mut().map_or(Receive::Empty, |channel| channel.receive(domain))
}

pub(crate) fn service_side(domain: u64) -> Option<bool> {
    channel_mut()?.service_side(domain)
}

pub(crate) fn peer_domain(domain: u64) -> Option<u64> {
    channel_mut()?.peer_domain(domain)
}

pub(crate) fn retire_service(domain: u64) -> bool {
    channel_mut().is_some_and(|channel| channel.retire_service(domain))
}

pub(crate) fn bind_service(domain: u64) -> bool {
    channel_mut().is_some_and(|channel| channel.bind_service(domain))
}

pub(crate) fn rollback_service_binding(domain: u64) -> bool {
    channel_mut().is_some_and(|channel| channel.rollback_service_binding(domain))
}

pub(crate) fn is_empty() -> bool {
    channel_mut().is_some_and(|channel| channel.is_empty())
}

pub(crate) fn last_message_sources() -> Option<(u64, u64)> {
    let channel = channel_mut()?;
    Some((
        channel.last_control_message_source?,
        channel.last_service_message_source?,
    ))
}

pub(crate) fn verify_channel_invariants() -> bool {
    let (control, service, replacement) = (1, 2, 3);
    let Some(mut channel) = Duplex::new(control, service) else {
        return false;
    };
    channel.receive(control) == Receive::Empty
        && channel.receive(service) == Receive::Empty
        && channel.service_side(control) == Some(false)
        && channel.service_side(service) == Some(true)
        && channel.peer_domain(control) == Some(service)
        && channel.peer_domain(service) == Some(control)
        && channel.service_side(replacement).is_none()
        && channel.send(control, 10)
        && channel.send(control, 20)
        && channel.send(control, 30)
        && channel.send(control, 40)
        && !channel.send(control, 50)
        && channel.receive(control) == Receive::Empty
        && channel.receive(service) == Receive::Message(10)
        && channel.receive(service) == Receive::Message(20)
        && channel.send(control, 50)
        && channel.receive(service) == Receive::Message(30)
        && channel.receive(service) == Receive::Message(40)
        && channel.receive(service) == Receive::Message(50)
        && channel.send(service, 60)
        && channel.receive(service) == Receive::Empty
        && channel.receive(control) == Receive::Message(60)
        && channel.send(control, 70)
        && channel.send(service, 80)
        && channel.retire_service(service)
        && !channel.retire_service(service)
        && !channel.send(service, 90)
        && channel.service_side(service).is_none()
        && channel.peer_domain(control).is_none()
        && channel.receive(service) == Receive::Empty
        && !channel.send(control, 90)
        && channel.receive(control) == Receive::Message(80)
        && channel.receive(control) == Receive::PeerClosed(service)
        && channel.bind_service(replacement)
        && channel.peer_domain(control) == Some(replacement)
        && channel.peer_domain(replacement) == Some(control)
        && channel.service_side(replacement) == Some(true)
        && !channel.bind_service(service)
        && channel.rollback_service_binding(replacement)
        && channel.service_side(replacement).is_none()
        && channel.receive(control) == Receive::Empty
        && channel.bind_service(replacement)
        && channel.receive(replacement) == Receive::Empty
        && channel.send(control, 100)
        && channel.receive(replacement) == Receive::Message(100)
        && channel.retire_service(replacement)
        && channel.receive(control) == Receive::PeerClosed(replacement)
        && channel.receive(control) == Receive::Empty
        && channel.is_empty()
        && channel.last_control_message_source == Some(service)
        && channel.last_service_message_source == Some(control)
}

fn channel_mut() -> Option<&'static mut Duplex> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &mut *core::ptr::addr_of_mut!(CONTROL_CHANNEL).cast::<Duplex>() })
}
