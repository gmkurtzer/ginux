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
const ACCESSED: u64 = 1 << 5;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const USER_TEST_ADDRESS: usize = 1usize << 39;
const EXPECTED_SERVICE_FAULT_OFFSET: usize = 197;
const ROOT_HANDLE_STACK_OFFSET: usize = PAGE_SIZE - 256;
const TEST_PATTERN: u64 = 0x4749_4e55_5855_5345;
const NO_EXECUTE: u64 = 1 << 63;
const LINUX_INIT_ARGUMENT: [u8; 6] = [b'/', b'i', b'n', b'i', b't', 0];
const LINUX_INIT_ENVIRONMENT: [u8; 8] = [b'G', b'I', b'N', b'U', b'X', b'=', b'1', 0];
const LINUX_PLATFORM: [u8; 7] = [b'x', b'8', b'6', b'_', b'6', b'4', 0];
const LINUX_TLS_THREAD_POINTER_OFFSET: usize = PAGE_SIZE - 64;
const LINUX_TLS_TCB_SIZE: usize = 16;
pub(crate) const USER_ADDRESS_SPACE_FRAMES: usize = 7;
const PAGE_ATTRIBUTE_TABLE: u64 = 1 << 12;

static mut AUXILIARY_ADDRESS_SPACES: [Option<RegisteredAuxiliaryAddressSpace>;
    domain::MAX_AUXILIARY_SERVICES] = [const { None }; domain::MAX_AUXILIARY_SERVICES];

pub(crate) fn verify_linux_elf_mapping(
    plan: &crate::linux_elf::LoadPlan<'_>,
    protected_start: u64,
    protected_size: usize,
    exec_name: &[u8],
) -> bool {
    let Some(kernel_frames_before) = memory::owner_live_frame_count(memory::FrameOwner::KERNEL)
    else {
        return false;
    };
    let Some(exec_node) = crate::initramfs::published_regular_file_node(exec_name) else {
        return false;
    };
    let exec_target = crate::initramfs::published_file_by_node(exec_node)
        .and_then(crate::linux_elf::measured_static_exec_plan);
    let Some(exec_plan) = exec_target else {
        return false;
    };
    let Some(exec_header) = exec_plan.read_only else {
        return false;
    };
    let Some((exec_mapping_plan, exec_mapping_count)) = cpu::linux_exec_mapping_plan(&exec_plan)
    else {
        return false;
    };
    let startup = crate::linux_elf::static_startup(plan);
    let first = plan.executable;
    let header = plan.read_only;
    let second = plan.writable;
    let mut initial_image_addresses = [0_usize; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    let mut initial_image_count = 0;
    for segment in [header, Some(first), Some(second)].into_iter().flatten() {
        for page in 0..segment.page_count {
            let Some(address) = segment
                .virtual_page
                .checked_add((page * PAGE_SIZE) as u64)
                .and_then(|address| usize::try_from(address).ok())
            else {
                return false;
            };
            let Some(slot) = initial_image_addresses.get_mut(initial_image_count) else {
                return false;
            };
            *slot = address;
            initial_image_count += 1;
        }
    }
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
    let header_indexes = header.map_or_else(
        || indexes(exec_header.virtual_page),
        |segment| indexes(segment.virtual_page),
    );
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
    let mmap_virtual_pages: [u64; cpu::LINUX_MMAP_SLOT_COUNT] = core::array::from_fn(|index| {
        mmap_virtual_page
            .checked_add((index * PAGE_SIZE) as u64)
            .unwrap_or(0)
    });
    let heap_indexes = indexes(heap_virtual_page);
    let stack_indexes = indexes(stack_virtual_page);
    let mmap_indexes: [[usize; 4]; cpu::LINUX_MMAP_SLOT_COUNT] = mmap_virtual_pages.map(indexes);
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
        || mmap_virtual_pages.contains(&0)
        || overlaps_protected(heap_virtual_page)
        || overlaps_protected(stack_virtual_page)
        || mmap_virtual_pages.iter().copied().any(overlaps_protected)
        || first_indexes[..3] != second_indexes[..3]
        || first_last_indexes[..3] != first_indexes[..3]
        || second_last_indexes[..3] != first_indexes[..3]
        || heap_indexes[..3] != first_indexes[..3]
        || stack_indexes[..3] != first_indexes[..3]
        || mmap_indexes
            .iter()
            .any(|indexes| indexes[..3] != first_indexes[..3])
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
    let Some(frames) = memory::allocate_frames::<16>(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let addresses = frames.map(memory::FrameLease::address);
    let [root, pdpt, directory, table, header_table, first_page, first_tail_page, first_third_page, first_fourth_page, first_fifth_page, header_page, header_tail_page, second_page, second_tail_page, heap_page, stack_page] =
        addresses;
    let kernel_root_physical = current_root();
    let mut exec_frames = [None; cpu::LINUX_EXEC_MAPPING_LIMIT];
    let mut exec_stack_frame = None;
    let mut exec_heap_frame = None;
    let mut exec_page_table_frames = [None; 6];
    let valid = (|| -> Option<bool> {
        unsafe {
            for frame in [root, pdpt, directory, table, header_table] {
                zero_frame(frame)?;
            }
            let kernel_root = mapped_frame::<u64>(kernel_root_physical)?;
            let root_entries = mapped_frame::<u64>(root)?;
            let pdpt_entries = mapped_frame::<u64>(pdpt)?;
            let directory_entries = mapped_frame::<u64>(directory)?;
            let mut table_entries = mapped_frame::<u64>(table)?;
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

            if header_indexes[2] != first_indexes[2] {
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
            let first_fifth_bytes =
                &mut *(mapped_frame::<u8>(first_fifth_page)? as *mut [u8; PAGE_SIZE]);
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
                || first.page_count == 5
                    && (!crate::linux_elf::materialize_page(first, 2, first_third_bytes)
                        || !crate::linux_elf::materialize_page(first, 3, first_fourth_bytes)
                        || !crate::linux_elf::materialize_page(first, 4, first_fifth_bytes))
                || second.page_count == 2
                    && !crate::linux_elf::materialize_page(second, 1, second_tail_bytes)
                || header.is_some_and(|segment| {
                    !crate::linux_elf::materialize(segment, header_bytes)
                        || segment.page_count == 2
                            && !crate::linux_elf::materialize_page(segment, 1, header_tail_bytes)
                })
                || zero_frame(heap_page).is_none()
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
            if first.page_count == 5 {
                table_entries
                    .add(first_indexes[3] + 2)
                    .write(first_third_page | flags(first));
                table_entries
                    .add(first_indexes[3] + 3)
                    .write(first_fourth_page | flags(first));
                table_entries
                    .add(first_indexes[3] + 4)
                    .write(first_fifth_page | flags(first));
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
            for indexes in mmap_indexes {
                table_entries.add(indexes[3]).write(0);
            }

            let stack_bytes = mapped_frame::<u8>(stack_page)?;
            let stack_offset = PAGE_SIZE - 48 * size_of::<u64>();
            let platform_offset = PAGE_SIZE - 6 * size_of::<u64>();
            let environment_offset = PAGE_SIZE - 4 * size_of::<u64>();
            let argument_offset = PAGE_SIZE - 2 * size_of::<u64>();
            let platform_address = stack_virtual_page + platform_offset as u64;
            let environment_address = stack_virtual_page + environment_offset as u64;
            let argument_address = stack_virtual_page + argument_offset as u64;
            let stack_words = stack_bytes.add(stack_offset).cast::<u64>();
            for (index, value) in [1_u64, argument_address, 0, environment_address, 0]
                .into_iter()
                .enumerate()
            {
                stack_words.add(index).write(value);
            }
            for (index, value) in
                crate::linux_elf::auxiliary_vector(startup, argument_address, platform_address)
                    .into_iter()
                    .enumerate()
            {
                stack_words.add(5 + index).write(value);
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
            let first_fourth_entry = if first.page_count >= 4 {
                table_entries.add(first_indexes[3] + 3).read()
            } else {
                0
            };
            let first_fifth_entry = if first.page_count == 5 {
                table_entries.add(first_indexes[3] + 4).read()
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
                && (first.page_count < 5
                    || first_fifth_entry & (PRESENT | USER) == PRESENT | USER
                        && first_fifth_entry & (WRITABLE | NO_EXECUTE) == 0)
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
                && mmap_indexes
                    .iter()
                    .all(|indexes| table_entries.add(indexes[3]).read() == 0)
                && header_entries_valid
                && crate::linux_elf::materialized_page_matches(first, 0, first_bytes)
                && (first.page_count == 1
                    || crate::linux_elf::materialized_page_matches(first, 1, first_tail_bytes))
                && (first.page_count < 3
                    || crate::linux_elf::materialized_page_matches(first, 2, first_third_bytes))
                && (first.page_count < 4
                    || crate::linux_elf::materialized_page_matches(first, 3, first_fourth_bytes))
                && (first.page_count < 5
                    || crate::linux_elf::materialized_page_matches(first, 4, first_fifth_bytes))
                && crate::linux_elf::materialized_page_matches(second, 0, second_bytes)
                && (second.page_count == 1
                    || crate::linux_elf::materialized_page_matches(second, 1, second_tail_bytes))
                && heap_bytes.iter().all(|byte| *byte == 0)
                && stack_words.read() == 1
                && stack_words.add(1).read() == argument_address
                && stack_words.add(3).read() == environment_address
                && stack_words.add(5).read() == 3
                && stack_words.add(6).read() == startup.program_headers
                && stack_words.add(7).read() == 4
                && stack_words.add(8).read() == crate::linux_elf::PROGRAM_HEADER_SIZE as u64
                && stack_words.add(9).read() == 5
                && stack_words.add(10).read() == u64::from(startup.program_header_count)
                && stack_words.add(11).read() == 6
                && stack_words.add(12).read() == PAGE_SIZE as u64
                && stack_words.add(13).read() == 7
                && stack_words.add(14).read() == startup.interpreter_base
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
                && stack_words.add(30).read() == startup.main_entry
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
            let mut initial_image_mappings =
                [cpu::LinuxActiveImageMapping::EMPTY; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
            for (index, address) in initial_image_addresses[..initial_image_count]
                .iter()
                .enumerate()
            {
                let entry = current_linux_leaf(*address as u64)?.read();
                let frame_address = entry & ADDRESS_MASK;
                let frame = frames
                    .iter()
                    .copied()
                    .find(|frame| frame.address() == frame_address)?;
                initial_image_mappings[index] = cpu::LinuxActiveImageMapping {
                    address: *address,
                    frame,
                    writable: entry & WRITABLE != 0,
                    executable: entry & NO_EXECUTE == 0,
                };
            }
            let rollback_frames = [
                header_page,
                first_page,
                first_tail_page,
                first_third_page,
                first_fourth_page,
                first_fifth_page,
                second_page,
                second_tail_page,
            ];
            let mut rollback_mappings = exec_mapping_plan;
            for (mapping, frame) in rollback_mappings[..exec_mapping_count]
                .iter_mut()
                .zip(rollback_frames)
            {
                mapping.frame = frame;
            }
            let exec_rollback_valid = activated
                && verify_current_linux_image_rollback(
                    &rollback_mappings[..exec_mapping_count],
                    &initial_image_mappings[..initial_image_count],
                );
            let observed = cpu::with_supervisor_user_access(|| {
                (first.virtual_address as *const u8).read_volatile() == first.bytes[0]
                    && (second.virtual_address as *const u64).read_volatile()
                        == u64::from_le_bytes(*b"PROBE!!\n")
                    && header.is_none_or(|segment| {
                        (segment.virtual_address as *const u8).read_volatile() == segment.bytes[0]
                    })
            });
            let execution_result = if exec_rollback_valid && observed {
                cpu::verify_linux_elf_entry(&cpu::LinuxElfExecution {
                    entry: startup.initial_entry as usize,
                    page_table_frames: [
                        frames[0],
                        frames[1],
                        frames[2],
                        frames[3],
                        frames[4],
                        memory::FrameLease::EMPTY,
                    ],
                    page_table_frame_count: 5,
                    stack: (stack_virtual_page + stack_offset as u64) as usize,
                    stack_frame: frames[15],
                    heap_frame: frames[14],
                    tls_frames: [memory::FrameLease::EMPTY; 2],
                    initial_fs_base: 0,
                    tls: 0,
                    tls_size: 0,
                    sibling_fs_base: 0,
                    sibling_tls: 0,
                    sibling_stack: 0,
                    sibling_stack_frame: memory::FrameLease::EMPTY,
                    code: first.virtual_address as usize,
                    code_size: first.memory_size,
                    loader_code: 0,
                    loader_code_size: 0,
                    data: second.virtual_page as usize,
                    data_size: second.memory_size,
                    relro: relro.virtual_address as usize,
                    relro_size: relro.memory_size as usize,
                    heap: heap_virtual_page as usize,
                    heap_size: PAGE_SIZE,
                    mmap: mmap_virtual_page as usize,
                    main_mappings: [cpu::LinuxFileMapping::EMPTY; 8],
                    dependency_base_min: [0; 2],
                    dependency_base_end: [0; 2],
                    dependency_mappings: [[cpu::LinuxRelativeFileMapping::EMPTY; 8]; 2],
                    exec_expected: true,
                    initial_image_count,
                    initial_image_mappings,
                })
            } else {
                cpu::LinuxElfResult::FAILED
            };
            if !cpu::finalize_linux_exec_page_table_frames(
                [
                    frames[0],
                    frames[1],
                    frames[2],
                    frames[3],
                    frames[4],
                    memory::FrameLease::EMPTY,
                ],
                5,
                true,
            ) {
                return None;
            }
            exec_page_table_frames = cpu::take_linux_exec_page_table_frames();
            if exec_page_table_frames[..5].iter().any(Option::is_none)
                || exec_page_table_frames[5].is_some()
            {
                return None;
            }
            table_entries = mapped_frame::<u64>(exec_page_table_frames[3]?.address())?;
            exec_frames = execution_result.exec_frames;
            if execution_result.exec_mapping_count < cpu::LINUX_EXEC_MAPPING_LIMIT {
                exec_stack_frame = exec_frames[execution_result.exec_mapping_count];
                exec_frames[execution_result.exec_mapping_count] = None;
            }
            exec_heap_frame = cpu::take_linux_exec_heap_frame();
            let retired_image_frames = execution_result.retired_image_frames;
            let retired_image_valid = execution_result.retired_image_count == initial_image_count
                && retired_image_frames[..initial_image_count]
                    .iter()
                    .zip(&initial_image_mappings[..initial_image_count])
                    .all(|(retired, mapping)| {
                        *retired == Some(mapping.frame)
                            && !memory::frames_owned_by(
                                memory::FrameOwner::KERNEL,
                                &[mapping.frame],
                            )
                            && frame_is_zero(mapping.frame.address())
                    })
                && retired_image_frames[initial_image_count..]
                    .iter()
                    .all(Option::is_none);
            let exec_mappings_valid = execution_result.exec_mapping_count == exec_mapping_count
                && exec_mapping_plan[..exec_mapping_count]
                    .iter()
                    .zip(&exec_frames[..exec_mapping_count])
                    .all(|(mapping, frame)| {
                        let Some(frame) = frame else {
                            return false;
                        };
                        let flags = PRESENT
                            | USER
                            | if mapping.writable { WRITABLE } else { 0 }
                            | if mapping.executable { 0 } else { NO_EXECUTE };
                        current_linux_leaf(mapping.address as u64).is_some_and(|entry| {
                            entry.read() & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE)
                                == frame.address() | flags
                        })
                    })
                && exec_frames[exec_mapping_count..]
                    .iter()
                    .all(Option::is_none)
                && initial_image_addresses[..initial_image_count]
                    .iter()
                    .all(|address| {
                        exec_mapping_plan[..exec_mapping_count]
                            .iter()
                            .any(|mapping| mapping.address == *address)
                            || current_linux_leaf(*address as u64)
                                .is_some_and(|entry| entry.read() == 0)
                    });
            let exec_stack_mapping_valid = exec_stack_frame.is_some_and(|frame| {
                table_entries.add(stack_indexes[3]).read()
                    & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == frame.address() | PRESENT | USER | WRITABLE | NO_EXECUTE
                    && frame != frames[15]
                    && !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frames[15]])
            });
            let exec_heap_mapping_valid = exec_heap_frame.is_some_and(|frame| {
                table_entries.add(heap_indexes[3]).read()
                    & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == frame.address() | PRESENT | USER | WRITABLE | NO_EXECUTE
                    && frame != frames[14]
                    && !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frames[14]])
                    && frame_is_zero(frame.address())
            });
            let executed = execution_result.valid
                && execution_result.exec_replaced
                && execution_result.exec_node == exec_node
                && execution_result.active_image_mapping_count == exec_mapping_count
                && retired_image_valid
                && exec_mappings_valid
                && exec_stack_mapping_valid
                && exec_heap_mapping_valid;
            let mapping_removed = mmap_indexes
                .iter()
                .all(|indexes| table_entries.add(indexes[3]).read() == 0);
            activate(kernel_root_physical);
            let restored = current_root() == kernel_root_physical;
            Some(executed && mapping_removed && restored)
        }
    })()
    .unwrap_or(false);
    let cleared = frames.iter().enumerate().all(|(index, frame)| unsafe {
        if memory::frames_owned_by(memory::FrameOwner::KERNEL, &[*frame]) {
            zero_frame(frame.address()).is_some()
        } else if (index < 5 && exec_page_table_frames[index].is_some())
            || (index == 15 && exec_stack_frame.is_some())
            || (index == 14 && exec_heap_frame.is_some())
        {
            true
        } else {
            frame_is_zero(frame.address())
        }
    });
    let mut exec_released = true;
    for frame in exec_frames.into_iter().flatten() {
        exec_released &= prepare_linux_frame(frame.address(), &[])
            && memory::release_frames(memory::FrameOwner::KERNEL, [frame]);
    }
    let exec_stack_released = exec_stack_frame.is_some_and(|frame| {
        prepare_linux_frame(frame.address(), &[])
            && memory::release_frames(memory::FrameOwner::KERNEL, [frame])
    });
    let exec_heap_released = exec_heap_frame.is_some_and(|frame| {
        prepare_linux_frame(frame.address(), &[])
            && memory::release_frames(memory::FrameOwner::KERNEL, [frame])
    });
    let exec_page_table_frames_released = exec_page_table_frames[..5].iter().all(|frame| {
        frame.is_some_and(|frame| {
            prepare_linux_frame(frame.address(), &[])
                && memory::release_frames(memory::FrameOwner::KERNEL, [frame])
        })
    }) && exec_page_table_frames[5].is_none();
    let mut original_frames_released = cleared;
    if cleared {
        for frame in frames {
            if memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame]) {
                original_frames_released &=
                    memory::release_frames(memory::FrameOwner::KERNEL, [frame]);
            }
        }
    }
    let released = valid
        && cleared
        && exec_released
        && exec_stack_released
        && exec_heap_released
        && exec_page_table_frames_released
        && original_frames_released;
    released
        && memory::owner_live_frame_count(memory::FrameOwner::KERNEL) == Some(kernel_frames_before)
}

