#![no_std]

pub const ETHERNET_ARP_FRAME_SIZE: usize = 42;
pub const ETHERNET_IPV4_MIN_FRAME_SIZE: usize = 34;

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
pub struct Ipv4PacketV0<'a> {
    pub source_ipv4: u32,
    pub destination_ipv4: u32,
    pub protocol: u8,
    pub payload: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ipv4ParseError {
    Length,
    Ethernet,
    Header,
    Checksum,
    Fragmented,
    Endpoint,
    Protocol,
}

pub fn parse_ipv4_icmp_v0<'a>(
    frame: &'a [u8],
    local: EndpointV4,
    peer: EndpointV4,
) -> Result<Ipv4PacketV0<'a>, Ipv4ParseError> {
    if frame.len() < ETHERNET_IPV4_MIN_FRAME_SIZE {
        return Err(Ipv4ParseError::Length);
    }
    if frame[0..6] != local.mac || frame[6..12] != peer.mac || frame[12..14] != [0x08, 0x00] {
        return Err(Ipv4ParseError::Ethernet);
    }
    let header = &frame[14..34];
    if header[0] != 0x45 || header[1] != 0 || header[8] == 0 {
        return Err(Ipv4ParseError::Header);
    }
    let total_length = usize::from(u16::from_be_bytes([header[2], header[3]]));
    if total_length < 20 || frame.len() != 14 + total_length {
        return Err(Ipv4ParseError::Length);
    }
    if checksum(header) != 0 {
        return Err(Ipv4ParseError::Checksum);
    }
    let fragment = u16::from_be_bytes([header[6], header[7]]);
    if fragment & 0xbfff != 0 {
        return Err(Ipv4ParseError::Fragmented);
    }
    let source_ipv4 = u32::from_be_bytes([header[12], header[13], header[14], header[15]]);
    let destination_ipv4 = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    if source_ipv4 != peer.ipv4 || destination_ipv4 != local.ipv4 {
        return Err(Ipv4ParseError::Endpoint);
    }
    if header[9] != 1 {
        return Err(Ipv4ParseError::Protocol);
    }
    Ok(Ipv4PacketV0 {
        source_ipv4,
        destination_ipv4,
        protocol: header[9],
        payload: &frame[34..],
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IcmpEchoRequestV0<'a> {
    pub identifier: u16,
    pub sequence: u16,
    pub payload: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IcmpParseError {
    Length,
    TypeCode,
    Checksum,
}

pub fn parse_icmp_echo_request_v0(message: &[u8]) -> Result<IcmpEchoRequestV0<'_>, IcmpParseError> {
    if message.len() < 8 {
        return Err(IcmpParseError::Length);
    }
    if message[0] != 8 || message[1] != 0 {
        return Err(IcmpParseError::TypeCode);
    }
    if checksum(message) != 0 {
        return Err(IcmpParseError::Checksum);
    }
    Ok(IcmpEchoRequestV0 {
        identifier: u16::from_be_bytes([message[4], message[5]]),
        sequence: u16::from_be_bytes([message[6], message[7]]),
        payload: &message[8..],
    })
}

fn checksum(bytes: &[u8]) -> u16 {
    let mut sum = 0_u32;
    let (words, remainder) = bytes.as_chunks::<2>();
    for word in words {
        sum += u32::from(u16::from_be_bytes([word[0], word[1]]));
    }
    if let Some(last) = remainder.first() {
        sum += u32::from(*last) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
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
    const IPV4_FRAME: [u8; 42] = [
        0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02, 0x08, 0x00, 0x45,
        0x00, 0x00, 0x1c, 0x12, 0x34, 0x40, 0x00, 0x40, 0x01, 0x10, 0x9d, 10, 0, 2, 2, 10, 0, 2,
        15, 0x08, 0x00, 0x39, 0x0f, 0xbe, 0xef, 0x00, 0x01,
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

    #[test]
    fn validates_exact_unfragmented_ipv4_icmp_header() {
        let peer = EndpointV4 {
            mac: [0x52, 0x55, 0x0a, 0, 2, 2],
            ipv4: PEER,
        };
        let packet = parse_ipv4_icmp_v0(&IPV4_FRAME, LOCAL, peer).unwrap();
        assert_eq!(packet.source_ipv4, PEER);
        assert_eq!(packet.destination_ipv4, LOCAL.ipv4);
        assert_eq!(packet.protocol, 1);
        assert_eq!(packet.payload, &IPV4_FRAME[34..]);
    }

    #[test]
    fn rejects_ipv4_checksum_fragment_and_endpoint_changes() {
        let peer = EndpointV4 {
            mac: [0x52, 0x55, 0x0a, 0, 2, 2],
            ipv4: PEER,
        };
        let mut frame = IPV4_FRAME;
        frame[24] ^= 1;
        assert_eq!(
            parse_ipv4_icmp_v0(&frame, LOCAL, peer),
            Err(Ipv4ParseError::Checksum)
        );
        frame = IPV4_FRAME;
        frame[20] = 0x20;
        frame[24] = 0x30;
        frame[25] = 0x9d;
        assert_eq!(
            parse_ipv4_icmp_v0(&frame, LOCAL, peer),
            Err(Ipv4ParseError::Fragmented)
        );
        frame = IPV4_FRAME;
        frame[33] ^= 1;
        frame[24] = 0x10;
        frame[25] = 0x9e;
        assert_eq!(
            parse_ipv4_icmp_v0(&frame, LOCAL, peer),
            Err(Ipv4ParseError::Endpoint)
        );
    }

    #[test]
    fn validates_icmp_echo_request_fields_and_checksum() {
        let echo = parse_icmp_echo_request_v0(&IPV4_FRAME[34..]).unwrap();
        assert_eq!(echo.identifier, 0xbeef);
        assert_eq!(echo.sequence, 1);
        assert!(echo.payload.is_empty());
    }

    #[test]
    fn rejects_icmp_type_code_and_checksum_changes() {
        let mut message: [u8; 8] = IPV4_FRAME[34..].try_into().unwrap();
        message[0] = 3;
        message[2] = 0x3e;
        assert_eq!(
            parse_icmp_echo_request_v0(&message),
            Err(IcmpParseError::TypeCode)
        );
        message = IPV4_FRAME[34..].try_into().unwrap();
        message[2] ^= 1;
        assert_eq!(
            parse_icmp_echo_request_v0(&message),
            Err(IcmpParseError::Checksum)
        );
    }
}
