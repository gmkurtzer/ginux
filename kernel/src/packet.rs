use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

pub(crate) const MAX_PACKET_SIZE: usize = 1514;
pub(crate) const PACKET_CAPACITY: usize = crate::control_abi::PACKET_QUEUE_CAPACITY;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PacketMetadata {
    pub(crate) source_domain: u64,
    pub(crate) service_identity: u64,
    pub(crate) sequence: u64,
    pub(crate) length: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PacketError {
    Busy,
    Empty,
    InvalidProducer,
    SequenceExhausted,
    Stale,
    TooLarge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PacketWait {
    Ready,
    Empty,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PacketSpace {
    Ready,
    Full,
    InvalidProducer,
    SequenceExhausted,
}

struct Mailbox {
    producer: Option<(u64, u64)>,
    next_sequence: u64,
    packets: [[u8; MAX_PACKET_SIZE]; PACKET_CAPACITY],
    metadata: [Option<PacketMetadata>; PACKET_CAPACITY],
    head: usize,
    len: usize,
    claim: Option<(u64, u64)>,
    last_delivery: Option<PacketMetadata>,
}

impl Mailbox {
    const fn new() -> Self {
        Self {
            producer: None,
            next_sequence: 1,
            packets: [[0; MAX_PACKET_SIZE]; PACKET_CAPACITY],
            metadata: [None; PACKET_CAPACITY],
            head: 0,
            len: 0,
            claim: None,
            last_delivery: None,
        }
    }

    fn bind(&mut self, domain: u64, service: u64) -> bool {
        if domain == 0 || service == 0 || self.producer.is_some() {
            return false;
        }
        self.producer = Some((domain, service));
        true
    }

    fn publish(&mut self, domain: u64, input: &[u8]) -> Result<u64, PacketError> {
        let Some((bound, service_identity)) = self.producer else {
            return Err(PacketError::InvalidProducer);
        };
        if bound != domain {
            return Err(PacketError::InvalidProducer);
        }
        if input.is_empty() || input.len() > MAX_PACKET_SIZE {
            return Err(PacketError::TooLarge);
        }
        if self.len == PACKET_CAPACITY {
            return Err(PacketError::Busy);
        }
        let Some(next) = self.next_sequence.checked_add(1) else {
            return Err(PacketError::SequenceExhausted);
        };
        let sequence = self.next_sequence;
        let tail = (self.head + self.len) % PACKET_CAPACITY;
        self.packets[tail][..input.len()].copy_from_slice(input);
        self.metadata[tail] = Some(PacketMetadata {
            source_domain: domain,
            service_identity,
            sequence,
            length: input.len(),
        });
        self.len += 1;
        self.next_sequence = next;
        Ok(sequence)
    }

    fn retire(&mut self, domain: u64) -> bool {
        if self.producer.is_none_or(|(bound, _)| bound != domain) {
            return false;
        }
        self.producer = None;
        true
    }

    fn retire_if_bound(&mut self, domain: u64) -> bool {
        if self.producer.is_some_and(|(bound, _)| bound == domain) {
            self.producer = None;
        }
        true
    }

    fn wait_state(&self, expected_sequence: u64) -> PacketWait {
        if self.len == 0 {
            return if self.next_sequence != u64::MAX && expected_sequence == self.next_sequence {
                PacketWait::Empty
            } else {
                PacketWait::Stale
            };
        }
        if self.metadata[self.head].is_some_and(|metadata| metadata.sequence == expected_sequence) {
            PacketWait::Ready
        } else {
            PacketWait::Stale
        }
    }

    fn space_state(&self, domain: u64) -> PacketSpace {
        if self.producer.is_none_or(|(bound, _)| bound != domain) {
            PacketSpace::InvalidProducer
        } else if self.next_sequence == u64::MAX {
            PacketSpace::SequenceExhausted
        } else if self.len == PACKET_CAPACITY {
            PacketSpace::Full
        } else {
            PacketSpace::Ready
        }
    }

    fn consume_with(
        &mut self,
        expected_sequence: u64,
        capacity: usize,
        deliver: impl FnOnce(PacketMetadata, &[u8]) -> bool,
    ) -> Result<PacketMetadata, PacketError> {
        if self.claim.is_some() {
            return Err(PacketError::Busy);
        }
        if self.len == 0 {
            return Err(PacketError::Empty);
        }
        let Some(metadata) = self.metadata[self.head] else {
            return Err(PacketError::Empty);
        };
        if expected_sequence != metadata.sequence {
            return Err(PacketError::Stale);
        }
        if capacity < metadata.length {
            return Err(PacketError::TooLarge);
        }
        if !deliver(metadata, &self.packets[self.head][..metadata.length]) {
            return Err(PacketError::TooLarge);
        }
        self.packets[self.head][..metadata.length].fill(0);
        self.metadata[self.head] = None;
        self.head = (self.head + 1) % PACKET_CAPACITY;
        self.len -= 1;
        self.last_delivery = Some(metadata);
        Ok(metadata)
    }

    fn claim_with(
        &mut self,
        domain: u64,
        expected_sequence: u64,
        capacity: usize,
        deliver: impl FnOnce(PacketMetadata, &[u8]) -> bool,
    ) -> Result<PacketMetadata, PacketError> {
        if domain == 0 || self.claim.is_some() {
            return Err(PacketError::Busy);
        }
        if self.len == 0 {
            return Err(PacketError::Empty);
        }
        let Some(metadata) = self.metadata[self.head] else {
            return Err(PacketError::Empty);
        };
        if expected_sequence != metadata.sequence {
            return Err(PacketError::Stale);
        }
        if capacity < metadata.length
            || !deliver(metadata, &self.packets[self.head][..metadata.length])
        {
            return Err(PacketError::TooLarge);
        }
        self.claim = Some((domain, expected_sequence));
        Ok(metadata)
    }

    fn acknowledge(
        &mut self,
        domain: u64,
        expected_sequence: u64,
    ) -> Result<PacketMetadata, PacketError> {
        if self.claim != Some((domain, expected_sequence)) {
            return Err(PacketError::Stale);
        }
        let Some(metadata) = self.metadata[self.head] else {
            return Err(PacketError::Empty);
        };
        if metadata.sequence != expected_sequence {
            return Err(PacketError::Stale);
        }
        self.packets[self.head][..metadata.length].fill(0);
        self.metadata[self.head] = None;
        self.head = (self.head + 1) % PACKET_CAPACITY;
        self.len -= 1;
        self.claim = None;
        self.last_delivery = Some(metadata);
        Ok(metadata)
    }

    fn release_claim_if_bound(&mut self, domain: u64) -> bool {
        if self.claim.is_some_and(|(bound, _)| bound == domain) {
            self.claim = None;
        }
        true
    }
}

static INSTALLED: AtomicBool = AtomicBool::new(false);
static mut MAILBOX: MaybeUninit<Mailbox> = MaybeUninit::uninit();
static mut EGRESS_MAILBOX: MaybeUninit<Mailbox> = MaybeUninit::uninit();

pub(crate) fn install() -> bool {
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return false;
    }
    unsafe {
        core::ptr::addr_of_mut!(MAILBOX).write(MaybeUninit::new(Mailbox::new()));
        core::ptr::addr_of_mut!(EGRESS_MAILBOX).write(MaybeUninit::new(Mailbox::new()));
    }
    true
}

pub(crate) fn bind_producer(domain: u64, service: u64) -> bool {
    mailbox_mut().is_some_and(|mailbox| mailbox.bind(domain, service))
}

pub(crate) fn publish(domain: u64, input: &[u8]) -> Result<u64, PacketError> {
    mailbox_mut()
        .ok_or(PacketError::InvalidProducer)?
        .publish(domain, input)
}

pub(crate) fn bind_egress_producer(domain: u64, service: u64) -> bool {
    egress_mailbox_mut().is_some_and(|mailbox| mailbox.bind(domain, service))
}

pub(crate) fn publish_egress(domain: u64, input: &[u8]) -> Result<u64, PacketError> {
    egress_mailbox_mut()
        .ok_or(PacketError::InvalidProducer)?
        .publish(domain, input)
}

pub(crate) fn retire_egress_producer(domain: u64) -> bool {
    egress_mailbox_mut().is_some_and(|mailbox| mailbox.retire(domain))
}

pub(crate) fn retire_egress_producer_if_bound(domain: u64) -> bool {
    domain != 0
        && egress_mailbox_mut().is_some_and(|mailbox| {
            mailbox.retire_if_bound(domain) && mailbox.release_claim_if_bound(domain)
        })
}

pub(crate) fn retire_producer(domain: u64) -> bool {
    mailbox_mut().is_some_and(|mailbox| mailbox.retire(domain))
}

pub(crate) fn retire_producer_if_bound(domain: u64) -> bool {
    domain != 0 && mailbox_mut().is_some_and(|mailbox| mailbox.retire_if_bound(domain))
}

pub(crate) fn consume_with(
    expected_sequence: u64,
    capacity: usize,
    deliver: impl FnOnce(PacketMetadata, &[u8]) -> bool,
) -> Result<PacketMetadata, PacketError> {
    mailbox_mut()
        .ok_or(PacketError::Empty)?
        .consume_with(expected_sequence, capacity, deliver)
}

pub(crate) fn consume_egress_with(
    expected_sequence: u64,
    capacity: usize,
    deliver: impl FnOnce(PacketMetadata, &[u8]) -> bool,
) -> Result<PacketMetadata, PacketError> {
    egress_mailbox_mut()
        .ok_or(PacketError::Empty)?
        .consume_with(expected_sequence, capacity, deliver)
}

pub(crate) fn claim_egress_with(
    domain: u64,
    expected_sequence: u64,
    capacity: usize,
    deliver: impl FnOnce(PacketMetadata, &[u8]) -> bool,
) -> Result<PacketMetadata, PacketError> {
    egress_mailbox_mut().ok_or(PacketError::Empty)?.claim_with(
        domain,
        expected_sequence,
        capacity,
        deliver,
    )
}

pub(crate) fn acknowledge_egress(
    domain: u64,
    expected_sequence: u64,
) -> Result<PacketMetadata, PacketError> {
    egress_mailbox_mut()
        .ok_or(PacketError::Empty)?
        .acknowledge(domain, expected_sequence)
}

pub(crate) fn last_delivery() -> Option<PacketMetadata> {
    mailbox_mut()?.last_delivery
}

pub(crate) fn pending_metadata() -> Option<PacketMetadata> {
    let mailbox = mailbox_mut()?;
    if mailbox.len == 0 {
        return None;
    }
    mailbox.metadata[mailbox.head]
}

pub(crate) fn egress_pending_metadata() -> Option<PacketMetadata> {
    let mailbox = egress_mailbox_mut()?;
    if mailbox.len == 0 {
        return None;
    }
    mailbox.metadata[mailbox.head]
}

pub(crate) fn egress_last_delivery() -> Option<PacketMetadata> {
    egress_mailbox_mut()?.last_delivery
}

pub(crate) fn egress_claim() -> Option<(u64, u64)> {
    egress_mailbox_mut()?.claim
}

pub(crate) fn wait_state(expected_sequence: u64) -> PacketWait {
    let Some(mailbox) = mailbox_mut() else {
        return PacketWait::Stale;
    };
    mailbox.wait_state(expected_sequence)
}

pub(crate) fn egress_wait_state(expected_sequence: u64) -> PacketWait {
    let Some(mailbox) = egress_mailbox_mut() else {
        return PacketWait::Stale;
    };
    mailbox.wait_state(expected_sequence)
}

pub(crate) fn space_state(domain: u64) -> PacketSpace {
    mailbox_mut().map_or(PacketSpace::InvalidProducer, |mailbox| {
        mailbox.space_state(domain)
    })
}

pub(crate) fn egress_space_state(domain: u64) -> PacketSpace {
    egress_mailbox_mut().map_or(PacketSpace::InvalidProducer, |mailbox| {
        mailbox.space_state(domain)
    })
}

pub(crate) fn next_sequence() -> Option<u64> {
    Some(mailbox_mut()?.next_sequence)
}

pub(crate) fn egress_next_sequence() -> Option<u64> {
    Some(egress_mailbox_mut()?.next_sequence)
}

pub(crate) fn egress_is_empty() -> bool {
    egress_mailbox_mut().is_some_and(|mailbox| mailbox.len == 0)
}

fn mailbox_mut() -> Option<&'static mut Mailbox> {
    INSTALLED
        .load(Ordering::Acquire)
        .then(|| unsafe { &mut *core::ptr::addr_of_mut!(MAILBOX).cast::<Mailbox>() })
}

fn egress_mailbox_mut() -> Option<&'static mut Mailbox> {
    INSTALLED
        .load(Ordering::Acquire)
        .then(|| unsafe { &mut *core::ptr::addr_of_mut!(EGRESS_MAILBOX).cast::<Mailbox>() })
}