pub(crate) fn verify_interpreted_elf_mapping(
    dependency_plan: &crate::linux_elf::DependencyGraphLoadPlan<'_, '_, '_>,
    protected_start: u64,
    protected_size: usize,
) -> bool {
    let Some(kernel_frames_before) = memory::owner_live_frame_count(memory::FrameOwner::KERNEL)
    else {
        return false;
    };
    let plan = &dependency_plan.process;
    let Some(main_ro) = plan.main.read_only else {
        return false;
    };
    let main_rx = plan.main.executable;
    let main_rw = plan.main.writable;
    let Some(main_tls) = plan.main.tls else {
        return false;
    };
    let Some(dependency_tls) = dependency_plan.dependencies[0].tls else {
        return false;
    };
    let Some(auxiliary_tls) = dependency_plan.dependencies[1].tls else {
        return false;
    };
    let Some(tls_layout) = crate::linux_elf::plan_static_tls_layout(
        main_tls,
        [dependency_tls, auxiliary_tls],
        LINUX_TLS_THREAD_POINTER_OFFSET,
        LINUX_TLS_TCB_SIZE,
        PAGE_SIZE,
    ) else {
        return false;
    };
    let main_tls_memory_size = tls_layout.main.memory_size;
    let main_tls_alignment = tls_layout.main.alignment;
    let main_tls_start_offset = tls_layout.main.start_offset;
    let dependency_tls_memory_size = tls_layout.dependencies[0].memory_size;
    let dependency_tls_alignment = tls_layout.dependencies[0].alignment;
    let dependency_tls_start_offset = tls_layout.dependencies[0].start_offset;
    let dependency_tls_offset = tls_layout.dependencies[0].thread_offset;
    let auxiliary_tls_memory_size = tls_layout.dependencies[1].memory_size;
    let auxiliary_tls_alignment = tls_layout.dependencies[1].alignment;
    let auxiliary_tls_start_offset = tls_layout.dependencies[1].start_offset;
    let auxiliary_tls_offset = tls_layout.dependencies[1].thread_offset;
    let tls_start_offset = tls_layout.extent_start;
    let tls_extent_size = tls_layout.extent_size;
    let interpreter_rx = plan.interpreter.executable;
    let interpreter_rw = plan.interpreter.writable;
    let dependency_rx = dependency_plan.dependencies[0].executable;
    let dependency_rw = dependency_plan.dependencies[0].writable;
    let auxiliary_rx = dependency_plan.dependencies[1].executable;
    let dependency_base_min = dependency_plan.dependency_bases;
    let (Some(first_dependency_base_end), Some(second_dependency_base_end)) = (
        dependency_base_min[0].checked_add(16 * PAGE_SIZE as u64),
        dependency_base_min[1].checked_add(16 * PAGE_SIZE as u64),
    ) else {
        return false;
    };
    let dependency_base_end = [first_dependency_base_end, second_dependency_base_end];
    let mut dependency_mappings = [[cpu::LinuxRelativeFileMapping::EMPTY; 8]; 2];
    let mut dependency_mapping_count = [0_usize; 2];
    let mut dependency_last_relative = [0_u64; 2];
    for dependency in 0..2 {
        let dependency_plan = &dependency_plan.dependencies[dependency];
        for segment in [
            Some(dependency_plan.executable),
            dependency_plan.read_only,
            Some(dependency_plan.writable),
        ]
        .into_iter()
        .flatten()
        {
            for page in 0..segment.page_count {
                let index = dependency_mapping_count[dependency];
                if index == dependency_mappings[dependency].len() {
                    return false;
                }
                let Some(virtual_page) =
                    segment.virtual_page.checked_add((page * PAGE_SIZE) as u64)
                else {
                    return false;
                };
                let Some(relative_address) =
                    virtual_page.checked_sub(dependency_base_min[dependency])
                else {
                    return false;
                };
                let Ok(relative_address_usize) = usize::try_from(relative_address) else {
                    return false;
                };
                let Some(offset) = segment.file_page.checked_add((page * PAGE_SIZE) as u64) else {
                    return false;
                };
                let Some(file_size) = crate::linux_elf::planned_file_page_size(segment, page)
                else {
                    return false;
                };
                dependency_mappings[dependency][index] = cpu::LinuxRelativeFileMapping {
                    relative_address: relative_address_usize,
                    offset,
                    file_size,
                    writable: segment.writable,
                    executable: segment.executable,
                };
                dependency_mapping_count[dependency] += 1;
                dependency_last_relative[dependency] =
                    dependency_last_relative[dependency].max(relative_address);
            }
        }
    }
    let Some(dependency_entry_offset) = dependency_plan.dependencies[0]
        .entry
        .checked_sub(dependency_rx.virtual_address)
        .and_then(|offset| usize::try_from(offset).ok())
    else {
        return false;
    };
    let Some(&dependency_entry_byte) = dependency_rx.bytes.get(dependency_entry_offset) else {
        return false;
    };
    let Some(auxiliary_entry_offset) = dependency_plan.dependencies[1]
        .entry
        .checked_sub(auxiliary_rx.virtual_address)
        .and_then(|offset| usize::try_from(offset).ok())
    else {
        return false;
    };
    let Some(&auxiliary_entry_byte) = auxiliary_rx.bytes.get(auxiliary_entry_offset) else {
        return false;
    };
    let Some(relro) = plan.main.relro else {
        return false;
    };
    let Some(interpreter_dynamic) = plan.interpreter.dynamic_relocations else {
        return false;
    };
    let Some(main_dynamic) = plan.main.dynamic_relocations else {
        return false;
    };
    if main_dynamic.init_function.is_none()
        || main_dynamic.preinit_array != main_dynamic.preinit_relocation_target
        || main_dynamic.init_array != Some(main_dynamic.relocation_target)
    {
        return false;
    }
    let (Some(main_preinit_target), Some(main_preinit_addend)) = (
        main_dynamic.preinit_relocation_target,
        main_dynamic.preinit_relocation_addend,
    ) else {
        return false;
    };
    let (Some(main_symbol), Some(main_jump_slot)) = (
        main_dynamic.symbol_relocation,
        main_dynamic.jump_slot_relocation,
    ) else {
        return false;
    };
    let (Some(main_auxiliary_symbol), Some(main_auxiliary_jump_slot)) = (
        main_dynamic.secondary_symbol_relocation,
        main_dynamic.secondary_jump_slot_relocation,
    ) else {
        return false;
    };
    let Some(dependency_dynamic) = dependency_plan.dependencies[0].dynamic_relocations else {
        return false;
    };
    let (Some(dependency_symbol), Some(dependency_jump_slot)) = (
        dependency_dynamic.symbol_relocation,
        dependency_dynamic.jump_slot_relocation,
    ) else {
        return false;
    };
    let Some(dependency_tls_relocation_target) = dependency_dynamic
        .tls_relocation_target
        .map(core::num::NonZeroU64::get)
    else {
        return false;
    };
    if dependency_dynamic.init_function.is_none()
        || dependency_dynamic.init_array != Some(dependency_dynamic.relocation_target)
    {
        return false;
    }
    let Some(auxiliary_dynamic) = dependency_plan.dependencies[1].dynamic_relocations else {
        return false;
    };
    let (Some(auxiliary_symbol), Some(auxiliary_jump_slot)) = (
        auxiliary_dynamic.symbol_relocation,
        auxiliary_dynamic.jump_slot_relocation,
    ) else {
        return false;
    };
    let Some(auxiliary_tls_relocation_target) = auxiliary_dynamic
        .tls_relocation_target
        .map(core::num::NonZeroU64::get)
    else {
        return false;
    };
    if auxiliary_dynamic.init_function.is_none()
        || auxiliary_dynamic.init_array != Some(auxiliary_dynamic.relocation_target)
    {
        return false;
    }
    let Some(interpreter_relocation_value) = plan
        .interpreter_base
        .checked_add(interpreter_dynamic.relocation_addend)
    else {
        return false;
    };
    let Some(main_relocation_value) = main_ro
        .virtual_page
        .checked_add(main_dynamic.relocation_addend)
    else {
        return false;
    };
    let Some(main_preinit_relocation_value) = main_ro.virtual_page.checked_add(main_preinit_addend)
    else {
        return false;
    };
    let startup = crate::linux_elf::interpreted_startup(plan);
    let heap_virtual_page = main_rw
        .virtual_page
        .checked_add(main_rw.page_count as u64 * PAGE_SIZE as u64)
        .unwrap_or(0);
    let stack_virtual_page = heap_virtual_page
        .checked_add(2 * PAGE_SIZE as u64)
        .unwrap_or(0);
    let mmap_virtual_page = stack_virtual_page
        .checked_add(2 * PAGE_SIZE as u64)
        .unwrap_or(0);
    let mmap_virtual_pages: [u64; cpu::LINUX_MMAP_SLOT_COUNT] = core::array::from_fn(|index| {
        mmap_virtual_page
            .checked_add((index * PAGE_SIZE) as u64)
            .unwrap_or(0)
    });
    let tls_virtual_page = mmap_virtual_page
        .checked_add((cpu::LINUX_MMAP_SLOT_COUNT * PAGE_SIZE) as u64)
        .unwrap_or(0);
    let sibling_tls_virtual_page = tls_virtual_page.checked_add(PAGE_SIZE as u64).unwrap_or(0);
    let sibling_stack_lower_guard = sibling_tls_virtual_page
        .checked_add(PAGE_SIZE as u64)
        .unwrap_or(0);
    let sibling_stack_virtual_page = sibling_stack_lower_guard
        .checked_add(PAGE_SIZE as u64)
        .unwrap_or(0);
    let sibling_stack_upper_guard = sibling_stack_virtual_page
        .checked_add(PAGE_SIZE as u64)
        .unwrap_or(0);
    let tls_base = tls_virtual_page
        .checked_add(LINUX_TLS_THREAD_POINTER_OFFSET as u64)
        .unwrap_or(0);
    let sibling_tls_base = sibling_tls_virtual_page
        .checked_add(LINUX_TLS_THREAD_POINTER_OFFSET as u64)
        .unwrap_or(0);
    let indexes = |address: u64| {
        [
            ((address >> 39) & 0x1ff) as usize,
            ((address >> 30) & 0x1ff) as usize,
            ((address >> 21) & 0x1ff) as usize,
            ((address >> 12) & 0x1ff) as usize,
        ]
    };
    let main_ro_indexes = indexes(main_ro.virtual_page);
    let main_indexes = indexes(main_rx.virtual_page);
    let main_rw_indexes = indexes(main_rw.virtual_page);
    let interpreter_indexes = indexes(interpreter_rx.virtual_page);
    let interpreter_rw_indexes = indexes(interpreter_rw.virtual_page);
    let dependency_window_start = dependency_base_min;
    let dependency_window_end: [u64; 2] = core::array::from_fn(|dependency| {
        dependency_base_end[dependency]
            .checked_sub(PAGE_SIZE as u64)
            .and_then(|last_base| last_base.checked_add(dependency_last_relative[dependency]))
            .and_then(|last_page| last_page.checked_add(PAGE_SIZE as u64))
            .unwrap_or(0)
    });
    if dependency_window_end.contains(&0) {
        return false;
    }
    let dependency_window_pages: [usize; 2] = core::array::from_fn(|dependency| {
        usize::try_from(
            dependency_window_end[dependency].saturating_sub(dependency_window_start[dependency])
                / PAGE_SIZE as u64,
        )
        .unwrap_or(0)
    });
    let heap_indexes = indexes(heap_virtual_page);
    let stack_indexes = indexes(stack_virtual_page);
    let mmap_indexes: [[usize; 4]; cpu::LINUX_MMAP_SLOT_COUNT] = mmap_virtual_pages.map(indexes);
    let tls_indexes = indexes(tls_virtual_page);
    let sibling_tls_indexes = indexes(sibling_tls_virtual_page);
    let sibling_stack_lower_guard_indexes = indexes(sibling_stack_lower_guard);
    let sibling_stack_indexes = indexes(sibling_stack_virtual_page);
    let sibling_stack_upper_guard_indexes = indexes(sibling_stack_upper_guard);
    let Some(protected_end) = protected_start.checked_add(protected_size as u64) else {
        return false;
    };
    let overlaps_protected = |segment: crate::linux_elf::PlannedSegment<'_>| {
        (0..segment.page_count).any(|page| {
            let start = segment
                .virtual_page
                .saturating_add((page * PAGE_SIZE) as u64);
            let end = start.saturating_add(PAGE_SIZE as u64);
            start < protected_end && protected_start < end
        })
    };
    let dependency_window_conflicts = (0..2).any(|dependency| {
        (0..dependency_window_pages[dependency]).any(|offset| {
            let page = dependency_window_start[dependency] + (offset * PAGE_SIZE) as u64;
            let end = page + PAGE_SIZE as u64;
            let overlaps_segment = |segment: crate::linux_elf::PlannedSegment<'_>| {
                (0..segment.page_count).any(|segment_page| {
                    page == segment.virtual_page + (segment_page * PAGE_SIZE) as u64
                })
            };
            page < protected_end && protected_start < end
                || [main_ro, main_rx, main_rw, interpreter_rx, interpreter_rw]
                    .into_iter()
                    .any(overlaps_segment)
                || mmap_virtual_pages.contains(&page)
                || [
                    heap_virtual_page,
                    stack_virtual_page,
                    tls_virtual_page,
                    sibling_tls_virtual_page,
                    sibling_stack_lower_guard,
                    sibling_stack_virtual_page,
                    sibling_stack_upper_guard,
                ]
                .contains(&page)
                || (0..dependency).any(|prior| {
                    page < dependency_window_end[prior] && dependency_window_start[prior] < end
                })
        })
    });
    let dependency_contracts_valid = (0..2).all(|dependency| {
        dependency_mapping_count[dependency] >= 2
            && dependency_mappings[dependency][..dependency_mapping_count[dependency]]
                .iter()
                .enumerate()
                .all(|(index, mapping)| {
                    mapping.file_size != 0
                        && mapping.file_size <= PAGE_SIZE
                        && mapping.relative_address.is_multiple_of(PAGE_SIZE)
                        && !(mapping.writable && mapping.executable)
                        && !dependency_mappings[dependency][..index]
                            .iter()
                            .any(|prior| {
                                prior.offset == mapping.offset
                                    && prior.writable == mapping.writable
                                    && prior.executable == mapping.executable
                            })
                })
    });
    if protected_size == 0
        || plan.interpreter.read_only.is_some()
        || main_ro.page_count != 1
        || main_rx.page_count != 5
        || main_rw.page_count != 2
        || interpreter_rx.page_count != 2
        || interpreter_rw.page_count != 1
        || !dependency_contracts_valid
        || dependency_base_min
            .iter()
            .chain(&dependency_base_end)
            .any(|base| !base.is_multiple_of(PAGE_SIZE as u64))
        || dependency_window_pages
            .iter()
            .any(|pages| !(1..=32).contains(pages))
        || main_ro_indexes[..2] != main_indexes[..2]
        || main_indexes[..2] != interpreter_indexes[..2]
        || main_ro_indexes[2] == main_indexes[2]
        || main_ro_indexes[2] == interpreter_indexes[2]
        || main_indexes[..3] != main_rw_indexes[..3]
        || interpreter_indexes[..3] != interpreter_rw_indexes[..3]
        || main_indexes[2] == interpreter_indexes[2]
        || main_indexes[0] != 0
        || main_indexes[1] != 0
        || main_ro_indexes[3] + main_ro.page_count > ENTRY_COUNT
        || main_indexes[3] + main_rx.page_count > ENTRY_COUNT
        || main_rw_indexes[3] + main_rw.page_count > ENTRY_COUNT
        || interpreter_indexes[3] + interpreter_rx.page_count > ENTRY_COUNT
        || interpreter_rw_indexes[3] + interpreter_rw.page_count > ENTRY_COUNT
        || dependency_window_start
            .iter()
            .any(|start| indexes(*start)[..3] != interpreter_indexes[..3])
        || dependency_window_end
            .iter()
            .any(|end| indexes(*end - PAGE_SIZE as u64)[..3] != interpreter_indexes[..3])
        || dependency_window_conflicts
        || heap_virtual_page == 0
        || stack_virtual_page == 0
        || mmap_virtual_page == 0
        || mmap_virtual_pages.contains(&0)
        || tls_virtual_page == 0
        || sibling_tls_virtual_page == 0
        || sibling_stack_lower_guard == 0
        || sibling_stack_virtual_page == 0
        || sibling_stack_upper_guard == 0
        || tls_base == 0
        || sibling_tls_base == 0
        || heap_indexes[..3] != main_indexes[..3]
        || stack_indexes[..3] != main_indexes[..3]
        || mmap_indexes
            .iter()
            .any(|indexes| indexes[..3] != main_indexes[..3])
        || tls_indexes[..3] != main_indexes[..3]
        || sibling_tls_indexes[..3] != main_indexes[..3]
        || sibling_stack_lower_guard_indexes[..3] != main_indexes[..3]
        || sibling_stack_indexes[..3] != main_indexes[..3]
        || sibling_stack_upper_guard_indexes[..3] != main_indexes[..3]
        || sibling_tls_indexes[3] == tls_indexes[3]
        || overlaps_protected(main_ro)
        || overlaps_protected(main_rx)
        || overlaps_protected(main_rw)
        || overlaps_protected(interpreter_rx)
        || overlaps_protected(interpreter_rw)
        || mmap_virtual_pages.iter().any(|page| {
            *page < protected_end && protected_start < page.saturating_add(PAGE_SIZE as u64)
        })
        || [
            heap_virtual_page,
            stack_virtual_page,
            tls_virtual_page,
            sibling_tls_virtual_page,
            sibling_stack_lower_guard,
            sibling_stack_virtual_page,
            sibling_stack_upper_guard,
        ]
        .into_iter()
        .any(|page| page < protected_end && protected_start < page.saturating_add(PAGE_SIZE as u64))
        || main_ro.writable
        || main_ro.executable
        || main_rx.writable
        || !main_rx.executable
        || !main_rw.writable
        || main_rw.executable
        || interpreter_rx.writable
        || !interpreter_rx.executable
        || !interpreter_rw.writable
        || interpreter_rw.executable
        || dependency_rx.writable
        || !dependency_rx.executable
        || !dependency_rw.writable
        || dependency_rw.executable
        || dependency_plan.dependencies.iter().any(|dependency| {
            dependency.executable.writable
                || !dependency.executable.executable
                || !dependency.writable.writable
                || dependency.writable.executable
                || dependency
                    .read_only
                    .is_some_and(|segment| segment.writable || segment.executable)
        })
        || main_tls.bytes.len() > main_tls_memory_size
        || dependency_tls.bytes.len() > dependency_tls_memory_size
        || auxiliary_tls.bytes.len() > auxiliary_tls_memory_size
        || !(tls_virtual_page as usize + main_tls_start_offset).is_multiple_of(main_tls_alignment)
        || !(tls_virtual_page as usize + dependency_tls_start_offset)
            .is_multiple_of(dependency_tls_alignment)
        || !(tls_virtual_page as usize + auxiliary_tls_start_offset)
            .is_multiple_of(auxiliary_tls_alignment)
        || !(sibling_tls_virtual_page as usize + main_tls_start_offset)
            .is_multiple_of(main_tls_alignment)
        || !(sibling_tls_virtual_page as usize + dependency_tls_start_offset)
            .is_multiple_of(dependency_tls_alignment)
        || !(sibling_tls_virtual_page as usize + auxiliary_tls_start_offset)
            .is_multiple_of(auxiliary_tls_alignment)
        || LINUX_TLS_THREAD_POINTER_OFFSET + LINUX_TLS_TCB_SIZE > PAGE_SIZE
    {
        return false;
    }

    let Some(frames) = memory::allocate_frames::<14>(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let addresses = frames.map(memory::FrameLease::address);
    let [root, pdpt, directory, main_ro_table, main_table, interpreter_table, interpreter_page, interpreter_tail_page, interpreter_data, heap_page, stack_page, tls_page, sibling_tls_page, sibling_stack_page] =
        addresses;
    let kernel_root_physical = current_root();
    let mut main_frames = [None; 8];
    let mut dependency_frames = [[None; 8]; 2];
    let mut exec_frames = [None; cpu::LINUX_EXEC_MAPPING_LIMIT];
    let mut exec_stack_frame = None;
    let mut exec_heap_frame = None;
    let mut exec_tls_frames = [None; 2];
    let mut exec_sibling_stack_frame = None;
    let mut exec_page_table_frames = [None; 6];
    let valid = (|| -> Option<bool> {
        unsafe {
            for frame in [
                root,
                pdpt,
                directory,
                main_ro_table,
                main_table,
                interpreter_table,
            ] {
                zero_frame(frame)?;
            }
            let kernel_root = mapped_frame::<u64>(kernel_root_physical)?;
            let root_entries = mapped_frame::<u64>(root)?;
            let pdpt_entries = mapped_frame::<u64>(pdpt)?;
            let directory_entries = mapped_frame::<u64>(directory)?;
            let mut main_ro_entries = mapped_frame::<u64>(main_ro_table)?;
            let mut main_entries = mapped_frame::<u64>(main_table)?;
            let mut interpreter_entries = mapped_frame::<u64>(interpreter_table)?;
            root_entries.add(256).write(kernel_root.add(256).read());
            root_entries.write(pdpt | PRESENT | WRITABLE | USER);

            let kernel_pdpt = mapped_frame::<u64>(kernel_root.read() & ADDRESS_MASK)?;
            core::ptr::copy_nonoverlapping(kernel_pdpt, pdpt_entries, ENTRY_COUNT);
            let kernel_directory =
                mapped_frame::<u64>(kernel_pdpt.add(main_indexes[1]).read() & ADDRESS_MASK)?;
            core::ptr::copy_nonoverlapping(kernel_directory, directory_entries, ENTRY_COUNT);
            pdpt_entries
                .add(main_indexes[1])
                .write(directory | PRESENT | WRITABLE | USER);

            for (directory_index, table_entries, table_frame) in [
                (main_ro_indexes[2], main_ro_entries, main_ro_table),
                (main_indexes[2], main_entries, main_table),
                (
                    interpreter_indexes[2],
                    interpreter_entries,
                    interpreter_table,
                ),
            ] {
                let original = kernel_directory.add(directory_index).read();
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
                    .add(directory_index)
                    .write(table_frame | PRESENT | WRITABLE | USER);
            }

            let payloads = [
                (interpreter_rx, 0, interpreter_page),
                (interpreter_rx, 1, interpreter_tail_page),
                (interpreter_rw, 0, interpreter_data),
            ];
            for (segment, page, frame) in payloads {
                let bytes = &mut *(mapped_frame::<u8>(frame)? as *mut [u8; PAGE_SIZE]);
                if !crate::linux_elf::materialize_page(segment, page, bytes) {
                    return None;
                }
            }
            zero_frame(heap_page)?;
            zero_frame(stack_page)?;
            zero_frame(sibling_stack_page)?;
            let tls_bytes = &mut *(mapped_frame::<u8>(tls_page)? as *mut [u8; PAGE_SIZE]);
            if !crate::linux_elf::materialize_static_tls_page(
                tls_layout,
                main_tls,
                [dependency_tls, auxiliary_tls],
                tls_base,
                tls_bytes,
            ) {
                return None;
            }
            let sibling_tls_bytes =
                &mut *(mapped_frame::<u8>(sibling_tls_page)? as *mut [u8; PAGE_SIZE]);
            if !crate::linux_elf::materialize_static_tls_page(
                tls_layout,
                main_tls,
                [dependency_tls, auxiliary_tls],
                sibling_tls_base,
                sibling_tls_bytes,
            ) {
                return None;
            }
            let flags = |segment: crate::linux_elf::PlannedSegment<'_>| {
                PRESENT
                    | USER
                    | if segment.writable { WRITABLE } else { 0 }
                    | if segment.executable { 0 } else { NO_EXECUTE }
            };
            main_ro_entries.add(main_ro_indexes[3]).write(0);
            for offset in 0..main_rx.page_count {
                main_entries.add(main_indexes[3] + offset).write(0);
            }
            for offset in 0..main_rw.page_count {
                main_entries.add(main_rw_indexes[3] + offset).write(0);
            }
            interpreter_entries
                .add(interpreter_indexes[3])
                .write(interpreter_page | flags(interpreter_rx));
            interpreter_entries
                .add(interpreter_indexes[3] + 1)
                .write(interpreter_tail_page | flags(interpreter_rx));
            interpreter_entries
                .add(interpreter_rw_indexes[3])
                .write(interpreter_data | flags(interpreter_rw));
            for dependency in 0..2 {
                for offset in 0..dependency_window_pages[dependency] {
                    let page = dependency_window_start[dependency] + (offset * PAGE_SIZE) as u64;
                    interpreter_entries.add(indexes(page)[3]).write(0);
                }
            }
            main_entries
                .add(heap_indexes[3])
                .write(heap_page | PRESENT | WRITABLE | USER | NO_EXECUTE);
            main_entries
                .add(stack_indexes[3])
                .write(stack_page | PRESENT | WRITABLE | USER | NO_EXECUTE);
            for indexes in mmap_indexes {
                main_entries.add(indexes[3]).write(0);
            }
            main_entries
                .add(tls_indexes[3])
                .write(tls_page | PRESENT | WRITABLE | USER | NO_EXECUTE);
            main_entries
                .add(sibling_tls_indexes[3])
                .write(sibling_tls_page | PRESENT | WRITABLE | USER | NO_EXECUTE);
            main_entries
                .add(sibling_stack_lower_guard_indexes[3])
                .write(0);
            main_entries
                .add(sibling_stack_indexes[3])
                .write(sibling_stack_page | PRESENT | WRITABLE | USER | NO_EXECUTE);
            main_entries
                .add(sibling_stack_upper_guard_indexes[3])
                .write(0);

            let stack_bytes = mapped_frame::<u8>(stack_page)?;
            let stack_offset = PAGE_SIZE - 48 * size_of::<u64>();
            let platform_offset = PAGE_SIZE - 6 * size_of::<u64>();
            let environment_offset = PAGE_SIZE - 4 * size_of::<u64>();
            let argument_offset = PAGE_SIZE - 2 * size_of::<u64>();
            let platform_address = stack_virtual_page + platform_offset as u64;
            let environment_address = stack_virtual_page + environment_offset as u64;
            let argument_address = stack_virtual_page + argument_offset as u64;
            let stack_words = stack_bytes.add(stack_offset).cast::<u64>();
            for (index, value) in [1_u64, argument_address, 0, environment_address, 0]
                .into_iter()
                .enumerate()
            {
                stack_words.add(index).write(value);
            }
            for (index, value) in
                crate::linux_elf::auxiliary_vector(startup, argument_address, platform_address)
                    .into_iter()
                    .enumerate()
            {
                stack_words.add(5 + index).write(value);
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

            let permissions_valid = main_ro_entries.add(main_ro_indexes[3]).read() == 0
                && (0..main_rx.page_count)
                    .all(|offset| main_entries.add(main_indexes[3] + offset).read() == 0)
                && (0..main_rw.page_count)
                    .all(|offset| main_entries.add(main_rw_indexes[3] + offset).read() == 0)
                && (0..interpreter_rx.page_count).all(|offset| {
                    interpreter_entries
                        .add(interpreter_indexes[3] + offset)
                        .read()
                        & (PRESENT | USER | WRITABLE | NO_EXECUTE)
                        == PRESENT | USER
                })
                && interpreter_entries.add(interpreter_rw_indexes[3]).read()
                    & (PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == PRESENT | USER | WRITABLE | NO_EXECUTE
                && (0..2).all(|dependency| {
                    (0..dependency_window_pages[dependency]).all(|offset| {
                        let page =
                            dependency_window_start[dependency] + (offset * PAGE_SIZE) as u64;
                        interpreter_entries.add(indexes(page)[3]).read() == 0
                    })
                })
                && main_entries.add(heap_indexes[3]).read()
                    & (PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == PRESENT | USER | WRITABLE | NO_EXECUTE
                && main_entries.add(stack_indexes[3]).read()
                    & (PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == PRESENT | USER | WRITABLE | NO_EXECUTE
                && mmap_indexes
                    .iter()
                    .all(|indexes| main_entries.add(indexes[3]).read() == 0)
                && main_entries.add(tls_indexes[3]).read()
                    & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == tls_page | PRESENT | USER | WRITABLE | NO_EXECUTE
                && main_entries.add(sibling_tls_indexes[3]).read()
                    & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == sibling_tls_page | PRESENT | USER | WRITABLE | NO_EXECUTE
                && main_entries
                    .add(sibling_stack_lower_guard_indexes[3])
                    .read()
                    == 0
                && main_entries.add(sibling_stack_indexes[3]).read()
                    & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == sibling_stack_page | PRESENT | USER | WRITABLE | NO_EXECUTE
                && main_entries
                    .add(sibling_stack_upper_guard_indexes[3])
                    .read()
                    == 0
                && stack_words.add(6).read() == startup.program_headers
                && stack_words.add(10).read() == u64::from(startup.program_header_count)
                && stack_words.add(14).read() == startup.interpreter_base
                && stack_words.add(30).read() == startup.main_entry;
            activate(root);
            let observed = current_root() == root
                && cpu::with_supervisor_user_access(|| {
                    (interpreter_rx.virtual_address as *const u8).read_volatile() == 0x7f
                        && (plan.initial_entry as *const u8).read_volatile() == 0x48
                        && ((interpreter_rx.virtual_page + PAGE_SIZE as u64) as *const u8)
                            .read_volatile()
                            == interpreter_rx.bytes[PAGE_SIZE]
                        && (interpreter_rw.virtual_address as *const u64).read_volatile()
                            == u64::from_le_bytes(*b"DYNAMIC!")
                        && core::slice::from_raw_parts(
                            (tls_virtual_page as usize + auxiliary_tls_start_offset) as *const u8,
                            auxiliary_tls.bytes.len(),
                        ) == auxiliary_tls.bytes
                        && core::slice::from_raw_parts(
                            (tls_virtual_page as usize
                                + auxiliary_tls_start_offset
                                + auxiliary_tls.bytes.len())
                                as *const u8,
                            auxiliary_tls_memory_size - auxiliary_tls.bytes.len(),
                        )
                        .iter()
                        .all(|byte| *byte == 0)
                        && core::slice::from_raw_parts(
                            (tls_virtual_page as usize + dependency_tls_start_offset) as *const u8,
                            dependency_tls.bytes.len(),
                        ) == dependency_tls.bytes
                        && core::slice::from_raw_parts(
                            (tls_virtual_page as usize
                                + dependency_tls_start_offset
                                + dependency_tls.bytes.len())
                                as *const u8,
                            dependency_tls_memory_size - dependency_tls.bytes.len(),
                        )
                        .iter()
                        .all(|byte| *byte == 0)
                        && core::slice::from_raw_parts(
                            (tls_virtual_page as usize + main_tls_start_offset) as *const u8,
                            main_tls.bytes.len(),
                        ) == main_tls.bytes
                        && core::slice::from_raw_parts(
                            (tls_virtual_page as usize
                                + main_tls_start_offset
                                + main_tls.bytes.len()) as *const u8,
                            main_tls_memory_size - main_tls.bytes.len(),
                        )
                        .iter()
                        .all(|byte| *byte == 0)
                        && (tls_base as *const u64).read_volatile() == tls_base
                        && ((tls_base + size_of::<u64>() as u64) as *const u64).read_volatile() == 0
                        && core::slice::from_raw_parts(
                            (sibling_tls_virtual_page as usize + tls_start_offset) as *const u8,
                            tls_extent_size - LINUX_TLS_TCB_SIZE,
                        ) == core::slice::from_raw_parts(
                            (tls_virtual_page as usize + tls_start_offset) as *const u8,
                            tls_extent_size - LINUX_TLS_TCB_SIZE,
                        )
                        && (sibling_tls_base as *const u64).read_volatile() == sibling_tls_base
                        && ((sibling_tls_base + size_of::<u64>() as u64) as *const u64)
                            .read_volatile()
                            == 0
                });
            let file_mapping = |segment: crate::linux_elf::PlannedSegment<'_>, page| {
                Some(cpu::LinuxFileMapping {
                    address: usize::try_from(
                        segment
                            .virtual_page
                            .checked_add((page * PAGE_SIZE) as u64)?,
                    )
                    .ok()?,
                    offset: segment.file_page.checked_add((page * PAGE_SIZE) as u64)?,
                    file_size: crate::linux_elf::planned_file_page_size(segment, page)?,
                    writable: segment.writable,
                    executable: segment.executable,
                })
            };
            let main_mappings = [
                file_mapping(main_ro, 0)?,
                file_mapping(main_rx, 0)?,
                file_mapping(main_rx, 1)?,
                file_mapping(main_rx, 2)?,
                file_mapping(main_rx, 3)?,
                file_mapping(main_rx, 4)?,
                file_mapping(main_rw, 0)?,
                file_mapping(main_rw, 1)?,
            ];
            let mut initial_image_addresses = [0_usize; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
            let mut initial_image_count = 0;
            for segment in [
                plan.interpreter.read_only,
                Some(interpreter_rx),
                Some(interpreter_rw),
            ]
            .into_iter()
            .flatten()
            {
                for page in 0..segment.page_count {
                    let address = segment
                        .virtual_page
                        .checked_add((page * PAGE_SIZE) as u64)
                        .and_then(|address| usize::try_from(address).ok())?;
                    let slot = initial_image_addresses.get_mut(initial_image_count)?;
                    *slot = address;
                    initial_image_count += 1;
                }
            }
            let mut initial_image_mappings =
                [cpu::LinuxActiveImageMapping::EMPTY; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
            for (index, address) in initial_image_addresses[..initial_image_count]
                .iter()
                .enumerate()
            {
                let entry = current_linux_leaf(*address as u64)?.read();
                let frame_address = entry & ADDRESS_MASK;
                let frame = frames
                    .iter()
                    .copied()
                    .find(|frame| frame.address() == frame_address)?;
                initial_image_mappings[index] = cpu::LinuxActiveImageMapping {
                    address: *address,
                    frame,
                    writable: entry & WRITABLE != 0,
                    executable: entry & NO_EXECUTE == 0,
                };
            }
            let execution_result = if observed {
                cpu::verify_linux_elf_entry(&cpu::LinuxElfExecution {
                    entry: startup.initial_entry as usize,
                    page_table_frames: [
                        frames[0], frames[1], frames[2], frames[3], frames[4], frames[5],
                    ],
                    page_table_frame_count: 6,
                    stack: (stack_virtual_page + stack_offset as u64) as usize,
                    stack_frame: frames[10],
                    heap_frame: frames[9],
                    tls_frames: [frames[11], frames[12]],
                    initial_fs_base: tls_base as usize,
                    tls: tls_virtual_page as usize + tls_start_offset,
                    tls_size: tls_extent_size,
                    sibling_fs_base: sibling_tls_base as usize,
                    sibling_tls: sibling_tls_virtual_page as usize + tls_start_offset,
                    sibling_stack: sibling_stack_virtual_page as usize,
                    sibling_stack_frame: frames[13],
                    code: main_rx.virtual_address as usize,
                    code_size: main_rx.memory_size,
                    loader_code: interpreter_rx.virtual_address as usize,
                    loader_code_size: interpreter_rx.memory_size,
                    data: main_rw.virtual_page as usize,
                    data_size: usize::try_from(
                        relro
                            .virtual_address
                            .checked_add(512)
                            .and_then(|end| end.checked_sub(main_rw.virtual_page))
                            .unwrap_or(0),
                    )
                    .unwrap_or(0),
                    relro: relro.virtual_address as usize,
                    relro_size: relro.memory_size as usize,
                    heap: heap_virtual_page as usize,
                    heap_size: PAGE_SIZE,
                    mmap: mmap_virtual_page as usize,
                    main_mappings,
                    dependency_base_min: dependency_base_min.map(|base| base as usize),
                    dependency_base_end: dependency_base_end.map(|base| base as usize),
                    dependency_mappings,
                    exec_expected: true,
                    initial_image_count,
                    initial_image_mappings,
                })
            } else {
                cpu::LinuxElfResult::FAILED
            };
            main_frames = execution_result.main_frames;
            dependency_frames = execution_result.dependency_frames;
            exec_frames = execution_result.exec_frames;
            if execution_result.exec_mapping_count <= cpu::LINUX_EXEC_MAPPING_LIMIT - 2 {
                exec_stack_frame = exec_frames[execution_result.exec_mapping_count];
                exec_frames[execution_result.exec_mapping_count] = None;
            }
            exec_heap_frame = cpu::take_linux_exec_heap_frame();
            exec_tls_frames = cpu::take_linux_exec_tls_frames();
            exec_sibling_stack_frame = cpu::take_linux_exec_sibling_stack_frame();
            if !cpu::finalize_linux_exec_page_table_frames(
                [
                    frames[0], frames[1], frames[2], frames[3], frames[4], frames[5],
                ],
                6,
                true,
            ) {
                return None;
            }
            exec_page_table_frames = cpu::take_linux_exec_page_table_frames();
            if !execution_result.valid || exec_page_table_frames.iter().any(Option::is_none) {
                return None;
            }
            main_ro_entries = mapped_frame::<u64>(exec_page_table_frames[3]?.address())?;
            main_entries = mapped_frame::<u64>(exec_page_table_frames[4]?.address())?;
            interpreter_entries = mapped_frame::<u64>(exec_page_table_frames[5]?.address())?;
            let expected_active_image_count = initial_image_count
                + main_mappings
                    .iter()
                    .filter(|mapping| mapping.address != 0)
                    .count()
                + dependency_mapping_count.iter().sum::<usize>();
            let executed = observed
                && execution_result.valid
                && execution_result.active_image_mapping_count == expected_active_image_count;
            let exec_stack_mapping_valid = executed
                && exec_stack_frame.is_some_and(|frame| {
                    main_entries.add(stack_indexes[3]).read()
                        & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                        == frame.address() | PRESENT | USER | WRITABLE | NO_EXECUTE
                        && frame != frames[10]
                        && !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frames[10]])
                });
            let exec_heap_mapping_valid = executed
                && exec_heap_frame.is_some_and(|frame| {
                    main_entries.add(heap_indexes[3]).read()
                        & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                        == frame.address() | PRESENT | USER | WRITABLE | NO_EXECUTE
                        && frame != frames[9]
                        && !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frames[9]])
                });
            let exec_tls_mappings_valid = executed
                && exec_tls_frames.iter().enumerate().all(|(index, frame)| {
                    frame.is_some_and(|frame| {
                        let old = frames[11 + index];
                        let virtual_page = if index == 0 {
                            tls_virtual_page
                        } else {
                            sibling_tls_virtual_page
                        };
                        main_entries.add(indexes(virtual_page)[3]).read()
                            & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                            == frame.address() | PRESENT | USER | WRITABLE | NO_EXECUTE
                            && frame != old
                            && !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[old])
                    })
                });
            let exec_sibling_stack_mapping_valid = executed
                && exec_sibling_stack_frame.is_some_and(|frame| {
                    main_entries.add(sibling_stack_indexes[3]).read()
                        & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                        == frame.address() | PRESENT | USER | WRITABLE | NO_EXECUTE
                        && frame != frames[13]
                        && !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frames[13]])
                });
            let mapping_removed = mmap_indexes
                .iter()
                .all(|indexes| main_entries.add(indexes[3]).read() == 0);
            let tls_runtime_valid = executed
                && main_entries.add(tls_indexes[3]).read()
                    & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == exec_tls_frames[0].map_or(0, memory::FrameLease::address)
                        | PRESENT
                        | USER
                        | WRITABLE
                        | NO_EXECUTE
                && cpu::with_supervisor_user_access(|| {
                    core::slice::from_raw_parts(
                        (tls_virtual_page as usize + auxiliary_tls_start_offset) as *const u8,
                        auxiliary_tls.bytes.len(),
                    ) == auxiliary_tls.bytes
                        && core::slice::from_raw_parts(
                            (tls_virtual_page as usize
                                + auxiliary_tls_start_offset
                                + auxiliary_tls.bytes.len())
                                as *const u8,
                            auxiliary_tls_memory_size - auxiliary_tls.bytes.len(),
                        )
                        .iter()
                        .all(|byte| *byte == 0)
                        && core::slice::from_raw_parts(
                            (tls_virtual_page as usize + dependency_tls_start_offset) as *const u8,
                            dependency_tls.bytes.len(),
                        ) == dependency_tls.bytes
                        && core::slice::from_raw_parts(
                            (tls_virtual_page as usize
                                + dependency_tls_start_offset
                                + dependency_tls.bytes.len())
                                as *const u8,
                            dependency_tls_memory_size - dependency_tls.bytes.len(),
                        )
                        .iter()
                        .all(|byte| *byte == 0)
                        && core::slice::from_raw_parts(
                            (tls_virtual_page as usize + main_tls_start_offset) as *const u8,
                            main_tls.bytes.len(),
                        ) == main_tls.bytes
                        && core::slice::from_raw_parts(
                            (tls_virtual_page as usize
                                + main_tls_start_offset
                                + main_tls.bytes.len()) as *const u8,
                            main_tls_memory_size - main_tls.bytes.len(),
                        )
                        .iter()
                        .all(|byte| *byte == 0)
                        && (tls_base as *const u64).read_volatile() == tls_base
                });
            let sibling_tls_runtime_valid = executed
                && main_entries.add(sibling_tls_indexes[3]).read()
                    & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == exec_tls_frames[1].map_or(0, memory::FrameLease::address)
                        | PRESENT
                        | USER
                        | WRITABLE
                        | NO_EXECUTE
                && cpu::with_supervisor_user_access(|| {
                    core::slice::from_raw_parts(
                        (sibling_tls_virtual_page as usize + tls_start_offset) as *const u8,
                        tls_extent_size - LINUX_TLS_TCB_SIZE,
                    ) == core::slice::from_raw_parts(
                        (tls_virtual_page as usize + tls_start_offset) as *const u8,
                        tls_extent_size - LINUX_TLS_TCB_SIZE,
                    ) && (sibling_tls_base as *const u64).read_volatile() == sibling_tls_base
                        && sibling_tls_base != tls_base
                });
            let sibling_stack_runtime_valid = executed
                && main_entries
                    .add(sibling_stack_lower_guard_indexes[3])
                    .read()
                    == 0
                && main_entries.add(sibling_stack_indexes[3]).read()
                    & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == exec_sibling_stack_frame.map_or(0, memory::FrameLease::address)
                        | PRESENT
                        | USER
                        | WRITABLE
                        | NO_EXECUTE
                && main_entries
                    .add(sibling_stack_upper_guard_indexes[3])
                    .read()
                    == 0
                && exec_sibling_stack_frame.is_some_and(|frame| {
                    mapped_frame::<u64>(frame.address()).is_some_and(|stack| {
                        (0..ENTRY_COUNT).all(|index| stack.add(index).read_volatile() == 0)
                    })
                });
            let main_frame_addresses = execution_result
                .main_frames
                .map(|frame| frame.map(memory::FrameLease::address).unwrap_or(0));
            let main_permissions_valid = main_ro_entries.add(main_ro_indexes[3]).read()
                & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                == main_frame_addresses[0] | PRESENT | USER | NO_EXECUTE
                && main_frame_addresses[1..6]
                    .iter()
                    .enumerate()
                    .all(|(offset, frame)| {
                        main_entries.add(main_indexes[3] + offset).read()
                            & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                            == *frame | PRESENT | USER
                    })
                && main_entries.add(main_rw_indexes[3]).read()
                    & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == main_frame_addresses[6] | PRESENT | USER | WRITABLE | NO_EXECUTE
                && main_entries.add(main_rw_indexes[3] + 1).read()
                    & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                    == main_frame_addresses[7] | PRESENT | USER | NO_EXECUTE;
            let selected_dependency_bases = execution_result.dependency_bases;
            let dependency_permissions_valid = executed
                && (0..2).all(|dependency| {
                    let selected_base = selected_dependency_bases[dependency];
                    let frame_addresses = execution_result.dependency_frames[dependency]
                        .map(|frame| frame.map(memory::FrameLease::address).unwrap_or(0));
                    selected_base >= dependency_base_min[dependency]
                        && selected_base < dependency_base_end[dependency]
                        && selected_base != dependency_plan.dependency_bases[dependency]
                        && selected_base.is_multiple_of(PAGE_SIZE as u64)
                        && dependency_mappings[dependency][..dependency_mapping_count[dependency]]
                            .iter()
                            .zip(&frame_addresses)
                            .all(|(mapping, frame)| {
                                let actual_page = selected_base
                                    .checked_add(mapping.relative_address as u64)
                                    .unwrap_or(0);
                                let expected = *frame
                                    | PRESENT
                                    | USER
                                    | if mapping.writable { WRITABLE } else { 0 }
                                    | if mapping.executable { 0 } else { NO_EXECUTE };
                                interpreter_entries.add(indexes(actual_page)[3]).read()
                                    & (ADDRESS_MASK | PRESENT | USER | WRITABLE | NO_EXECUTE)
                                    == expected
                            })
                });
            let selected_dependency_base = selected_dependency_bases[0];
            let interpreter_relocated = cpu::with_supervisor_user_access(|| {
                (interpreter_dynamic.relocation_target as *const u64).read_volatile()
                    == interpreter_relocation_value
            });
            let main_image_valid = executed
                && main_permissions_valid
                && cpu::with_supervisor_user_access(|| {
                    (main_ro.virtual_address as *const u8).read_volatile() == 0x7f
                        && (main_rx.virtual_address as *const u8).read_volatile()
                            == main_rx.bytes[0]
                        && (plan.main.entry as *const u8).read_volatile() == 0x48
                });
            let (
                main_preinit_value,
                main_relative_value,
                main_symbol_value,
                main_jump_value,
                main_auxiliary_symbol_value,
                main_auxiliary_jump_value,
            ) = if main_image_valid {
                cpu::with_supervisor_user_access(|| {
                    (
                        (main_preinit_target as *const u64).read_volatile(),
                        (main_dynamic.relocation_target as *const u64).read_volatile(),
                        (main_symbol.target as *const u64).read_volatile(),
                        (main_jump_slot.target as *const u64).read_volatile(),
                        (main_auxiliary_symbol.target as *const u64).read_volatile(),
                        (main_auxiliary_jump_slot.target as *const u64).read_volatile(),
                    )
                })
            } else {
                (0, 0, 0, 0, 0, 0)
            };
            let main_relative_relocated = main_preinit_value == main_preinit_relocation_value
                && main_relative_value == main_relocation_value;
            let relocate_dependency_address = |address: u64| {
                address
                    .checked_sub(dependency_plan.dependency_bases[0])
                    .and_then(|relative| selected_dependency_base.checked_add(relative))
            };
            let actual_dependency_entry =
                relocate_dependency_address(dependency_plan.dependencies[0].entry).unwrap_or(0);
            let actual_dependency_relative_value = selected_dependency_base
                .checked_add(dependency_dynamic.relocation_addend)
                .unwrap_or(0);
            let main_symbol_relocated = main_symbol_value == actual_dependency_entry;
            let main_jump_relocated = main_jump_value == actual_dependency_entry;
            let dependency_relocated = executed
                && dependency_permissions_valid
                && relocate_dependency_address(dependency_rx.virtual_address).is_some_and(
                    |actual_dependency_rx| {
                        relocate_dependency_address(dependency_dynamic.relocation_target)
                            .zip(relocate_dependency_address(dependency_symbol.target))
                            .zip(relocate_dependency_address(dependency_jump_slot.target))
                            .zip(relocate_dependency_address(
                                dependency_tls_relocation_target,
                            ))
                            .is_some_and(
                                |(((relative_target, symbol_target), jump_target), tls_target)| {
                                    cpu::with_supervisor_user_access(|| {
                                        (actual_dependency_rx as *const u8).read_volatile()
                                            == dependency_rx.bytes[0]
                                            && (actual_dependency_entry as *const u8)
                                                .read_volatile()
                                                == dependency_entry_byte
                                            && (relative_target as *const u64).read_volatile()
                                                == actual_dependency_relative_value
                                            && (symbol_target as *const u64).read_volatile()
                                                == actual_dependency_entry
                                            && (jump_target as *const u64).read_volatile()
                                                == actual_dependency_entry
                                            && (tls_target as *const u64).read_volatile()
                                                == dependency_tls_offset
                                    })
                                },
                            )
                    },
                );
            let selected_auxiliary_base = selected_dependency_bases[1];
            let relocate_auxiliary_address = |address: u64| {
                address
                    .checked_sub(dependency_plan.dependency_bases[1])
                    .and_then(|relative| selected_auxiliary_base.checked_add(relative))
            };
            let actual_auxiliary_entry =
                relocate_auxiliary_address(dependency_plan.dependencies[1].entry).unwrap_or(0);
            let main_auxiliary_symbol_relocated =
                main_auxiliary_symbol_value == actual_auxiliary_entry;
            let main_auxiliary_jump_relocated = main_auxiliary_jump_value == actual_auxiliary_entry;
            let main_relocated = main_relative_relocated
                && main_symbol_relocated
                && main_jump_relocated
                && main_auxiliary_symbol_relocated
                && main_auxiliary_jump_relocated;
            let actual_auxiliary_relative_value = selected_auxiliary_base
                .checked_add(auxiliary_dynamic.relocation_addend)
                .unwrap_or(0);
            let auxiliary_relocated = executed
                && dependency_permissions_valid
                && relocate_auxiliary_address(auxiliary_rx.virtual_address).is_some_and(
                    |actual_auxiliary_rx| {
                        relocate_auxiliary_address(auxiliary_dynamic.relocation_target)
                            .zip(relocate_auxiliary_address(auxiliary_symbol.target))
                            .zip(relocate_auxiliary_address(auxiliary_jump_slot.target))
                            .zip(relocate_auxiliary_address(auxiliary_tls_relocation_target))
                            .is_some_and(
                                |(((relative_target, symbol_target), jump_target), tls_target)| {
                                    cpu::with_supervisor_user_access(|| {
                                        (actual_auxiliary_rx as *const u8).read_volatile()
                                            == auxiliary_rx.bytes[0]
                                            && (actual_auxiliary_entry as *const u8).read_volatile()
                                                == auxiliary_entry_byte
                                            && (relative_target as *const u64).read_volatile()
                                                == actual_auxiliary_relative_value
                                            && (symbol_target as *const u64).read_volatile()
                                                == actual_auxiliary_entry
                                            && (jump_target as *const u64).read_volatile()
                                                == actual_auxiliary_entry
                                            && (tls_target as *const u64).read_volatile()
                                                == auxiliary_tls_offset
                                    })
                                },
                            )
                    },
                );
            activate(kernel_root_physical);
            if !permissions_valid {
                crate::serial_write_bytes(b"\r\nginux: linked dynamic permissions invalid");
            }
            if !executed {
                crate::serial_write_bytes(b"\r\nginux: linked dynamic workload incomplete");
            }
            if !mapping_removed {
                crate::serial_write_bytes(b"\r\nginux: linked dynamic mmap teardown incomplete");
            }
            if !tls_runtime_valid {
                crate::serial_write_bytes(b"\r\nginux: initial Linux TLS runtime invalid");
            }
            if !sibling_tls_runtime_valid {
                crate::serial_write_bytes(b"\r\nginux: sibling Linux TLS runtime invalid");
            }
            if !sibling_stack_runtime_valid {
                crate::serial_write_bytes(b"\r\nginux: sibling Linux stack runtime invalid");
            }
            if !exec_stack_mapping_valid {
                crate::serial_write_bytes(b"\r\nginux: replacement Linux stack invalid");
            }
            if !exec_heap_mapping_valid {
                crate::serial_write_bytes(b"\r\nginux: replacement Linux heap invalid");
            }
            if !exec_tls_mappings_valid {
                crate::serial_write_bytes(b"\r\nginux: replacement Linux TLS mappings invalid");
            }
            if !exec_sibling_stack_mapping_valid {
                crate::serial_write_bytes(b"\r\nginux: replacement Linux sibling stack invalid");
            }
            if !main_permissions_valid {
                crate::serial_write_bytes(b"\r\nginux: userspace main mapping invalid");
            }
            if !dependency_permissions_valid {
                crate::serial_write_bytes(b"\r\nginux: userspace dependency mapping invalid");
            }
            if !interpreter_relocated {
                crate::serial_write_bytes(b"\r\nginux: linked interpreter relocation mismatch");
            }
            if !main_relocated {
                crate::serial_write_bytes(b"\r\nginux: linked PIE relocation mismatch");
                if !main_relative_relocated {
                    crate::serial_write_bytes(b" relative");
                }
                if !main_symbol_relocated {
                    crate::serial_write_bytes(b" GLOB_DAT");
                }
                if !main_jump_relocated {
                    crate::serial_write_bytes(b" JUMP_SLOT");
                }
                if !main_auxiliary_symbol_relocated {
                    crate::serial_write_bytes(b" AUX_GLOB_DAT");
                }
                if !main_auxiliary_jump_relocated {
                    crate::serial_write_bytes(b" AUX_JUMP_SLOT");
                }
            }
            if !dependency_relocated {
                crate::serial_write_bytes(b"\r\nginux: linked DSO relocation mismatch");
            }
            if !auxiliary_relocated {
                crate::serial_write_bytes(b"\r\nginux: linked auxiliary DSO relocation mismatch");
            }
            Some(
                permissions_valid
                    && executed
                    && mapping_removed
                    && tls_runtime_valid
                    && sibling_tls_runtime_valid
                    && sibling_stack_runtime_valid
                    && exec_stack_mapping_valid
                    && exec_heap_mapping_valid
                    && exec_tls_mappings_valid
                    && exec_sibling_stack_mapping_valid
                    && main_permissions_valid
                    && main_image_valid
                    && dependency_permissions_valid
                    && interpreter_relocated
                    && main_relocated
                    && dependency_relocated
                    && auxiliary_relocated
                    && current_root() == kernel_root_physical,
            )
        }
    })()
    .unwrap_or(false);
    let cleared = frames.iter().enumerate().all(|(index, frame)| unsafe {
        if memory::frames_owned_by(memory::FrameOwner::KERNEL, &[*frame]) {
            zero_frame(frame.address()).is_some()
        } else if (index == 9 && exec_heap_frame.is_some())
            || (index < 6 && exec_page_table_frames[index].is_some())
            || (index == 10 && exec_stack_frame.is_some())
            || (index == 11 && exec_tls_frames[0].is_some())
            || (index == 12 && exec_tls_frames[1].is_some())
            || (index == 13 && exec_sibling_stack_frame.is_some())
        {
            true
        } else {
            frame_is_zero(frame.address())
        }
    });
    let sibling_resources_scrubbed = cleared
        && (exec_tls_frames[1].is_some() || unsafe { frame_is_zero(sibling_tls_page) })
        && (exec_sibling_stack_frame.is_some() || unsafe { frame_is_zero(sibling_stack_page) });
    let dependency_cleared = dependency_frames
        .iter()
        .flatten()
        .flatten()
        .all(|frame| unsafe { zero_frame(frame.address()).is_some() });
    let main_cleared = main_frames
        .iter()
        .flatten()
        .all(|frame| unsafe { zero_frame(frame.address()).is_some() });
    let mut exec_frames_released = true;
    for frame in exec_frames.into_iter().flatten() {
        exec_frames_released &= prepare_linux_frame(frame.address(), &[])
            && memory::release_frames(memory::FrameOwner::KERNEL, [frame]);
    }
    let exec_stack_frame_released = exec_stack_frame.is_some_and(|frame| {
        prepare_linux_frame(frame.address(), &[])
            && memory::release_frames(memory::FrameOwner::KERNEL, [frame])
    });
    let exec_heap_frame_released = exec_heap_frame.is_some_and(|frame| {
        prepare_linux_frame(frame.address(), &[])
            && memory::release_frames(memory::FrameOwner::KERNEL, [frame])
    });
    let exec_tls_frames_released = exec_tls_frames.into_iter().all(|frame| {
        frame.is_some_and(|frame| {
            prepare_linux_frame(frame.address(), &[])
                && memory::release_frames(memory::FrameOwner::KERNEL, [frame])
        })
    });
    let exec_sibling_stack_frame_released = exec_sibling_stack_frame.is_some_and(|frame| {
        prepare_linux_frame(frame.address(), &[])
            && memory::release_frames(memory::FrameOwner::KERNEL, [frame])
    });
    let exec_page_table_frames_released = exec_page_table_frames.into_iter().all(|frame| {
        frame.is_some_and(|frame| {
            prepare_linux_frame(frame.address(), &[])
                && memory::release_frames(memory::FrameOwner::KERNEL, [frame])
        })
    });
    let mut original_frames_released = sibling_resources_scrubbed;
    if sibling_resources_scrubbed {
        for frame in frames {
            if memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame]) {
                original_frames_released &=
                    memory::release_frames(memory::FrameOwner::KERNEL, [frame]);
            }
        }
    }
    let mut dependency_frames_released = dependency_cleared;
    if dependency_cleared {
        for frame in dependency_frames.into_iter().flatten().flatten() {
            dependency_frames_released &=
                memory::release_frames(memory::FrameOwner::KERNEL, [frame]);
        }
    }
    let mut main_frames_released = main_cleared;
    if main_cleared {
        for frame in main_frames.into_iter().flatten() {
            main_frames_released &= memory::release_frames(memory::FrameOwner::KERNEL, [frame]);
        }
    }
    let frame_count_restored =
        memory::owner_live_frame_count(memory::FrameOwner::KERNEL) == Some(kernel_frames_before);
    valid
        && original_frames_released
        && exec_frames_released
        && exec_stack_frame_released
        && exec_heap_frame_released
        && exec_tls_frames_released
        && exec_sibling_stack_frame_released
        && exec_page_table_frames_released
        && main_frames_released
        && dependency_frames_released
        && frame_count_restored
}

