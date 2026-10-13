use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

pub(crate) const MAX_PACKET_SIZE: usize = 1514;

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

struct Mailbox {
    producer: Option<(u64, u64)>,
    next_sequence: u64,
    packet: [u8; MAX_PACKET_SIZE],
    metadata: Option<PacketMetadata>,
    last_delivery: Option<PacketMetadata>,
}

impl Mailbox {
    const fn new() -> Self {
        Self {
            producer: None,
            next_sequence: 1,
            packet: [0; MAX_PACKET_SIZE],
            metadata: None,
            last_delivery: None,
        }
    }

    fn bind(&mut self, domain: u64, service: u64) -> bool {
        if domain == 0 || service == 0 || self.producer.is_some() || self.metadata.is_some() {
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
        if self.metadata.is_some() {
            return Err(PacketError::Busy);
        }
        let Some(next) = self.next_sequence.checked_add(1) else {
            return Err(PacketError::SequenceExhausted);
        };
        let sequence = self.next_sequence;
        self.packet[..input.len()].copy_from_slice(input);
        self.metadata = Some(PacketMetadata {
            source_domain: domain,
            service_identity,
            sequence,
            length: input.len(),
        });
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

    fn consume_with(
        &mut self,
        expected_sequence: u64,
        capacity: usize,
        deliver: impl FnOnce(PacketMetadata, &[u8]) -> bool,
    ) -> Result<PacketMetadata, PacketError> {
        let Some(metadata) = self.metadata else {
            return Err(PacketError::Empty);
        };
        if expected_sequence != metadata.sequence {
            return Err(PacketError::Stale);
        }
        if capacity < metadata.length {
            return Err(PacketError::TooLarge);
        }
        if !deliver(metadata, &self.packet[..metadata.length]) {
            return Err(PacketError::TooLarge);
        }
        self.packet[..metadata.length].fill(0);
        self.metadata = None;
        self.last_delivery = Some(metadata);
        Ok(metadata)
    }
}

static INSTALLED: AtomicBool = AtomicBool::new(false);
static mut MAILBOX: MaybeUninit<Mailbox> = MaybeUninit::uninit();

pub(crate) fn install() -> bool {
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return false;
    }
    unsafe {
        core::ptr::addr_of_mut!(MAILBOX).write(MaybeUninit::new(Mailbox::new()));
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

pub(crate) fn retire_producer(domain: u64) -> bool {
    mailbox_mut().is_some_and(|mailbox| mailbox.retire(domain))
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

pub(crate) fn last_delivery() -> Option<PacketMetadata> {
    mailbox_mut()?.last_delivery
}

pub(crate) fn pending_metadata() -> Option<PacketMetadata> {
    mailbox_mut()?.metadata
}

fn mailbox_mut() -> Option<&'static mut Mailbox> {
    INSTALLED
        .load(Ordering::Acquire)
        .then(|| unsafe { &mut *core::ptr::addr_of_mut!(MAILBOX).cast::<Mailbox>() })
}

pub(crate) fn verify_invariants() -> bool {
    let mut mailbox = Mailbox::new();
    let packet = [0x5a; 42];
    let mut output = [0; 42];
    !mailbox.bind(0, 1)
        && mailbox.bind(2, 7)
        && !mailbox.bind(3, 7)
        && mailbox.publish(3, &packet) == Err(PacketError::InvalidProducer)
        && mailbox.publish(2, &[]) == Err(PacketError::TooLarge)
        && mailbox.publish(2, &[0; MAX_PACKET_SIZE + 1]) == Err(PacketError::TooLarge)
        && mailbox.publish(2, &packet) == Ok(1)
        && mailbox.publish(2, &packet) == Err(PacketError::Busy)
        && mailbox.retire(2)
        && mailbox.publish(2, &packet) == Err(PacketError::InvalidProducer)
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
        && output == packet
        && mailbox.consume_with(1, output.len(), |_, _| false) == Err(PacketError::Empty)
}
