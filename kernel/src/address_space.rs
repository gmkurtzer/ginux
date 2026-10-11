use core::arch::{asm, x86_64::__cpuid};

use crate::{arch::x86_64::cpu, memory};

const PAGE_SIZE: usize = 4096;
const ENTRY_COUNT: usize = 512;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const USER_TEST_ADDRESS: usize = 1usize << 39;
const TEST_PATTERN: u64 = 0x4749_4e55_5855_5345;
const NO_EXECUTE: u64 = 1 << 63;
const EXTENDED_CPUID_MAX: u32 = 0x8000_0000;
const EXTENDED_FEATURES: u32 = 0x8000_0001;
const NX_FEATURE: u32 = 1 << 20;
const EFER_MSR: u32 = 0xc000_0080;
const EFER_NXE: u64 = 1 << 11;

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

pub(crate) fn verify_control_plane_wx_layout() -> bool {
    if !enable_execute_disable() {
        return false;
    }
    let kernel_root = current_root();
    let Some(space) = WxAddressSpace::new(kernel_root) else {
        return false;
    };
    if !space.permissions_are_valid(kernel_root) {
        return false;
    }

    // SAFETY: Both mapped pages and all page tables have unique lifetime-long
    // ownership. The kernel mapping is preserved and interrupts are disabled.
    unsafe {
        space.code_frame.cast::<u8>().write_volatile(0x90);
        space.stack_frame.cast::<u64>().write_volatile(0);
        activate(space.root);
        let code_visible = (USER_TEST_ADDRESS as *const u8).read_volatile() == 0x90;
        let stack_address = USER_TEST_ADDRESS + 2 * PAGE_SIZE;
        (stack_address as *mut u64).write_volatile(TEST_PATTERN);
        let stack_writable = space.stack_frame.cast::<u64>().read_volatile() == TEST_PATTERN;
        activate(kernel_root);
        let restored = current_root() == kernel_root;
        space.code_frame.cast::<u8>().write_volatile(0);
        space.stack_frame.cast::<u64>().write_volatile(0);
        code_visible && stack_writable && restored
    }
}

pub(crate) fn verify_ring3_control_plane(
    program: &[u8],
    host_handle: u64,
    console_handle: u64,
) -> bool {
    if !enable_execute_disable() {
        return false;
    }
    let kernel_root = current_root();
    let Some(space) = WxAddressSpace::new(kernel_root) else {
        return false;
    };
    if !space.permissions_are_valid(kernel_root) {
        return false;
    }
    if program.is_empty() || program.len() > PAGE_SIZE {
        return false;
    }
    unsafe {
        for (offset, byte) in program.iter().enumerate() {
            space.code_frame.add(offset).write(*byte);
        }
        activate(space.root);
        let handled = cpu::verify_user_syscall(
            USER_TEST_ADDRESS,
            USER_TEST_ADDRESS + 3 * PAGE_SIZE,
            host_handle,
            console_handle,
        );
        activate(kernel_root);
        handled && current_root() == kernel_root
    }
}

struct WxAddressSpace {
    root: u64,
    code_frame: *mut u8,
    stack_frame: *mut u8,
}

impl WxAddressSpace {
    fn new(kernel_root: u64) -> Option<Self> {
        let root = memory::allocate_frame()?;
        let pdpt = memory::allocate_frame()?;
        let directory = memory::allocate_frame()?;
        let table = memory::allocate_frame()?;
        let code = memory::allocate_frame()?;
        let stack = memory::allocate_frame()?;
        let frames = [root, pdpt, directory, table, code, stack];
        for left in 0..frames.len() {
            if frames[left] % PAGE_SIZE as u64 != 0 || frames[left + 1..].contains(&frames[left]) {
                return None;
            }
        }
        unsafe {
            for frame in [root, pdpt, directory, table] {
                zero_frame(frame);
            }
            let root_entries = root as *mut u64;
            root_entries.write((kernel_root as *const u64).read());
            root_entries.add(1).write(pdpt | PRESENT | WRITABLE | USER);
            (pdpt as *mut u64).write(directory | PRESENT | WRITABLE | USER);
            (directory as *mut u64).write(table | PRESENT | WRITABLE | USER);
            let entries = table as *mut u64;
            entries.write(code | PRESENT | USER);
            entries.add(1).write(0);
            entries
                .add(2)
                .write(stack | PRESENT | WRITABLE | USER | NO_EXECUTE);
        }
        Some(Self {
            root,
            code_frame: code as *mut u8,
            stack_frame: stack as *mut u8,
        })
    }

    fn permissions_are_valid(&self, kernel_root: u64) -> bool {
        unsafe {
            let root = self.root as *const u64;
            if root.read() != (kernel_root as *const u64).read() || root.read() & USER != 0 {
                return false;
            }
            let pdpt = (root.add(1).read() & ADDRESS_MASK) as *const u64;
            let directory = (pdpt.read() & ADDRESS_MASK) as *const u64;
            let table = (directory.read() & ADDRESS_MASK) as *const u64;
            let code = table.read();
            let guard = table.add(1).read();
            let stack = table.add(2).read();
            code & (PRESENT | USER) == (PRESENT | USER)
                && code & (WRITABLE | NO_EXECUTE) == 0
                && guard == 0
                && stack & (PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == (PRESENT | WRITABLE | USER | NO_EXECUTE)
                && code & ADDRESS_MASK == self.code_frame as u64
                && stack & ADDRESS_MASK == self.stack_frame as u64
        }
    }
}

fn enable_execute_disable() -> bool {
    if __cpuid(EXTENDED_CPUID_MAX).eax < EXTENDED_FEATURES
        || __cpuid(EXTENDED_FEATURES).edx & NX_FEATURE == 0
    {
        return false;
    }
    unsafe {
        let efer = read_msr(EFER_MSR);
        write_msr(EFER_MSR, efer | EFER_NXE);
        read_msr(EFER_MSR) & EFER_NXE != 0
    }
}

unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags));
    (u64::from(high) << 32) | u64::from(low)
}

unsafe fn write_msr(msr: u32, value: u64) {
    asm!("wrmsr", in("ecx") msr, in("eax") value as u32, in("edx") (value >> 32) as u32, options(nomem, nostack, preserves_flags));
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