struct RegisteredAuxiliaryAddressSpace {
    domain: u64,
    space: WxAddressSpace,
}

pub(crate) fn expected_service_fault_instruction(service_entry_offset: usize) -> Option<u64> {
    USER_TEST_ADDRESS
        .checked_add(service_entry_offset)?
        .checked_add(EXPECTED_SERVICE_FAULT_OFFSET)
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

pub(crate) fn protect_current_linux_pages_read_only(address: u64, frames: &[u64]) -> bool {
    if !(1..=cpu::LINUX_MMAP_SLOT_COUNT).contains(&frames.len())
        || address >= 1 << 47
        || !address.is_multiple_of(PAGE_SIZE as u64)
        || frames
            .iter()
            .any(|frame| !frame.is_multiple_of(PAGE_SIZE as u64))
        || !memory::frame_addresses_owned_by(memory::FrameOwner::KERNEL, frames)
    {
        return false;
    }
    let mut leaves = [core::ptr::null_mut(); cpu::LINUX_MMAP_SLOT_COUNT];
    let mut values = [0_u64; cpu::LINUX_MMAP_SLOT_COUNT];
    for (index, frame) in frames.iter().enumerate() {
        let page = address + index as u64 * PAGE_SIZE as u64;
        let Some(leaf) = current_linux_leaf(page) else {
            return false;
        };
        let value = unsafe { leaf.read() };
        if value & ADDRESS_MASK != *frame
            || value & (PRESENT | WRITABLE | USER | NO_EXECUTE)
                != PRESENT | WRITABLE | USER | NO_EXECUTE
        {
            return false;
        }
        leaves[index] = leaf;
        values[index] = value;
    }
    for index in 0..frames.len() {
        let page = address + index as u64 * PAGE_SIZE as u64;
        unsafe {
            leaves[index].write(values[index] & !WRITABLE);
            asm!("invlpg [{}]", in(reg) page, options(nostack, preserves_flags));
        }
    }
    leaves[..frames.len()]
        .iter()
        .all(|leaf| unsafe { leaf.read() } & WRITABLE == 0)
}

pub(crate) fn replace_current_linux_pages(
    address: u64,
    old_frames: &[Option<memory::FrameLease>],
    old_writable: &[bool],
    new_frames: &[memory::FrameLease],
    new_writable: bool,
) -> bool {
    if !(1..=cpu::LINUX_MMAP_SLOT_COUNT).contains(&new_frames.len())
        || old_frames.len() != new_frames.len()
        || old_writable.len() != new_frames.len()
        || address >= 1 << 47
        || !address.is_multiple_of(PAGE_SIZE as u64)
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, new_frames)
        || old_frames.iter().flatten().any(|frame| {
            !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[*frame])
                || new_frames.contains(frame)
        })
    {
        return false;
    }
    let mut leaves = [core::ptr::null_mut(); cpu::LINUX_MMAP_SLOT_COUNT];
    for index in 0..new_frames.len() {
        let page = address + (index * PAGE_SIZE) as u64;
        let Some(leaf) = current_linux_leaf(page) else {
            return false;
        };
        let value = unsafe { leaf.read() };
        let valid = old_frames[index].map_or(value == 0, |frame| {
            value & ADDRESS_MASK == frame.address()
                && value & (PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == PRESENT | USER | NO_EXECUTE | if old_writable[index] { WRITABLE } else { 0 }
        });
        if !valid {
            return false;
        }
        leaves[index] = leaf;
    }
    let new_flags = PRESENT | USER | NO_EXECUTE | if new_writable { WRITABLE } else { 0 };
    for (index, frame) in new_frames.iter().enumerate() {
        let page = address + (index * PAGE_SIZE) as u64;
        unsafe {
            leaves[index].write(frame.address() | new_flags);
            asm!("invlpg [{}]", in(reg) page, options(nostack, preserves_flags));
        }
    }
    leaves[..new_frames.len()]
        .iter()
        .zip(new_frames)
        .all(|(leaf, frame)| {
            (unsafe { leaf.read() } & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE))
                == (frame.address() | new_flags)
        })
}

