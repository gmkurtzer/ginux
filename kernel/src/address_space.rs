use core::{arch::asm, mem::size_of};

use crate::{
    arch::x86_64::{
        apic::LocalApic,
        cpu, paging,
        timer::{self, LocalApicCalibration},
    },
    domain, ipc, memory, network, scheduler,
};

const PAGE_SIZE: usize = 4096;
const ENTRY_COUNT: usize = 512;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const LARGE_PAGE: u64 = 1 << 7;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const USER_TEST_ADDRESS: usize = 1usize << 39;
const EXPECTED_FAULT_OFFSET: usize = 515;
const TEST_PATTERN: u64 = 0x4749_4e55_5855_5345;
const NO_EXECUTE: u64 = 1 << 63;
const LINUX_INIT_ARGUMENT: [u8; 6] = [b'/', b'i', b'n', b'i', b't', 0];
const LINUX_INIT_ENVIRONMENT: [u8; 8] = [b'G', b'I', b'N', b'U', b'X', b'=', b'1', 0];
const LINUX_PLATFORM: [u8; 7] = [b'x', b'8', b'6', b'_', b'6', b'4', 0];
pub(crate) const USER_ADDRESS_SPACE_FRAMES: usize = 7;
const PAGE_ATTRIBUTE_TABLE: u64 = 1 << 12;

static mut AUXILIARY_ADDRESS_SPACES: [Option<RegisteredAuxiliaryAddressSpace>;
    domain::MAX_AUXILIARY_SERVICES] = [const { None }; domain::MAX_AUXILIARY_SERVICES];

