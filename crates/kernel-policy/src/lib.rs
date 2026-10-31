#![no_std]

pub fn valid_domain_personality(personality: &[u8]) -> bool {
    personality == b"linux"
}

pub fn valid_container_name(name: &[u8], maximum_size: usize) -> bool {
    !name.is_empty()
        && name.len() <= maximum_size
        && name
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub fn valid_container_init(init: &[u8], maximum_size: usize) -> bool {
    !init.is_empty()
        && init.len() <= maximum_size
        && init[0] == b'/'
        && init.iter().all(u8::is_ascii_graphic)
}

pub fn valid_vtd_write_fault_record(high: u64, source: u16) -> bool {
    const VALID: u64 = 1 << 63;
    const TRANSACTION_READ: u64 = 1 << 62;
    const PASID_SHIFT: u64 = 40;
    const PASID_MASK: u64 = 0xffff;
    const REASON_SHIFT: u64 = 32;
    const REASON_MASK: u64 = 0xff;
    const WRITE_PERMISSION_FAULT: u64 = 5;
    const SOURCE_MASK: u64 = 0xffff;
    const ALLOWED_BITS: u64 =
        VALID | (PASID_MASK << PASID_SHIFT) | (REASON_MASK << REASON_SHIFT) | SOURCE_MASK;

    let pasid = high >> PASID_SHIFT & PASID_MASK;
    high & VALID != 0
        && high & TRANSACTION_READ == 0
        && high & !ALLOWED_BITS == 0
        && matches!(pasid, 0 | PASID_MASK)
        && high >> REASON_SHIFT & REASON_MASK == WRITE_PERMISSION_FAULT
        && high & SOURCE_MASK == u64::from(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personality_accepts_only_linux() {
        assert!(valid_domain_personality(b"linux"));
        assert!(!valid_domain_personality(b"Linux"));
        assert!(!valid_domain_personality(b""));
    }

    #[test]
    fn container_name_enforces_kernel_policy() {
        assert!(valid_container_name(b"rocky-10_test", 32));
        assert!(!valid_container_name(b"", 32));
        assert!(!valid_container_name(b"rocky/latest", 32));
        assert!(!valid_container_name(b"too-long", 7));
    }

    #[test]
    fn container_init_requires_absolute_graphic_path() {
        assert!(valid_container_init(b"/sbin/init", 64));
        assert!(!valid_container_init(b"sbin/init", 64));
        assert!(!valid_container_init(b"/sbin/ init", 64));
        assert!(!valid_container_init(b"/too/long", 8));
    }

    #[test]
    fn vtd_write_fault_accepts_qemu_10_and_11_no_pasid_encodings() {
        const VALID: u64 = 1 << 63;
        const SOURCE: u16 = 0x10;
        const WRITE_PERMISSION: u64 = 5 << 32;
        let qemu_10 = VALID | (0xffff << 40) | WRITE_PERMISSION | u64::from(SOURCE);
        let qemu_11 = VALID | WRITE_PERMISSION | u64::from(SOURCE);

        assert!(valid_vtd_write_fault_record(qemu_10, SOURCE));
        assert!(valid_vtd_write_fault_record(qemu_11, SOURCE));
        assert!(!valid_vtd_write_fault_record(qemu_11 | (1 << 62), SOURCE));
        assert!(!valid_vtd_write_fault_record(qemu_11 | (1 << 61), SOURCE));
        assert!(!valid_vtd_write_fault_record(qemu_11, SOURCE + 1));
        assert!(!valid_vtd_write_fault_record(qemu_11 ^ (1 << 32), SOURCE));
    }
}