pub(crate) fn reserve_current_linux_pages(
    address: u64,
    old_frames: &[Option<memory::FrameLease>],
    old_writable: &[bool],
) -> bool {
    if !(1..=cpu::LINUX_MMAP_SLOT_COUNT).contains(&old_frames.len())
        || old_writable.len() != old_frames.len()
        || address >= 1 << 47
        || !address.is_multiple_of(PAGE_SIZE as u64)
        || old_frames
            .iter()
            .flatten()
            .any(|frame| !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[*frame]))
    {
        return false;
    }
    let mut leaves = [core::ptr::null_mut(); cpu::LINUX_MMAP_SLOT_COUNT];
    let mut values = [0_u64; cpu::LINUX_MMAP_SLOT_COUNT];
    for index in 0..old_frames.len() {
        let page = address + (index * PAGE_SIZE) as u64;
        let Some(leaf) = current_linux_leaf(page) else {
            return false;
        };
        let value = unsafe { leaf.read() };
        let valid = old_frames[index].map_or(value == 0, |frame| {
            value & ADDRESS_MASK == frame.address()
                && value & (PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == PRESENT | USER | NO_EXECUTE | if old_writable[index] { WRITABLE } else { 0 }
        });
        if !valid {
            return false;
        }
        leaves[index] = leaf;
        values[index] = value;
    }
    for (index, leaf) in leaves[..old_frames.len()].iter().enumerate() {
        let page = address + (index * PAGE_SIZE) as u64;
        unsafe {
            leaf.write(0);
            asm!("invlpg [{}]", in(reg) page, options(nostack, preserves_flags));
        }
    }
    if leaves[..old_frames.len()]
        .iter()
        .all(|leaf| unsafe { leaf.read() == 0 })
    {
        return true;
    }
    for (index, leaf) in leaves[..old_frames.len()].iter().enumerate() {
        let page = address + (index * PAGE_SIZE) as u64;
        unsafe {
            leaf.write(values[index]);
            asm!("invlpg [{}]", in(reg) page, options(nostack, preserves_flags));
        }
    }
    false
}

