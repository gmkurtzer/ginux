use core::arch::{asm, x86_64::__cpuid};

use crate::{
    arch::x86_64::{
        apic::LocalApic,
        cpu,
        timer::{self, LocalApicCalibration},
    },
    ipc, memory, scheduler,
};

const PAGE_SIZE: usize = 4096;
const ENTRY_COUNT: usize = 512;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const USER_TEST_ADDRESS: usize = 1usize << 39;
const EXPECTED_FAULT_OFFSET: usize = 333;
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
    frames: [memory::FrameLease; 5],
}

impl AddressSpace {
    fn with_single_user_page(kernel_root: u64) -> Option<Self> {
        let frames = memory::allocate_frames::<5>(memory::FrameOwner::KERNEL)?;
        let addresses = frames.map(memory::FrameLease::address);
        let [root, user_pdpt, user_directory, user_table, user_frame] = addresses;
        if addresses.iter().any(|frame| frame % PAGE_SIZE as u64 != 0) {
            return None;
        }
        for left in 0..addresses.len() {
            if addresses[left + 1..].contains(&addresses[left]) {
                return None;
            }
        }

        // SAFETY: Every frame was uniquely leased from mapped conventional
        // memory and remains owned until this address space is released.
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
        Some(Self {
            root,
            user_frame,
            frames,
        })
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

    fn release(self) -> bool {
        let frames = self.frames;
        unsafe {
            for frame in &frames {
                zero_frame(frame.address());
            }
        }
        memory::release_frames(memory::FrameOwner::KERNEL, frames)
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
    let valid = unsafe {
        (space.user_frame as *mut u64).write_volatile(0);
        activate(space.root);
        let activated = current_root() == space.root;
        (USER_TEST_ADDRESS as *mut u64).write_volatile(TEST_PATTERN);
        let aliased = (space.user_frame as *const u64).read_volatile() == TEST_PATTERN;
        activate(kernel_root);
        let restored = current_root() == kernel_root;
        (space.user_frame as *mut u64).write_volatile(0);
        activated && aliased && restored
    };
    valid && space.release()
}

pub(crate) fn verify_control_plane_wx_layout() -> bool {
    if !enable_execute_disable() {
        return false;
    }
    let kernel_root = current_root();
    let Some(space) = WxAddressSpace::new(kernel_root, memory::FrameOwner::KERNEL) else {
        return false;
    };
    if !space.permissions_are_valid(kernel_root) {
        return false;
    }

    // SAFETY: Both mapped pages and all page tables have unique leases until
    // release. The kernel mapping is preserved and interrupts are disabled.
    let valid = unsafe {
        space.code_frame.cast::<u8>().write_volatile(0x90);
        space.first_stack_frame.cast::<u64>().write_volatile(0);
        space.second_stack_frame.cast::<u64>().write_volatile(0);
        activate(space.root);
        let code_visible = (USER_TEST_ADDRESS as *const u8).read_volatile() == 0x90;
        let first_stack_address = USER_TEST_ADDRESS + 2 * PAGE_SIZE;
        let second_stack_address = USER_TEST_ADDRESS + 4 * PAGE_SIZE;
        (first_stack_address as *mut u64).write_volatile(TEST_PATTERN);
        (second_stack_address as *mut u64).write_volatile(!TEST_PATTERN);
        let stacks_writable = space.first_stack_frame.cast::<u64>().read_volatile() == TEST_PATTERN
            && space.second_stack_frame.cast::<u64>().read_volatile() == !TEST_PATTERN;
        activate(kernel_root);
        let restored = current_root() == kernel_root;
        space.code_frame.cast::<u8>().write_volatile(0);
        space.first_stack_frame.cast::<u64>().write_volatile(0);
        space.second_stack_frame.cast::<u64>().write_volatile(0);
        code_visible && stacks_writable && restored
    };
    valid && space.release()
}

pub(crate) fn verify_ring3_control_plane(
    program: &[u8],
    entry_offsets: [usize; 3],
    domains: [u64; 2],
    handles: [[u64; 3]; 2],
    local_apic: &LocalApic,
    apic_timer: &LocalApicCalibration,
) -> bool {
    if !enable_execute_disable() {
        return false;
    }
    let kernel_root = current_root();
    let (Some(control_owner), Some(service_owner)) = (
        memory::FrameOwner::domain(domains[0]),
        memory::FrameOwner::domain(domains[1]),
    ) else {
        return false;
    };
    let Some(first_space) = WxAddressSpace::new(kernel_root, control_owner) else {
        return false;
    };
    let Some(mut second_space) = WxAddressSpace::new(kernel_root, service_owner) else {
        let _ = first_space.release();
        return false;
    };
    if !first_space.permissions_are_valid(kernel_root)
        || !second_space.permissions_are_valid(kernel_root)
        || !first_space.is_disjoint_from(&second_space)
    {
        return false;
    }
    let [control_entry_offset, service_entry_offset, service_restart_offset] = entry_offsets;
    let Some(expected_fault_offset) = control_entry_offset.checked_add(EXPECTED_FAULT_OFFSET)
    else {
        return false;
    };
    if program.is_empty()
        || program.len() > PAGE_SIZE
        || entry_offsets.iter().any(|entry| *entry >= program.len())
        || expected_fault_offset >= program.len()
        || service_entry_offset >= program.len()
        || service_restart_offset >= program.len()
    {
        return false;
    }
    unsafe {
        for (offset, byte) in program.iter().enumerate() {
            first_space.code_frame.add(offset).write(*byte);
            second_space.code_frame.add(offset).write(*byte);
        }
        first_space
            .first_stack_frame
            .cast::<u64>()
            .write_volatile(TEST_PATTERN);
        second_space
            .first_stack_frame
            .cast::<u64>()
            .write_volatile(!TEST_PATTERN);
        activate(first_space.root);
        let first_marker = ((USER_TEST_ADDRESS + 2 * PAGE_SIZE) as *const u64).read_volatile();
        activate(second_space.root);
        let second_marker = ((USER_TEST_ADDRESS + 2 * PAGE_SIZE) as *const u64).read_volatile();
        activate(kernel_root);
        first_space
            .first_stack_frame
            .cast::<u64>()
            .write_volatile(0);
        second_space
            .first_stack_frame
            .cast::<u64>()
            .write_volatile(0);
        if first_marker != TEST_PATTERN || second_marker != !TEST_PATTERN {
            return false;
        }
        if !ipc::initialize_control_channel(domains[0], domains[1])
            || !scheduler::initialize_runtime(
                domains,
                [first_space.root, second_space.root],
                [
                    USER_TEST_ADDRESS + control_entry_offset,
                    USER_TEST_ADDRESS + service_entry_offset,
                ],
                [
                    USER_TEST_ADDRESS + 3 * PAGE_SIZE,
                    USER_TEST_ADDRESS + 3 * PAGE_SIZE,
                ],
                handles,
                USER_TEST_ADDRESS + expected_fault_offset,
                USER_TEST_ADDRESS + service_restart_offset,
            )
        {
            return false;
        }
        if !timer::arm_scheduler_preemption(local_apic, apic_timer) {
            return false;
        }
        activate(first_space.root);
        let handled = cpu::verify_user_syscall(
            USER_TEST_ADDRESS + control_entry_offset,
            USER_TEST_ADDRESS + 3 * PAGE_SIZE,
            handles[0][0],
            handles[0][1],
            handles[0][2],
            domains[1],
        );
        activate(kernel_root);
        let preemption_complete = timer::finish_scheduler_preemption(local_apic);
        let valid = handled
            && scheduler::verify_runtime_complete()
            && ipc::is_empty()
            && current_root() == kernel_root
            && preemption_complete;
        let owner_adopted = scheduler::service_address_space_owner()
            .and_then(memory::FrameOwner::domain)
            .is_some_and(|owner| second_space.adopt_transferred_owner(owner));
        valid && owner_adopted && first_space.release() && second_space.release()
    }
}

struct WxAddressSpace {
    root: u64,
    code_frame: *mut u8,
    first_stack_frame: *mut u8,
    second_stack_frame: *mut u8,
    frames: [memory::FrameLease; 7],
    owner: memory::FrameOwner,
}

impl WxAddressSpace {
    fn new(kernel_root: u64, owner: memory::FrameOwner) -> Option<Self> {
        let frames = memory::allocate_frames::<7>(owner)?;
        let addresses = frames.map(memory::FrameLease::address);
        let [root, pdpt, directory, table, code, first_stack, second_stack] = addresses;
        for left in 0..addresses.len() {
            if addresses[left] % PAGE_SIZE as u64 != 0
                || addresses[left + 1..].contains(&addresses[left])
            {
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
                .write(first_stack | PRESENT | WRITABLE | USER | NO_EXECUTE);
            entries.add(3).write(0);
            entries
                .add(4)
                .write(second_stack | PRESENT | WRITABLE | USER | NO_EXECUTE);
        }
        Some(Self {
            root,
            code_frame: code as *mut u8,
            first_stack_frame: first_stack as *mut u8,
            second_stack_frame: second_stack as *mut u8,
            frames,
            owner,
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
            let first_guard = table.add(1).read();
            let first_stack = table.add(2).read();
            let second_guard = table.add(3).read();
            let second_stack = table.add(4).read();
            code & (PRESENT | USER) == (PRESENT | USER)
                && code & (WRITABLE | NO_EXECUTE) == 0
                && first_guard == 0
                && first_stack & (PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == (PRESENT | WRITABLE | USER | NO_EXECUTE)
                && second_guard == 0
                && second_stack & (PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == (PRESENT | WRITABLE | USER | NO_EXECUTE)
                && code & ADDRESS_MASK == self.code_frame as u64
                && first_stack & ADDRESS_MASK == self.first_stack_frame as u64
                && second_stack & ADDRESS_MASK == self.second_stack_frame as u64
        }
    }

    fn is_disjoint_from(&self, other: &Self) -> bool {
        self.root != other.root
            && self.code_frame != other.code_frame
            && self.first_stack_frame != other.first_stack_frame
            && self.second_stack_frame != other.second_stack_frame
    }

    fn adopt_transferred_owner(&mut self, owner: memory::FrameOwner) -> bool {
        if !memory::frames_owned_by(owner, &self.frames) {
            return false;
        }
        self.owner = owner;
        true
    }

    fn release(self) -> bool {
        let frames = self.frames;
        unsafe {
            for frame in &frames {
                zero_frame(frame.address());
            }
        }
        memory::release_frames(self.owner, frames)
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
