use crate::owned_state::OwnedState;

const CAPACITY: usize = 4;

static CONTROL_CHANNEL: OwnedState<Duplex> = OwnedState::empty();

struct Duplex {
    control_domain: u64,
    service_domain: Option<u64>,
    requests: Channel,
    replies: Channel,
    service_closures: Channel,
    last_control_message_source: Option<u64>,
    last_service_message_source: Option<u64>,
    next_request_sequence: u64,
    next_reply_sequence: u64,
    last_control_message_sequence: u64,
    last_service_message_sequence: u64,
    next_closure_sequence: u64,
    last_control_closure_domain: Option<u64>,
    last_control_closure_sequence: u64,
    last_control_delivery: Option<DeliveryAudit>,
    last_service_delivery: Option<DeliveryAudit>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MessageEnvelope {
    source: u64,
    sequence: u64,
    value: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeliveryAudit {
    kind: u8,
    source: u64,
    sequence: u64,
    value: u64,
}

const MESSAGE_DELIVERY: u8 = 1;
const PEER_CLOSURE_DELIVERY: u8 = 2;

enum SendPath {
    Failed,
    Queued,
    Direct(u64),
}

const EMPTY_MESSAGE: MessageEnvelope = MessageEnvelope {
    source: 0,
    sequence: 0,
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
            next_request_sequence: 1,
            next_reply_sequence: 1,
            last_control_message_sequence: 0,
            last_service_message_sequence: 0,
            next_closure_sequence: 1,
            last_control_closure_domain: None,
            last_control_closure_sequence: 0,
            last_control_delivery: None,
            last_service_delivery: None,
        })
    }

    fn send(&mut self, domain: u64, message: u64) -> bool {
        self.send_with_direct(domain, message, |_, _| false)
    }

    fn send_with_direct(
        &mut self,
        domain: u64,
        message: u64,
        direct: impl FnOnce(bool, u64) -> bool,
    ) -> bool {
        if self.service_side(domain) == Some(true) {
            let outcome = Self::send_sequenced(
                &mut self.replies,
                &mut self.next_reply_sequence,
                domain,
                message,
                |sequence| direct(false, sequence),
            );
            if let SendPath::Direct(sequence) = outcome {
                self.last_control_message_source = Some(domain);
                self.last_control_message_sequence = sequence;
                self.last_control_delivery = Some(DeliveryAudit {
                    kind: MESSAGE_DELIVERY,
                    source: domain,
                    sequence,
                    value: message,
                });
            }
            !matches!(outcome, SendPath::Failed)
        } else if self.service_side(domain) == Some(false) && self.service_domain.is_some() {
            let outcome = Self::send_sequenced(
                &mut self.requests,
                &mut self.next_request_sequence,
                domain,
                message,
                |sequence| direct(true, sequence),
            );
            if let SendPath::Direct(sequence) = outcome {
                self.last_service_message_source = Some(domain);
                self.last_service_message_sequence = sequence;
                self.last_service_delivery = Some(DeliveryAudit {
                    kind: MESSAGE_DELIVERY,
                    source: domain,
                    sequence,
                    value: message,
                });
            }
            !matches!(outcome, SendPath::Failed)
        } else {
            false
        }
    }

    fn send_sequenced(
        channel: &mut Channel,
        next_sequence: &mut u64,
        source: u64,
        value: u64,
        direct: impl FnOnce(u64) -> bool,
    ) -> SendPath {
        let Some(next) = next_sequence.checked_add(1) else {
            return SendPath::Failed;
        };
        let sequence = *next_sequence;
        if channel.len == 0 && direct(sequence) {
            *next_sequence = next;
            return SendPath::Direct(sequence);
        }
        if !channel.send(MessageEnvelope {
            source,
            sequence,
            value,
        }) {
            return SendPath::Failed;
        }
        *next_sequence = next;
        SendPath::Queued
    }

    fn receive(&mut self, domain: u64) -> Receive {
        if self.service_side(domain) == Some(true) {
            let Some(message) = self.requests.receive() else {
                return Receive::Empty;
            };
            if message.source != self.control_domain
                || message.sequence == 0
                || message.sequence <= self.last_service_message_sequence
            {
                return Receive::Empty;
            }
            self.last_service_message_source = Some(message.source);
            self.last_service_message_sequence = message.sequence;
            self.last_service_delivery = Some(DeliveryAudit {
                kind: MESSAGE_DELIVERY,
                source: message.source,
                sequence: message.sequence,
                value: message.value,
            });
            Receive::Message(message.value)
        } else if self.service_side(domain) == Some(false) {
            if let Some(message) = self.replies.receive() {
                if message.source == 0
                    || message.source == self.control_domain
                    || message.sequence == 0
                    || message.sequence <= self.last_control_message_sequence
                {
                    return Receive::Empty;
                }
                self.last_control_message_source = Some(message.source);
                self.last_control_message_sequence = message.sequence;
                self.last_control_delivery = Some(DeliveryAudit {
                    kind: MESSAGE_DELIVERY,
                    source: message.source,
                    sequence: message.sequence,
                    value: message.value,
                });
                Receive::Message(message.value)
            } else if let Some(closure) = self.service_closures.receive() {
                if closure.source != closure.value
                    || closure.sequence == 0
                    || closure.sequence <= self.last_control_closure_sequence
                {
                    return Receive::Empty;
                }
                self.last_control_closure_domain = Some(closure.source);
                self.last_control_closure_sequence = closure.sequence;
                self.last_control_delivery = Some(DeliveryAudit {
                    kind: PEER_CLOSURE_DELIVERY,
                    source: closure.source,
                    sequence: closure.sequence,
                    value: closure.value,
                });
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
        let Some(next) = self.next_closure_sequence.checked_add(1) else {
            return false;
        };
        if !self.service_closures.send(MessageEnvelope {
            source: domain,
            sequence: self.next_closure_sequence,
            value: domain,
        }) {
            return false;
        }
        self.requests.clear();
        self.last_service_delivery = None;
        self.service_domain = None;
        self.next_closure_sequence = next;
        true
    }

    fn bind_service(&mut self, domain: u64) -> bool {
        if domain == 0
            || domain == self.control_domain
            || self.service_domain.is_some()
            || self.last_service_delivery.is_some()
        {
            return false;
        }
        self.service_domain = Some(domain);
        true
    }

    fn rollback_service_binding(&mut self, domain: u64) -> bool {
        if self.service_domain != Some(domain)
            || self.requests.len != 0
            || self.last_service_delivery.is_some()
        {
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

    fn delivery_audit(&self, domain: u64) -> Option<DeliveryAudit> {
        if self.service_domain == Some(domain) {
            self.last_service_delivery
        } else if self.control_domain == domain {
            self.last_control_delivery
        } else {
            None
        }
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
    if CONTROL_CHANNEL.get().is_some() {
        return false;
    }
    let Some(channel) = Duplex::new(control_domain, service_domain) else {
        return false;
    };
    CONTROL_CHANNEL.try_install(channel).is_ok()
}

pub(crate) fn send_or_wake(
    domain: u64,
    message: u64,
    wake: impl FnOnce(bool, u64) -> bool,
) -> bool {
    channel_mut().is_some_and(|channel| channel.send_with_direct(domain, message, wake))
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

pub(crate) fn last_message_audit() -> Option<((u64, u64), (u64, u64))> {
    let channel = channel_mut()?;
    Some((
        (
            channel.last_control_message_source?,
            channel.last_control_message_sequence,
        ),
        (
            channel.last_service_message_source?,
            channel.last_service_message_sequence,
        ),
    ))
}

pub(crate) fn last_closure_audit() -> Option<(u64, u64)> {
    let channel = channel_mut()?;
    Some((
        channel.last_control_closure_domain?,
        channel.last_control_closure_sequence,
    ))
}

pub(crate) fn delivery_descriptor(
    domain: u64,
) -> Option<[u8; crate::control_abi::IPC_DELIVERY_DESCRIPTOR_SIZE]> {
    let audit = channel_mut()?.delivery_audit(domain)?;
    Some(crate::control_abi::ipc_delivery_descriptor(
        audit.kind,
        audit.source,
        audit.sequence,
        audit.value,
    ))
}

pub(crate) fn verify_channel_invariants() -> bool {
    let (control, service, replacement) = (1, 2, 3);
    let Some(mut out_of_order) = Duplex::new(control, service) else {
        return false;
    };
    let out_of_order_rejected = out_of_order.requests.send(MessageEnvelope {
        source: control,
        sequence: 2,
        value: 10,
    }) && out_of_order.receive(service) == Receive::Message(10)
        && out_of_order.requests.send(MessageEnvelope {
            source: control,
            sequence: 1,
            value: 20,
        })
        && out_of_order.receive(service) == Receive::Empty;
    let Some(mut zero_sequence) = Duplex::new(control, service) else {
        return false;
    };
    let zero_sequence_rejected = zero_sequence.requests.send(MessageEnvelope {
        source: control,
        sequence: 0,
        value: 10,
    }) && zero_sequence.receive(service) == Receive::Empty;
    let Some(mut stale_closure) = Duplex::new(control, service) else {
        return false;
    };
    let stale_closure_rejected = stale_closure.service_closures.send(MessageEnvelope {
        source: service,
        sequence: 2,
        value: service,
    }) && stale_closure.receive(control)
        == Receive::PeerClosed(service)
        && stale_closure.service_closures.send(MessageEnvelope {
            source: service,
            sequence: 1,
            value: service,
        })
        && stale_closure.receive(control) == Receive::Empty;
    let Some(mut zero_closure) = Duplex::new(control, service) else {
        return false;
    };
    let zero_closure_rejected = zero_closure.service_closures.send(MessageEnvelope {
        source: service,
        sequence: 0,
        value: service,
    }) && zero_closure.receive(control) == Receive::Empty;
    let Some(mut exhausted_request) = Duplex::new(control, service) else {
        return false;
    };
    exhausted_request.next_request_sequence = u64::MAX;
    let exhausted_request_rejected = !exhausted_request.send_with_direct(control, 10, |_, _| true)
        && exhausted_request.requests.len == 0
        && exhausted_request.last_service_message_source.is_none()
        && exhausted_request.last_service_delivery.is_none();
    let Some(mut exhausted_reply) = Duplex::new(control, service) else {
        return false;
    };
    exhausted_reply.next_reply_sequence = u64::MAX;
    let exhausted_reply_rejected = !exhausted_reply.send(service, 10)
        && exhausted_reply.replies.len == 0
        && exhausted_reply.last_control_message_source.is_none();
    let Some(mut exhausted_closure) = Duplex::new(control, service) else {
        return false;
    };
    let exhausted_closure_rejected = exhausted_closure.send(control, 10) && {
        exhausted_closure.next_closure_sequence = u64::MAX;
        !exhausted_closure.retire_service(service)
            && exhausted_closure.service_domain == Some(service)
            && exhausted_closure.receive(service) == Receive::Message(10)
    };
    let Some(mut direct) = Duplex::new(control, service) else {
        return false;
    };
    let direct_sequence_shared = direct.send_with_direct(control, 10, |service_side, sequence| {
        service_side && sequence == 1
    }) && direct.requests.len == 0
        && direct.last_service_message_source == Some(control)
        && direct.last_service_message_sequence == 1
        && direct.send_with_direct(control, 20, |_, _| false)
        && direct.send_with_direct(control, 30, |_, _| true)
        && direct.requests.len == 2
        && direct.receive(service) == Receive::Message(20)
        && direct.receive(service) == Receive::Message(30)
        && direct.last_service_message_sequence == 3
        && direct.last_service_delivery
            == Some(DeliveryAudit {
                kind: MESSAGE_DELIVERY,
                source: control,
                sequence: 3,
                value: 30,
            });
    let Some(mut channel) = Duplex::new(control, service) else {
        return false;
    };
    out_of_order_rejected
        && zero_sequence_rejected
        && stale_closure_rejected
        && zero_closure_rejected
        && exhausted_request_rejected
        && exhausted_reply_rejected
        && exhausted_closure_rejected
        && direct_sequence_shared
        && channel.receive(control) == Receive::Empty
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
        && channel.last_service_delivery.is_none()
        && !channel.retire_service(service)
        && !channel.send(service, 90)
        && channel.service_side(service).is_none()
        && channel.peer_domain(control).is_none()
        && channel.receive(service) == Receive::Empty
        && !channel.send(control, 90)
        && channel.receive(control) == Receive::Message(80)
        && channel.receive(control) == Receive::PeerClosed(service)
        && channel.bind_service(replacement)
        && channel.delivery_audit(replacement).is_none()
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
        && channel.last_control_message_sequence == 2
        && channel.last_service_message_source == Some(control)
        && channel.last_service_message_sequence == 7
        && channel.last_control_closure_domain == Some(replacement)
        && channel.last_control_closure_sequence == 2
        && channel.last_service_delivery.is_none()
        && channel.last_control_delivery
            == Some(DeliveryAudit {
                kind: PEER_CLOSURE_DELIVERY,
                source: replacement,
                sequence: 2,
                value: replacement,
            })
}

fn channel_mut() -> Option<&'static mut Duplex> {
    // SAFETY: IPC mutation is serialized on the bootstrap CPU until Gate 8
    // supplies the runtime lock for this explicit owner root.
    Some(unsafe { &mut *CONTROL_CHANNEL.get_mut_ptr()? })
}