pub(crate) fn map_current_linux_pages(
    address: u64,
    frames: &[memory::FrameLease],
    writable: bool,
) -> bool {
    if !(1..=cpu::LINUX_MMAP_SLOT_COUNT).contains(&frames.len())
        || address >= 1 << 47
        || !address.is_multiple_of(PAGE_SIZE as u64)
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, frames)
    {
        return false;
    }
    let mut leaves = [core::ptr::null_mut(); cpu::LINUX_MMAP_SLOT_COUNT];
    for (index, stored_leaf) in leaves.iter_mut().enumerate().take(frames.len()) {
        let Some(leaf) = current_linux_leaf(address + (index * PAGE_SIZE) as u64) else {
            return false;
        };
        if unsafe { leaf.read() } != 0 {
            return false;
        }
        *stored_leaf = leaf;
    }
    let flags = PRESENT | USER | NO_EXECUTE | if writable { WRITABLE } else { 0 };
    for (index, frame) in frames.iter().enumerate() {
        let page = address + (index * PAGE_SIZE) as u64;
        unsafe {
            leaves[index].write(frame.address() | flags);
            asm!("invlpg [{}]", in(reg) page, options(nostack, preserves_flags));
        }
    }
    let installed = leaves[..frames.len()]
        .iter()
        .zip(frames)
        .all(|(leaf, frame)| unsafe {
            leaf.read() & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE)
                == frame.address() | flags
        });
    if installed {
        return true;
    }
    for (index, leaf) in leaves[..frames.len()].iter().enumerate() {
        let page = address + (index * PAGE_SIZE) as u64;
        unsafe {
            leaf.write(0);
            asm!("invlpg [{}]", in(reg) page, options(nostack, preserves_flags));
        }
    }
    false
}

