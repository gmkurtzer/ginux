use core::arch::asm;

use crate::memory;

const PAGE_SIZE: usize = 4096;
const ENTRY_COUNT: usize = 512;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const USER_TEST_ADDRESS: usize = 1usize << 39;
const TEST_PATTERN: u64 = 0x4749_4e55_5855_5345;

struct AddressSpace {
    root: u64,
    user_frame: u64,
}

impl AddressSpace {
    fn with_single_user_page(kernel_root: u64) -> Option<Self> {
        let root = memory::allocate_frame()?;
        let user_pdpt = memory::allocate_frame()?;
        let user_directory = memory::allocate_frame()?;
        let user_table = memory::allocate_frame()?;
        let user_frame = memory::allocate_frame()?;
        let frames = [root, user_pdpt, user_directory, user_table, user_frame];
        if frames.iter().any(|frame| frame % PAGE_SIZE as u64 != 0) {
            return None;
        }
        for left in 0..frames.len() {
            if frames[left + 1..].contains(&frames[left]) {
                return None;
            }
        }

        // SAFETY: Every frame was uniquely allocated from mapped conventional
        // memory. They remain owned by this address space for kernel lifetime.
        unsafe {
            for frame in [root, user_pdpt, user_directory, user_table] {
                zero_frame(frame);
            }
            let kernel_pml4 = kernel_root as *const u64;
            let root_entries = root as *mut u64;
            root_entries.write(kernel_pml4.read());
            root_entries
                .add(1)
                .write(user_pdpt | PRESENT | WRITABLE | USER);
            (user_pdpt as *mut u64).write(user_directory | PRESENT | WRITABLE | USER);
            (user_directory as *mut u64).write(user_table | PRESENT | WRITABLE | USER);
            (user_table as *mut u64).write(user_frame | PRESENT | WRITABLE | USER);
        }
        Some(Self { root, user_frame })
    }

    fn layout_is_isolated(&self, kernel_root: u64) -> bool {
        // SAFETY: Both roots and the page-table frames reachable from this
        // address space remain allocated and identity mapped.
        unsafe {
            let root = self.root as *const u64;
            let kernel = kernel_root as *const u64;
            let kernel_entry = kernel.read();
            if root.read() != kernel_entry || kernel_entry & USER != 0 {
                return false;
            }
            let user_root_entry = root.add(1).read();
            if user_root_entry & (PRESENT | WRITABLE | USER) != (PRESENT | WRITABLE | USER) {
                return false;
            }
            for index in 2..ENTRY_COUNT {
                if root.add(index).read() != 0 {
                    return false;
                }
            }
            let pdpt = (user_root_entry & ADDRESS_MASK) as *const u64;
            let directory = (pdpt.read() & ADDRESS_MASK) as *const u64;
            let table = (directory.read() & ADDRESS_MASK) as *const u64;
            table.read() & ADDRESS_MASK == self.user_frame
                && table.read() & (PRESENT | WRITABLE | USER) == (PRESENT | WRITABLE | USER)
                && table.add(1).read() == 0
        }
    }
}

pub(crate) fn verify_isolated_user_mapping() -> bool {
    let kernel_root = current_root();
    let Some(space) = AddressSpace::with_single_user_page(kernel_root) else {
        return false;
    };
    if !space.layout_is_isolated(kernel_root) {
        return false;
    }

    // SAFETY: The new root preserves the active supervisor-only kernel entry,
    // and maps the test virtual page to its exclusively owned frame. Interrupts
    // remain disabled. The original root is restored before returning.
    unsafe {
        (space.user_frame as *mut u64).write_volatile(0);
        activate(space.root);
        let activated = current_root() == space.root;
        (USER_TEST_ADDRESS as *mut u64).write_volatile(TEST_PATTERN);
        let aliased = (space.user_frame as *const u64).read_volatile() == TEST_PATTERN;
        activate(kernel_root);
        let restored = current_root() == kernel_root;
        (space.user_frame as *mut u64).write_volatile(0);
        activated && aliased && restored
    }
}

fn current_root() -> u64 {
    let root: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) root, options(nomem, nostack, preserves_flags));
    }
    root & ADDRESS_MASK
}

unsafe fn activate(root: u64) {
    asm!("mov cr3, {}", in(reg) root, options(nostack, preserves_flags));
}

unsafe fn zero_frame(frame: u64) {
    let entries = frame as *mut u64;
    for index in 0..ENTRY_COUNT {
        entries.add(index).write(0);
    }
}
