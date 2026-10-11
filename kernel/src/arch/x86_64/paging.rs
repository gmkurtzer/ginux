use core::arch::asm;

use crate::{pe::ImageSummary, BootInfo, UefiMemoryDescriptor, EFI_RUNTIME_SERVICES_CODE};

const ENTRY_COUNT: usize = 512;
const DIRECTORY_COUNT: usize = 4;
const LARGE_PAGE_SIZE: u64 = 2 * 1024 * 1024;
const PAGE_SIZE: usize = 4096;
const MAX_FINE_TABLES: usize = 8;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const LARGE_PAGE: u64 = 1 << 7;
const NO_EXECUTE: u64 = 1 << 63;
const ADDRESS_MASK: usize = 0x000f_ffff_ffff_f000;
const CR0_WRITE_PROTECT: usize = 1 << 16;

pub(crate) const INITIAL_IDENTITY_LIMIT: usize = 1usize << 32;

#[derive(Clone, Copy)]
#[repr(C, align(4096))]
struct PageTable([u64; ENTRY_COUNT]);

static mut PML4: PageTable = PageTable([0; ENTRY_COUNT]);
static mut PDPT: PageTable = PageTable([0; ENTRY_COUNT]);
static mut PAGE_DIRECTORIES: [PageTable; DIRECTORY_COUNT] =
    [PageTable([0; ENTRY_COUNT]); DIRECTORY_COUNT];
static mut FINE_PAGE_TABLES: [PageTable; MAX_FINE_TABLES] =
    [PageTable([0; ENTRY_COUNT]); MAX_FINE_TABLES];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PagingError {
    RequiredAddressOutsideMap,
    TableAddressOutsideMap,
    TableMisaligned,
    GuardMisaligned,
    FineTableCapacity,
    ExecutableMapInvalid,
    ActivationFailed,
    HardeningFailed,
}