pub(crate) fn map_current_linux_page(address: u64, frame: u64) -> bool {
    map_current_linux_page_with_permissions(address, frame, true, false)
}

pub(crate) fn map_current_linux_read_only_page(address: u64, frame: u64) -> bool {
    map_current_linux_page_with_permissions(address, frame, false, false)
}

pub(crate) fn map_current_linux_file_page(
    address: u64,
    frame: u64,
    writable: bool,
    executable: bool,
) -> bool {
    if writable && executable {
        return false;
    }
    map_current_linux_page_with_permissions(address, frame, writable, executable)
}

fn current_linux_tls_frames<'main, 'interpreter, 'dependency>(
    dependency_plan: &crate::linux_elf::DependencyGraphLoadPlan<'main, 'interpreter, 'dependency>,
    thread_pointers: [u64; 2],
) -> Option<(
    crate::linux_elf::StaticTlsLayout,
    crate::linux_elf::TlsTemplate<'main>,
    [crate::linux_elf::TlsTemplate<'dependency>; 2],
    [u64; 2],
)> {
    let main = dependency_plan.process.main.tls?;
    let dependencies = [
        dependency_plan.dependencies[0].tls?,
        dependency_plan.dependencies[1].tls?,
    ];
    let layout = crate::linux_elf::plan_static_tls_layout(
        main,
        dependencies,
        LINUX_TLS_THREAD_POINTER_OFFSET,
        LINUX_TLS_TCB_SIZE,
        PAGE_SIZE,
    )?;
    let mut frames = [0_u64; 2];
    for (index, thread_pointer) in thread_pointers.into_iter().enumerate() {
        let page = thread_pointer & !(PAGE_SIZE as u64 - 1);
        if thread_pointer != page.checked_add(layout.thread_pointer_offset as u64)? {
            return None;
        }
        let entry = unsafe { current_linux_leaf(page)?.read() };
        if entry & (PRESENT | WRITABLE | USER | NO_EXECUTE)
            != PRESENT | WRITABLE | USER | NO_EXECUTE
        {
            return None;
        }
        frames[index] = entry & ADDRESS_MASK;
        if !memory::frame_addresses_owned_by(memory::FrameOwner::KERNEL, &[frames[index]])
            || mapped_frame::<u8>(frames[index]).is_none()
        {
            return None;
        }
    }
    if frames[0] == frames[1] {
        return None;
    }
    Some((layout, main, dependencies, frames))
}

pub(crate) fn current_linux_tls_rematerializable(
    dependency_plan: &crate::linux_elf::DependencyGraphLoadPlan<'_, '_, '_>,
    thread_pointers: [u64; 2],
) -> bool {
    current_linux_tls_frames(dependency_plan, thread_pointers).is_some()
}

pub(crate) fn materialize_linux_exec_tls_frames(
    dependency_plan: &crate::linux_elf::DependencyGraphLoadPlan<'_, '_, '_>,
    thread_pointers: [u64; 2],
    frames: [memory::FrameLease; 2],
) -> bool {
    if frames[0] == frames[1] || !memory::frames_owned_by(memory::FrameOwner::KERNEL, &frames) {
        return false;
    }
    let Some((layout, main, dependencies, _)) =
        current_linux_tls_frames(dependency_plan, thread_pointers)
    else {
        return false;
    };
    for (frame, thread_pointer) in frames.into_iter().zip(thread_pointers) {
        let Some(address) = frame.direct_map_address() else {
            return false;
        };
        // SAFETY: each generation-tagged frame is live, unique, and private to
        // the pending exec transaction.
        let page = unsafe { &mut *(address as *mut [u8; PAGE_SIZE]) };
        if !crate::linux_elf::materialize_static_tls_page(
            layout,
            main,
            dependencies,
            thread_pointer,
            page,
        ) {
            return false;
        }
    }
    true
}

pub(crate) fn replace_current_linux_image_and_hierarchy(
    mappings: &[cpu::LinuxExecMapping],
    old_mappings: &[cpu::LinuxActiveImageMapping],
    old_hierarchy: &[memory::FrameLease],
    new_hierarchy: &[memory::FrameLease],
) -> bool {
    if old_hierarchy.len() != new_hierarchy.len()
        || !(5..=6).contains(&old_hierarchy.len())
        || current_root() != old_hierarchy[0].address()
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, old_hierarchy)
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, new_hierarchy)
        || old_hierarchy.iter().enumerate().any(|(index, frame)| {
            old_hierarchy[..index].contains(frame) || new_hierarchy.contains(frame)
        })
        || new_hierarchy
            .iter()
            .enumerate()
            .any(|(index, frame)| new_hierarchy[..index].contains(frame))
        || old_mappings.iter().any(|mapping| {
            old_hierarchy.contains(&mapping.frame) || new_hierarchy.contains(&mapping.frame)
        })
    {
        return false;
    }
    let Some(stale_generation) = old_hierarchy[0].generation().checked_add(1) else {
        return false;
    };
    let Some(stale_root) =
        memory::FrameLease::from_parts(old_hierarchy[0].address(), stale_generation)
    else {
        return false;
    };
    if memory::frames_owned_by(memory::FrameOwner::KERNEL, &[stale_root])
        || prepare_linux_hierarchy_replacement(
            mappings,
            old_mappings,
            old_hierarchy,
            new_hierarchy,
            true,
        )
        || current_root() != old_hierarchy[0].address()
    {
        return false;
    }
    if !prepare_linux_hierarchy_replacement(
        mappings,
        old_mappings,
        old_hierarchy,
        new_hierarchy,
        false,
    ) {
        return false;
    }
    let mut retired_mappings = [memory::FrameLease::EMPTY; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    for (output, mapping) in retired_mappings.iter_mut().zip(old_mappings) {
        *output = mapping.frame;
    }
    let retired_mappings = &retired_mappings[..old_mappings.len()];
    if !memory::frame_leases_releasable(memory::FrameOwner::KERNEL, retired_mappings)
        || !memory::frame_leases_releasable(memory::FrameOwner::KERNEL, old_hierarchy)
    {
        return false;
    }
    unsafe { activate(new_hierarchy[0].address()) };
    if current_root() != new_hierarchy[0].address() {
        unsafe { activate(old_hierarchy[0].address()) };
        return false;
    }
    memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, retired_mappings)
        && memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, old_hierarchy)
}

