#![no_std]

pub const ETHERNET_ARP_FRAME_SIZE: usize = 42;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EndpointV4 {
    pub mac: [u8; 6],
    pub ipv4: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArpReplyV4 {
    pub sender_mac: [u8; 6],
    pub sender_ipv4: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArpParseError {
    Length,
    Ethernet,
    ArpHeader,
    Sender,
    Target,
}

pub fn parse_arp_reply_v4(
    frame: &[u8],
    local: EndpointV4,
    peer_ipv4: u32,
) -> Result<ArpReplyV4, ArpParseError> {
    if frame.len() != ETHERNET_ARP_FRAME_SIZE {
        return Err(ArpParseError::Length);
    }
    if frame[0..6] != local.mac || frame[12..14] != [0x08, 0x06] {
        return Err(ArpParseError::Ethernet);
    }
    if frame[14..22] != [0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02] {
        return Err(ArpParseError::ArpHeader);
    }
    let sender_mac: [u8; 6] = frame[6..12].try_into().map_err(|_| ArpParseError::Sender)?;
    if sender_mac != frame[22..28]
        || sender_mac == [0; 6]
        || sender_mac == [0xff; 6]
        || sender_mac[0] & 1 != 0
    {
        return Err(ArpParseError::Sender);
    }
    let sender_ipv4 = u32::from_be_bytes(
        frame[28..32]
            .try_into()
            .map_err(|_| ArpParseError::Sender)?,
    );
    if sender_ipv4 != peer_ipv4
        || frame[32..38] != local.mac
        || frame[38..42] != local.ipv4.to_be_bytes()
    {
        return Err(ArpParseError::Target);
    }
    Ok(ArpReplyV4 {
        sender_mac,
        sender_ipv4,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeighborObservation {
    Learned,
    Refreshed,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeighborStateV4 {
    peer_ipv4: u32,
    mac: Option<[u8; 6]>,
    accepted: u64,
}

impl NeighborStateV4 {
    pub const fn new(peer_ipv4: u32) -> Self {
        Self {
            peer_ipv4,
            mac: None,
            accepted: 0,
        }
    }

    pub fn observe(&mut self, reply: ArpReplyV4) -> NeighborObservation {
        if reply.sender_ipv4 != self.peer_ipv4 {
            return NeighborObservation::Conflict;
        }
        match self.mac {
            None => {
                self.mac = Some(reply.sender_mac);
                self.accepted = 1;
                NeighborObservation::Learned
            }
            Some(mac) if mac == reply.sender_mac => {
                let Some(next) = self.accepted.checked_add(1) else {
                    return NeighborObservation::Conflict;
                };
                self.accepted = next;
                NeighborObservation::Refreshed
            }
            Some(_) => NeighborObservation::Conflict,
        }
    }

    pub const fn mac(&self) -> Option<[u8; 6]> {
        self.mac
    }

    pub const fn accepted(&self) -> u64 {
        self.accepted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCAL: EndpointV4 = EndpointV4 {
        mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
        ipv4: u32::from_be_bytes([10, 0, 2, 15]),
    };
    const PEER: u32 = u32::from_be_bytes([10, 0, 2, 2]);
    const FRAME: [u8; 42] = [
        0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02, 0x08, 0x06, 0x00,
        0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02, 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02, 10, 0, 2, 2,
        0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 10, 0, 2, 15,
    ];

    #[test]
    fn parses_exact_arp_reply() {
        let reply = parse_arp_reply_v4(&FRAME, LOCAL, PEER).unwrap();
        assert_eq!(reply.sender_mac, [0x52, 0x55, 0x0a, 0, 2, 2]);
        assert_eq!(reply.sender_ipv4, PEER);
    }

    #[test]
    fn rejects_malformed_or_misdirected_replies() {
        let mut frame = FRAME;
        frame[21] = 1;
        assert_eq!(
            parse_arp_reply_v4(&frame, LOCAL, PEER),
            Err(ArpParseError::ArpHeader)
        );
        frame = FRAME;
        frame[0] ^= 1;
        assert_eq!(
            parse_arp_reply_v4(&frame, LOCAL, PEER),
            Err(ArpParseError::Ethernet)
        );
    }

    #[test]
    fn learns_refreshes_and_rejects_conflict_without_mutation() {
        let reply = parse_arp_reply_v4(&FRAME, LOCAL, PEER).unwrap();
        let mut state = NeighborStateV4::new(PEER);
        assert_eq!(state.observe(reply), NeighborObservation::Learned);
        assert_eq!(state.observe(reply), NeighborObservation::Refreshed);
        let conflict = ArpReplyV4 {
            sender_mac: [0x52, 0x55, 0x0a, 0, 2, 3],
            ..reply
        };
        assert_eq!(state.observe(conflict), NeighborObservation::Conflict);
        assert_eq!(state.mac(), Some(reply.sender_mac));
        assert_eq!(state.accepted(), 2);
    }
}
