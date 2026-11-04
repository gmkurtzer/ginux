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

/// Whether `path` names an entry strictly below the directory `ancestor`.
/// Both are canonical slash-separated paths without a trailing slash.
pub fn strict_path_descendant(ancestor: &[u8], path: &[u8]) -> bool {
    path.len() > ancestor.len() && path.starts_with(ancestor) && path[ancestor.len()] == b'/'
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

/// First directory-stream position of the upper layer in a merged
/// (overlay-over-image) directory. Lower-layer cursors stay below it, so a
/// position handed to user space as `d_off` stays valid between calls.
pub const MERGED_DIRECTORY_UPPER_BASE: usize = 1 << 32;

pub enum MergedDirectoryEntry<L, U> {
    Lower(L),
    Upper(U),
}

/// Returns the next visible entry of a merged directory at or after
/// `position`, with the position that follows it.
///
/// Like Linux overlayfs (`fs/overlayfs/readdir.c`, `ovl_dir_read_merged`),
/// lower entries come first in their own order, minus whiteouts, followed by
/// the upper entries that do not replace a lower name. `lower(cursor)` and
/// `upper(cursor)` yield `None` on error, `Some(None)` at the end, or the
/// entry with its next cursor; `lower_visible` rejects whited-out lower
/// entries and `upper_only` rejects upper entries already reported in place
/// of a lower entry with the same name.
pub fn merged_directory_next<L, U>(
    mut position: usize,
    mut lower: impl FnMut(usize) -> Option<Option<(usize, L)>>,
    mut upper: impl FnMut(usize) -> Option<Option<(usize, U)>>,
    mut lower_visible: impl FnMut(&L) -> bool,
    mut upper_only: impl FnMut(&U) -> bool,
) -> Option<Option<(usize, MergedDirectoryEntry<L, U>)>> {
    while position < MERGED_DIRECTORY_UPPER_BASE {
        let Some((next, entry)) = lower(position)? else {
            position = MERGED_DIRECTORY_UPPER_BASE;
            break;
        };
        if next >= MERGED_DIRECTORY_UPPER_BASE {
            return None;
        }
        if lower_visible(&entry) {
            return Some(Some((next, MergedDirectoryEntry::Lower(entry))));
        }
        position = next;
    }
    let mut cursor = position - MERGED_DIRECTORY_UPPER_BASE;
    loop {
        let Some((next, entry)) = upper(cursor)? else {
            return Some(None);
        };
        if upper_only(&entry) {
            let next = MERGED_DIRECTORY_UPPER_BASE.checked_add(next)?;
            return Some(Some((next, MergedDirectoryEntry::Upper(entry))));
        }
        cursor = next;
    }
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
    fn strict_path_descendant_matches_whole_components() {
        assert!(strict_path_descendant(b"tmp/a", b"tmp/a/b"));
        assert!(strict_path_descendant(b"tmp", b"tmp/a/b"));
        assert!(!strict_path_descendant(b"tmp/a", b"tmp/a"));
        assert!(!strict_path_descendant(b"tmp/a", b"tmp/ab"));
        assert!(!strict_path_descendant(b"tmp/a/b", b"tmp/a"));
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

    // Lower layer modelled like the published rootfs: cursor 0 starts the
    // list, cursor i + 1 names entry i, and entries form a linked list.
    const LOWER: [(&str, usize); 4] = [("b", 3), ("shells", 2), ("a", 4), ("motd", 5)];
    const LOWER_HEAD: usize = 1;
    const LOWER_END: usize = 5;

    fn lower(cursor: usize) -> Option<Option<(usize, &'static str)>> {
        let index = match cursor {
            0 => LOWER_HEAD - 1,
            LOWER_END => return Some(None),
            cursor => cursor - 1,
        };
        let (name, next) = LOWER.get(index)?;
        Some(Some((*next, *name)))
    }

    fn upper(names: &[&'static str], cursor: usize) -> Option<Option<(usize, &'static str)>> {
        Some(names.get(cursor).map(|name| (cursor + 1, *name)))
    }

    fn merged(
        position: usize,
        upper_names: &[&'static str],
        whiteouts: &[&str],
    ) -> Option<Option<(usize, &'static str)>> {
        let step = merged_directory_next(
            position,
            lower,
            |cursor| upper(upper_names, cursor),
            |name| !whiteouts.contains(name),
            |name| LOWER.iter().all(|(lower, _)| lower != name),
        )?;
        Some(step.map(|(next, entry)| {
            (
                next,
                match entry {
                    MergedDirectoryEntry::Lower(name) | MergedDirectoryEntry::Upper(name) => name,
                },
            )
        }))
    }

    #[test]
    fn merged_directory_hides_whiteouts_and_lists_upper_entries_once() {
        // "motd" is a copy-up of a lower entry; "new" and "dir" exist only upper.
        let upper_names = ["new", "motd", "dir"];
        let mut names = [""; 8];
        let mut count = 0;
        let mut position = 0;
        while let Some((next, name)) = merged(position, &upper_names, &["shells"]).unwrap() {
            names[count] = name;
            count += 1;
            position = next;
        }
        assert_eq!(names[..count], ["b", "a", "motd", "new", "dir"]);
        assert_eq!(merged(position, &upper_names, &["shells"]), Some(None));
    }

    #[test]
    fn merged_directory_resumes_from_every_returned_position() {
        let upper_names = ["new", "motd", "dir"];
        let expected = ["b", "a", "motd", "new", "dir"];
        let mut position = 0;
        for index in 0..expected.len() {
            // A fresh call at each returned d_off neither skips nor repeats.
            let (next, name) = merged(position, &upper_names, &["shells"])
                .unwrap()
                .unwrap();
            assert_eq!(name, expected[index]);
            let mut rest = 0;
            let mut cursor = next;
            while let Some((after, _)) = merged(cursor, &upper_names, &["shells"]).unwrap() {
                rest += 1;
                cursor = after;
            }
            assert_eq!(rest, expected.len() - index - 1);
            position = next;
        }
    }

    #[test]
    fn merged_directory_without_lower_entries_lists_upper_only() {
        let whiteouts = ["b", "shells", "a", "motd"];
        let (next, name) = merged(0, &["x"], &whiteouts).unwrap().unwrap();
        assert_eq!((next, name), (MERGED_DIRECTORY_UPPER_BASE + 1, "x"));
        assert_eq!(merged(next, &["x"], &whiteouts), Some(None));
        assert_eq!(merged(0, &[], &whiteouts), Some(None));
    }
}