pub(crate) fn verify_linux_elf_mapping(
    plan: &crate::linux_elf::LoadPlan<'_>,
    protected_start: u64,
    protected_size: usize,
) -> bool {
    let first = plan.executable;
    let header = plan.read_only;
    let second = plan.writable;
    let Some(relro) = plan.relro else {
        return false;
    };
    let indexes = |address: u64| {
        [
            ((address >> 39) & 0x1ff) as usize,
            ((address >> 30) & 0x1ff) as usize,
            ((address >> 21) & 0x1ff) as usize,
            ((address >> 12) & 0x1ff) as usize,
        ]
    };
    let first_indexes = indexes(first.virtual_page);
    let header_indexes = header.map_or([0; 4], |segment| indexes(segment.virtual_page));
    let second_indexes = indexes(second.virtual_page);
    let first_last_page = first
        .virtual_page
        .checked_add((first.page_count - 1) as u64 * PAGE_SIZE as u64)
        .unwrap_or(0);
    let second_last_page = second
        .virtual_page
        .checked_add((second.page_count - 1) as u64 * PAGE_SIZE as u64)
        .unwrap_or(0);
    let header_last_page = header.map_or(0, |segment| {
        segment
            .virtual_page
            .checked_add((segment.page_count - 1) as u64 * PAGE_SIZE as u64)
            .unwrap_or(0)
    });
    let first_last_indexes = indexes(first_last_page);
    let second_last_indexes = indexes(second_last_page);
    let header_last_indexes = indexes(header_last_page);
    let heap_virtual_page = second
        .virtual_page
        .checked_add(second.page_count as u64 * PAGE_SIZE as u64)
        .unwrap_or(0);
    let stack_virtual_page = heap_virtual_page
        .checked_add(2 * PAGE_SIZE as u64)
        .unwrap_or(0);
    let mmap_virtual_page = stack_virtual_page
        .checked_add(2 * PAGE_SIZE as u64)
        .unwrap_or(0);
    let mmap_tail_virtual_page = mmap_virtual_page.checked_add(PAGE_SIZE as u64).unwrap_or(0);
    let heap_indexes = indexes(heap_virtual_page);
    let stack_indexes = indexes(stack_virtual_page);
    let mmap_indexes = indexes(mmap_virtual_page);
    let mmap_tail_indexes = indexes(mmap_tail_virtual_page);
    let Some(protected_end) = protected_start.checked_add(protected_size as u64) else {
        return false;
    };
    let overlaps_protected = |page: u64| {
        page.checked_add(PAGE_SIZE as u64)
            .is_none_or(|end| page < protected_end && protected_start < end)
    };
    let header_valid = header.is_none_or(|segment| {
        header_last_page != 0
            && !overlaps_protected(segment.virtual_page)
            && !overlaps_protected(header_last_page)
            && header_indexes[..2] == first_indexes[..2]
            && header_indexes[2] != first_indexes[2]
            && header_last_indexes[..3] == header_indexes[..3]
            && segment.virtual_address == segment.virtual_page
            && !segment.writable
            && !segment.executable
            && header_last_page < first.virtual_page
    });
    if protected_size == 0
        || !header_valid
        || overlaps_protected(first.virtual_page)
        || overlaps_protected(first_last_page)
        || overlaps_protected(second.virtual_page)
        || overlaps_protected(second_last_page)
        || first_last_page == 0
        || second_last_page == 0
        || heap_virtual_page == 0
        || stack_virtual_page == 0
        || mmap_virtual_page == 0
        || mmap_tail_virtual_page == 0
        || overlaps_protected(heap_virtual_page)
        || overlaps_protected(stack_virtual_page)
        || overlaps_protected(mmap_virtual_page)
        || overlaps_protected(mmap_tail_virtual_page)
        || first_indexes[..3] != second_indexes[..3]
        || first_last_indexes[..3] != first_indexes[..3]
        || second_last_indexes[..3] != first_indexes[..3]
        || heap_indexes[..3] != first_indexes[..3]
        || stack_indexes[..3] != first_indexes[..3]
        || mmap_indexes[..3] != first_indexes[..3]
        || mmap_tail_indexes[..3] != first_indexes[..3]
        || first_indexes[0] != 0
        || first_indexes[1] != 0
        || second.virtual_address != second.virtual_page
        || first.writable
        || !first.executable
        || !second.writable
        || second.executable
        || relro.virtual_address != second_last_page
        || relro.memory_size == 0
        || relro.memory_size > PAGE_SIZE as u64
        || first_last_page >= second.virtual_page
        || plan.entry < first.virtual_address
        || plan.entry
            >= first
                .virtual_address
                .saturating_add(first.memory_size as u64)
        || !cpu::verify_linux_copy_bounds(second.virtual_page, second.memory_size)
    {
        return false;
    }
    let Some(frames) = memory::allocate_frames::<17>(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let addresses = frames.map(memory::FrameLease::address);
    let [root, pdpt, directory, table, header_table, first_page, first_tail_page, first_third_page, first_fourth_page, header_page, header_tail_page, second_page, second_tail_page, heap_page, stack_page, mmap_page, mmap_tail_page] =
        addresses;
    let kernel_root_physical = current_root();
    let valid = (|| -> Option<bool> {
        unsafe {
            for frame in [root, pdpt, directory, table, header_table] {
                zero_frame(frame)?;
            }
            let kernel_root = mapped_frame::<u64>(kernel_root_physical)?;
            let root_entries = mapped_frame::<u64>(root)?;
            let pdpt_entries = mapped_frame::<u64>(pdpt)?;
            let directory_entries = mapped_frame::<u64>(directory)?;
            let table_entries = mapped_frame::<u64>(table)?;
            let header_table_entries = mapped_frame::<u64>(header_table)?;
            root_entries.add(256).write(kernel_root.add(256).read());
            root_entries.write(pdpt | PRESENT | WRITABLE | USER);

            let kernel_pdpt_physical = kernel_root.read() & ADDRESS_MASK;
            let kernel_pdpt = mapped_frame::<u64>(kernel_pdpt_physical)?;
            core::ptr::copy_nonoverlapping(kernel_pdpt, pdpt_entries, ENTRY_COUNT);
            let kernel_directory_physical = kernel_pdpt.read() & ADDRESS_MASK;
            let kernel_directory = mapped_frame::<u64>(kernel_directory_physical)?;
            core::ptr::copy_nonoverlapping(kernel_directory, directory_entries, ENTRY_COUNT);
            pdpt_entries.write(directory | PRESENT | WRITABLE | USER);

            let original = kernel_directory.add(first_indexes[2]).read();
            if original & PRESENT == 0
                || original & LARGE_PAGE != 0 && original & PAGE_ATTRIBUTE_TABLE != 0
            {
                return None;
            }
            if original & LARGE_PAGE != 0 {
                let base = original & ADDRESS_MASK;
                let flags = original & !ADDRESS_MASK & !LARGE_PAGE;
                for index in 0..ENTRY_COUNT {
                    table_entries
                        .add(index)
                        .write((base + (index * PAGE_SIZE) as u64) | flags);
                }
            } else {
                let original_table = mapped_frame::<u64>(original & ADDRESS_MASK)?;
                core::ptr::copy_nonoverlapping(original_table, table_entries, ENTRY_COUNT);
            }
            directory_entries
                .add(first_indexes[2])
                .write(table | PRESENT | WRITABLE | USER);

            if header.is_some() {
                let original = kernel_directory.add(header_indexes[2]).read();
                if original & PRESENT == 0
                    || original & LARGE_PAGE != 0 && original & PAGE_ATTRIBUTE_TABLE != 0
                {
                    return None;
                }
                if original & LARGE_PAGE != 0 {
                    let base = original & ADDRESS_MASK;
                    let flags = original & !ADDRESS_MASK & !LARGE_PAGE;
                    for index in 0..ENTRY_COUNT {
                        header_table_entries
                            .add(index)
                            .write((base + (index * PAGE_SIZE) as u64) | flags);
                    }
                } else {
                    let original_table = mapped_frame::<u64>(original & ADDRESS_MASK)?;
                    core::ptr::copy_nonoverlapping(
                        original_table,
                        header_table_entries,
                        ENTRY_COUNT,
                    );
                }
                directory_entries
                    .add(header_indexes[2])
                    .write(header_table | PRESENT | WRITABLE | USER);
            }

            let first_bytes = &mut *(mapped_frame::<u8>(first_page)? as *mut [u8; PAGE_SIZE]);
            let first_tail_bytes =
                &mut *(mapped_frame::<u8>(first_tail_page)? as *mut [u8; PAGE_SIZE]);
            let first_third_bytes =
                &mut *(mapped_frame::<u8>(first_third_page)? as *mut [u8; PAGE_SIZE]);
            let first_fourth_bytes =
                &mut *(mapped_frame::<u8>(first_fourth_page)? as *mut [u8; PAGE_SIZE]);
            let second_bytes = &mut *(mapped_frame::<u8>(second_page)? as *mut [u8; PAGE_SIZE]);
            let second_tail_bytes =
                &mut *(mapped_frame::<u8>(second_tail_page)? as *mut [u8; PAGE_SIZE]);
            let header_bytes = &mut *(mapped_frame::<u8>(header_page)? as *mut [u8; PAGE_SIZE]);
            let header_tail_bytes =
                &mut *(mapped_frame::<u8>(header_tail_page)? as *mut [u8; PAGE_SIZE]);
            if !crate::linux_elf::materialize(first, first_bytes)
                || !crate::linux_elf::materialize(second, second_bytes)
                || first.page_count > 1
                    && !crate::linux_elf::materialize_page(first, 1, first_tail_bytes)
                || first.page_count == 3
                    && !crate::linux_elf::materialize_page(first, 2, first_third_bytes)
                || first.page_count == 4
                    && (!crate::linux_elf::materialize_page(first, 2, first_third_bytes)
                        || !crate::linux_elf::materialize_page(first, 3, first_fourth_bytes))
                || second.page_count == 2
                    && !crate::linux_elf::materialize_page(second, 1, second_tail_bytes)
                || header.is_some_and(|segment| {
                    !crate::linux_elf::materialize(segment, header_bytes)
                        || segment.page_count == 2
                            && !crate::linux_elf::materialize_page(segment, 1, header_tail_bytes)
                })
                || zero_frame(heap_page).is_none()
                || zero_frame(mmap_page).is_none()
                || zero_frame(mmap_tail_page).is_none()
            {
                return None;
            }
            let heap_bytes = &mut *(mapped_frame::<u8>(heap_page)? as *mut [u8; PAGE_SIZE]);
            let flags = |segment: crate::linux_elf::PlannedSegment<'_>| {
                PRESENT
                    | USER
                    | if segment.writable { WRITABLE } else { 0 }
                    | if segment.executable { 0 } else { NO_EXECUTE }
            };
            table_entries
                .add(first_indexes[3])
                .write(first_page | flags(first));
            if first.page_count > 1 {
                table_entries
                    .add(first_indexes[3] + 1)
                    .write(first_tail_page | flags(first));
            }
            if first.page_count == 3 {
                table_entries
                    .add(first_indexes[3] + 2)
                    .write(first_third_page | flags(first));
            }
            if first.page_count == 4 {
                table_entries
                    .add(first_indexes[3] + 2)
                    .write(first_third_page | flags(first));
                table_entries
                    .add(first_indexes[3] + 3)
                    .write(first_fourth_page | flags(first));
            }
            if let Some(segment) = header {
                header_table_entries
                    .add(header_indexes[3])
                    .write(header_page | flags(segment));
                if segment.page_count == 2 {
                    header_table_entries
                        .add(header_indexes[3] + 1)
                        .write(header_tail_page | flags(segment));
                }
            }
            table_entries
                .add(second_indexes[3])
                .write(second_page | flags(second));
            if second.page_count == 2 {
                table_entries
                    .add(second_indexes[3] + 1)
                    .write(second_tail_page | flags(second));
            }
            table_entries
                .add(heap_indexes[3])
                .write(heap_page | PRESENT | WRITABLE | USER | NO_EXECUTE);
            table_entries
                .add(stack_indexes[3])
                .write(stack_page | PRESENT | WRITABLE | USER | NO_EXECUTE);
            table_entries.add(mmap_indexes[3]).write(0);
            table_entries.add(mmap_tail_indexes[3]).write(0);

            let stack_bytes = mapped_frame::<u8>(stack_page)?;
            let stack_offset = PAGE_SIZE - 48 * size_of::<u64>();
            let platform_offset = PAGE_SIZE - 6 * size_of::<u64>();
            let environment_offset = PAGE_SIZE - 4 * size_of::<u64>();
            let argument_offset = PAGE_SIZE - 2 * size_of::<u64>();
            let platform_address = stack_virtual_page + platform_offset as u64;
            let environment_address = stack_virtual_page + environment_offset as u64;
            let argument_address = stack_virtual_page + argument_offset as u64;
            let stack_words = stack_bytes.add(stack_offset).cast::<u64>();
            for (index, value) in [
                1_u64,
                argument_address,
                0,
                environment_address,
                0,
                3,
                plan.program_headers,
                4,
                crate::linux_elf::PROGRAM_HEADER_SIZE as u64,
                5,
                u64::from(plan.program_header_count),
                6,
                PAGE_SIZE as u64,
                7,
                0,
                8,
                0,
                11,
                0,
                12,
                0,
                13,
                0,
                14,
                0,
                23,
                0,
                31,
                argument_address,
                9,
                plan.entry,
                17,
                100,
                15,
                platform_address,
                16,
                0,
                26,
                0,
                0,
                0,
            ]
            .into_iter()
            .enumerate()
            {
                stack_words.add(index).write(value);
            }
            core::ptr::copy_nonoverlapping(
                LINUX_PLATFORM.as_ptr(),
                stack_bytes.add(platform_offset),
                LINUX_PLATFORM.len(),
            );
            core::ptr::copy_nonoverlapping(
                LINUX_INIT_ENVIRONMENT.as_ptr(),
                stack_bytes.add(environment_offset),
                LINUX_INIT_ENVIRONMENT.len(),
            );
            core::ptr::copy_nonoverlapping(
                LINUX_INIT_ARGUMENT.as_ptr(),
                stack_bytes.add(argument_offset),
                LINUX_INIT_ARGUMENT.len(),
            );

            let first_entry = table_entries.add(first_indexes[3]).read();
            let first_tail_entry = if first.page_count > 1 {
                table_entries.add(first_indexes[3] + 1).read()
            } else {
                0
            };
            let first_third_entry = if first.page_count >= 3 {
                table_entries.add(first_indexes[3] + 2).read()
            } else {
                0
            };
            let first_fourth_entry = if first.page_count == 4 {
                table_entries.add(first_indexes[3] + 3).read()
            } else {
                0
            };
            let header_entry = header_table_entries.add(header_indexes[3]).read();
            let header_tail_entry = if header.is_some_and(|segment| segment.page_count == 2) {
                header_table_entries.add(header_indexes[3] + 1).read()
            } else {
                0
            };
            let second_entry = table_entries.add(second_indexes[3]).read();
            let second_tail_entry = if second.page_count == 2 {
                table_entries.add(second_indexes[3] + 1).read()
            } else {
                0
            };
            let heap_entry = table_entries.add(heap_indexes[3]).read();
            let stack_entry = table_entries.add(stack_indexes[3]).read();
            let lower_guard = table_entries.add(stack_indexes[3] - 1).read();
            let upper_guard = table_entries.add(stack_indexes[3] + 1).read();
            let mmap_entry = table_entries.add(mmap_indexes[3]).read();
            let header_entries_valid = header.is_none_or(|segment| {
                header_entry & (PRESENT | USER | NO_EXECUTE) == PRESENT | USER | NO_EXECUTE
                    && header_entry & WRITABLE == 0
                    && (segment.page_count == 1
                        || header_tail_entry & (PRESENT | USER | NO_EXECUTE)
                            == PRESENT | USER | NO_EXECUTE
                            && header_tail_entry & WRITABLE == 0)
                    && crate::linux_elf::materialized_page_matches(segment, 0, header_bytes)
                    && (segment.page_count == 1
                        || crate::linux_elf::materialized_page_matches(
                            segment,
                            1,
                            header_tail_bytes,
                        ))
            });
            let entries_valid = root_entries.add(256).read() == kernel_root.add(256).read()
                && root_entries.add(256).read() & USER == 0
                && first_entry & (PRESENT | USER) == PRESENT | USER
                && first_entry & (WRITABLE | NO_EXECUTE) == 0
                && (first.page_count == 1
                    || first_tail_entry & (PRESENT | USER) == PRESENT | USER
                        && first_tail_entry & (WRITABLE | NO_EXECUTE) == 0)
                && (first.page_count < 3
                    || first_third_entry & (PRESENT | USER) == PRESENT | USER
                        && first_third_entry & (WRITABLE | NO_EXECUTE) == 0)
                && (first.page_count < 4
                    || first_fourth_entry & (PRESENT | USER) == PRESENT | USER
                        && first_fourth_entry & (WRITABLE | NO_EXECUTE) == 0)
                && second_entry & (PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == PRESENT | WRITABLE | USER | NO_EXECUTE
                && (second.page_count == 1
                    || second_tail_entry & (PRESENT | WRITABLE | USER | NO_EXECUTE)
                        == PRESENT | WRITABLE | USER | NO_EXECUTE)
                && heap_entry & (PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == PRESENT | WRITABLE | USER | NO_EXECUTE
                && stack_entry & (PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == PRESENT | WRITABLE | USER | NO_EXECUTE
                && lower_guard & USER == 0
                && upper_guard & USER == 0
                && mmap_entry == 0
                && table_entries.add(mmap_tail_indexes[3]).read() == 0
                && header_entries_valid
                && crate::linux_elf::materialized_page_matches(first, 0, first_bytes)
                && (first.page_count == 1
                    || crate::linux_elf::materialized_page_matches(first, 1, first_tail_bytes))
                && (first.page_count < 3
                    || crate::linux_elf::materialized_page_matches(first, 2, first_third_bytes))
                && (first.page_count < 4
                    || crate::linux_elf::materialized_page_matches(first, 3, first_fourth_bytes))
                && crate::linux_elf::materialized_page_matches(second, 0, second_bytes)
                && (second.page_count == 1
                    || crate::linux_elf::materialized_page_matches(second, 1, second_tail_bytes))
                && heap_bytes.iter().all(|byte| *byte == 0)
                && stack_words.read() == 1
                && stack_words.add(1).read() == argument_address
                && stack_words.add(3).read() == environment_address
                && stack_words.add(5).read() == 3
                && stack_words.add(6).read() == plan.program_headers
                && stack_words.add(7).read() == 4
                && stack_words.add(8).read() == crate::linux_elf::PROGRAM_HEADER_SIZE as u64
                && stack_words.add(9).read() == 5
                && stack_words.add(10).read() == u64::from(plan.program_header_count)
                && stack_words.add(11).read() == 6
                && stack_words.add(12).read() == PAGE_SIZE as u64
                && stack_words.add(13).read() == 7
                && stack_words.add(14).read() == 0
                && stack_words.add(15).read() == 8
                && stack_words.add(16).read() == 0
                && stack_words.add(17).read() == 11
                && stack_words.add(18).read() == 0
                && stack_words.add(19).read() == 12
                && stack_words.add(20).read() == 0
                && stack_words.add(21).read() == 13
                && stack_words.add(22).read() == 0
                && stack_words.add(23).read() == 14
                && stack_words.add(24).read() == 0
                && stack_words.add(25).read() == 23
                && stack_words.add(26).read() == 0
                && stack_words.add(27).read() == 31
                && stack_words.add(28).read() == argument_address
                && stack_words.add(29).read() == 9
                && stack_words.add(30).read() == plan.entry
                && stack_words.add(31).read() == 17
                && stack_words.add(32).read() == 100
                && stack_words.add(33).read() == 15
                && stack_words.add(34).read() == platform_address
                && stack_words.add(35).read() == 16
                && stack_words.add(36).read() == 0
                && stack_words.add(37).read() == 26
                && stack_words.add(38).read() == 0
                && stack_words.add(39).read() == 0
                && stack_words.add(40).read() == 0
                && core::slice::from_raw_parts(
                    stack_bytes.add(platform_offset),
                    LINUX_PLATFORM.len(),
                ) == LINUX_PLATFORM
                && core::slice::from_raw_parts(
                    stack_bytes.add(environment_offset),
                    LINUX_INIT_ENVIRONMENT.len(),
                ) == LINUX_INIT_ENVIRONMENT
                && core::slice::from_raw_parts(
                    stack_bytes.add(argument_offset),
                    LINUX_INIT_ARGUMENT.len(),
                ) == LINUX_INIT_ARGUMENT;
            if !entries_valid {
                return Some(false);
            }

            activate(root);
            let activated = current_root() == root;
            let observed = cpu::with_supervisor_user_access(|| {
                (first.virtual_address as *const u8).read_volatile() == first.bytes[0]
                    && (second.virtual_address as *const u64).read_volatile()
                        == u64::from_le_bytes(*b"PROBE!!\n")
                    && header.is_none_or(|segment| {
                        (segment.virtual_address as *const u8).read_volatile() == segment.bytes[0]
                    })
            });
            let executed = activated
                && observed
                && cpu::verify_linux_elf_entry(cpu::LinuxElfExecution {
                    entry: plan.entry as usize,
                    stack: (stack_virtual_page + stack_offset as u64) as usize,
                    code: first.virtual_address as usize,
                    code_size: first.memory_size,
                    data: second.virtual_page as usize,
                    data_size: second.memory_size,
                    relro: relro.virtual_address as usize,
                    relro_size: relro.memory_size as usize,
                    heap: heap_virtual_page as usize,
                    heap_size: PAGE_SIZE,
                    mmap: mmap_virtual_page as usize,
                    mmap_frames: [mmap_page, mmap_tail_page],
                });
            let mapping_removed = table_entries.add(mmap_indexes[3]).read() == 0
                && table_entries.add(mmap_tail_indexes[3]).read() == 0;
            activate(kernel_root_physical);
            let restored = current_root() == kernel_root_physical;
            Some(executed && mapping_removed && restored)
        }
    })()
    .unwrap_or(false);
    let cleared = frames
        .iter()
        .all(|frame| unsafe { zero_frame(frame.address()).is_some() });
    valid && cleared && memory::release_frames(memory::FrameOwner::KERNEL, frames)
}

struct RegisteredAuxiliaryAddressSpace {
    domain: u64,
    space: WxAddressSpace,
}

pub(crate) fn expected_service_fault_instruction(control_entry_offset: usize) -> Option<u64> {
    USER_TEST_ADDRESS
        .checked_add(control_entry_offset)?
        .checked_add(EXPECTED_FAULT_OFFSET)
        .and_then(|address| u64::try_from(address).ok())
}

pub(crate) fn copy_from_current_user(address: u64, output: &mut [u8]) -> bool {
    if output.is_empty() {
        return false;
    }
    let Some(end) = address.checked_add(output.len() as u64) else {
        return false;
    };
    if address >= 1 << 47 || end > 1 << 47 {
        return false;
    }
    let root = current_root();
    if !current_user_root_is_owned(root) {
        return false;
    }
    if !user_range_has_permissions(root, address, end, 0) {
        return false;
    }
    unsafe {
        cpu::with_supervisor_user_access(|| {
            for (offset, byte) in output.iter_mut().enumerate() {
                *byte = (address as *const u8).add(offset).read_volatile();
            }
        });
    }
    true
}

pub(crate) fn copy_to_current_user(address: u64, input: &[u8]) -> bool {
    if input.is_empty() {
        return false;
    }
    let Some(end) = address.checked_add(input.len() as u64) else {
        return false;
    };
    if address >= 1 << 47 || end > 1 << 47 {
        return false;
    }
    let root = current_root();
    if !current_user_root_is_owned(root) {
        return false;
    }
    if !user_range_has_permissions(root, address, end, WRITABLE) {
        return false;
    }
    unsafe {
        cpu::with_supervisor_user_access(|| {
            for (offset, byte) in input.iter().enumerate() {
                (address as *mut u8).add(offset).write_volatile(*byte);
            }
        });
    }
    true
}

pub(crate) fn protect_current_user_page_read_only(address: u64) -> bool {
    if address >= 1 << 47 || !address.is_multiple_of(PAGE_SIZE as u64) {
        return false;
    }
    let indexes = [
        ((address >> 39) & 0x1ff) as usize,
        ((address >> 30) & 0x1ff) as usize,
        ((address >> 21) & 0x1ff) as usize,
        ((address >> 12) & 0x1ff) as usize,
    ];
    let mut table = current_root();
    for (level, index) in indexes.into_iter().enumerate() {
        let Some(entries) = mapped_frame::<u64>(table & ADDRESS_MASK) else {
            return false;
        };
        let entry = unsafe { entries.add(index).read() };
        if entry & (PRESENT | WRITABLE | USER) != PRESENT | WRITABLE | USER
            || (level == 1 || level == 2) && entry & LARGE_PAGE != 0
        {
            return false;
        }
        if level == 3 {
            if entry & NO_EXECUTE == 0 {
                return false;
            }
            unsafe {
                entries.add(index).write(entry & !WRITABLE);
                asm!("invlpg [{}]", in(reg) address, options(nostack, preserves_flags));
            }
            return unsafe { entries.add(index).read() & WRITABLE == 0 };
        }
        table = entry & ADDRESS_MASK;
    }
    false
}

pub(crate) fn map_current_linux_page(address: u64, frame: u64) -> bool {
    if address >= 1 << 47
        || !address.is_multiple_of(PAGE_SIZE as u64)
        || !frame.is_multiple_of(PAGE_SIZE as u64)
        || !memory::frame_addresses_owned_by(memory::FrameOwner::KERNEL, &[frame])
    {
        return false;
    }
    let Some(entry) = current_linux_leaf(address) else {
        return false;
    };
    unsafe {
        if entry.read() != 0 {
            return false;
        }
        entry.write(frame | PRESENT | WRITABLE | USER | NO_EXECUTE);
        asm!("invlpg [{}]", in(reg) address, options(nostack, preserves_flags));
        entry.read() & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE)
            == (frame | PRESENT | WRITABLE | USER | NO_EXECUTE)
    }
}

pub(crate) fn unmap_current_linux_page(address: u64, frame: u64) -> bool {
    if address >= 1 << 47
        || !address.is_multiple_of(PAGE_SIZE as u64)
        || !frame.is_multiple_of(PAGE_SIZE as u64)
        || !memory::frame_addresses_owned_by(memory::FrameOwner::KERNEL, &[frame])
    {
        return false;
    }
    let Some(entry) = current_linux_leaf(address) else {
        return false;
    };
    unsafe {
        let value = entry.read();
        if value & ADDRESS_MASK != frame
            || value & (PRESENT | USER | NO_EXECUTE) != PRESENT | USER | NO_EXECUTE
        {
            return false;
        }
        entry.write(0);
        asm!("invlpg [{}]", in(reg) address, options(nostack, preserves_flags));
        entry.read() == 0 && zero_frame(frame).is_some()
    }
}

pub(crate) fn zero_current_linux_page(address: u64, frame: u64) -> bool {
    if address >= 1 << 47
        || !address.is_multiple_of(PAGE_SIZE as u64)
        || !frame.is_multiple_of(PAGE_SIZE as u64)
        || !memory::frame_addresses_owned_by(memory::FrameOwner::KERNEL, &[frame])
    {
        return false;
    }
    let Some(entry) = current_linux_leaf(address) else {
        return false;
    };
    let value = unsafe { entry.read() };
    if value & ADDRESS_MASK != frame
        || value & (PRESENT | USER | NO_EXECUTE) != PRESENT | USER | NO_EXECUTE
    {
        return false;
    }
    unsafe { zero_frame(frame).is_some() }
}

fn current_linux_leaf(address: u64) -> Option<*mut u64> {
    let indexes = [
        ((address >> 39) & 0x1ff) as usize,
        ((address >> 30) & 0x1ff) as usize,
        ((address >> 21) & 0x1ff) as usize,
        ((address >> 12) & 0x1ff) as usize,
    ];
    let mut table = current_root();
    for (level, index) in indexes.into_iter().enumerate() {
        if !memory::frame_addresses_owned_by(memory::FrameOwner::KERNEL, &[table & ADDRESS_MASK]) {
            return None;
        }
        let entries = mapped_frame::<u64>(table & ADDRESS_MASK)?;
        if level == 3 {
            return Some(unsafe { entries.add(index) });
        }
        let entry = unsafe { entries.add(index).read() };
        if entry & (PRESENT | USER) != PRESENT | USER
            || (level == 1 || level == 2) && entry & LARGE_PAGE != 0
        {
            return None;
        }
        table = entry & ADDRESS_MASK;
    }
    None
}

fn current_user_root_is_owned(root: u64) -> bool {
    if scheduler::current_address_space_root() != Some(root) {
        return false;
    }
    let Some(owner) = scheduler::current_domain().and_then(memory::FrameOwner::domain) else {
        return false;
    };
    let Some(frames) = paging::validated_user_root_frames(root) else {
        return false;
    };
    memory::frame_addresses_owned_by(owner, &frames)
}

fn user_range_has_permissions(root: u64, start: u64, end: u64, required: u64) -> bool {
    if start >= end {
        return false;
    }
    let page_mask = !(PAGE_SIZE as u64 - 1);
    let mut page = start & page_mask;
    let last = (end - 1) & page_mask;
    loop {
        if !user_address_has_permissions(root, page, required) {
            return false;
        }
        if page == last {
            return true;
        }
        let Some(next) = page.checked_add(PAGE_SIZE as u64) else {
            return false;
        };
        page = next;
    }
}

fn user_address_has_permissions(root: u64, address: u64, required: u64) -> bool {
    let indexes = [
        ((address >> 39) & 0x1ff) as usize,
        ((address >> 30) & 0x1ff) as usize,
        ((address >> 21) & 0x1ff) as usize,
        ((address >> 12) & 0x1ff) as usize,
    ];
    let mut table = root;
    for (level, index) in indexes.into_iter().enumerate() {
        let Some(entries) = mapped_frame::<u64>(table & ADDRESS_MASK) else {
            return false;
        };
        let entry = unsafe { entries.add(index).read() };
        if entry & (PRESENT | USER | required) != PRESENT | USER | required
            || (level == 1 || level == 2) && entry & LARGE_PAGE != 0
        {
            return false;
        }
        if level == 3 {
            return true;
        }
        table = entry & ADDRESS_MASK;
    }
    false
}

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
                zero_frame(frame)?;
            }
            let kernel_pml4 = mapped_frame::<u64>(kernel_root)?;
            let root_entries = mapped_frame::<u64>(root)?;
            root_entries.write(kernel_pml4.read());
            root_entries
                .add(1)
                .write(user_pdpt | PRESENT | WRITABLE | USER);
            root_entries.add(256).write(kernel_pml4.add(256).read());
            mapped_frame::<u64>(user_pdpt)?.write(user_directory | PRESENT | WRITABLE | USER);
            mapped_frame::<u64>(user_directory)?.write(user_table | PRESENT | WRITABLE | USER);
            mapped_frame::<u64>(user_table)?.write(user_frame | PRESENT | WRITABLE | USER);
        }
        Some(Self {
            root,
            user_frame,
            frames,
        })
    }

    fn layout_is_isolated(&self, kernel_root: u64) -> bool {
        // SAFETY: Both roots and the page-table frames reachable from this
        // address space remain allocated and reachable through the direct map.
        unsafe {
            let Some(root) = mapped_frame::<u64>(self.root) else {
                return false;
            };
            let Some(kernel) = mapped_frame::<u64>(kernel_root) else {
                return false;
            };
            let kernel_entry = kernel.read();
            let direct_entry = kernel.add(256).read();
            if root.read() != kernel_entry
                || kernel_entry & USER != 0
                || root.add(256).read() != direct_entry
                || direct_entry & USER != 0
            {
                return false;
            }
            let user_root_entry = root.add(1).read();
            if user_root_entry & (PRESENT | WRITABLE | USER) != (PRESENT | WRITABLE | USER) {
                return false;
            }
            for index in 2..ENTRY_COUNT {
                if index != 256 && root.add(index).read() != 0 {
                    return false;
                }
            }
            let Some(pdpt) = mapped_frame::<u64>(user_root_entry & ADDRESS_MASK) else {
                return false;
            };
            let Some(directory) = mapped_frame::<u64>(pdpt.read() & ADDRESS_MASK) else {
                return false;
            };
            let Some(table) = mapped_frame::<u64>(directory.read() & ADDRESS_MASK) else {
                return false;
            };
            table.read() & ADDRESS_MASK == self.user_frame
                && table.read() & (PRESENT | WRITABLE | USER) == (PRESENT | WRITABLE | USER)
                && table.add(1).read() == 0
        }
    }

    fn release(self) -> bool {
        let frames = self.frames;
        unsafe {
            for frame in &frames {
                if zero_frame(frame.address()).is_none() {
                    return false;
                }
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
    let Some(user_frame) = mapped_frame::<u64>(space.user_frame) else {
        let _ = space.release();
        return false;
    };

    // SAFETY: The new root preserves the active supervisor-only kernel entry,
    // and maps the test virtual page to its exclusively owned frame. Interrupts
    // remain disabled. The original root is restored before returning.
    let valid = unsafe {
        user_frame.write_volatile(0);
        activate(space.root);
        let activated = current_root() == space.root;
        cpu::with_supervisor_user_access(|| {
            (USER_TEST_ADDRESS as *mut u64).write_volatile(TEST_PATTERN);
        });
        let aliased = user_frame.read_volatile() == TEST_PATTERN;
        activate(kernel_root);
        let restored = current_root() == kernel_root;
        user_frame.write_volatile(0);
        activated && aliased && restored
    };
    valid && space.release()
}

pub(crate) fn verify_control_plane_wx_layout() -> bool {
    if !cpu::execute_disable_enabled() {
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
        let first_stack_address = USER_TEST_ADDRESS + 2 * PAGE_SIZE;
        let second_stack_address = USER_TEST_ADDRESS + 4 * PAGE_SIZE;
        let code_visible = cpu::with_supervisor_user_access(|| {
            let visible = (USER_TEST_ADDRESS as *const u8).read_volatile() == 0x90;
            (first_stack_address as *mut u64).write_volatile(TEST_PATTERN);
            (second_stack_address as *mut u64).write_volatile(!TEST_PATTERN);
            visible
        });
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

pub(crate) fn verify_auxiliary_address_space_lifecycle() -> bool {
    let Some(first_auxiliary) =
        domain::initialize_boot_auxiliary_service(1, 2, crate::capability::Rights::WRITE)
    else {
        return false;
    };
    let Some(second_auxiliary) =
        domain::initialize_boot_auxiliary_service(1, 2, crate::capability::Rights::WRITE)
    else {
        let _ = domain::discard_unstarted_auxiliary_service(first_auxiliary);
        return false;
    };
    let (Some(first_handle), Some(first_owner), Some(second_handle), Some(second_owner)) = (
        domain::auxiliary_boot_handle(first_auxiliary, 0),
        memory::FrameOwner::domain(first_auxiliary),
        domain::auxiliary_boot_handle(second_auxiliary, 0),
        memory::FrameOwner::domain(second_auxiliary),
    ) else {
        let _ = domain::discard_unstarted_auxiliary_service(first_auxiliary);
        let _ = domain::discard_unstarted_auxiliary_service(second_auxiliary);
        return false;
    };
    let kernel_root = current_root();
    let Some(first_space) = WxAddressSpace::new(kernel_root, first_owner) else {
        let _ = domain::discard_unstarted_auxiliary_service(first_auxiliary);
        let _ = domain::discard_unstarted_auxiliary_service(second_auxiliary);
        return false;
    };
    let Some(second_space) = WxAddressSpace::new(kernel_root, second_owner) else {
        let _ = first_space.release();
        let _ = domain::discard_unstarted_auxiliary_service(first_auxiliary);
        let _ = domain::discard_unstarted_auxiliary_service(second_auxiliary);
        return false;
    };
    let valid = first_space.permissions_are_valid(kernel_root)
        && second_space.permissions_are_valid(kernel_root)
        && first_space.is_disjoint_from(&second_space)
        && memory::owner_live_frame_count(first_owner) == Some(USER_ADDRESS_SPACE_FRAMES)
        && memory::owner_live_frame_count(second_owner) == Some(USER_ADDRESS_SPACE_FRAMES)
        && domain::handle_permits(
            first_auxiliary,
            first_handle,
            2,
            crate::capability::Rights::WRITE,
        )
        && domain::handle_permits(
            second_auxiliary,
            second_handle,
            2,
            crate::capability::Rights::WRITE,
        )
        && !domain::discard_unstarted_auxiliary_service(first_auxiliary)
        && !domain::discard_unstarted_auxiliary_service(second_auxiliary)
        && register_auxiliary_address_space(first_auxiliary, first_space)
        && register_auxiliary_address_space(second_auxiliary, second_space)
        && registered_auxiliary_address_space_count() == 2
        && domain::attach_thread(first_auxiliary)
        && domain::attach_thread(second_auxiliary)
        && !domain::attach_thread(first_auxiliary)
        && !domain::attach_thread(second_auxiliary)
        && !release_auxiliary_address_space(first_auxiliary)
        && !release_auxiliary_address_space(second_auxiliary)
        && registered_auxiliary_address_space_count() == 2
        && domain::detach_thread(first_auxiliary)
        && domain::detach_thread(second_auxiliary)
        && release_auxiliary_address_space(first_auxiliary)
        && registered_auxiliary_address_space_count() == 1
        && memory::owner_live_frame_count(second_owner) == Some(USER_ADDRESS_SPACE_FRAMES)
        && release_auxiliary_address_space(second_auxiliary)
        && registered_auxiliary_address_space_count() == 0;
    if memory::owner_live_frame_count(first_owner) != Some(0)
        || memory::owner_live_frame_count(second_owner) != Some(0)
        || !domain::discard_unstarted_auxiliary_service(first_auxiliary)
        || !domain::discard_unstarted_auxiliary_service(second_auxiliary)
    {
        return false;
    }
    valid
        && domain::domain_is_retired(first_auxiliary)
        && domain::domain_is_retired(second_auxiliary)
        && !domain::handle_permits(
            first_auxiliary,
            first_handle,
            2,
            crate::capability::Rights::WRITE,
        )
        && !domain::handle_permits(
            second_auxiliary,
            second_handle,
            2,
            crate::capability::Rights::WRITE,
        )
}

fn registered_auxiliary_address_space_count() -> usize {
    let registry = core::ptr::addr_of!(AUXILIARY_ADDRESS_SPACES);
    unsafe { (*registry).iter().flatten().count() }
}

pub(crate) fn load_and_queue_auxiliary_from_current_user(
    domain_id: u64,
    source: u64,
    length: u64,
) -> bool {
    let Ok(length) = usize::try_from(length) else {
        return false;
    };
    if !(1..=PAGE_SIZE).contains(&length) {
        return false;
    }
    let (Some(handle), Some(owner)) = (
        domain::auxiliary_boot_handle(domain_id, 0),
        memory::FrameOwner::domain(domain_id),
    ) else {
        return false;
    };
    let kernel_root = current_root();
    let Some(space) = WxAddressSpace::new(kernel_root, owner) else {
        return false;
    };
    // SAFETY: The target code frame is exclusively leased to this domain and
    // remains live for the duration of the bounded copy.
    let code = unsafe { core::slice::from_raw_parts_mut(space.code_frame, length) };
    if !space.permissions_are_valid(kernel_root) || !copy_from_current_user(source, code) {
        let _ = space.release();
        return false;
    }
    let root = space.root;
    if !register_auxiliary_address_space(domain_id, space) {
        return false;
    }
    if !scheduler::queue_auxiliary(
        domain_id,
        root,
        USER_TEST_ADDRESS,
        USER_TEST_ADDRESS + 3 * PAGE_SIZE,
        handle,
    ) {
        let _ = release_auxiliary_address_space(domain_id);
        return false;
    }
    true
}

pub(crate) fn stop_auxiliary_from_current_user(
    supervisor: u64,
    capability: u64,
    domain_id: u64,
) -> bool {
    if domain::stop_auxiliary_service(supervisor, capability, domain_id) {
        return true;
    }
    scheduler::cancel_ready_auxiliary(domain_id)
        && release_auxiliary_address_space(domain_id)
        && domain::stop_auxiliary_service(supervisor, capability, domain_id)
}

pub(crate) fn initialize_staged_auxiliary_from_current_user(
    domain_id: u64,
    source: u64,
    length: u64,
) -> bool {
    let Ok(length) = usize::try_from(length) else {
        return false;
    };
    if !(1..=PAGE_SIZE).contains(&length) || !scheduler::auxiliary_is_staged(domain_id) {
        return false;
    }
    let registry = core::ptr::addr_of_mut!(AUXILIARY_ADDRESS_SPACES);
    let Some(registered) = (unsafe { (*registry).iter_mut().flatten() })
        .find(|registered| registered.domain == domain_id)
    else {
        return false;
    };
    let offset = PAGE_SIZE - length;
    // SAFETY: The private stack frame remains owned by this exact staged
    // domain and the bounded range cannot leave that single NX page.
    let payload = unsafe {
        core::slice::from_raw_parts_mut(registered.space.first_stack_frame.add(offset), length)
    };
    if !copy_from_current_user(source, payload) {
        return false;
    }
    scheduler::initialize_staged_auxiliary(
        domain_id,
        (USER_TEST_ADDRESS + 3 * PAGE_SIZE - length) as u64,
        length as u64,
    )
}

fn register_auxiliary_address_space(domain_id: u64, space: WxAddressSpace) -> bool {
    let registry = core::ptr::addr_of_mut!(AUXILIARY_ADDRESS_SPACES);
    let Some(slot) =
        (0..domain::MAX_AUXILIARY_SERVICES).find(|index| unsafe { (*registry)[*index].is_none() })
    else {
        let _ = space.release();
        return false;
    };
    if unsafe {
        (*registry)
            .iter()
            .flatten()
            .any(|registered| registered.domain == domain_id)
    } || !domain::bind_auxiliary_address_space(domain_id, space.root)
    {
        let _ = space.release();
        return false;
    }
    unsafe {
        (*registry)[slot] = Some(RegisteredAuxiliaryAddressSpace {
            domain: domain_id,
            space,
        });
    }
    true
}

pub(crate) fn release_auxiliary_address_space(domain_id: u64) -> bool {
    let registry = core::ptr::addr_of_mut!(AUXILIARY_ADDRESS_SPACES);
    let Some(slot) = (0..domain::MAX_AUXILIARY_SERVICES).find(|index| unsafe {
        (*registry)[*index]
            .as_ref()
            .is_some_and(|registered| registered.domain == domain_id)
    }) else {
        return false;
    };
    let Some(registered) = (unsafe { (*registry)[slot].as_ref() }) else {
        return false;
    };
    let root = registered.space.root;
    let owner = registered.space.owner;
    let frames = registered.space.frames;
    if !memory::frames_owned_by(owner, &frames)
        || !domain::unbind_auxiliary_address_space(domain_id, root)
    {
        return false;
    }
    unsafe {
        for frame in &frames {
            if zero_frame(frame.address()).is_none() {
                let _ = domain::bind_auxiliary_address_space(domain_id, root);
                return false;
            }
        }
    }
    if !memory::release_frames(owner, frames) {
        let _ = domain::bind_auxiliary_address_space(domain_id, root);
        return false;
    }
    unsafe {
        (*registry)[slot] = None;
    }
    true
}

pub(crate) fn activate_kernel_root_for_auxiliary(domain_id: u64) -> bool {
    let registry = core::ptr::addr_of!(AUXILIARY_ADDRESS_SPACES);
    let Some(registered) =
        (unsafe { (*registry).iter().flatten() }).find(|registered| registered.domain == domain_id)
    else {
        return false;
    };
    if current_root() != registered.space.root {
        return false;
    }
    unsafe {
        activate(registered.space.kernel_root);
    }
    current_root() == registered.space.kernel_root
}

pub(crate) fn verify_ring3_control_plane(
    program: &[u8],
    entry_offsets: [usize; 3],
    domains: [u64; 2],
    handles: [[u64; 3]; 2],
    local_apic: &LocalApic,
    apic_timer: &LocalApicCalibration,
) -> bool {
    if !cpu::execute_disable_enabled() {
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
    let Some(expected_fault_instruction) = expected_service_fault_instruction(control_entry_offset)
    else {
        return false;
    };
    let expected_fault_offset = expected_fault_instruction as usize - USER_TEST_ADDRESS;
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
        let first_marker = cpu::with_supervisor_user_access(|| {
            ((USER_TEST_ADDRESS + 2 * PAGE_SIZE) as *const u64).read_volatile()
        });
        activate(second_space.root);
        let second_marker = cpu::with_supervisor_user_access(|| {
            ((USER_TEST_ADDRESS + 2 * PAGE_SIZE) as *const u64).read_volatile()
        });
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
        let Some(service_owner_domain) = scheduler::service_address_space_owner() else {
            return false;
        };
        let Some(replacement_owner) = memory::FrameOwner::domain(service_owner_domain) else {
            return false;
        };
        if !valid
            || !second_space.adopt_transferred_owner(replacement_owner)
            || memory::owner_live_frame_count(service_owner) != Some(0)
            || !first_space.release()
            || !second_space.release()
            || !domain::complete_service_frame_quarantine(service_owner_domain)
        {
            return false;
        }
        memory::owner_live_frame_count(control_owner) == Some(0)
            && memory::owner_live_frame_count(service_owner) == Some(0)
            && memory::owner_live_frame_count(replacement_owner) == Some(0)
    }
}

pub(crate) fn verify_ring3_auxiliary_exit() -> bool {
    let Some((executed_kind, executed_domain, executed_sequence)) =
        domain::last_lifecycle_event_audit()
    else {
        return false;
    };
    let Some(owner) = memory::FrameOwner::domain(executed_domain) else {
        return false;
    };
    executed_kind == crate::control_abi::LIFECYCLE_AUXILIARY_EXITED
        && executed_sequence == 7
        && domain::last_lifecycle_event_detail() == Some(42)
        && domain::last_lifecycle_service_identity().is_some()
        && scheduler::current_domain().is_none()
        && registered_auxiliary_address_space_count() == 0
        && memory::owner_live_frame_count(owner) == Some(0)
        && domain::domain_is_retired(executed_domain)
        && domain::take_lifecycle_event().is_none()
}

pub(crate) fn verify_ring3_network_service() -> bool {
    const NETWORK_PROBE_PROGRAM: &[u8] = &[
        0x4c, 0x89, 0xd7, 0x31, 0xf6, 0x31, 0xd2, // invalid handle, command 0, argument 0
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x83, 0xf8, 0xfd, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0x89,
        0xcf, 0x6a, 0x0c, 0x5e, // valid handle, unknown command 12
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x83, 0xf8, 0xfd, 0x74, 0x02, 0x0f, 0x0b, 0x31, 0xf6,
        0x6a, 0x01, 0x5a, // command 0 with invalid reserved argument 1
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x83, 0xf8, 0xfd, 0x74, 0x02, 0x0f, 0x0b, 0x31, 0xd2,
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, // valid command 0 probe
        0x48, 0xbb, 0x09, 0x00, 0xdc, 0x05, 0x01, 0, 0, 0, 0x48, 0x39, 0xd8, 0x74, 0x02, 0x0f,
        0x0b, 0x6a, 0x01, 0x5e, // command 1: link initially down
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x85, 0xc0, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x02, 0x5e,
        0x6a, 0x01, 0x5a, // command 2: set link up
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x85, 0xc0, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x01, 0x5e,
        0x31, 0xd2, // command 1: observe link up
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0x83, 0xf8, 0x01, 0x74, 0x02, 0x0f, 0x0b, 0x6a,
        0x03, 0x5e, // command 3: reject address 1 with invalid prefix 33
        0x48, 0xba, 0x01, 0, 0, 0, 0x21, 0, 0, 0, 0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x83, 0xf8,
        0xfd, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x03, 0x5e, // set 10.0.2.15/24
        0x48, 0xba, 0x0f, 0x02, 0x00, 0x0a, 0x18, 0, 0, 0, 0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x85,
        0xc0, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x04, 0x5e, 0x31, 0xd2, // query IPv4
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0xbb, 0x0f, 0x02, 0x00, 0x0a, 0x18, 0, 0, 0, 0x48,
        0x39, 0xd8, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x05, 0x5e, // reject 10.0.3.2
        0x48, 0xba, 0x02, 0x03, 0x00, 0x0a, 0, 0, 0, 0, 0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x83,
        0xf8, 0xfd, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x05, 0x5e, // set gateway 10.0.2.2
        0x48, 0xba, 0x02, 0x02, 0x00, 0x0a, 0, 0, 0, 0, 0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x85,
        0xc0, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x06, 0x5e, 0x31, 0xd2, // query gateway
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0xbb, 0x02, 0x02, 0x00, 0x0a, 0, 0, 0, 0, 0x48,
        0x39, 0xd8, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x02, 0x5e, // no-op: link already up
        0x6a, 0x01, 0x5a, 0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x85, 0xc0, 0x74, 0x02, 0x0f, 0x0b,
        0x6a, 0x07, 0x5e, 0x31, 0xd2, // query generation: three effective changes
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0x83, 0xf8, 0x03, 0x74, 0x02, 0x0f, 0x0b, 0x6a,
        0x03, 0x5e, // move IPv4 to 10.0.4.15/24; gateway must clear
        0x48, 0xba, 0x0f, 0x04, 0x00, 0x0a, 0x18, 0, 0, 0, 0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x85,
        0xc0, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x06, 0x5e, 0x31, 0xd2, 0xb8, 0x10, 0, 0, 0, 0xcd,
        0x80, 0x48, 0x85, 0xc0, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x07,
        0x5e, // generation is now 4
        0x31, 0xd2, 0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0x83, 0xf8, 0x04, 0x74, 0x02, 0x0f,
        0x0b, 0x48, 0x83, 0xec, 0x38, // invalid 192.168.50.1/24 route via 10.0.4.2
        0x48, 0xb8, 0x01, 0x32, 0xa8, 0xc0, 0x18, 0, 0, 0, 0x48, 0x89, 0x04, 0x24, 0x48, 0xb8,
        0x02, 0x04, 0x00, 0x0a, 0, 0, 0, 0, 0x48, 0x89, 0x44, 0x24, 0x08, 0x48, 0x8d, 0x14, 0x24,
        0x6a, 0x08, 0x5e, 0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x83, 0xf8, 0xfd, 0x74, 0x02, 0x0f,
        0x0b, 0xc6, 0x04, 0x24, 0x00, 0x6a, 0x08, 0x5e, // canonical route succeeds
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x85, 0xc0, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x09, 0x5e,
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0x83, 0xf8, 0x10, 0x74, 0x02, 0x0f, 0x0b, 0x48,
        0xbb, 0x00, 0x32, 0xa8, 0xc0, 0x18, 0, 0, 0, 0x48, 0x39, 0x1c, 0x24, 0x74, 0x02, 0x0f,
        0x0b, 0x48, 0xbb, 0x02, 0x04, 0x00, 0x0a, 0, 0, 0, 0, 0x48, 0x39, 0x5c, 0x24, 0x08, 0x74,
        0x02, 0x0f, 0x0b, 0x6a, 0x07, 0x5e, 0x31, 0xd2, // route advances generation to 5
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0x83, 0xf8, 0x05, 0x74, 0x02, 0x0f, 0x0b, 0x6a,
        0x0a, 0x5e, 0x48, 0x8d, 0x14, 0x24, // copy coherent 56-byte snapshot
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0x83, 0xf8, 0x38, 0x74, 0x02, 0x0f, 0x0b, 0x48,
        0xc7, 0xc3, 0x01, 0, 0, 0, 0x48, 0x39, 0x1c, 0x24, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0xc7,
        0xc3, 0x05, 0, 0, 0, 0x48, 0x39, 0x5c, 0x24, 0x08, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0xc7,
        0xc3, 0x01, 0, 0, 0, 0x48, 0x39, 0x5c, 0x24, 0x10, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0xbb,
        0x0f, 0x04, 0x00, 0x0a, 0x18, 0, 0, 0, 0x48, 0x39, 0x5c, 0x24, 0x18, 0x74, 0x02, 0x0f,
        0x0b, 0x48, 0x31, 0xdb, 0x48, 0x39, 0x5c, 0x24, 0x20, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0xbb,
        0x00, 0x32, 0xa8, 0xc0, 0x18, 0, 0, 0, 0x48, 0x39, 0x5c, 0x24, 0x28, 0x74, 0x02, 0x0f,
        0x0b, 0x48, 0xbb, 0x02, 0x04, 0x00, 0x0a, 0, 0, 0, 0, 0x48, 0x39, 0x5c, 0x24, 0x30, 0x74,
        0x02, 0x0f, 0x0b, 0x48, 0x83, 0xc4, 0x38, 0x6a, 0x2a, 0x5e, 0xb8, 0x02, 0, 0, 0, 0xcd,
        0x80, 0x0f, 0x0b,
    ];
    let Some(auxiliary) = domain::initialize_boot_auxiliary_service(
        3,
        crate::control_abi::NETWORK_DEVICE_OBJECT,
        crate::capability::Rights::READ.union(crate::capability::Rights::WRITE),
    ) else {
        return false;
    };
    let (Some(handle), Some(identity), Some(owner)) = (
        domain::auxiliary_boot_handle(auxiliary, 0),
        domain::domain_service_identity(auxiliary),
        memory::FrameOwner::domain(auxiliary),
    ) else {
        return false;
    };
    let kernel_root = current_root();
    let Some(space) = WxAddressSpace::new(kernel_root, owner) else {
        return false;
    };
    if !space.permissions_are_valid(kernel_root) {
        return false;
    }
    unsafe {
        for (offset, byte) in NETWORK_PROBE_PROGRAM.iter().enumerate() {
            space.code_frame.add(offset).write(*byte);
        }
    }
    let root = space.root;
    let entry = USER_TEST_ADDRESS;
    let stack = USER_TEST_ADDRESS + 3 * PAGE_SIZE;
    if !register_auxiliary_address_space(auxiliary, space)
        || !scheduler::queue_auxiliary(auxiliary, root, entry, stack, handle)
        || !cpu::activate_user_root(root)
    {
        return false;
    }
    cpu::execute_auxiliary(entry, stack, handle, auxiliary, identity);
    let event = domain::take_lifecycle_event();
    current_root() == kernel_root
        && scheduler::current_domain().is_none()
        && registered_auxiliary_address_space_count() == 0
        && memory::owner_live_frame_count(owner) == Some(0)
        && domain::domain_is_retired(auxiliary)
        && !domain::handle_permits(
            auxiliary,
            handle,
            crate::control_abi::NETWORK_DEVICE_OBJECT,
            crate::capability::Rights::READ,
        )
        && !domain::handle_permits(
            auxiliary,
            handle,
            crate::control_abi::NETWORK_DEVICE_OBJECT,
            crate::capability::Rights::WRITE,
        )
        && network::link_is_up()
        && network::ipv4_configuration() == 0x0000_0018_0a00_040f
        && network::ipv4_gateway() == 0
        && network::generation() == 5
        && network::route() == (0x0000_0018_c0a8_3200, 0x0a00_0402)
        && event
            == Some(
                (42_u64 << 56)
                    | (auxiliary << 8)
                    | u64::from(crate::control_abi::LIFECYCLE_AUXILIARY_EXITED),
            )
        && domain::last_lifecycle_event_audit()
            == Some((crate::control_abi::LIFECYCLE_AUXILIARY_EXITED, auxiliary, 8))
        && domain::last_lifecycle_event_detail() == Some(42)
        && domain::last_lifecycle_service_identity() == Some(identity)
        && domain::take_lifecycle_event().is_none()
}

pub(crate) fn verify_ring3_network_observer() -> bool {
    const NETWORK_OBSERVER_PROGRAM: &[u8] = &[
        0x48, 0x89, 0xcf, 0x48, 0x83, 0xec, 0x38, 0x6a, 0x0a, 0x5e, 0x48, 0x8d, 0x14, 0x24, 0xb8,
        0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0x83, 0xf8, 0x38, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0x83,
        0x7c, 0x24, 0x08, 0x05, 0x74, 0x02, 0x0f, 0x0b, // generation 5
        0x6a, 0x02, 0x5e, 0x31, 0xd2, // read-only handle cannot set link down
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x83, 0xf8, 0xfd, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x0a,
        0x5e, 0x48, 0x8d, 0x14, 0x24, 0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0x83, 0xf8, 0x38,
        0x74, 0x02, 0x0f, 0x0b, 0x48, 0x83, 0x7c, 0x24, 0x08, 0x05, 0x74, 0x02, 0x0f, 0x0b, 0x48,
        0x83, 0x7c, 0x24, 0x10, 0x01, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0x83, 0xc4, 0x38, 0x6a, 0x2b,
        0x5e, 0xb8, 0x02, 0, 0, 0, 0xcd, 0x80, 0x0f, 0x0b,
    ];
    let Some(auxiliary) = domain::initialize_boot_auxiliary_service(
        3,
        crate::control_abi::NETWORK_DEVICE_OBJECT,
        crate::capability::Rights::READ,
    ) else {
        return false;
    };
    let (Some(handle), Some(identity), Some(owner)) = (
        domain::auxiliary_boot_handle(auxiliary, 0),
        domain::domain_service_identity(auxiliary),
        memory::FrameOwner::domain(auxiliary),
    ) else {
        return false;
    };
    let kernel_root = current_root();
    let Some(space) = WxAddressSpace::new(kernel_root, owner) else {
        return false;
    };
    if !space.permissions_are_valid(kernel_root) {
        return false;
    }
    unsafe {
        for (offset, byte) in NETWORK_OBSERVER_PROGRAM.iter().enumerate() {
            space.code_frame.add(offset).write(*byte);
        }
    }
    let root = space.root;
    let entry = USER_TEST_ADDRESS;
    let stack = USER_TEST_ADDRESS + 3 * PAGE_SIZE;
    if !register_auxiliary_address_space(auxiliary, space)
        || !scheduler::queue_auxiliary(auxiliary, root, entry, stack, handle)
        || !cpu::activate_user_root(root)
    {
        return false;
    }
    cpu::execute_auxiliary(entry, stack, handle, auxiliary, identity);
    let event = domain::take_lifecycle_event();
    current_root() == kernel_root
        && scheduler::current_domain().is_none()
        && registered_auxiliary_address_space_count() == 0
        && memory::owner_live_frame_count(owner) == Some(0)
        && domain::domain_is_retired(auxiliary)
        && !domain::handle_permits(
            auxiliary,
            handle,
            crate::control_abi::NETWORK_DEVICE_OBJECT,
            crate::capability::Rights::READ,
        )
        && !domain::handle_permits(
            auxiliary,
            handle,
            crate::control_abi::NETWORK_DEVICE_OBJECT,
            crate::capability::Rights::WRITE,
        )
        && network::link_is_up()
        && network::generation() == 5
        && event
            == Some(
                (43_u64 << 56)
                    | (auxiliary << 8)
                    | u64::from(crate::control_abi::LIFECYCLE_AUXILIARY_EXITED),
            )
        && domain::last_lifecycle_event_audit()
            == Some((crate::control_abi::LIFECYCLE_AUXILIARY_EXITED, auxiliary, 9))
        && domain::last_lifecycle_event_detail() == Some(43)
        && domain::last_lifecycle_service_identity() == Some(identity)
        && domain::take_lifecycle_event().is_none()
}

pub(crate) fn verify_ring3_network_configurator() -> bool {
    const NETWORK_CONFIGURATOR_PROGRAM: &[u8] = &[
        0x48, 0x89, 0xcf, 0x48, 0x83, 0xec, 0x40, 0x48, 0xb8, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33,
        0x22, 0x11, 0x48, 0x89, 0x04, 0x24, 0x6a, 0x0a, 0x5e, 0x48, 0x8d, 0x14,
        0x24, // snapshot denied
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x83, 0xf8, 0xfd, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0xbb,
        0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x48, 0x39, 0x1c, 0x24, 0x74, 0x02, 0x0f,
        0x0b, 0x6a, 0x02, 0x5e, 0x31, 0xd2, // set link down
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x85, 0xc0, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x01, 0x5e,
        0x31, 0xd2, // direct link query denied
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x83, 0xf8, 0xfd, 0x74, 0x02, 0x0f, 0x0b, 0x6a, 0x02,
        0x5e, 0x6a, 0x01, 0x5a, // restore link up
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x85, 0xc0, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0xc7, 0x04,
        0x24, 0x01, 0, 0, 0, // apply v1, stale generation 6
        0x48, 0xc7, 0x44, 0x24, 0x08, 0x06, 0, 0, 0, 0x48, 0xc7, 0x44, 0x24, 0x10, 0, 0, 0, 0,
        0x48, 0xb8, 0x0f, 0x04, 0x00, 0x0a, 0x18, 0, 0, 0, 0x48, 0x89, 0x44, 0x24, 0x18, 0x31,
        0xc0, 0x48, 0x89, 0x44, 0x24, 0x20, 0x48, 0xb8, 0x00, 0x32, 0xa8, 0xc0, 0x18, 0, 0, 0,
        0x48, 0x89, 0x44, 0x24, 0x28, 0x48, 0xb8, 0x02, 0x04, 0x00, 0x0a, 0, 0, 0, 0, 0x48, 0x89,
        0x44, 0x24, 0x30, 0x31, 0xc0, 0x48, 0x89, 0x44, 0x24, 0x38, 0x48, 0x8d, 0x14, 0x24, 0x6a,
        0x0b, 0x5e, 0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x83, 0xf8, 0xfb, 0x74, 0x02, 0x0f, 0x0b,
        0x48, 0xc7, 0x44, 0x24, 0x08, 0x07, 0, 0, 0, // generation 7 applies link down
        0xb8, 0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0x83, 0xf8, 0x08, 0x74, 0x02, 0x0f, 0x0b, 0x48,
        0xc7, 0x44, 0x24, 0x08, 0x08, 0, 0, 0, 0x48, 0xc7, 0x44, 0x24, 0x10, 0x01, 0, 0, 0, 0xb8,
        0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0x83, 0xf8, 0x09, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0x83,
        0xc4, 0x40, 0x6a, 0x2c, 0x5e, 0xb8, 0x02, 0, 0, 0, 0xcd, 0x80, 0x0f, 0x0b,
    ];
    let Some(auxiliary) = domain::initialize_boot_auxiliary_service(
        3,
        crate::control_abi::NETWORK_DEVICE_OBJECT,
        crate::capability::Rights::WRITE,
    ) else {
        return false;
    };
    let (Some(handle), Some(identity), Some(owner)) = (
        domain::auxiliary_boot_handle(auxiliary, 0),
        domain::domain_service_identity(auxiliary),
        memory::FrameOwner::domain(auxiliary),
    ) else {
        return false;
    };
    let kernel_root = current_root();
    let Some(space) = WxAddressSpace::new(kernel_root, owner) else {
        return false;
    };
    if !space.permissions_are_valid(kernel_root) {
        return false;
    }
    unsafe {
        for (offset, byte) in NETWORK_CONFIGURATOR_PROGRAM.iter().enumerate() {
            space.code_frame.add(offset).write(*byte);
        }
    }
    let root = space.root;
    let entry = USER_TEST_ADDRESS;
    let stack = USER_TEST_ADDRESS + 3 * PAGE_SIZE;
    if !register_auxiliary_address_space(auxiliary, space)
        || !scheduler::queue_auxiliary(auxiliary, root, entry, stack, handle)
        || !cpu::activate_user_root(root)
    {
        return false;
    }
    cpu::execute_auxiliary(entry, stack, handle, auxiliary, identity);
    let event = domain::take_lifecycle_event();
    current_root() == kernel_root
        && scheduler::current_domain().is_none()
        && registered_auxiliary_address_space_count() == 0
        && memory::owner_live_frame_count(owner) == Some(0)
        && domain::domain_is_retired(auxiliary)
        && !domain::handle_permits(
            auxiliary,
            handle,
            crate::control_abi::NETWORK_DEVICE_OBJECT,
            crate::capability::Rights::READ,
        )
        && !domain::handle_permits(
            auxiliary,
            handle,
            crate::control_abi::NETWORK_DEVICE_OBJECT,
            crate::capability::Rights::WRITE,
        )
        && network::link_is_up()
        && network::generation() == 9
        && event
            == Some(
                (44_u64 << 56)
                    | (auxiliary << 8)
                    | u64::from(crate::control_abi::LIFECYCLE_AUXILIARY_EXITED),
            )
        && domain::last_lifecycle_event_audit()
            == Some((
                crate::control_abi::LIFECYCLE_AUXILIARY_EXITED,
                auxiliary,
                10,
            ))
        && domain::last_lifecycle_event_detail() == Some(44)
        && domain::last_lifecycle_service_identity() == Some(identity)
        && domain::take_lifecycle_event().is_none()
}

pub(crate) fn verify_ring3_network_fault_containment() -> bool {
    const INVALID_OPCODE_PROGRAM: [u8; 2] = [0x0f, 0x0b];
    let Some(auxiliary) = domain::initialize_boot_auxiliary_service(
        3,
        crate::control_abi::NETWORK_DEVICE_OBJECT,
        crate::capability::Rights::READ,
    ) else {
        return false;
    };
    let (Some(handle), Some(identity), Some(owner)) = (
        domain::auxiliary_boot_handle(auxiliary, 0),
        domain::domain_service_identity(auxiliary),
        memory::FrameOwner::domain(auxiliary),
    ) else {
        return false;
    };
    let kernel_root = current_root();
    let Some(space) = WxAddressSpace::new(kernel_root, owner) else {
        return false;
    };
    if !space.permissions_are_valid(kernel_root) {
        return false;
    }
    unsafe {
        for (offset, byte) in INVALID_OPCODE_PROGRAM.iter().enumerate() {
            space.code_frame.add(offset).write(*byte);
        }
    }
    let root = space.root;
    let entry = USER_TEST_ADDRESS;
    let stack = USER_TEST_ADDRESS + 3 * PAGE_SIZE;
    if !register_auxiliary_address_space(auxiliary, space)
        || !scheduler::queue_auxiliary(auxiliary, root, entry, stack, handle)
        || !cpu::activate_user_root(root)
    {
        return false;
    }
    cpu::execute_auxiliary(entry, stack, handle, auxiliary, identity);
    let event = domain::take_lifecycle_event();
    current_root() == kernel_root
        && scheduler::current_domain().is_none()
        && registered_auxiliary_address_space_count() == 0
        && memory::owner_live_frame_count(owner) == Some(0)
        && domain::domain_is_retired(auxiliary)
        && !domain::handle_permits(
            auxiliary,
            handle,
            crate::control_abi::NETWORK_DEVICE_OBJECT,
            crate::capability::Rights::READ,
        )
        && network::link_is_up()
        && network::ipv4_configuration() == 0x0000_0018_0a00_040f
        && network::ipv4_gateway() == 0
        && network::generation() == 9
        && network::route() == (0x0000_0018_c0a8_3200, 0x0a00_0402)
        && event
            == Some((auxiliary << 8) | u64::from(crate::control_abi::LIFECYCLE_AUXILIARY_FAULT))
        && domain::last_lifecycle_event_audit()
            == Some((crate::control_abi::LIFECYCLE_AUXILIARY_FAULT, auxiliary, 11))
        && domain::last_lifecycle_event_detail() == Some(0)
        && domain::last_lifecycle_service_identity() == Some(identity)
        && domain::delivered_fault_matches(
            auxiliary,
            domain::FaultMetadata {
                vector: 6,
                error: 0,
                address: 0,
                instruction: entry as u64,
            },
        )
        && domain::take_lifecycle_event().is_none()
}

pub(crate) fn verify_ring3_network_recovery() -> bool {
    const NETWORK_RECOVERY_PROGRAM: &[u8] = &[
        0x48, 0x89, 0xcf, 0x48, 0x83, 0xec, 0x38, 0x6a, 0x0a, 0x5e, 0x48, 0x8d, 0x14, 0x24, 0xb8,
        0x10, 0, 0, 0, 0xcd, 0x80, 0x48, 0x83, 0xf8, 0x38, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0x83,
        0x7c, 0x24, 0x08, 0x09, 0x74, 0x02, 0x0f, 0x0b, // retained generation 9
        0x48, 0xbb, 0x00, 0x32, 0xa8, 0xc0, 0x18, 0, 0, 0, 0x48, 0x39, 0x5c, 0x24, 0x28, 0x74,
        0x02, 0x0f, 0x0b, 0x48, 0xbb, 0x02, 0x04, 0x00, 0x0a, 0, 0, 0, 0, 0x48, 0x39, 0x5c, 0x24,
        0x30, 0x74, 0x02, 0x0f, 0x0b, 0x48, 0x83, 0xc4, 0x38, 0x6a, 0x2d, 0x5e, 0xb8, 0x02, 0, 0,
        0, 0xcd, 0x80, 0x0f, 0x0b,
    ];
    let Some(auxiliary) = domain::initialize_boot_auxiliary_service(
        3,
        crate::control_abi::NETWORK_DEVICE_OBJECT,
        crate::capability::Rights::READ,
    ) else {
        return false;
    };
    let (Some(handle), Some(identity), Some(owner)) = (
        domain::auxiliary_boot_handle(auxiliary, 0),
        domain::domain_service_identity(auxiliary),
        memory::FrameOwner::domain(auxiliary),
    ) else {
        return false;
    };
    let kernel_root = current_root();
    let Some(space) = WxAddressSpace::new(kernel_root, owner) else {
        return false;
    };
    if !space.permissions_are_valid(kernel_root) {
        return false;
    }
    unsafe {
        for (offset, byte) in NETWORK_RECOVERY_PROGRAM.iter().enumerate() {
            space.code_frame.add(offset).write(*byte);
        }
    }
    let root = space.root;
    let entry = USER_TEST_ADDRESS;
    let stack = USER_TEST_ADDRESS + 3 * PAGE_SIZE;
    if !register_auxiliary_address_space(auxiliary, space)
        || !scheduler::queue_auxiliary(auxiliary, root, entry, stack, handle)
        || !cpu::activate_user_root(root)
    {
        return false;
    }
    cpu::execute_auxiliary(entry, stack, handle, auxiliary, identity);
    let event = domain::take_lifecycle_event();
    current_root() == kernel_root
        && scheduler::current_domain().is_none()
        && registered_auxiliary_address_space_count() == 0
        && memory::owner_live_frame_count(owner) == Some(0)
        && domain::domain_is_retired(auxiliary)
        && !domain::handle_permits(
            auxiliary,
            handle,
            crate::control_abi::NETWORK_DEVICE_OBJECT,
            crate::capability::Rights::READ,
        )
        && network::link_is_up()
        && network::generation() == 9
        && network::route() == (0x0000_0018_c0a8_3200, 0x0a00_0402)
        && event
            == Some(
                (45_u64 << 56)
                    | (auxiliary << 8)
                    | u64::from(crate::control_abi::LIFECYCLE_AUXILIARY_EXITED),
            )
        && domain::last_lifecycle_event_audit()
            == Some((
                crate::control_abi::LIFECYCLE_AUXILIARY_EXITED,
                auxiliary,
                12,
            ))
        && domain::last_lifecycle_event_detail() == Some(45)
        && domain::last_lifecycle_service_identity() == Some(identity)
        && domain::take_lifecycle_event().is_none()
}

pub(crate) fn verify_ring3_auxiliary_fault() -> bool {
    const INVALID_OPCODE_PROGRAM: [u8; 2] = [0x0f, 0x0b];
    let Some(auxiliary) =
        domain::initialize_boot_auxiliary_service(1, 2, crate::capability::Rights::WRITE)
    else {
        return false;
    };
    let (Some(handle), Some(identity), Some(owner)) = (
        domain::auxiliary_boot_handle(auxiliary, 0),
        domain::domain_service_identity(auxiliary),
        memory::FrameOwner::domain(auxiliary),
    ) else {
        return false;
    };
    let kernel_root = current_root();
    let Some(space) = WxAddressSpace::new(kernel_root, owner) else {
        return false;
    };
    if !space.permissions_are_valid(kernel_root) {
        return false;
    }
    unsafe {
        for (offset, byte) in INVALID_OPCODE_PROGRAM.iter().enumerate() {
            space.code_frame.add(offset).write(*byte);
        }
    }
    let root = space.root;
    let entry = USER_TEST_ADDRESS;
    let stack = USER_TEST_ADDRESS + 3 * PAGE_SIZE;
    if !register_auxiliary_address_space(auxiliary, space)
        || !scheduler::queue_auxiliary(auxiliary, root, entry, stack, handle)
        || !cpu::activate_user_root(root)
    {
        return false;
    }
    cpu::execute_auxiliary(entry, stack, handle, auxiliary, identity);
    let event = domain::take_lifecycle_event();
    current_root() == kernel_root
        && scheduler::current_domain().is_none()
        && registered_auxiliary_address_space_count() == 0
        && memory::owner_live_frame_count(owner) == Some(0)
        && domain::domain_is_retired(auxiliary)
        && !domain::handle_permits(auxiliary, handle, 2, crate::capability::Rights::WRITE)
        && event
            == Some((auxiliary << 8) | u64::from(crate::control_abi::LIFECYCLE_AUXILIARY_FAULT))
        && domain::last_lifecycle_event_audit()
            == Some((crate::control_abi::LIFECYCLE_AUXILIARY_FAULT, auxiliary, 13))
        && domain::last_lifecycle_event_detail() == Some(0)
        && domain::last_lifecycle_service_identity() == Some(identity)
        && domain::delivered_fault_matches(
            auxiliary,
            domain::FaultMetadata {
                vector: 6,
                error: 0,
                address: 0,
                instruction: entry as u64,
            },
        )
}

struct WxAddressSpace {
    kernel_root: u64,
    root: u64,
    code_frame: *mut u8,
    first_stack_frame: *mut u8,
    second_stack_frame: *mut u8,
    frames: [memory::FrameLease; USER_ADDRESS_SPACE_FRAMES],
    owner: memory::FrameOwner,
}

impl WxAddressSpace {
    fn new(kernel_root: u64, owner: memory::FrameOwner) -> Option<Self> {
        let frames = memory::allocate_frames::<USER_ADDRESS_SPACE_FRAMES>(owner)?;
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
                zero_frame(frame)?;
            }
            let root_entries = mapped_frame::<u64>(root)?;
            let kernel_entries = mapped_frame::<u64>(kernel_root)?;
            root_entries.write(kernel_entries.read());
            root_entries.add(1).write(pdpt | PRESENT | WRITABLE | USER);
            root_entries.add(256).write(kernel_entries.add(256).read());
            mapped_frame::<u64>(pdpt)?.write(directory | PRESENT | WRITABLE | USER);
            mapped_frame::<u64>(directory)?.write(table | PRESENT | WRITABLE | USER);
            let entries = mapped_frame::<u64>(table)?;
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
            kernel_root,
            root,
            code_frame: mapped_frame::<u8>(code)?,
            first_stack_frame: mapped_frame::<u8>(first_stack)?,
            second_stack_frame: mapped_frame::<u8>(second_stack)?,
            frames,
            owner,
        })
    }

    fn permissions_are_valid(&self, kernel_root: u64) -> bool {
        unsafe {
            let Some(root) = mapped_frame::<u64>(self.root) else {
                return false;
            };
            let Some(kernel) = mapped_frame::<u64>(kernel_root) else {
                return false;
            };
            if root.read() != kernel.read()
                || root.read() & USER != 0
                || root.add(256).read() != kernel.add(256).read()
                || root.add(256).read() & USER != 0
            {
                return false;
            }
            let Some(pdpt) = mapped_frame::<u64>(root.add(1).read() & ADDRESS_MASK) else {
                return false;
            };
            let Some(directory) = mapped_frame::<u64>(pdpt.read() & ADDRESS_MASK) else {
                return false;
            };
            let Some(table) = mapped_frame::<u64>(directory.read() & ADDRESS_MASK) else {
                return false;
            };
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
                && code & ADDRESS_MASK == self.frames[4].address()
                && first_stack & ADDRESS_MASK == self.frames[5].address()
                && second_stack & ADDRESS_MASK == self.frames[6].address()
                && paging::direct_map_range_address(self.frames[4].address(), PAGE_SIZE)
                    == Some(self.code_frame as usize)
                && paging::direct_map_range_address(self.frames[5].address(), PAGE_SIZE)
                    == Some(self.first_stack_frame as usize)
                && paging::direct_map_range_address(self.frames[6].address(), PAGE_SIZE)
                    == Some(self.second_stack_frame as usize)
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
                if zero_frame(frame.address()).is_none() {
                    return false;
                }
            }
        }
        memory::release_frames(self.owner, frames)
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

fn mapped_frame<T>(frame: u64) -> Option<*mut T> {
    if !frame.is_multiple_of(PAGE_SIZE as u64) {
        return None;
    }
    paging::direct_map_range_address(frame, PAGE_SIZE).map(|address| address as *mut T)
}

unsafe fn zero_frame(frame: u64) -> Option<()> {
    let entries = mapped_frame::<u64>(frame)?;
    for index in 0..ENTRY_COUNT {
        entries.add(index).write(0);
    }
    Some(())
}
