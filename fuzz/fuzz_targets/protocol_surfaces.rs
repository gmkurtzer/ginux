#![no_main]

// Fuzz harness #2 (v2.6-4): ginux `crates/protocol-state` network parsers.
//
// Rank 1 of the reports/fuzz-quality.md §d inventory: raw network bytes
// (ARP/IPv4/ICMP) are the farthest untrusted-input surface in the kernel, and
// every parser here is a pure function over `&[u8]` in a host-buildable crate.
//
// Kernel-faithful modeling: the host's own endpoint (LOCAL) and the expected
// peer (PEER) are runtime constants in the kernel — the FRAME is the only
// attacker-controlled input, so both are FIXED here. Feeding attacker-chosen
// endpoints would explore branches the kernel cannot reach (false positives).
//
// MAX_FRAME bound: the kernel can only deliver <= 65,535+14 bytes to these
// parsers (IPv4 total_length is u16, enforced by `frame.len() != 14 +
// total_length`). Inputs larger than ~131,076 bytes would overflow the u32
// accumulator in the crate's internal `checksum()` — a kernel-unreachable
// state, so the harness caps at 65,600 to avoid false findings.

use libfuzzer_sys::fuzz_target;

const LOCAL: ginux_protocol_state::EndpointV4 = ginux_protocol_state::EndpointV4 {
    mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
    ipv4: u32::from_be_bytes([10, 0, 2, 15]),
};
const PEER: ginux_protocol_state::EndpointV4 = ginux_protocol_state::EndpointV4 {
    mac: [0x52, 0x55, 0x0a, 0x00, 0x02, 0x02],
    ipv4: u32::from_be_bytes([10, 0, 2, 2]),
};
const PEER_IPV4: u32 = u32::from_be_bytes([10, 0, 2, 2]);
const MAX_FRAME: usize = 65_600;

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_FRAME)];

    // 1) ARP reply (exact 42-byte ethernet+arp frame; attacker bytes) +
    //    neighbor state machine over accepted replies. The kernel's neighbor
    //    state persists ACROSS frames, so the harness observes the parsed
    //    reply twice (Learned+Refreshed) and a second independently-parsed
    //    reply with a mutated sender MAC (Conflict path).
    if let Ok(reply) = ginux_protocol_state::parse_arp_reply_v4(data, LOCAL, PEER_IPV4) {
        let mut state = ginux_protocol_state::NeighborStateV4::new(PEER_IPV4);
        let _ = state.observe(reply);
        let _ = state.observe(reply);
        let mut second = data.to_vec();
        second[6] ^= 0x01;
        second[22] ^= 0x01;
        if let Ok(reply2) = ginux_protocol_state::parse_arp_reply_v4(&second, LOCAL, PEER_IPV4) {
            let _ = state.observe(reply2);
        }
        let _ = state.mac();
        let _ = state.accepted();
    }

    // 2) IPv4+ICMP frame, then the ICMP echo request carried in its payload
    //    (the kernel parses this exact chain).
    if let Ok(packet) = ginux_protocol_state::parse_ipv4_icmp_v0(data, LOCAL, PEER) {
        let _ = (packet.source_ipv4, packet.destination_ipv4, packet.protocol);
        if let Ok(echo) = ginux_protocol_state::parse_icmp_echo_request_v0(packet.payload) {
            let _ = (echo.identifier, echo.sequence, echo.payload.len());
        }
    }

    // 3) direct ICMP echo request (attacker-shaped message, kernel-bounded size).
    if let Ok(echo) = ginux_protocol_state::parse_icmp_echo_request_v0(data) {
        let _ = (echo.identifier, echo.sequence, echo.payload.len());
    }

    // 4) reply builder over an owned mutable copy (kernel passes the received
    //    frame buffer; rejected inputs must not mutate it — upstream test
    //    does_not_mutate_rejected_echo_request).
    let mut frame = data.to_vec();
    let _ = ginux_protocol_state::build_icmp_echo_reply_v0(&mut frame, LOCAL, PEER);
});
