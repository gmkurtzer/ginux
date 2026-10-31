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
}