fn prepare_linux_hierarchy_replacement(
    mappings: &[cpu::LinuxExecMapping],
    old_mappings: &[cpu::LinuxActiveImageMapping],
    old_hierarchy: &[memory::FrameLease],
    new_hierarchy: &[memory::FrameLease],
    force_verification_failure: bool,
) -> bool {
    if mappings.is_empty()
        || mappings.len() > cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT
        || old_mappings.is_empty()
        || old_mappings.len() > cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT
        || mappings.iter().enumerate().any(|(index, mapping)| {
            mappings[..index]
                .iter()
                .any(|prior| prior.address == mapping.address || prior.frame == mapping.frame)
                || new_hierarchy
                    .iter()
                    .any(|frame| frame.address() == mapping.frame)
        })
        || old_mappings.iter().enumerate().any(|(index, mapping)| {
            old_mappings[..index]
                .iter()
                .any(|prior| prior.address == mapping.address || prior.frame == mapping.frame)
        })
    {
        return false;
    }
    for (old, new) in old_hierarchy
        .iter()
        .copied()
        .zip(new_hierarchy.iter().copied())
    {
        let (Some(source), Some(destination)) = (
            mapped_frame::<u64>(old.address()),
            mapped_frame::<u64>(new.address()),
        ) else {
            return false;
        };
        unsafe { core::ptr::copy_nonoverlapping(source, destination, ENTRY_COUNT) };
    }
    for frame in new_hierarchy.iter().copied() {
        let Some(entries) = mapped_frame::<u64>(frame.address()) else {
            return false;
        };
        for index in 0..ENTRY_COUNT {
            let value = unsafe { entries.add(index).read() };
            if value & PRESENT == 0 {
                continue;
            }
            if let Some(position) = old_hierarchy
                .iter()
                .position(|old| old.address() == value & ADDRESS_MASK)
            {
                unsafe {
                    entries
                        .add(index)
                        .write((value & !ADDRESS_MASK) | new_hierarchy[position].address());
                }
            }
        }
    }
    for mapping in old_mappings {
        let Some(entry) = linux_leaf_in_root(new_hierarchy[0].address(), mapping.address as u64)
        else {
            return false;
        };
        let expected = mapping.frame.address()
            | PRESENT
            | USER
            | if mapping.writable { WRITABLE } else { 0 }
            | if mapping.executable { 0 } else { NO_EXECUTE };
        if unsafe { entry.read() } & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE)
            != expected
        {
            return false;
        }
        unsafe { entry.write(0) };
    }
    for mapping in mappings {
        if mapping.address == 0
            || !(mapping.address as u64).is_multiple_of(PAGE_SIZE as u64)
            || !mapping.frame.is_multiple_of(PAGE_SIZE as u64)
            || mapping.writable && mapping.executable
            || !memory::frame_addresses_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
        {
            return false;
        }
        let Some(entry) = linux_leaf_in_root(new_hierarchy[0].address(), mapping.address as u64)
        else {
            return false;
        };
        let flags = PRESENT
            | USER
            | if mapping.writable { WRITABLE } else { 0 }
            | if mapping.executable { 0 } else { NO_EXECUTE };
        unsafe { entry.write(mapping.frame | flags) };
    }
    !force_verification_failure
        && mappings.iter().all(|mapping| {
            let flags = PRESENT
                | USER
                | if mapping.writable { WRITABLE } else { 0 }
                | if mapping.executable { 0 } else { NO_EXECUTE };
            linux_leaf_in_root(new_hierarchy[0].address(), mapping.address as u64).is_some_and(
                |entry| unsafe {
                    entry.read() & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE)
                        == mapping.frame | flags
                },
            )
        })
        && old_mappings.iter().all(|mapping| {
            mappings
                .iter()
                .any(|replacement| replacement.address == mapping.address)
                || linux_leaf_in_root(new_hierarchy[0].address(), mapping.address as u64)
                    .is_some_and(|entry| unsafe { entry.read() == 0 })
        })
}

pub(crate) fn clone_linux_process_hierarchy(
    parent_mappings: &[cpu::LinuxActiveImageMapping],
    parent_hierarchy: &[memory::FrameLease],
    child_hierarchy: &[memory::FrameLease],
    child_pages: &[memory::FrameLease],
) -> bool {
    if parent_mappings.is_empty()
        || parent_mappings.len() != child_pages.len()
        || parent_mappings.len() > cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT
        || !(4..=6).contains(&parent_hierarchy.len())
        || parent_hierarchy.len() != child_hierarchy.len()
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, parent_hierarchy)
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, child_hierarchy)
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, child_pages)
        || parent_hierarchy.iter().enumerate().any(|(index, frame)| {
            parent_hierarchy[..index].contains(frame)
                || child_hierarchy.contains(frame)
                || child_pages.contains(frame)
        })
        || child_hierarchy.iter().enumerate().any(|(index, frame)| {
            child_hierarchy[..index].contains(frame) || child_pages.contains(frame)
        })
        || child_pages
            .iter()
            .enumerate()
            .any(|(index, frame)| child_pages[..index].contains(frame))
        || parent_mappings.iter().enumerate().any(|(index, mapping)| {
            mapping.address == 0
                || !mapping.address.is_multiple_of(PAGE_SIZE)
                || mapping.writable && mapping.executable
                || !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
                || parent_hierarchy.contains(&mapping.frame)
                || child_hierarchy.contains(&mapping.frame)
                || child_pages.contains(&mapping.frame)
                || parent_mappings[..index]
                    .iter()
                    .any(|prior| prior.address == mapping.address || prior.frame == mapping.frame)
        })
    {
        return false;
    }
    let mut visited_hierarchy = [false; 6];
    let mut visited_mappings = [false; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    if !validate_complete_linux_process_user_tree(
        parent_hierarchy[0].address(),
        0,
        0,
        parent_hierarchy,
        parent_mappings,
        &mut visited_hierarchy,
        &mut visited_mappings,
    ) || visited_hierarchy[..parent_hierarchy.len()]
        .iter()
        .any(|visited| !visited)
        || visited_mappings[..parent_mappings.len()]
            .iter()
            .any(|visited| !visited)
    {
        return false;
    }

    let mut child_mappings = [cpu::LinuxExecMapping {
        address: 0,
        frame: 0,
        writable: false,
        executable: false,
    }; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    for ((child, parent), frame) in child_mappings
        .iter_mut()
        .zip(parent_mappings)
        .zip(child_pages)
    {
        *child = cpu::LinuxExecMapping {
            address: parent.address,
            frame: frame.address(),
            writable: parent.writable,
            executable: parent.executable,
        };
    }
    prepare_linux_hierarchy_replacement(
        &child_mappings[..parent_mappings.len()],
        parent_mappings,
        parent_hierarchy,
        child_hierarchy,
        false,
    )
}

pub(crate) fn validate_linux_process_address_space(
    hierarchy: &[memory::FrameLease],
    mappings: &[cpu::LinuxActiveImageMapping],
) -> bool {
    if mappings.is_empty()
        || mappings.len() > cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT
        || !(4..=6).contains(&hierarchy.len())
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, hierarchy)
        || hierarchy
            .iter()
            .enumerate()
            .any(|(index, frame)| hierarchy[..index].contains(frame))
        || mappings.iter().enumerate().any(|(index, mapping)| {
            mapping.address == 0
                || !mapping.address.is_multiple_of(PAGE_SIZE)
                || mapping.writable && mapping.executable
                || !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
                || hierarchy.contains(&mapping.frame)
                || mappings[..index]
                    .iter()
                    .any(|prior| prior.address == mapping.address || prior.frame == mapping.frame)
        })
    {
        return false;
    }
    let mut visited_hierarchy = [false; 6];
    let mut visited_mappings = [false; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    validate_complete_linux_process_user_tree(
        hierarchy[0].address(),
        0,
        0,
        hierarchy,
        mappings,
        &mut visited_hierarchy,
        &mut visited_mappings,
    ) && visited_hierarchy[..hierarchy.len()]
        .iter()
        .all(|visited| *visited)
        && visited_mappings[..mappings.len()]
            .iter()
            .all(|visited| *visited)
}

pub(crate) fn inherit_linux_process_kernel_mappings(root: memory::FrameLease) -> bool {
    if !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[root])
        || root.address() == current_root()
    {
        return false;
    }
    let (Some(kernel), Some(process)) = (
        mapped_frame::<u64>(current_root()),
        mapped_frame::<u64>(root.address()),
    ) else {
        return false;
    };
    let indexes = [0_usize, 256];
    let values = indexes.map(|index| unsafe { kernel.add(index).read() });
    if values
        .iter()
        .any(|value| value & PRESENT == 0 || value & USER != 0)
    {
        return false;
    }
    for (index, value) in indexes.into_iter().zip(values) {
        unsafe { process.add(index).write(value) };
    }
    indexes.into_iter().zip(values).all(|(index, value)| {
        let actual = unsafe { process.add(index).read() };
        actual & !ACCESSED == value & !ACCESSED && actual & USER == 0
    })
}

pub(crate) fn verify_linux_process_root_activation(root: memory::FrameLease) -> bool {
    let previous = current_root();
    let rflags: u64;
    unsafe { asm!("pushfq", "pop {}", out(reg) rflags, options(preserves_flags)) };
    if !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[root])
        || root.address() == previous
        || rflags & (1 << 9) != 0
    {
        return false;
    }
    let (Some(kernel), Some(process)) = (
        mapped_frame::<u64>(previous),
        mapped_frame::<u64>(root.address()),
    ) else {
        return false;
    };
    if [0_usize, 256].into_iter().any(|index| {
        let expected = unsafe { kernel.add(index).read() };
        let candidate = unsafe { process.add(index).read() };
        expected & PRESENT == 0
            || expected & USER != 0
            || candidate & !ACCESSED != expected & !ACCESSED
    }) {
        return false;
    }
    unsafe { activate(root.address()) };
    let selected = current_root() == root.address();
    let process_visible = mapped_frame::<u64>(root.address()).is_some_and(|active| {
        [0_usize, 256]
            .into_iter()
            .all(|index| unsafe { active.add(index).read() } & USER == 0)
    });
    unsafe { activate(previous) };
    selected && process_visible && current_root() == previous
}

pub(crate) fn execute_linux_process_child(
    root: memory::FrameLease,
    entry: u64,
    stack: u64,
    fs_base: u64,
    general: [u64; 15],
) -> bool {
    let previous = current_root();
    let (Ok(entry), Ok(stack)) = (usize::try_from(entry), usize::try_from(stack)) else {
        return false;
    };
    if !verify_linux_process_root_activation(root) || current_root() != previous {
        return false;
    }
    unsafe { activate(root.address()) };
    let selected = current_root() == root.address();
    let executed = selected && cpu::execute_linux_process_child(entry, stack, fs_base, general);
    unsafe { activate(previous) };
    selected && executed && current_root() == previous
}

fn validate_complete_linux_process_user_tree(
    table: u64,
    level: usize,
    prefix: u64,
    hierarchy: &[memory::FrameLease],
    mappings: &[cpu::LinuxActiveImageMapping],
    visited_hierarchy: &mut [bool; 6],
    visited_mappings: &mut [bool; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT],
) -> bool {
    let Some(hierarchy_index) = hierarchy.iter().position(|frame| frame.address() == table) else {
        return false;
    };
    if level > 3 || visited_hierarchy[hierarchy_index] {
        return false;
    }
    visited_hierarchy[hierarchy_index] = true;
    let Some(entries) = mapped_frame::<u64>(table) else {
        return false;
    };
    let shift = [39_u32, 30, 21, 12][level];
    for index in 0..ENTRY_COUNT {
        let value = unsafe { entries.add(index).read() };
        if value & (PRESENT | USER) != PRESENT | USER {
            continue;
        }
        if level == 0 && index >= 256 {
            return false;
        }
        let address = prefix | (index as u64) << shift;
        if level < 3 {
            if value & LARGE_PAGE != 0
                || !validate_complete_linux_process_user_tree(
                    value & ADDRESS_MASK,
                    level + 1,
                    address,
                    hierarchy,
                    mappings,
                    visited_hierarchy,
                    visited_mappings,
                )
            {
                return false;
            }
            continue;
        }
        let Some(mapping_index) = mappings
            .iter()
            .position(|mapping| mapping.address as u64 == address)
        else {
            return false;
        };
        let mapping = mappings[mapping_index];
        let expected = mapping.frame.address()
            | PRESENT
            | USER
            | if mapping.writable { WRITABLE } else { 0 }
            | if mapping.executable { 0 } else { NO_EXECUTE };
        if visited_mappings[mapping_index]
            || value & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE) != expected
        {
            return false;
        }
        visited_mappings[mapping_index] = true;
    }
    true
}

pub(crate) fn verify_linux_process_hierarchy_clone() -> bool {
    const HIERARCHY_FRAMES: usize = 4;
    const PRIVATE_PAGES: usize = 2;
    const TOTAL_FRAMES: usize = HIERARCHY_FRAMES + PRIVATE_PAGES;
    let Some(before) = memory::owner_live_frame_count(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let Some(parent) = memory::allocate_frames::<TOTAL_FRAMES>(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let Some(child) = memory::allocate_frames::<TOTAL_FRAMES>(memory::FrameOwner::KERNEL) else {
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &parent);
        return false;
    };
    let parent_hierarchy: [memory::FrameLease; HIERARCHY_FRAMES] =
        core::array::from_fn(|index| parent[index]);
    let parent_pages: [memory::FrameLease; PRIVATE_PAGES] =
        core::array::from_fn(|index| parent[HIERARCHY_FRAMES + index]);
    let child_hierarchy: [memory::FrameLease; HIERARCHY_FRAMES] =
        core::array::from_fn(|index| child[index]);
    let child_pages: [memory::FrameLease; PRIVATE_PAGES] =
        core::array::from_fn(|index| child[HIERARCHY_FRAMES + index]);

    let result = (|| -> Option<bool> {
        let [parent_root, parent_pdpt, parent_directory, parent_table] =
            parent_hierarchy.map(memory::FrameLease::address);
        let root = mapped_frame::<u64>(parent_root)?;
        let pdpt = mapped_frame::<u64>(parent_pdpt)?;
        let directory = mapped_frame::<u64>(parent_directory)?;
        let table = mapped_frame::<u64>(parent_table)?;
        unsafe {
            root.add(1).write(parent_pdpt | PRESENT | WRITABLE | USER);
            pdpt.write(parent_directory | PRESENT | WRITABLE | USER);
            directory.write(parent_table | PRESENT | WRITABLE | USER);
            table.write(parent_pages[0].address() | PRESENT | USER);
            table
                .add(1)
                .write(parent_pages[1].address() | PRESENT | WRITABLE | USER | NO_EXECUTE);
            (parent_pages[0].direct_map_address()? as *mut u8).write(0x31);
            (parent_pages[1].direct_map_address()? as *mut u8).write(0x32);
        }
        let parent_mappings = [
            cpu::LinuxActiveImageMapping {
                address: USER_TEST_ADDRESS,
                frame: parent_pages[0],
                writable: false,
                executable: true,
            },
            cpu::LinuxActiveImageMapping {
                address: USER_TEST_ADDRESS + PAGE_SIZE,
                frame: parent_pages[1],
                writable: true,
                executable: false,
            },
        ];

        let stale_generation = child_hierarchy[0].generation().checked_add(1)?;
        let mut stale_hierarchy = child_hierarchy;
        stale_hierarchy[0] =
            memory::FrameLease::from_parts(child_hierarchy[0].address(), stale_generation)?;
        if clone_linux_process_hierarchy(
            &parent_mappings,
            &parent_hierarchy,
            &stale_hierarchy,
            &child_pages,
        ) || clone_linux_process_hierarchy(
            &parent_mappings[..1],
            &parent_hierarchy,
            &child_hierarchy,
            &child_pages[..1],
        ) || clone_linux_process_hierarchy(
            &parent_mappings,
            &parent_hierarchy,
            &child_hierarchy,
            &parent_pages,
        ) || child_hierarchy.iter().any(|frame| {
            frame.direct_map_address().is_none_or(|address| unsafe {
                core::slice::from_raw_parts(address as *const u8, PAGE_SIZE)
                    .iter()
                    .any(|byte| *byte != 0)
            })
        }) || !crate::linux_process::copy_private_pages(&parent_pages, &child_pages)
            || !clone_linux_process_hierarchy(
                &parent_mappings,
                &parent_hierarchy,
                &child_hierarchy,
                &child_pages,
            )
        {
            return Some(false);
        }

        let child_root = child_hierarchy[0].address();
        let code = unsafe { linux_leaf_in_root(child_root, USER_TEST_ADDRESS as u64)?.read() };
        let data = unsafe {
            linux_leaf_in_root(child_root, (USER_TEST_ADDRESS + PAGE_SIZE) as u64)?.read()
        };
        let parent_code =
            unsafe { linux_leaf_in_root(parent_root, USER_TEST_ADDRESS as u64)?.read() };
        let parent_data = unsafe {
            linux_leaf_in_root(parent_root, (USER_TEST_ADDRESS + PAGE_SIZE) as u64)?.read()
        };
        let child_code_address = child_pages[0].direct_map_address()?;
        let parent_code_address = parent_pages[0].direct_map_address()?;
        unsafe { (child_code_address as *mut u8).write(0x7f) };

        Some(
            code & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE)
                == child_pages[0].address() | PRESENT | USER
                && data & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == child_pages[1].address() | PRESENT | WRITABLE | USER | NO_EXECUTE
                && parent_code & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == parent_pages[0].address() | PRESENT | USER
                && parent_data & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == parent_pages[1].address() | PRESENT | WRITABLE | USER | NO_EXECUTE
                && unsafe { (parent_code_address as *const u8).read() } == 0x31,
        )
    })()
    .unwrap_or(false);

    let child_released = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &child);
    let parent_released = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &parent);
    result
        && child_released
        && parent_released
        && memory::owner_live_frame_count(memory::FrameOwner::KERNEL) == Some(before)
}