/// Installs a transitional 4 GiB identity map backed by 2 MiB pages.
///
/// # Safety
///
/// The caller must ensure interrupts are disabled and that the current x86-64
/// UEFI environment uses identity-addressable physical memory for the supplied
/// pointers and the statically linked page-table objects.
pub(crate) unsafe fn install_initial_identity_map(
    boot_info: &BootInfo,
    image: &ImageSummary,
    required_addresses: &[usize],
    guard_pages: &[usize],
) -> Result<(), PagingError> {
    if required_addresses
        .iter()
        .any(|address| *address >= INITIAL_IDENTITY_LIMIT)
    {
        return Err(PagingError::RequiredAddressOutsideMap);
    }

    let pml4_address = core::ptr::addr_of!(PML4) as usize;
    let pdpt_address = core::ptr::addr_of!(PDPT) as usize;
    let directories_address = core::ptr::addr_of!(PAGE_DIRECTORIES) as usize;
    let fine_tables_address = core::ptr::addr_of!(FINE_PAGE_TABLES) as usize;

    for address in [
        pml4_address,
        pdpt_address,
        directories_address,
        fine_tables_address,
    ] {
        if address >= INITIAL_IDENTITY_LIMIT {
            return Err(PagingError::TableAddressOutsideMap);
        }
        if address & 0xfff != 0 {
            return Err(PagingError::TableMisaligned);
        }
    }

    if guard_pages
        .iter()
        .any(|address| *address >= INITIAL_IDENTITY_LIMIT || *address % PAGE_SIZE != 0)
    {
        return Err(PagingError::GuardMisaligned);
    }

    let pml4 = core::ptr::addr_of_mut!(PML4.0).cast::<u64>();
    let pdpt = core::ptr::addr_of_mut!(PDPT.0).cast::<u64>();
    let directories = core::ptr::addr_of_mut!(PAGE_DIRECTORIES).cast::<PageTable>();

    zero_entries(pml4);
    zero_entries(pdpt);

    pml4.write((pdpt_address as u64) | PRESENT | WRITABLE);

    for directory_index in 0..DIRECTORY_COUNT {
        let directory = directories.add(directory_index);
        let entries = core::ptr::addr_of_mut!((*directory).0).cast::<u64>();
        zero_entries(entries);

        pdpt.add(directory_index)
            .write((directory as u64) | PRESENT | WRITABLE);

        for entry_index in 0..ENTRY_COUNT {
            let page_index = directory_index * ENTRY_COUNT + entry_index;
            let physical_address = page_index as u64 * LARGE_PAGE_SIZE;
            entries
                .add(entry_index)
                .write(physical_address | PRESENT | WRITABLE | LARGE_PAGE | NO_EXECUTE);
        }
    }

    let fine_tables = core::ptr::addr_of_mut!(FINE_PAGE_TABLES).cast::<PageTable>();
    let mut fine_regions = [usize::MAX; MAX_FINE_TABLES];
    let mut fine_count = 0;
    for guard in guard_pages {
        let region = *guard / LARGE_PAGE_SIZE as usize;
        let fine_index = ensure_fine_region(
            region,
            &mut fine_regions,
            &mut fine_count,
            fine_tables,
            directories,
        )?;
        let page = (*guard % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
        let entries = core::ptr::addr_of_mut!((*fine_tables.add(fine_index)).0).cast::<u64>();
        entries.add(page).write(0);
    }

    let descriptor_count = boot_info.memory_map_size / boot_info.memory_descriptor_size;
    for index in 0..descriptor_count {
        let descriptor = read_descriptor(boot_info, index);
        let Some((start, end)) = runtime_code_bounds(descriptor)? else {
            continue;
        };
        let first_region = start / LARGE_PAGE_SIZE as usize;
        let last_region = (end - 1) / LARGE_PAGE_SIZE as usize;
        for region in first_region..=last_region {
            ensure_fine_region(
                region,
                &mut fine_regions,
                &mut fine_count,
                fine_tables,
                directories,
            )?;
        }
    }
    for &(start, end) in image.executable_ranges() {
        let first_region = start / LARGE_PAGE_SIZE as usize;
        let last_region = (end - 1) / LARGE_PAGE_SIZE as usize;
        for region in first_region..=last_region {
            ensure_fine_region(
                region,
                &mut fine_regions,
                &mut fine_count,
                fine_tables,
                directories,
            )?;
        }
    }
    for index in 0..descriptor_count {
        let descriptor = read_descriptor(boot_info, index);
        let Some((start, end)) = runtime_code_bounds(descriptor)? else {
            continue;
        };
        for address in (start..end).step_by(PAGE_SIZE) {
            let region = address / LARGE_PAGE_SIZE as usize;
            let Some(fine_index) = fine_regions[..fine_count]
                .iter()
                .position(|candidate| *candidate == region)
            else {
                return Err(PagingError::ExecutableMapInvalid);
            };
            let page = (address % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
            let entries = core::ptr::addr_of_mut!((*fine_tables.add(fine_index)).0).cast::<u64>();
            let value = entries.add(page).read();
            entries.add(page).write(value & !NO_EXECUTE);
        }
    }
    for &(start, end) in image.executable_ranges() {
        for address in (start..end).step_by(PAGE_SIZE) {
            let region = address / LARGE_PAGE_SIZE as usize;
            let Some(fine_index) = fine_regions[..fine_count]
                .iter()
                .position(|candidate| *candidate == region)
            else {
                return Err(PagingError::ExecutableMapInvalid);
            };
            let page = (address % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
            let entries = core::ptr::addr_of_mut!((*fine_tables.add(fine_index)).0).cast::<u64>();
            let value = entries.add(page).read();
            entries.add(page).write(value & !(NO_EXECUTE | WRITABLE));
        }
    }

    const TABLE_PAGE_COUNT: usize = 2 + DIRECTORY_COUNT + MAX_FINE_TABLES;
    let mut table_pages = [0_usize; TABLE_PAGE_COUNT];
    table_pages[0] = pml4_address;
    table_pages[1] = pdpt_address;
    for index in 0..DIRECTORY_COUNT {
        table_pages[2 + index] = directories_address + index * PAGE_SIZE;
    }
    for index in 0..MAX_FINE_TABLES {
        table_pages[2 + DIRECTORY_COUNT + index] = fine_tables_address + index * PAGE_SIZE;
    }
    for address in table_pages {
        ensure_fine_region(
            address / LARGE_PAGE_SIZE as usize,
            &mut fine_regions,
            &mut fine_count,
            fine_tables,
            directories,
        )?;
    }
    for address in table_pages {
        let region = address / LARGE_PAGE_SIZE as usize;
        let Some(fine_index) = fine_regions[..fine_count]
            .iter()
            .position(|candidate| *candidate == region)
        else {
            return Err(PagingError::HardeningFailed);
        };
        let page = (address % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
        let entries = core::ptr::addr_of_mut!((*fine_tables.add(fine_index)).0).cast::<u64>();
        let value = entries.add(page).read();
        if value & PRESENT == 0 {
            return Err(PagingError::HardeningFailed);
        }
        entries.add(page).write(value & !WRITABLE);
    }

    // Loading CR3 serializes the page-table writes and activates the new root.
    asm!(
        "mov cr3, {}",
        in(reg) pml4_address,
        options(nostack, preserves_flags)
    );

    let active_root: usize;
    asm!(
        "mov {}, cr3",
        out(reg) active_root,
        options(nomem, nostack, preserves_flags)
    );

    if active_root & ADDRESS_MASK != pml4_address {
        return Err(PagingError::ActivationFailed);
    }

    let mut cr0: usize;
    asm!(
        "mov {}, cr0",
        out(reg) cr0,
        options(nomem, nostack, preserves_flags)
    );
    cr0 |= CR0_WRITE_PROTECT;
    asm!(
        "mov cr0, {}",
        in(reg) cr0,
        options(nostack, preserves_flags)
    );
    asm!(
        "mov {}, cr0",
        out(reg) cr0,
        options(nomem, nostack, preserves_flags)
    );
    if cr0 & CR0_WRITE_PROTECT == 0 {
        return Err(PagingError::HardeningFailed);
    }

    for (index, region) in fine_regions[..fine_count].iter().enumerate() {
        let entries = core::ptr::addr_of!((*fine_tables.add(index)).0).cast::<u64>();
        for guard in guard_pages {
            if *guard / LARGE_PAGE_SIZE as usize == *region {
                let page = (*guard % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
                if entries.add(page).read() != 0 {
                    return Err(PagingError::HardeningFailed);
                }
            }
        }
    }
    for address in table_pages {
        let region = address / LARGE_PAGE_SIZE as usize;
        let Some(fine_index) = fine_regions[..fine_count]
            .iter()
            .position(|candidate| *candidate == region)
        else {
            return Err(PagingError::HardeningFailed);
        };
        let page = (address % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
        let entries = core::ptr::addr_of!((*fine_tables.add(fine_index)).0).cast::<u64>();
        let value = entries.add(page).read();
        if value & PRESENT == 0 || value & WRITABLE != 0 {
            return Err(PagingError::HardeningFailed);
        }
    }
    if !identity_map_obeys_wx(boot_info, image) {
        return Err(PagingError::HardeningFailed);
    }

    Ok(())
}

unsafe fn ensure_fine_region(
    region: usize,
    fine_regions: &mut [usize; MAX_FINE_TABLES],
    fine_count: &mut usize,
    fine_tables: *mut PageTable,
    directories: *mut PageTable,
) -> Result<usize, PagingError> {
    if let Some(index) = fine_regions[..*fine_count]
        .iter()
        .position(|candidate| *candidate == region)
    {
        return Ok(index);
    }
    if *fine_count == MAX_FINE_TABLES || region >= DIRECTORY_COUNT * ENTRY_COUNT {
        return Err(PagingError::FineTableCapacity);
    }
    let index = *fine_count;
    fine_regions[index] = region;
    *fine_count += 1;
    let table = fine_tables.add(index);
    let entries = core::ptr::addr_of_mut!((*table).0).cast::<u64>();
    zero_entries(entries);
    let region_base = region as u64 * LARGE_PAGE_SIZE;
    for entry in 0..ENTRY_COUNT {
        entries.add(entry).write(
            (region_base + entry as u64 * PAGE_SIZE as u64) | PRESENT | WRITABLE | NO_EXECUTE,
        );
    }
    let directory = directories.add(region / ENTRY_COUNT);
    let directory_entries = core::ptr::addr_of_mut!((*directory).0).cast::<u64>();
    directory_entries
        .add(region % ENTRY_COUNT)
        .write((table as u64) | PRESENT | WRITABLE);
    Ok(index)
}

pub(crate) fn supervisor_write_protect_enabled() -> bool {
    let cr0: usize;
    unsafe {
        asm!(
            "mov {}, cr0",
            out(reg) cr0,
            options(nomem, nostack, preserves_flags)
        );
    }
    cr0 & CR0_WRITE_PROTECT != 0
}

pub(crate) fn static_page_tables_are_read_only() -> bool {
    const TABLE_PAGE_COUNT: usize = 2 + DIRECTORY_COUNT + MAX_FINE_TABLES;
    let mut table_pages = [0_usize; TABLE_PAGE_COUNT];
    table_pages[0] = core::ptr::addr_of!(PML4) as usize;
    table_pages[1] = core::ptr::addr_of!(PDPT) as usize;
    let directories_address = core::ptr::addr_of!(PAGE_DIRECTORIES) as usize;
    let fine_tables_address = core::ptr::addr_of!(FINE_PAGE_TABLES) as usize;
    for index in 0..DIRECTORY_COUNT {
        table_pages[2 + index] = directories_address + index * PAGE_SIZE;
    }
    for index in 0..MAX_FINE_TABLES {
        table_pages[2 + DIRECTORY_COUNT + index] = fine_tables_address + index * PAGE_SIZE;
    }
    table_pages.into_iter().all(|address| unsafe {
        let region = address / LARGE_PAGE_SIZE as usize;
        let directory = core::ptr::addr_of!(PAGE_DIRECTORIES)
            .cast::<PageTable>()
            .add(region / ENTRY_COUNT);
        let directory_entries = core::ptr::addr_of!((*directory).0).cast::<u64>();
        let directory_entry = directory_entries.add(region % ENTRY_COUNT).read();
        if directory_entry & PRESENT == 0 || directory_entry & LARGE_PAGE != 0 {
            return false;
        }
        let table = (directory_entry as usize & ADDRESS_MASK) as *const u64;
        let page = (address % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
        let entry = table.add(page).read();
        entry & PRESENT != 0 && entry & WRITABLE == 0
    })
}

pub(crate) fn identity_map_obeys_wx(boot_info: &BootInfo, image: &ImageSummary) -> bool {
    unsafe {
        let directories = core::ptr::addr_of!(PAGE_DIRECTORIES).cast::<PageTable>();
        for region in 0..DIRECTORY_COUNT * ENTRY_COUNT {
            let directory = directories.add(region / ENTRY_COUNT);
            let entries = core::ptr::addr_of!((*directory).0).cast::<u64>();
            let entry = entries.add(region % ENTRY_COUNT).read();
            if entry & PRESENT == 0 {
                return false;
            }
            let region_start = region * LARGE_PAGE_SIZE as usize;
            let region_end = region_start + LARGE_PAGE_SIZE as usize;
            if entry & LARGE_PAGE != 0 {
                if entry & NO_EXECUTE == 0
                    || range_contains_executable(boot_info, image, region_start, region_end)
                {
                    return false;
                }
                continue;
            }
            let table = (entry as usize & ADDRESS_MASK) as *const u64;
            for page in 0..ENTRY_COUNT {
                let leaf = table.add(page).read();
                if leaf & PRESENT == 0 {
                    continue;
                }
                let address = region_start + page * PAGE_SIZE;
                let image_code = image
                    .executable_ranges()
                    .iter()
                    .any(|(start, end)| address < *end && *start < address + PAGE_SIZE);
                let executable = image_code
                    || range_contains_runtime_code(boot_info, address, address + PAGE_SIZE);
                if executable == (leaf & NO_EXECUTE != 0) || (image_code && leaf & WRITABLE != 0) {
                    return false;
                }
            }
        }
    }
    true
}

unsafe fn read_descriptor(boot_info: &BootInfo, index: usize) -> UefiMemoryDescriptor {
    let address = boot_info
        .memory_map
        .add(index * boot_info.memory_descriptor_size);
    core::ptr::read_unaligned(address.cast::<UefiMemoryDescriptor>())
}

fn runtime_code_bounds(
    descriptor: UefiMemoryDescriptor,
) -> Result<Option<(usize, usize)>, PagingError> {
    if descriptor.memory_type != EFI_RUNTIME_SERVICES_CODE {
        return Ok(None);
    }
    let start = usize::try_from(descriptor.physical_start)
        .map_err(|_| PagingError::ExecutableMapInvalid)?;
    let size = descriptor
        .page_count
        .checked_mul(PAGE_SIZE as u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(PagingError::ExecutableMapInvalid)?;
    let end = start
        .checked_add(size)
        .ok_or(PagingError::ExecutableMapInvalid)?
        .min(INITIAL_IDENTITY_LIMIT);
    if start % PAGE_SIZE != 0 || size == 0 {
        return Err(PagingError::ExecutableMapInvalid);
    }
    Ok((start < end).then_some((start, end)))
}

fn range_contains_runtime_code(boot_info: &BootInfo, start: usize, end: usize) -> bool {
    let descriptor_count = boot_info.memory_map_size / boot_info.memory_descriptor_size;
    for index in 0..descriptor_count {
        let descriptor = unsafe { read_descriptor(boot_info, index) };
        let Ok(Some((code_start, code_end))) = runtime_code_bounds(descriptor) else {
            continue;
        };
        if start < code_end && code_start < end {
            return true;
        }
    }
    false
}

fn range_contains_executable(
    boot_info: &BootInfo,
    image: &ImageSummary,
    start: usize,
    end: usize,
) -> bool {
    range_contains_runtime_code(boot_info, start, end)
        || image
            .executable_ranges()
            .iter()
            .any(|(code_start, code_end)| start < *code_end && *code_start < end)
}

unsafe fn zero_entries(entries: *mut u64) {
    for index in 0..ENTRY_COUNT {
        entries.add(index).write(0);
    }
}
