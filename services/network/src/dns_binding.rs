const ETHERNET_HEADER_LENGTH: usize = 14;
const UDP_HEADER_LENGTH: usize = 8;
const DNS_HEADER_LENGTH: usize = 12;

#[derive(Clone, Copy)]
pub(crate) struct DnsQueryBinding {
    transaction_id: [u8; 2],
    response_addresses: [u8; 8],
    response_ports: [u8; 4],
    question_hash: u64,
}

fn ipv4_udp(frame: &[u8]) -> Option<(usize, usize)> {
    if frame.get(12..14)? != [0x08, 0x00] {
        return None;
    }
    let ip = ETHERNET_HEADER_LENGTH;
    if frame.get(ip + 9).copied()? != 17 {
        return None;
    }
    let header_length = usize::from(frame.get(ip).copied()? & 0x0f).checked_mul(4)?;
    if header_length < 20 {
        return None;
    }
    let udp = ip.checked_add(header_length)?;
    frame.get(udp..udp.checked_add(UDP_HEADER_LENGTH)?)?;
    Some((ip, udp))
}

fn skip_dns_name(message: &[u8], mut cursor: usize) -> Option<usize> {
    loop {
        let length = *message.get(cursor)?;
        if length == 0 {
            return cursor.checked_add(1);
        }
        if length & 0xc0 == 0xc0 {
            message.get(cursor + 1)?;
            return cursor.checked_add(2);
        }
        if length & 0xc0 != 0 || length > 63 {
            return None;
        }
        cursor = cursor.checked_add(1 + usize::from(length))?;
        message.get(..cursor)?;
    }
}

fn hash_question(question: &[u8]) -> u64 {
    question.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

pub(crate) fn capture_dns_query(frame: &[u8]) -> Option<DnsQueryBinding> {
    let (ip, udp) = ipv4_udp(frame)?;
    if frame.get(udp + 2..udp + 4)? != [0, 53] {
        return None;
    }
    let message = frame.get(udp.checked_add(UDP_HEADER_LENGTH)?..)?;
    let end = skip_dns_name(message, DNS_HEADER_LENGTH)?.checked_add(4)?;
    message.get(..end)?;
    let question = message.get(DNS_HEADER_LENGTH..end)?;
    let mut response_addresses = [0; 8];
    response_addresses[..4].copy_from_slice(frame.get(ip + 16..ip + 20)?);
    response_addresses[4..].copy_from_slice(frame.get(ip + 12..ip + 16)?);
    let mut response_ports = [0, 53, 0, 0];
    response_ports[2..].copy_from_slice(frame.get(udp..udp + 2)?);
    Some(DnsQueryBinding {
        transaction_id: message.get(..2)?.try_into().ok()?,
        response_addresses,
        response_ports,
        question_hash: hash_question(question),
    })
}

pub(crate) fn first_bound_dns_a_record(frame: &[u8], binding: &DnsQueryBinding) -> Option<u32> {
    let (ip, udp) = ipv4_udp(frame)?;
    if frame.get(ip + 12..ip + 20)? != binding.response_addresses
        || frame.get(udp..udp + 4)? != binding.response_ports
    {
        return None;
    }
    let message = frame.get(udp.checked_add(UDP_HEADER_LENGTH)?..)?;
    let flags = u16::from_be_bytes(message.get(2..4)?.try_into().ok()?);
    let questions = u16::from_be_bytes(message.get(4..6)?.try_into().ok()?);
    let answers = usize::from(u16::from_be_bytes(message.get(6..8)?.try_into().ok()?));
    if message.get(..2)? != binding.transaction_id
        || flags & 0x8000 == 0
        || flags & 0x000f != 0
        || questions != 1
    {
        return None;
    }
    let mut cursor = skip_dns_name(message, DNS_HEADER_LENGTH)?.checked_add(4)?;
    message.get(..cursor)?;
    let question = message.get(DNS_HEADER_LENGTH..cursor)?;
    if hash_question(question) != binding.question_hash {
        return None;
    }
    for _ in 0..answers {
        cursor = skip_dns_name(message, cursor)?;
        let record_type = u16::from_be_bytes(message.get(cursor..cursor + 2)?.try_into().ok()?);
        let class = u16::from_be_bytes(message.get(cursor + 2..cursor + 4)?.try_into().ok()?);
        let length = usize::from(u16::from_be_bytes(
            message.get(cursor + 8..cursor + 10)?.try_into().ok()?,
        ));
        let data = cursor.checked_add(10)?;
        let end = data.checked_add(length)?;
        let bytes = message.get(data..end)?;
        if record_type == 1 && class == 1 && length == 4 {
            return Some(u32::from_be_bytes(bytes.try_into().ok()?));
        }
        cursor = end;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{capture_dns_query, first_bound_dns_a_record};

    fn frame(response: bool) -> std::vec::Vec<u8> {
        let mut output = std::vec![0_u8; 14 + 20 + 8 + 12];
        output[12..14].copy_from_slice(&0x0800_u16.to_be_bytes());
        output[14] = 0x45;
        output[23] = 17;
        let (source, destination, source_port, destination_port) = if response {
            (0x0a00_0203_u32, 0x0a00_020f_u32, 53_u16, 49_152_u16)
        } else {
            (0x0a00_020f_u32, 0x0a00_0203_u32, 49_152_u16, 53_u16)
        };
        output[26..30].copy_from_slice(&source.to_be_bytes());
        output[30..34].copy_from_slice(&destination.to_be_bytes());
        output[34..36].copy_from_slice(&source_port.to_be_bytes());
        output[36..38].copy_from_slice(&destination_port.to_be_bytes());
        let dns = 42;
        output[dns..dns + 2].copy_from_slice(&0x1234_u16.to_be_bytes());
        output[dns + 2..dns + 4]
            .copy_from_slice(&(if response { 0x8180_u16 } else { 0x0100_u16 }).to_be_bytes());
        output[dns + 4..dns + 6].copy_from_slice(&1_u16.to_be_bytes());
        output[dns + 6..dns + 8].copy_from_slice(&(u16::from(response)).to_be_bytes());
        output.extend_from_slice(b"\x08registry\x06docker\x02io\x00\x00\x01\x00\x01");
        if response {
            output.extend_from_slice(&[
                0xc0, 0x0c, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3c, 0x00, 0x04, 1, 2, 3, 4,
            ]);
        }
        output
    }

    #[test]
    fn accepts_only_the_bound_dns_response() {
        let binding = capture_dns_query(&frame(false)).expect("query binding");
        assert_eq!(
            first_bound_dns_a_record(&frame(true), &binding),
            Some(0x0102_0304)
        );
    }

    #[test]
    fn rejects_wrong_transaction_endpoint_port_and_question() {
        let binding = capture_dns_query(&frame(false)).expect("query binding");
        let mut mutations = [frame(true), frame(true), frame(true), frame(true)];
        mutations[0][42] ^= 1;
        mutations[1][26] ^= 1;
        mutations[2][36..38].copy_from_slice(&49_153_u16.to_be_bytes());
        mutations[3][55] ^= 1;
        for response in mutations {
            assert_eq!(first_bound_dns_a_record(&response, &binding), None);
        }
    }
}