fn verify_current_linux_image_rollback(
    mappings: &[cpu::LinuxExecMapping],
    old_mappings: &[cpu::LinuxActiveImageMapping],
) -> bool {
    let mut mapping_entries = [0_u64; cpu::LINUX_EXEC_MAPPING_LIMIT];
    let mut clear_entries = [0_u64; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    for (index, mapping) in mappings.iter().enumerate() {
        let Some(entry) = current_linux_leaf(mapping.address as u64) else {
            return false;
        };
        mapping_entries[index] = unsafe { entry.read() };
    }
    for (index, mapping) in old_mappings.iter().enumerate() {
        let Some(entry) = current_linux_leaf(mapping.address as u64) else {
            return false;
        };
        clear_entries[index] = unsafe { entry.read() };
    }
    let rollback_rejected = !replace_current_linux_image_inner(mappings, old_mappings, true);
    let rollback_restored = mappings.iter().enumerate().all(|(index, mapping)| {
        current_linux_leaf(mapping.address as u64)
            .is_some_and(|entry| unsafe { entry.read() } == mapping_entries[index])
    }) && old_mappings.iter().enumerate().all(|(index, mapping)| {
        current_linux_leaf(mapping.address as u64)
            .is_some_and(|entry| unsafe { entry.read() } == clear_entries[index])
            && memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
    });
    let mut stale_mappings =
        [cpu::LinuxActiveImageMapping::EMPTY; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    stale_mappings[..old_mappings.len()].copy_from_slice(old_mappings);
    let Some(stale_generation) = stale_mappings[0].frame.generation().checked_add(1) else {
        return false;
    };
    let Some(stale_frame) =
        memory::FrameLease::from_parts(stale_mappings[0].frame.address(), stale_generation)
    else {
        return false;
    };
    stale_mappings[0].frame = stale_frame;
    let stale_rejected =
        !replace_current_linux_image_inner(mappings, &stale_mappings[..old_mappings.len()], false)
            && mappings.iter().enumerate().all(|(index, mapping)| {
                current_linux_leaf(mapping.address as u64)
                    .is_some_and(|entry| unsafe { entry.read() } == mapping_entries[index])
            })
            && old_mappings.iter().enumerate().all(|(index, mapping)| {
                current_linux_leaf(mapping.address as u64)
                    .is_some_and(|entry| unsafe { entry.read() } == clear_entries[index])
                    && memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
            });
    rollback_rejected && rollback_restored && stale_rejected
}

fn replace_current_linux_image_inner(
    mappings: &[cpu::LinuxExecMapping],
    old_mappings: &[cpu::LinuxActiveImageMapping],
    force_verification_failure: bool,
) -> bool {
    if mappings.is_empty()
        || mappings.len() > cpu::LINUX_EXEC_MAPPING_LIMIT
        || old_mappings.is_empty()
        || old_mappings.len() > cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT
        || mappings.iter().enumerate().any(|(index, mapping)| {
            mapping.address == 0
                || !(mapping.address as u64).is_multiple_of(PAGE_SIZE as u64)
                || !mapping.frame.is_multiple_of(PAGE_SIZE as u64)
                || mapping.writable && mapping.executable
                || !memory::frame_addresses_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
                || mappings[..index]
                    .iter()
                    .any(|prior| prior.address == mapping.address)
        })
        || old_mappings.iter().enumerate().any(|(index, mapping)| {
            mapping.address == 0
                || !mapping.address.is_multiple_of(PAGE_SIZE)
                || mapping.writable && mapping.executable
                || old_mappings[..index]
                    .iter()
                    .any(|prior| prior.address == mapping.address || prior.frame == mapping.frame)
        })
    {
        return false;
    }
    let mut old_frames = [memory::FrameLease::EMPTY; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    for (output, mapping) in old_frames.iter_mut().zip(old_mappings) {
        *output = mapping.frame;
    }
    if !memory::frame_leases_releasable(
        memory::FrameOwner::KERNEL,
        &old_frames[..old_mappings.len()],
    ) {
        return false;
    }
    let mut old_mapping_entries = [0_u64; cpu::LINUX_EXEC_MAPPING_LIMIT];
    let mut old_clear_entries = [0_u64; cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    let mut mapping_entries = [core::ptr::null_mut(); cpu::LINUX_EXEC_MAPPING_LIMIT];
    let mut clear_entries = [core::ptr::null_mut(); cpu::LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    for (index, mapping) in mappings.iter().enumerate() {
        let Some(entry) = current_linux_leaf(mapping.address as u64) else {
            return false;
        };
        let value = unsafe { entry.read() };
        mapping_entries[index] = entry;
        old_mapping_entries[index] = value;
    }
    for (index, mapping) in old_mappings.iter().enumerate() {
        let Some(entry) = current_linux_leaf(mapping.address as u64) else {
            return false;
        };
        let value = unsafe { entry.read() };
        let expected = mapping.frame.address()
            | PRESENT
            | USER
            | if mapping.writable { WRITABLE } else { 0 }
            | if mapping.executable { 0 } else { NO_EXECUTE };
        if value & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE) != expected {
            return false;
        }
        clear_entries[index] = entry;
        old_clear_entries[index] = value;
    }
    for (mapping, entry) in old_mappings.iter().zip(clear_entries) {
        unsafe {
            entry.write(0);
            asm!("invlpg [{}]", in(reg) mapping.address, options(nostack, preserves_flags));
        }
    }
    for (mapping, entry) in mappings.iter().zip(mapping_entries) {
        let flags = PRESENT
            | USER
            | if mapping.writable { WRITABLE } else { 0 }
            | if mapping.executable { 0 } else { NO_EXECUTE };
        unsafe {
            entry.write(mapping.frame | flags);
            asm!("invlpg [{}]", in(reg) mapping.address, options(nostack, preserves_flags));
        }
    }
    let installed = !force_verification_failure
        && mappings.iter().all(|mapping| {
            let flags = PRESENT
                | USER
                | if mapping.writable { WRITABLE } else { 0 }
                | if mapping.executable { 0 } else { NO_EXECUTE };
            current_linux_leaf(mapping.address as u64).is_some_and(|entry| unsafe {
                entry.read() & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE)
                    == (mapping.frame | flags)
            })
        })
        && old_mappings.iter().all(|old_mapping| {
            mappings
                .iter()
                .any(|mapping| mapping.address == old_mapping.address)
                || current_linux_leaf(old_mapping.address as u64)
                    .is_some_and(|entry| unsafe { entry.read() == 0 })
        });
    if installed {
        return memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &old_frames[..old_mappings.len()],
        );
    }
    for (mapping, value) in old_mappings.iter().zip(old_clear_entries) {
        if let Some(entry) = current_linux_leaf(mapping.address as u64) {
            unsafe {
                entry.write(value);
                asm!("invlpg [{}]", in(reg) mapping.address, options(nostack, preserves_flags));
            }
        }
    }
    for (mapping, value) in mappings.iter().zip(old_mapping_entries) {
        if !old_mappings
            .iter()
            .any(|old_mapping| old_mapping.address == mapping.address)
        {
            if let Some(entry) = current_linux_leaf(mapping.address as u64) {
                unsafe {
                    entry.write(value);
                    asm!("invlpg [{}]", in(reg) mapping.address, options(nostack, preserves_flags));
                }
            }
        }
    }
    false
}

fn map_current_linux_page_with_permissions(
    address: u64,
    frame: u64,
    writable: bool,
    executable: bool,
) -> bool {
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
        let flags = PRESENT
            | USER
            | if writable { WRITABLE } else { 0 }
            | if executable { 0 } else { NO_EXECUTE };
        entry.write(frame | flags);
        asm!("invlpg [{}]", in(reg) address, options(nostack, preserves_flags));
        entry.read() & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE) == (frame | flags)
    }
}

pub(crate) fn prepare_linux_frame(frame: u64, bytes: &[u8]) -> bool {
    if bytes.len() > PAGE_SIZE
        || !frame.is_multiple_of(PAGE_SIZE as u64)
        || !memory::frame_addresses_owned_by(memory::FrameOwner::KERNEL, &[frame])
    {
        return false;
    }
    unsafe {
        if zero_frame(frame).is_none() {
            return false;
        }
        let Some(output) = mapped_frame::<u8>(frame) else {
            return false;
        };
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
    }
    true
}

pub(crate) fn materialize_linux_exec_frame(
    frame: memory::FrameLease,
    segment: crate::linux_elf::PlannedSegment<'_>,
    page_index: usize,
) -> bool {
    if page_index >= segment.page_count
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame])
    {
        return false;
    }
    let Some(address) = frame.direct_map_address() else {
        return false;
    };
    // SAFETY: the generation-tagged lease is live, uniquely selected for this
    // not-yet-published exec transaction, and covers exactly one page.
    let output = unsafe { &mut *(address as *mut [u8; PAGE_SIZE]) };
    crate::linux_elf::materialize_page(segment, page_index, output)
}

pub(crate) fn materialize_linux_exec_stack_frame(
    frame: memory::FrameLease,
    offset: usize,
    image: &[u8],
) -> bool {
    if offset
        .checked_add(image.len())
        .is_none_or(|end| end > PAGE_SIZE)
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame])
    {
        return false;
    }
    let Some(address) = frame.direct_map_address() else {
        return false;
    };
    // SAFETY: this generation-tagged frame is live, private to the pending
    // exec transaction, and has not yet been installed in userspace.
    let output = unsafe { &mut *(address as *mut [u8; PAGE_SIZE]) };
    output.fill(0);
    output[offset..offset + image.len()].copy_from_slice(image);
    true
}

pub(crate) fn current_linux_mapping_matches(
    address: u64,
    frame: memory::FrameLease,
    writable: bool,
    executable: bool,
) -> bool {
    if address >= 1 << 47
        || !address.is_multiple_of(PAGE_SIZE as u64)
        || writable && executable
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame])
    {
        return false;
    }
    let flags = PRESENT
        | USER
        | if writable { WRITABLE } else { 0 }
        | if executable { 0 } else { NO_EXECUTE };
    current_linux_leaf(address).is_some_and(|entry| unsafe {
        entry.read() & (ADDRESS_MASK | PRESENT | WRITABLE | USER | NO_EXECUTE)
            == frame.address() | flags
    })
}

pub(crate) fn current_linux_page_absent(address: u64) -> bool {
    address < 1 << 47
        && address.is_multiple_of(PAGE_SIZE as u64)
        && current_linux_leaf(address).is_some_and(|entry| unsafe { entry.read() == 0 })
}

pub(crate) fn current_root_address() -> u64 {
    current_root()
}

pub(crate) fn current_linux_hierarchy_matches(address: u64, frames: &[memory::FrameLease]) -> bool {
    if address >= 1 << 47 || frames.is_empty() || current_root() != frames[0].address() {
        return false;
    }
    let indexes = [
        ((address >> 39) & 0x1ff) as usize,
        ((address >> 30) & 0x1ff) as usize,
        ((address >> 21) & 0x1ff) as usize,
        ((address >> 12) & 0x1ff) as usize,
    ];
    let mut table = current_root();
    for index in indexes[..3].iter().copied() {
        if !frames.iter().any(|frame| frame.address() == table) {
            return false;
        }
        let Some(entries) = mapped_frame::<u64>(table) else {
            return false;
        };
        let entry = unsafe { entries.add(index).read() };
        if entry & (PRESENT | USER) != PRESENT | USER || entry & LARGE_PAGE != 0 {
            return false;
        }
        table = entry & ADDRESS_MASK;
    }
    frames.iter().any(|frame| frame.address() == table)
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
    linux_leaf_in_root(current_root(), address)
}

fn linux_leaf_in_root(root: u64, address: u64) -> Option<*mut u64> {
    let indexes = [
        ((address >> 39) & 0x1ff) as usize,
        ((address >> 30) & 0x1ff) as usize,
        ((address >> 21) & 0x1ff) as usize,
        ((address >> 12) & 0x1ff) as usize,
    ];
    let mut table = root;
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
    handles: [[u64; 4]; 2],
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
    let Some(expected_fault_instruction) = expected_service_fault_instruction(service_entry_offset)
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
        first_space
            .first_stack_frame
            .add(ROOT_HANDLE_STACK_OFFSET)
            .cast::<u64>()
            .write_volatile(handles[0][3]);
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
        let root_handle_seeded = first_space
            .first_stack_frame
            .add(ROOT_HANDLE_STACK_OFFSET)
            .cast::<u64>()
            .read_volatile()
            == handles[0][3];
        if first_marker != TEST_PATTERN || second_marker != !TEST_PATTERN || !root_handle_seeded {
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
            handles[0][3],
            domains[1],
        );
        activate(kernel_root);
        let preemption_complete = timer::finish_scheduler_preemption(local_apic);
        let runtime_complete = scheduler::verify_runtime_complete();
        let ipc_empty = ipc::is_empty();
        let root_restored = current_root() == kernel_root;
        if !handled {
            crate::serial_write_bytes(b"\r\nginux: control proof syscall state incomplete");
        }
        if !runtime_complete {
            crate::serial_write_bytes(b"\r\nginux: control proof scheduler state incomplete");
        }
        if !ipc_empty {
            crate::serial_write_bytes(b"\r\nginux: control proof IPC state not drained");
        }
        if !root_restored {
            crate::serial_write_bytes(b"\r\nginux: control proof kernel root not restored");
        }
        if !preemption_complete {
            crate::serial_write_bytes(b"\r\nginux: control proof APIC preemption incomplete");
        }
        let valid =
            handled && runtime_complete && ipc_empty && root_restored && preemption_complete;
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

unsafe fn frame_is_zero(frame: u64) -> bool {
    mapped_frame::<u64>(frame)
        .is_some_and(|entries| (0..ENTRY_COUNT).all(|index| entries.add(index).read() == 0))
}
