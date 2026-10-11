use core::arch::asm;

const ENTRY_COUNT: usize = 512;
const DIRECTORY_COUNT: usize = 4;
const LARGE_PAGE_SIZE: u64 = 2 * 1024 * 1024;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const LARGE_PAGE: u64 = 1 << 7;
const ADDRESS_MASK: usize = 0x000f_ffff_ffff_f000;

pub(crate) const INITIAL_IDENTITY_LIMIT: usize = 1usize << 32;

#[derive(Clone, Copy)]
#[repr(C, align(4096))]
struct PageTable([u64; ENTRY_COUNT]);

static mut PML4: PageTable = PageTable([0; ENTRY_COUNT]);
static mut PDPT: PageTable = PageTable([0; ENTRY_COUNT]);
static mut PAGE_DIRECTORIES: [PageTable; DIRECTORY_COUNT] =
    [PageTable([0; ENTRY_COUNT]); DIRECTORY_COUNT];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PagingError {
    RequiredAddressOutsideMap,
    TableAddressOutsideMap,
    TableMisaligned,
    ActivationFailed,
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

    for address in [pml4_address, pdpt_address, directories_address] {
        if address >= INITIAL_IDENTITY_LIMIT {
            return Err(PagingError::TableAddressOutsideMap);
        }
        if address & 0xfff != 0 {
            return Err(PagingError::TableMisaligned);
        }
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

    Ok(())
}

unsafe fn zero_entries(entries: *mut u64) {
    for index in 0..ENTRY_COUNT {
        entries.add(index).write(0);
    }
}
