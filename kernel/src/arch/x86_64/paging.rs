use core::arch::asm;

const ENTRY_COUNT: usize = 512;
const DIRECTORY_COUNT: usize = 4;
const LARGE_PAGE_SIZE: u64 = 2 * 1024 * 1024;
const PAGE_SIZE: usize = 4096;
const MAX_FINE_TABLES: usize = 8;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const LARGE_PAGE: u64 = 1 << 7;
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
                .write(physical_address | PRESENT | WRITABLE | LARGE_PAGE);
        }
    }

    let fine_tables = core::ptr::addr_of_mut!(FINE_PAGE_TABLES).cast::<PageTable>();
    let mut fine_regions = [usize::MAX; MAX_FINE_TABLES];
    let mut fine_count = 0;
    for guard in guard_pages {
        let region = *guard / LARGE_PAGE_SIZE as usize;
        let fine_index = if let Some(index) = fine_regions[..fine_count]
            .iter()
            .position(|candidate| *candidate == region)
        {
            index
        } else {
            if fine_count == MAX_FINE_TABLES {
                return Err(PagingError::FineTableCapacity);
            }
            let index = fine_count;
            fine_regions[index] = region;
            fine_count += 1;
            let table = fine_tables.add(index);
            let entries = core::ptr::addr_of_mut!((*table).0).cast::<u64>();
            zero_entries(entries);
            let region_base = region as u64 * LARGE_PAGE_SIZE;
            for entry in 0..ENTRY_COUNT {
                entries
                    .add(entry)
                    .write((region_base + entry as u64 * PAGE_SIZE as u64) | PRESENT | WRITABLE);
            }
            let directory = directories.add(region / ENTRY_COUNT);
            let directory_entries = core::ptr::addr_of_mut!((*directory).0).cast::<u64>();
            directory_entries
                .add(region % ENTRY_COUNT)
                .write((table as u64) | PRESENT | WRITABLE);
            index
        };
        let page = (*guard % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
        let entries = core::ptr::addr_of_mut!((*fine_tables.add(fine_index)).0).cast::<u64>();
        entries.add(page).write(0);
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

    Ok(())
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

unsafe fn zero_entries(entries: *mut u64) {
    for index in 0..ENTRY_COUNT {
        entries.add(index).write(0);
    }
}