pub(crate) fn verify_invariants() -> bool {
    let mut mailbox = Mailbox::new();
    let packet = [0x5a; 42];
    let mut output = [0; 42];
    let mut egress = Mailbox::new();
    !mailbox.bind(0, 1)
        && mailbox.bind(2, 7)
        && mailbox.retire_if_bound(3)
        && mailbox.space_state(2) == PacketSpace::Ready
        && mailbox.wait_state(1) == PacketWait::Empty
        && mailbox.wait_state(2) == PacketWait::Stale
        && !mailbox.bind(3, 7)
        && mailbox.publish(3, &packet) == Err(PacketError::InvalidProducer)
        && mailbox.publish(2, &[]) == Err(PacketError::TooLarge)
        && mailbox.publish(2, &[0; MAX_PACKET_SIZE + 1]) == Err(PacketError::TooLarge)
        && mailbox.publish(2, &packet) == Ok(1)
        && mailbox.wait_state(1) == PacketWait::Ready
        && mailbox.wait_state(2) == PacketWait::Stale
        && mailbox.publish(2, &packet) == Ok(2)
        && mailbox.publish(2, &packet) == Ok(3)
        && mailbox.publish(2, &packet) == Ok(4)
        && mailbox.publish(2, &packet) == Err(PacketError::Busy)
        && mailbox.space_state(2) == PacketSpace::Full
        && mailbox.space_state(3) == PacketSpace::InvalidProducer
        && mailbox.retire(2)
        && mailbox.publish(2, &packet) == Err(PacketError::InvalidProducer)
        && mailbox.bind(3, 8)
        && mailbox.publish(3, &packet) == Err(PacketError::Busy)
        && mailbox.consume_with(2, output.len(), |_, _| false) == Err(PacketError::Stale)
        && mailbox.consume_with(1, output.len(), |_, packet| {
            output.copy_from_slice(packet);
            true
        }) == Ok(PacketMetadata {
            source_domain: 2,
            service_identity: 7,
            sequence: 1,
            length: packet.len(),
        })
        && mailbox.space_state(3) == PacketSpace::Ready
        && output == packet
        && mailbox.publish(3, &packet) == Ok(5)
        && mailbox.consume_with(1, output.len(), |_, _| false) == Err(PacketError::Stale)
        && mailbox.consume_with(2, output.len(), |_, _| true).is_ok()
        && mailbox.consume_with(3, output.len(), |_, _| true).is_ok()
        && mailbox.consume_with(4, output.len(), |_, _| true).is_ok()
        && mailbox.consume_with(5, output.len(), |_, _| true).is_ok()
        && mailbox.consume_with(5, output.len(), |_, _| false) == Err(PacketError::Empty)
        && mailbox.wait_state(6) == PacketWait::Empty
        && mailbox.wait_state(5) == PacketWait::Stale
        && {
            mailbox.next_sequence = u64::MAX;
            mailbox.wait_state(u64::MAX) == PacketWait::Stale
                && mailbox.space_state(3) == PacketSpace::SequenceExhausted
        }
        && egress.bind(9, 10)
        && egress.publish(9, &[0xa5; 8]) == Ok(1)
        && egress.len == 1
        && mailbox.len == 0
        && egress
            .claim_with(11, 1, 8, |metadata, packet| {
                metadata.source_domain == 9
                    && metadata.service_identity == 10
                    && packet == [0xa5; 8]
            })
            .is_ok()
        && egress.len == 1
        && egress.claim_with(12, 1, 8, |_, _| true) == Err(PacketError::Busy)
        && egress.consume_with(1, 8, |_, _| true) == Err(PacketError::Busy)
        && egress.release_claim_if_bound(12)
        && egress.claim == Some((11, 1))
        && egress.release_claim_if_bound(11)
        && egress.claim.is_none()
        && egress
            .claim_with(12, 1, 8, |metadata, packet| {
                metadata.source_domain == 9
                    && metadata.service_identity == 10
                    && packet == [0xa5; 8]
            })
            .is_ok()
        && egress.acknowledge(11, 1) == Err(PacketError::Stale)
        && egress.acknowledge(12, 1).is_ok()
        && egress.len == 0
}
