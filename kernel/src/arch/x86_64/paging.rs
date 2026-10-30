use core::{arch::asm, cell::UnsafeCell};

use crate::{pe::ImageSummary, BootInfo, UefiMemoryDescriptor, EFI_RUNTIME_SERVICES_CODE};

const ENTRY_COUNT: usize = 512;
const DIRECTORY_COUNT: usize = 4;
const LARGE_PAGE_SIZE: u64 = 2 * 1024 * 1024;
const PAGE_SIZE: usize = 4096;
const MAX_FINE_TABLES: usize = 128;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const WRITE_THROUGH: u64 = 1 << 3;
const ACCESSED: u64 = 1 << 5;
const CACHE_DISABLE: u64 = 1 << 4;
const DIRTY: u64 = 1 << 6;
const LARGE_PAGE: u64 = 1 << 7;
const NO_EXECUTE: u64 = 1 << 63;
const ADDRESS_MASK: usize = 0x000f_ffff_ffff_f000;
const CR0_WRITE_PROTECT: usize = 1 << 16;

pub(crate) const INITIAL_IDENTITY_LIMIT: usize = 1usize << 32;
pub(crate) const DIRECT_MAP_BASE: usize = 0xffff_8000_0000_0000;
const DIRECT_MAP_PML4_INDEX: usize = 256;
const MMIO_PDPT_INDEX: usize = ENTRY_COUNT - 1;
const MMIO_WINDOW_BASE: usize = DIRECT_MAP_BASE + MMIO_PDPT_INDEX * (1 << 30);
const USER_REGION_BASE: u64 = 1_u64 << 39;

#[derive(Clone, Copy)]
pub(crate) struct MmioMapping {
    physical_page: u64,
    physical_start: u64,
    virtual_page: usize,
    length: usize,
    mapped_length: usize,
    writable: bool,
}

pub(crate) struct InitialMmioMappings {
    pub(crate) block_transport: MmioMapping,
    pub(crate) block_msix: MmioMapping,
    pub(crate) iommu: MmioMapping,
    pub(crate) network_common: MmioMapping,
    pub(crate) network_device: MmioMapping,
    pub(crate) network_notification: MmioMapping,
    pub(crate) network_msix: MmioMapping,
}

impl MmioMapping {
    pub(crate) fn address(self, physical: u64, length: usize) -> Option<usize> {
        if length == 0 || physical < self.physical_start {
            return None;
        }
        let requested_offset = usize::try_from(physical - self.physical_start).ok()?;
        let requested_end = requested_offset.checked_add(length)?;
        let page_offset = usize::try_from(physical - self.physical_page).ok()?;
        (requested_end <= self.length)
            .then(|| self.virtual_page.checked_add(page_offset))
            .flatten()
    }
}

pub(crate) fn user_context_matches_layout(instruction: u64, stack: u64) -> bool {
    let code_end = USER_REGION_BASE + PAGE_SIZE as u64;
    let first_stack_start = USER_REGION_BASE + 2 * PAGE_SIZE as u64;
    let first_stack_end = USER_REGION_BASE + 3 * PAGE_SIZE as u64;
    let second_stack_start = USER_REGION_BASE + 4 * PAGE_SIZE as u64;
    let second_stack_end = USER_REGION_BASE + 5 * PAGE_SIZE as u64;
    (USER_REGION_BASE..code_end).contains(&instruction)
        && ((first_stack_start < stack && stack <= first_stack_end)
            || (second_stack_start < stack && stack <= second_stack_end))
}

pub(crate) fn verify_user_context_layout_invariants() -> bool {
    let page = PAGE_SIZE as u64;
    user_context_matches_layout(USER_REGION_BASE, USER_REGION_BASE + 3 * page)
        && user_context_matches_layout(USER_REGION_BASE + page - 1, USER_REGION_BASE + 2 * page + 1)
        && user_context_matches_layout(USER_REGION_BASE, USER_REGION_BASE + 5 * page)
        && !user_context_matches_layout(USER_REGION_BASE - 1, USER_REGION_BASE + 3 * page)
        && !user_context_matches_layout(USER_REGION_BASE + page, USER_REGION_BASE + 3 * page)
        && !user_context_matches_layout(USER_REGION_BASE, USER_REGION_BASE + 2 * page)
        && !user_context_matches_layout(USER_REGION_BASE, USER_REGION_BASE + 3 * page + 1)
        && !user_context_matches_layout(USER_REGION_BASE, USER_REGION_BASE + 4 * page)
        && !user_context_matches_layout(USER_REGION_BASE, USER_REGION_BASE + 5 * page + 1)
}

pub(crate) struct ValidatedUserRootFrames {
    base: [u64; 15],
    base_count: usize,
    dma: Option<u64>,
    native_executable_pages: [u64; 3],
    native_image_pages: u8,
    native_stack_page: u8,
    native_stack_pages: u8,
    native_table: u64,
}

impl ValidatedUserRootFrames {
    pub(crate) fn base(&self) -> &[u64] {
        &self.base[..self.base_count]
    }

    pub(crate) const fn base_count(&self) -> usize {
        self.base_count
    }

    pub(crate) fn frames_owned_by(&self, owner: crate::memory::FrameOwner) -> bool {
        if self.base_count <= self.base.len() {
            return crate::memory::frame_addresses_owned_by(owner, self.base());
        }
        let Some(table) = direct_map_range_address(self.native_table, PAGE_SIZE) else {
            return false;
        };
        let table = table as *const u64;
        let hierarchy = &self.base[..4];
        crate::memory::frame_addresses_owned_by(owner, hierarchy)
            && (0..usize::from(self.native_image_pages)).all(|index| {
                let frame = unsafe { table.add(index).read() } & ADDRESS_MASK as u64;
                crate::memory::frame_addresses_owned_by(owner, &[frame])
            })
            && (0..usize::from(self.native_stack_pages))
                .map(|offset| usize::from(self.native_stack_page) + offset)
                .chain(core::iter::once(
                    usize::from(self.native_stack_page) + usize::from(self.native_stack_pages) + 1,
                ))
                .all(|index| {
                    let frame = unsafe { table.add(index).read() } & ADDRESS_MASK as u64;
                    crate::memory::frame_addresses_owned_by(owner, &[frame])
                })
    }

    pub(crate) fn dma(&self) -> Option<u64> {
        self.dma
    }

    pub(crate) fn context_matches(&self, instruction: u64, stack: u64) -> bool {
        if self.base_count == 7 {
            return user_context_matches_layout(instruction, stack);
        }
        let stack_start = USER_REGION_BASE + u64::from(self.native_stack_page) * PAGE_SIZE as u64;
        let stack_end = stack_start + u64::from(self.native_stack_pages) * PAGE_SIZE as u64;
        let executable_page = instruction
            .checked_sub(USER_REGION_BASE)
            .map(|offset| offset / PAGE_SIZE as u64)
            .filter(|page| *page < u64::from(self.native_image_pages))
            .is_some_and(|page| {
                let page = page as usize;
                self.native_executable_pages[page / 64] & (1_u64 << (page % 64)) != 0
            });
        executable_page && stack_start < stack && stack <= stack_end
    }
}

pub(crate) fn validated_user_root_frames(root: u64) -> Option<ValidatedUserRootFrames> {
    if root == 0 || root >= INITIAL_IDENTITY_LIMIT as u64 || !root.is_multiple_of(PAGE_SIZE as u64)
    {
        return None;
    }
    let entries = direct_map_range_address(root, PAGE_SIZE)? as *const u64;
    let kernel_entry = unsafe {
        core::ptr::addr_of!((*PAGING.pml4.get()).0)
            .cast::<u64>()
            .read()
    };
    let kernel_direct_entry = unsafe {
        core::ptr::addr_of!((*PAGING.pml4.get()).0)
            .cast::<u64>()
            .add(DIRECT_MAP_PML4_INDEX)
            .read()
    };
    let shared = unsafe { entries.read() };
    let user = unsafe { entries.add(1).read() };
    let direct = unsafe { entries.add(DIRECT_MAP_PML4_INDEX).read() };
    if shared != kernel_entry
        || direct != kernel_direct_entry
        || shared & PRESENT == 0
        || shared & USER != 0
        || direct & PRESENT == 0
        || direct & USER != 0
        || user & (PRESENT | WRITABLE | USER) != (PRESENT | WRITABLE | USER)
        || user & (LARGE_PAGE | NO_EXECUTE) != 0
        || !(2..ENTRY_COUNT).all(|index| {
            index == DIRECT_MAP_PML4_INDEX || unsafe { entries.add(index).read() == 0 }
        })
    {
        return None;
    }
    let pdpt_address = user & ADDRESS_MASK as u64;
    let pdpt_entry = single_user_table_entry(pdpt_address)?;
    let directory_address = pdpt_entry & ADDRESS_MASK as u64;
    let directory_entry = single_user_table_entry(directory_address)?;
    let table_address = directory_entry & ADDRESS_MASK as u64;
    if table_address == 0 || table_address >= INITIAL_IDENTITY_LIMIT as u64 {
        return None;
    }
    let table = direct_map_range_address(table_address, PAGE_SIZE)? as *const u64;
    let code = unsafe { table.read() };
    let first_guard = unsafe { table.add(1).read() };
    let first_stack = unsafe { table.add(2).read() };
    let second_guard = unsafe { table.add(3).read() };
    let second_stack = unsafe { table.add(4).read() };
    let device = unsafe { table.add(5).read() };
    let dma = unsafe { table.add(6).read() };
    let notification = unsafe { table.add(7).read() };
    let code_address = code & ADDRESS_MASK as u64;
    let first_stack_address = first_stack & ADDRESS_MASK as u64;
    let second_stack_address = second_stack & ADDRESS_MASK as u64;
    let device_address = device & ADDRESS_MASK as u64;
    let dma_address = dma & ADDRESS_MASK as u64;
    let notification_address = notification & ADDRESS_MASK as u64;
    let device_flags = PRESENT | USER | WRITE_THROUGH | CACHE_DISABLE | NO_EXECUTE;
    let device_valid = device == 0
        || (device_address >= INITIAL_IDENTITY_LIMIT as u64
            && device & device_flags == device_flags
            && device & LARGE_PAGE == 0
            && (device & DIRTY == 0 || device & WRITABLE != 0)
            && device & !(ADDRESS_MASK as u64 | device_flags | WRITABLE | ACCESSED | DIRTY) == 0);
    let dma_flags = PRESENT | WRITABLE | USER | NO_EXECUTE;
    let dma_valid = dma == 0
        || (dma_address != 0
            && dma_address < INITIAL_IDENTITY_LIMIT as u64
            && dma & dma_flags == dma_flags
            && dma & LARGE_PAGE == 0
            && dma & !(ADDRESS_MASK as u64 | dma_flags | ACCESSED | DIRTY) == 0);
    let notification_flags = PRESENT | WRITABLE | USER | WRITE_THROUGH | CACHE_DISABLE | NO_EXECUTE;
    let notification_valid = notification == 0
        || (device != 0
            && notification_address >= INITIAL_IDENTITY_LIMIT as u64
            && notification & notification_flags == notification_flags
            && notification & LARGE_PAGE == 0
            && notification & !(ADDRESS_MASK as u64 | notification_flags | ACCESSED | DIRTY) == 0
            && notification_address != device_address);
    let legacy_frames = [
        root,
        pdpt_address,
        directory_address,
        table_address,
        code_address,
        first_stack_address,
        second_stack_address,
    ];
    let legacy_valid = code & (PRESENT | USER) == (PRESENT | USER)
        && code & (WRITABLE | NO_EXECUTE | LARGE_PAGE) == 0
        && first_guard == 0
        && first_stack & (PRESENT | WRITABLE | USER | NO_EXECUTE)
            == (PRESENT | WRITABLE | USER | NO_EXECUTE)
        && first_stack & LARGE_PAGE == 0
        && second_guard == 0
        && second_stack & (PRESENT | WRITABLE | USER | NO_EXECUTE)
            == (PRESENT | WRITABLE | USER | NO_EXECUTE)
        && second_stack & LARGE_PAGE == 0
        && device_valid
        && dma_valid
        && notification_valid
        && (8..ENTRY_COUNT).all(|index| unsafe { table.add(index).read() == 0 })
        && legacy_frames
            .iter()
            .all(|frame| *frame != 0 && *frame < INITIAL_IDENTITY_LIMIT as u64)
        && legacy_frames
            .iter()
            .enumerate()
            .all(|(index, frame)| !legacy_frames[index + 1..].contains(frame))
        && (dma == 0 || !legacy_frames.contains(&dma_address));
    if legacy_valid {
        let mut base = [0_u64; 15];
        base[..legacy_frames.len()].copy_from_slice(&legacy_frames);
        return Some(ValidatedUserRootFrames {
            base,
            base_count: legacy_frames.len(),
            dma: (dma != 0).then_some(dma_address),
            native_executable_pages: [0; 3],
            native_image_pages: 0,
            native_stack_page: 0,
            native_stack_pages: 0,
            native_table: 0,
        });
    }

    let native_entry = |index: usize| unsafe { table.add(index).read() };
    let owned_flags = PRESENT | USER;
    let read_only_nx = owned_flags | NO_EXECUTE;
    let executable = owned_flags;
    let stack_flags = owned_flags | WRITABLE | NO_EXECUTE;
    let mmio_flags = stack_flags | WRITE_THROUGH | CACHE_DISABLE;
    let flags_match = |entry: u64, expected: u64| {
        entry & !(ACCESSED | DIRTY) == expected && (entry & DIRTY == 0 || expected & WRITABLE != 0)
    };
    let network_layout = native_entry(15) & PRESENT != 0;
    let controller_layout = !network_layout && native_entry(7) & PRESENT != 0;
    let image_capacity = if network_layout {
        160
    } else if controller_layout {
        crate::address_space::NATIVE_CONTROLLER_IMAGE_PAGES
    } else {
        7
    };
    let stack_pages = if network_layout { 16 } else { 2 };
    let stack_page = image_capacity + 1;
    let boot_info_page = stack_page + stack_pages + 1;
    let image_pages = (0..image_capacity)
        .take_while(|index| native_entry(*index) & PRESENT != 0)
        .count();
    let mut native_executable_pages = [0_u64; 3];
    let image_valid = image_pages != 0
        && (image_pages..image_capacity).all(|index| native_entry(index) == 0)
        && (0..image_pages).all(|index| {
            let entry = native_entry(index);
            let address = entry & ADDRESS_MASK as u64;
            let valid = address != 0
                && address < INITIAL_IDENTITY_LIMIT as u64
                && (flags_match(entry, address | executable)
                    || flags_match(entry, address | read_only_nx)
                    || flags_match(entry, address | stack_flags));
            if valid && entry & NO_EXECUTE == 0 {
                native_executable_pages[index / 64] |= 1_u64 << (index % 64);
            }
            valid
        })
        && native_executable_pages != [0; 3];
    let native_dma = if controller_layout {
        0
    } else {
        native_entry(boot_info_page + 1)
    };
    let native_dma_address = native_dma & ADDRESS_MASK as u64;
    let dma_valid = native_dma == 0
        || (flags_match(native_dma, native_dma_address | stack_flags)
            && native_dma_address != 0
            && native_dma_address < INITIAL_IDENTITY_LIMIT as u64);
    let common_page = boot_info_page + 2;
    let notification_page = boot_info_page + 3;
    let mmio_valid = if native_entry(common_page) == 0 && native_entry(notification_page) == 0 {
        true
    } else {
        flags_match(
            native_entry(common_page),
            (native_entry(common_page) & ADDRESS_MASK as u64) | mmio_flags,
        ) && flags_match(
            native_entry(notification_page),
            (native_entry(notification_page) & ADDRESS_MASK as u64) | mmio_flags,
        ) && native_entry(common_page) & ADDRESS_MASK as u64 >= INITIAL_IDENTITY_LIMIT as u64
            && native_entry(notification_page) & ADDRESS_MASK as u64
                >= INITIAL_IDENTITY_LIMIT as u64
            && native_entry(common_page) & ADDRESS_MASK as u64
                != native_entry(notification_page) & ADDRESS_MASK as u64
    };
    let stack_index = 4 + image_pages;
    let native_frame_count = stack_index + stack_pages + 1;
    let native_frame_at = |index: usize| -> u64 {
        match index {
            0 => root,
            1 => pdpt_address,
            2 => directory_address,
            3 => table_address,
            index if index < 4 + image_pages => native_entry(index - 4) & ADDRESS_MASK as u64,
            index if (stack_index..stack_index + stack_pages).contains(&index) => {
                native_entry(stack_page + index - stack_index) & ADDRESS_MASK as u64
            }
            index if index == stack_index + stack_pages => {
                native_entry(boot_info_page) & ADDRESS_MASK as u64
            }
            _ => 0,
        }
    };
    let native_frames_valid = (0..native_frame_count).all(|index| {
        let frame = native_frame_at(index);
        frame != 0
            && frame < INITIAL_IDENTITY_LIMIT as u64
            && (index + 1..native_frame_count).all(|other| native_frame_at(other) != frame)
    });
    (image_valid
        && native_entry(stack_page - 1) == 0
        && (0..stack_pages).all(|offset| {
            flags_match(
                native_entry(stack_page + offset),
                native_frame_at(stack_index + offset) | stack_flags,
            )
        })
        && native_entry(stack_page + stack_pages) == 0
        && flags_match(
            native_entry(boot_info_page),
            native_frame_at(stack_index + stack_pages) | read_only_nx,
        )
        && (!controller_layout
            || (native_entry(common_page) == 0 && native_entry(notification_page) == 0))
        && dma_valid
        && mmio_valid
        && (notification_page + 1..ENTRY_COUNT)
            .all(|index| unsafe { table.add(index).read() == 0 })
        && native_frames_valid
        && (native_dma == 0
            || !(0..native_frame_count).any(|index| native_frame_at(index) == native_dma_address)))
    .then_some(ValidatedUserRootFrames {
        base: {
            let mut frames = [0; 15];
            for (index, frame) in frames
                .iter_mut()
                .enumerate()
                .take(native_frame_count.min(15))
            {
                *frame = native_frame_at(index);
            }
            frames
        },
        base_count: native_frame_count,
        dma: (native_dma != 0).then_some(native_dma_address),
        native_executable_pages,
        native_image_pages: image_pages as u8,
        native_stack_page: stack_page as u8,
        native_stack_pages: stack_pages as u8,
        native_table: table_address,
    })
}

fn single_user_table_entry(table_address: u64) -> Option<u64> {
    if table_address == 0 || table_address >= INITIAL_IDENTITY_LIMIT as u64 {
        return None;
    }
    let table = direct_map_range_address(table_address, PAGE_SIZE)? as *const u64;
    let entry = unsafe { table.read() };
    (entry & (PRESENT | WRITABLE | USER) == (PRESENT | WRITABLE | USER)
        && entry & (LARGE_PAGE | NO_EXECUTE) == 0
        && (1..ENTRY_COUNT).all(|index| unsafe { table.add(index).read() == 0 }))
    .then_some(entry)
}

#[derive(Clone, Copy)]
#[repr(C, align(4096))]
struct PageTable([u64; ENTRY_COUNT]);

struct PagingOwner {
    pml4: UnsafeCell<PageTable>,
    pdpt: UnsafeCell<PageTable>,
    direct_pdpt: UnsafeCell<PageTable>,
    page_directories: UnsafeCell<[PageTable; DIRECTORY_COUNT]>,
    fine_page_tables: UnsafeCell<[PageTable; MAX_FINE_TABLES]>,
    mmio_directory: UnsafeCell<PageTable>,
    mmio_table: UnsafeCell<PageTable>,
}

unsafe impl Sync for PagingOwner {}

static PAGING: PagingOwner = PagingOwner {
    pml4: UnsafeCell::new(PageTable([0; ENTRY_COUNT])),
    pdpt: UnsafeCell::new(PageTable([0; ENTRY_COUNT])),
    direct_pdpt: UnsafeCell::new(PageTable([0; ENTRY_COUNT])),
    page_directories: UnsafeCell::new([PageTable([0; ENTRY_COUNT]); DIRECTORY_COUNT]),
    fine_page_tables: UnsafeCell::new([PageTable([0; ENTRY_COUNT]); MAX_FINE_TABLES]),
    mmio_directory: UnsafeCell::new(PageTable([0; ENTRY_COUNT])),
    mmio_table: UnsafeCell::new(PageTable([0; ENTRY_COUNT])),
};

pub(crate) fn kernel_root_address() -> u64 {
    PAGING.pml4.get().cast_const() as u64
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum PagingError {
    RequiredAddressOutsideMap,
    TableAddressOutsideMap,
    TableMisaligned,
    GuardMisaligned,
    FineTableCapacity,
    MmioMapInvalid,
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
    mmio_regions: [(u64, usize); 7],
) -> Result<InitialMmioMappings, PagingError> {
    let [mmio, auxiliary_mmio, platform_mmio, device_mmio, device_config_mmio, device_notification_mmio, device_msix_mmio] =
        mmio_regions;
    let (mmio_physical, mmio_length) = mmio;
    let (auxiliary_mmio_physical, auxiliary_mmio_length) = auxiliary_mmio;
    let (platform_mmio_physical, platform_mmio_length) = platform_mmio;
    let (device_mmio_physical, device_mmio_length) = device_mmio;
    let (device_config_mmio_physical, device_config_mmio_length) = device_config_mmio;
    let (device_notification_mmio_physical, device_notification_mmio_length) =
        device_notification_mmio;
    let (device_msix_mmio_physical, device_msix_mmio_length) = device_msix_mmio;
    if required_addresses
        .iter()
        .any(|address| *address >= INITIAL_IDENTITY_LIMIT)
    {
        return Err(PagingError::RequiredAddressOutsideMap);
    }

    let pml4_address = PAGING.pml4.get().cast_const() as usize;
    let pdpt_address = PAGING.pdpt.get().cast_const() as usize;
    let direct_pdpt_address = PAGING.direct_pdpt.get().cast_const() as usize;
    let directories_address = PAGING.page_directories.get().cast_const() as usize;
    let fine_tables_address = PAGING.fine_page_tables.get().cast_const() as usize;
    let mmio_directory_address = PAGING.mmio_directory.get().cast_const() as usize;
    let mmio_table_address = PAGING.mmio_table.get().cast_const() as usize;

    for address in [
        pml4_address,
        pdpt_address,
        direct_pdpt_address,
        directories_address,
        fine_tables_address,
        mmio_directory_address,
        mmio_table_address,
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

    let pml4 = core::ptr::addr_of_mut!((*PAGING.pml4.get()).0).cast::<u64>();
    let pdpt = core::ptr::addr_of_mut!((*PAGING.pdpt.get()).0).cast::<u64>();
    let direct_pdpt = core::ptr::addr_of_mut!((*PAGING.direct_pdpt.get()).0).cast::<u64>();
    let directories = PAGING.page_directories.get().cast::<PageTable>();

    zero_entries(pml4);
    zero_entries(pdpt);
    zero_entries(direct_pdpt);

    pml4.write((pdpt_address as u64) | PRESENT | WRITABLE);
    pml4.add(DIRECT_MAP_PML4_INDEX)
        .write((direct_pdpt_address as u64) | PRESENT | WRITABLE);

    for directory_index in 0..DIRECTORY_COUNT {
        let directory = directories.add(directory_index);
        let entries = core::ptr::addr_of_mut!((*directory).0).cast::<u64>();
        zero_entries(entries);

        pdpt.add(directory_index)
            .write((directory as u64) | PRESENT | WRITABLE);
        direct_pdpt
            .add(directory_index)
            .write((directory as u64) | PRESENT | WRITABLE);

        for entry_index in 0..ENTRY_COUNT {
            let page_index = directory_index * ENTRY_COUNT + entry_index;
            let physical_address = page_index as u64 * LARGE_PAGE_SIZE;
            entries
                .add(entry_index)
                .write(physical_address | PRESENT | WRITABLE | LARGE_PAGE | NO_EXECUTE);
        }
    }

    let mmio_page = mmio_physical & !(PAGE_SIZE as u64 - 1);
    let mmio_offset =
        usize::try_from(mmio_physical - mmio_page).map_err(|_| PagingError::MmioMapInvalid)?;
    let mmio_span = mmio_offset
        .checked_add(mmio_length)
        .ok_or(PagingError::MmioMapInvalid)?;
    let mmio_pages = mmio_span
        .checked_add(PAGE_SIZE - 1)
        .ok_or(PagingError::MmioMapInvalid)?
        / PAGE_SIZE;
    let mmio_mapped_end = mmio_page.checked_add((mmio_pages * PAGE_SIZE) as u64);
    if mmio_length == 0
        || mmio_page < INITIAL_IDENTITY_LIMIT as u64
        || mmio_pages == 0
        || mmio_pages > ENTRY_COUNT
        || mmio_page & !(ADDRESS_MASK as u64) != 0
        || mmio_mapped_end.is_none_or(|end| end > ADDRESS_MASK as u64 + PAGE_SIZE as u64)
    {
        return Err(PagingError::MmioMapInvalid);
    }
    let auxiliary_mmio_page = auxiliary_mmio_physical & !(PAGE_SIZE as u64 - 1);
    let auxiliary_mmio_offset = usize::try_from(auxiliary_mmio_physical - auxiliary_mmio_page)
        .map_err(|_| PagingError::MmioMapInvalid)?;
    let auxiliary_mmio_span = auxiliary_mmio_offset
        .checked_add(auxiliary_mmio_length)
        .ok_or(PagingError::MmioMapInvalid)?;
    let auxiliary_mmio_pages = auxiliary_mmio_span
        .checked_add(PAGE_SIZE - 1)
        .ok_or(PagingError::MmioMapInvalid)?
        / PAGE_SIZE;
    let auxiliary_mmio_mapped_end =
        auxiliary_mmio_page.checked_add((auxiliary_mmio_pages * PAGE_SIZE) as u64);
    let auxiliary_mmio_index = ENTRY_COUNT
        .checked_sub(auxiliary_mmio_pages)
        .ok_or(PagingError::MmioMapInvalid)?;
    if auxiliary_mmio_length == 0
        || auxiliary_mmio_pages == 0
        || auxiliary_mmio_index < mmio_pages
        || auxiliary_mmio_page & !(ADDRESS_MASK as u64) != 0
        || auxiliary_mmio_mapped_end.is_none_or(|end| end > ADDRESS_MASK as u64 + PAGE_SIZE as u64)
        || ranges_overlap_u64(
            (mmio_page, mmio_mapped_end.unwrap_or(mmio_page)),
            (
                auxiliary_mmio_page,
                auxiliary_mmio_mapped_end.unwrap_or(auxiliary_mmio_page),
            ),
        )
    {
        return Err(PagingError::MmioMapInvalid);
    }
    let platform_mmio_page = platform_mmio_physical & !(PAGE_SIZE as u64 - 1);
    let platform_mmio_offset = usize::try_from(platform_mmio_physical - platform_mmio_page)
        .map_err(|_| PagingError::MmioMapInvalid)?;
    let platform_mmio_span = platform_mmio_offset
        .checked_add(platform_mmio_length)
        .ok_or(PagingError::MmioMapInvalid)?;
    let platform_mmio_pages = platform_mmio_span
        .checked_add(PAGE_SIZE - 1)
        .ok_or(PagingError::MmioMapInvalid)?
        / PAGE_SIZE;
    let platform_mmio_index = mmio_pages;
    let platform_mmio_mapped_end =
        platform_mmio_page.checked_add((platform_mmio_pages * PAGE_SIZE) as u64);
    if platform_mmio_length == 0
        || platform_mmio_pages == 0
        || platform_mmio_index
            .checked_add(platform_mmio_pages)
            .is_none_or(|end| end > auxiliary_mmio_index)
        || platform_mmio_page & !(ADDRESS_MASK as u64) != 0
        || platform_mmio_mapped_end.is_none_or(|end| end > ADDRESS_MASK as u64 + PAGE_SIZE as u64)
        || ranges_overlap_u64(
            (mmio_page, mmio_mapped_end.unwrap_or(mmio_page)),
            (
                platform_mmio_page,
                platform_mmio_mapped_end.unwrap_or(platform_mmio_page),
            ),
        )
        || ranges_overlap_u64(
            (
                auxiliary_mmio_page,
                auxiliary_mmio_mapped_end.unwrap_or(auxiliary_mmio_page),
            ),
            (
                platform_mmio_page,
                platform_mmio_mapped_end.unwrap_or(platform_mmio_page),
            ),
        )
    {
        return Err(PagingError::MmioMapInvalid);
    }
    let device_mmio_page = device_mmio_physical & !(PAGE_SIZE as u64 - 1);
    let device_mmio_offset = usize::try_from(device_mmio_physical - device_mmio_page)
        .map_err(|_| PagingError::MmioMapInvalid)?;
    let device_mmio_span = device_mmio_offset
        .checked_add(device_mmio_length)
        .ok_or(PagingError::MmioMapInvalid)?;
    let device_mmio_pages = device_mmio_span
        .checked_add(PAGE_SIZE - 1)
        .ok_or(PagingError::MmioMapInvalid)?
        / PAGE_SIZE;
    let device_mmio_index = platform_mmio_index
        .checked_add(platform_mmio_pages)
        .ok_or(PagingError::MmioMapInvalid)?;
    let device_mmio_mapped_end =
        device_mmio_page.checked_add((device_mmio_pages * PAGE_SIZE) as u64);
    if device_mmio_length == 0
        || device_mmio_pages == 0
        || device_mmio_index
            .checked_add(device_mmio_pages)
            .is_none_or(|end| end > auxiliary_mmio_index)
        || device_mmio_page & !(ADDRESS_MASK as u64) != 0
        || device_mmio_mapped_end.is_none_or(|end| end > ADDRESS_MASK as u64 + PAGE_SIZE as u64)
        || ranges_overlap_u64(
            (mmio_page, mmio_mapped_end.unwrap_or(mmio_page)),
            (
                device_mmio_page,
                device_mmio_mapped_end.unwrap_or(device_mmio_page),
            ),
        )
        || ranges_overlap_u64(
            (
                auxiliary_mmio_page,
                auxiliary_mmio_mapped_end.unwrap_or(auxiliary_mmio_page),
            ),
            (
                device_mmio_page,
                device_mmio_mapped_end.unwrap_or(device_mmio_page),
            ),
        )
        || ranges_overlap_u64(
            (
                platform_mmio_page,
                platform_mmio_mapped_end.unwrap_or(platform_mmio_page),
            ),
            (
                device_mmio_page,
                device_mmio_mapped_end.unwrap_or(device_mmio_page),
            ),
        )
    {
        return Err(PagingError::MmioMapInvalid);
    }
    let device_config_mmio_page = device_config_mmio_physical & !(PAGE_SIZE as u64 - 1);
    let device_config_mmio_offset =
        usize::try_from(device_config_mmio_physical - device_config_mmio_page)
            .map_err(|_| PagingError::MmioMapInvalid)?;
    let device_config_mmio_span = device_config_mmio_offset
        .checked_add(device_config_mmio_length)
        .ok_or(PagingError::MmioMapInvalid)?;
    let device_config_mmio_pages = device_config_mmio_span
        .checked_add(PAGE_SIZE - 1)
        .ok_or(PagingError::MmioMapInvalid)?
        / PAGE_SIZE;
    let device_config_mmio_index = device_mmio_index
        .checked_add(device_mmio_pages)
        .ok_or(PagingError::MmioMapInvalid)?;
    let device_config_mmio_mapped_end =
        device_config_mmio_page.checked_add((device_config_mmio_pages * PAGE_SIZE) as u64);
    let existing_mmio_ranges = [
        (mmio_page, mmio_mapped_end.unwrap_or(mmio_page)),
        (
            auxiliary_mmio_page,
            auxiliary_mmio_mapped_end.unwrap_or(auxiliary_mmio_page),
        ),
        (
            platform_mmio_page,
            platform_mmio_mapped_end.unwrap_or(platform_mmio_page),
        ),
        (
            device_mmio_page,
            device_mmio_mapped_end.unwrap_or(device_mmio_page),
        ),
    ];
    if device_config_mmio_length == 0
        || device_config_mmio_pages == 0
        || device_config_mmio_index
            .checked_add(device_config_mmio_pages)
            .is_none_or(|end| end > auxiliary_mmio_index)
        || device_config_mmio_page & !(ADDRESS_MASK as u64) != 0
        || device_config_mmio_mapped_end
            .is_none_or(|end| end > ADDRESS_MASK as u64 + PAGE_SIZE as u64)
        || existing_mmio_ranges.into_iter().any(|range| {
            ranges_overlap_u64(
                range,
                (
                    device_config_mmio_page,
                    device_config_mmio_mapped_end.unwrap_or(device_config_mmio_page),
                ),
            )
        })
    {
        return Err(PagingError::MmioMapInvalid);
    }
    let device_notification_mmio_page = device_notification_mmio_physical & !(PAGE_SIZE as u64 - 1);
    let device_notification_mmio_offset =
        usize::try_from(device_notification_mmio_physical - device_notification_mmio_page)
            .map_err(|_| PagingError::MmioMapInvalid)?;
    let device_notification_mmio_span = device_notification_mmio_offset
        .checked_add(device_notification_mmio_length)
        .ok_or(PagingError::MmioMapInvalid)?;
    let device_notification_mmio_pages = device_notification_mmio_span
        .checked_add(PAGE_SIZE - 1)
        .ok_or(PagingError::MmioMapInvalid)?
        / PAGE_SIZE;
    let device_notification_mmio_index = device_config_mmio_index
        .checked_add(device_config_mmio_pages)
        .ok_or(PagingError::MmioMapInvalid)?;
    let device_notification_mmio_mapped_end = device_notification_mmio_page
        .checked_add((device_notification_mmio_pages * PAGE_SIZE) as u64);
    let existing_mmio_ranges = [
        (mmio_page, mmio_mapped_end.unwrap_or(mmio_page)),
        (
            auxiliary_mmio_page,
            auxiliary_mmio_mapped_end.unwrap_or(auxiliary_mmio_page),
        ),
        (
            platform_mmio_page,
            platform_mmio_mapped_end.unwrap_or(platform_mmio_page),
        ),
        (
            device_mmio_page,
            device_mmio_mapped_end.unwrap_or(device_mmio_page),
        ),
        (
            device_config_mmio_page,
            device_config_mmio_mapped_end.unwrap_or(device_config_mmio_page),
        ),
    ];
    if device_notification_mmio_length == 0
        || device_notification_mmio_pages == 0
        || device_notification_mmio_index
            .checked_add(device_notification_mmio_pages)
            .is_none_or(|end| end > auxiliary_mmio_index)
        || device_notification_mmio_page & !(ADDRESS_MASK as u64) != 0
        || device_notification_mmio_mapped_end
            .is_none_or(|end| end > ADDRESS_MASK as u64 + PAGE_SIZE as u64)
        || existing_mmio_ranges.into_iter().any(|range| {
            ranges_overlap_u64(
                range,
                (
                    device_notification_mmio_page,
                    device_notification_mmio_mapped_end.unwrap_or(device_notification_mmio_page),
                ),
            )
        })
    {
        return Err(PagingError::MmioMapInvalid);
    }
    let device_msix_mmio_page = device_msix_mmio_physical & !(PAGE_SIZE as u64 - 1);
    let device_msix_mmio_offset =
        usize::try_from(device_msix_mmio_physical - device_msix_mmio_page)
            .map_err(|_| PagingError::MmioMapInvalid)?;
    let device_msix_mmio_span = device_msix_mmio_offset
        .checked_add(device_msix_mmio_length)
        .ok_or(PagingError::MmioMapInvalid)?;
    let device_msix_mmio_pages = device_msix_mmio_span
        .checked_add(PAGE_SIZE - 1)
        .ok_or(PagingError::MmioMapInvalid)?
        / PAGE_SIZE;
    let device_msix_mmio_index = auxiliary_mmio_index
        .checked_sub(device_msix_mmio_pages)
        .ok_or(PagingError::MmioMapInvalid)?;
    let device_msix_mmio_mapped_end =
        device_msix_mmio_page.checked_add((device_msix_mmio_pages * PAGE_SIZE) as u64);
    let existing_mmio_ranges = [
        (mmio_page, mmio_mapped_end.unwrap_or(mmio_page)),
        (
            auxiliary_mmio_page,
            auxiliary_mmio_mapped_end.unwrap_or(auxiliary_mmio_page),
        ),
        (
            platform_mmio_page,
            platform_mmio_mapped_end.unwrap_or(platform_mmio_page),
        ),
        (
            device_mmio_page,
            device_mmio_mapped_end.unwrap_or(device_mmio_page),
        ),
        (
            device_config_mmio_page,
            device_config_mmio_mapped_end.unwrap_or(device_config_mmio_page),
        ),
        (
            device_notification_mmio_page,
            device_notification_mmio_mapped_end.unwrap_or(device_notification_mmio_page),
        ),
    ];
    if device_msix_mmio_length == 0
        || device_msix_mmio_pages == 0
        || device_msix_mmio_index
            < device_notification_mmio_index
                .checked_add(device_notification_mmio_pages)
                .ok_or(PagingError::MmioMapInvalid)?
        || device_msix_mmio_page & !(ADDRESS_MASK as u64) != 0
        || device_msix_mmio_mapped_end
            .is_none_or(|end| end > ADDRESS_MASK as u64 + PAGE_SIZE as u64)
        || existing_mmio_ranges.into_iter().any(|range| {
            ranges_overlap_u64(
                range,
                (
                    device_msix_mmio_page,
                    device_msix_mmio_mapped_end.unwrap_or(device_msix_mmio_page),
                ),
            )
        })
    {
        return Err(PagingError::MmioMapInvalid);
    }
    let mmio_directory = core::ptr::addr_of_mut!((*PAGING.mmio_directory.get()).0).cast::<u64>();
    let mmio_table = core::ptr::addr_of_mut!((*PAGING.mmio_table.get()).0).cast::<u64>();
    zero_entries(mmio_directory);
    zero_entries(mmio_table);
    for index in 0..mmio_pages {
        mmio_table.add(index).write(
            (mmio_page + (index * PAGE_SIZE) as u64)
                | PRESENT
                | WRITABLE
                | CACHE_DISABLE
                | NO_EXECUTE,
        );
    }
    for index in 0..auxiliary_mmio_pages {
        mmio_table.add(auxiliary_mmio_index + index).write(
            (auxiliary_mmio_page + (index * PAGE_SIZE) as u64)
                | PRESENT
                | WRITABLE
                | CACHE_DISABLE
                | NO_EXECUTE,
        );
    }
    for index in 0..platform_mmio_pages {
        mmio_table.add(platform_mmio_index + index).write(
            (platform_mmio_page + (index * PAGE_SIZE) as u64)
                | PRESENT
                | WRITABLE
                | CACHE_DISABLE
                | NO_EXECUTE,
        );
    }
    for index in 0..device_mmio_pages {
        mmio_table.add(device_mmio_index + index).write(
            (device_mmio_page + (index * PAGE_SIZE) as u64)
                | PRESENT
                | WRITABLE
                | CACHE_DISABLE
                | NO_EXECUTE,
        );
    }
    for index in 0..device_config_mmio_pages {
        mmio_table.add(device_config_mmio_index + index).write(
            (device_config_mmio_page + (index * PAGE_SIZE) as u64)
                | PRESENT
                | CACHE_DISABLE
                | NO_EXECUTE,
        );
    }
    for index in 0..device_notification_mmio_pages {
        mmio_table
            .add(device_notification_mmio_index + index)
            .write(
                (device_notification_mmio_page + (index * PAGE_SIZE) as u64)
                    | PRESENT
                    | WRITABLE
                    | CACHE_DISABLE
                    | NO_EXECUTE,
            );
    }
    for index in 0..device_msix_mmio_pages {
        mmio_table.add(device_msix_mmio_index + index).write(
            (device_msix_mmio_page + (index * PAGE_SIZE) as u64)
                | PRESENT
                | WRITABLE
                | CACHE_DISABLE
                | NO_EXECUTE,
        );
    }
    mmio_directory.write((mmio_table_address as u64) | PRESENT | WRITABLE);
    direct_pdpt
        .add(MMIO_PDPT_INDEX)
        .write((mmio_directory_address as u64) | PRESENT | WRITABLE);
    let mmio_mapping = MmioMapping {
        physical_page: mmio_page,
        physical_start: mmio_physical,
        virtual_page: MMIO_WINDOW_BASE,
        length: mmio_length,
        mapped_length: mmio_pages * PAGE_SIZE,
        writable: true,
    };
    let auxiliary_mmio_mapping = MmioMapping {
        physical_page: auxiliary_mmio_page,
        physical_start: auxiliary_mmio_physical,
        virtual_page: MMIO_WINDOW_BASE + auxiliary_mmio_index * PAGE_SIZE,
        length: auxiliary_mmio_length,
        mapped_length: auxiliary_mmio_pages * PAGE_SIZE,
        writable: true,
    };
    let platform_mmio_mapping = MmioMapping {
        physical_page: platform_mmio_page,
        physical_start: platform_mmio_physical,
        virtual_page: MMIO_WINDOW_BASE + platform_mmio_index * PAGE_SIZE,
        length: platform_mmio_length,
        mapped_length: platform_mmio_pages * PAGE_SIZE,
        writable: true,
    };
    let device_mmio_mapping = MmioMapping {
        physical_page: device_mmio_page,
        physical_start: device_mmio_physical,
        virtual_page: MMIO_WINDOW_BASE + device_mmio_index * PAGE_SIZE,
        length: device_mmio_length,
        mapped_length: device_mmio_pages * PAGE_SIZE,
        writable: true,
    };
    let device_config_mmio_mapping = MmioMapping {
        physical_page: device_config_mmio_page,
        physical_start: device_config_mmio_physical,
        virtual_page: MMIO_WINDOW_BASE + device_config_mmio_index * PAGE_SIZE,
        length: device_config_mmio_length,
        mapped_length: device_config_mmio_pages * PAGE_SIZE,
        writable: false,
    };
    let device_notification_mmio_mapping = MmioMapping {
        physical_page: device_notification_mmio_page,
        physical_start: device_notification_mmio_physical,
        virtual_page: MMIO_WINDOW_BASE + device_notification_mmio_index * PAGE_SIZE,
        length: device_notification_mmio_length,
        mapped_length: device_notification_mmio_pages * PAGE_SIZE,
        writable: true,
    };
    let device_msix_mmio_mapping = MmioMapping {
        physical_page: device_msix_mmio_page,
        physical_start: device_msix_mmio_physical,
        virtual_page: MMIO_WINDOW_BASE + device_msix_mmio_index * PAGE_SIZE,
        length: device_msix_mmio_length,
        mapped_length: device_msix_mmio_pages * PAGE_SIZE,
        writable: true,
    };

    let fine_tables = PAGING.fine_page_tables.get().cast::<PageTable>();
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
    let (image_start, image_end) = image.image_range();
    let first_region = image_start / LARGE_PAGE_SIZE as usize;
    let last_region = (image_end - 1) / LARGE_PAGE_SIZE as usize;
    for region in first_region..=last_region {
        ensure_fine_region(
            region,
            &mut fine_regions,
            &mut fine_count,
            fine_tables,
            directories,
        )?;
    }
    for address in (image_start..image_end).step_by(PAGE_SIZE) {
        let region = address / LARGE_PAGE_SIZE as usize;
        let Some(fine_index) = fine_regions[..fine_count]
            .iter()
            .position(|candidate| *candidate == region)
        else {
            return Err(PagingError::ExecutableMapInvalid);
        };
        let page = (address % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
        let entries = core::ptr::addr_of_mut!((*fine_tables.add(fine_index)).0).cast::<u64>();
        let writable = image
            .writable_ranges()
            .iter()
            .any(|(start, end)| address < *end && *start < address + PAGE_SIZE);
        if !writable {
            let value = entries.add(page).read();
            entries.add(page).write(value & !WRITABLE);
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

    if auxiliary_mmio_page < INITIAL_IDENTITY_LIMIT as u64 {
        let auxiliary_end = auxiliary_mmio_mapped_end.ok_or(PagingError::MmioMapInvalid)?;
        if auxiliary_end > INITIAL_IDENTITY_LIMIT as u64 {
            return Err(PagingError::MmioMapInvalid);
        }
        for physical in (auxiliary_mmio_page..auxiliary_end).step_by(PAGE_SIZE) {
            let physical = usize::try_from(physical).map_err(|_| PagingError::MmioMapInvalid)?;
            let region = physical / LARGE_PAGE_SIZE as usize;
            let fine_index = ensure_fine_region(
                region,
                &mut fine_regions,
                &mut fine_count,
                fine_tables,
                directories,
            )?;
            let page = (physical % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
            let entries = core::ptr::addr_of_mut!((*fine_tables.add(fine_index)).0).cast::<u64>();
            entries.add(page).write(0);
        }
    }
    if platform_mmio_page < INITIAL_IDENTITY_LIMIT as u64 {
        let platform_end = platform_mmio_mapped_end.ok_or(PagingError::MmioMapInvalid)?;
        if platform_end > INITIAL_IDENTITY_LIMIT as u64 {
            return Err(PagingError::MmioMapInvalid);
        }
        for physical in (platform_mmio_page..platform_end).step_by(PAGE_SIZE) {
            let physical = usize::try_from(physical).map_err(|_| PagingError::MmioMapInvalid)?;
            let region = physical / LARGE_PAGE_SIZE as usize;
            let fine_index = ensure_fine_region(
                region,
                &mut fine_regions,
                &mut fine_count,
                fine_tables,
                directories,
            )?;
            let page = (physical % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
            let entries = core::ptr::addr_of_mut!((*fine_tables.add(fine_index)).0).cast::<u64>();
            entries.add(page).write(0);
        }
    }
    if device_notification_mmio_page < INITIAL_IDENTITY_LIMIT as u64 {
        let notification_end =
            device_notification_mmio_mapped_end.ok_or(PagingError::MmioMapInvalid)?;
        if notification_end > INITIAL_IDENTITY_LIMIT as u64 {
            return Err(PagingError::MmioMapInvalid);
        }
        for physical in (device_notification_mmio_page..notification_end).step_by(PAGE_SIZE) {
            let physical = usize::try_from(physical).map_err(|_| PagingError::MmioMapInvalid)?;
            let region = physical / LARGE_PAGE_SIZE as usize;
            let fine_index = ensure_fine_region(
                region,
                &mut fine_regions,
                &mut fine_count,
                fine_tables,
                directories,
            )?;
            let page = (physical % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
            let entries = core::ptr::addr_of_mut!((*fine_tables.add(fine_index)).0).cast::<u64>();
            entries.add(page).write(0);
        }
    }
    if device_msix_mmio_page < INITIAL_IDENTITY_LIMIT as u64 {
        let msix_end = device_msix_mmio_mapped_end.ok_or(PagingError::MmioMapInvalid)?;
        if msix_end > INITIAL_IDENTITY_LIMIT as u64 {
            return Err(PagingError::MmioMapInvalid);
        }
        for physical in (device_msix_mmio_page..msix_end).step_by(PAGE_SIZE) {
            let physical = usize::try_from(physical).map_err(|_| PagingError::MmioMapInvalid)?;
            let region = physical / LARGE_PAGE_SIZE as usize;
            let fine_index = ensure_fine_region(
                region,
                &mut fine_regions,
                &mut fine_count,
                fine_tables,
                directories,
            )?;
            let page = (physical % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
            let entries = core::ptr::addr_of_mut!((*fine_tables.add(fine_index)).0).cast::<u64>();
            entries.add(page).write(0);
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

    const TABLE_PAGE_COUNT: usize = 5 + DIRECTORY_COUNT + MAX_FINE_TABLES;
    let mut table_pages = [0_usize; TABLE_PAGE_COUNT];
    table_pages[0] = pml4_address;
    table_pages[1] = pdpt_address;
    table_pages[2] = direct_pdpt_address;
    table_pages[3] = mmio_directory_address;
    table_pages[4] = mmio_table_address;
    for index in 0..DIRECTORY_COUNT {
        table_pages[5 + index] = directories_address + index * PAGE_SIZE;
    }
    for index in 0..MAX_FINE_TABLES {
        table_pages[5 + DIRECTORY_COUNT + index] = fine_tables_address + index * PAGE_SIZE;
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

    Ok(InitialMmioMappings {
        block_transport: mmio_mapping,
        block_msix: auxiliary_mmio_mapping,
        iommu: platform_mmio_mapping,
        network_common: device_mmio_mapping,
        network_device: device_config_mmio_mapping,
        network_notification: device_notification_mmio_mapping,
        network_msix: device_msix_mmio_mapping,
    })
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

pub(crate) fn direct_map_range_address(physical: u64, length: usize) -> Option<usize> {
    if length == 0 {
        return None;
    }
    let physical = usize::try_from(physical).ok()?;
    let end = physical.checked_add(length)?;
    if end > INITIAL_IDENTITY_LIMIT {
        return None;
    }
    DIRECT_MAP_BASE.checked_add(physical)
}

pub(crate) fn direct_map_address(physical: usize) -> Option<usize> {
    direct_map_range_address(physical as u64, 1)
}

pub(crate) fn direct_map_translation_is_bounded() -> bool {
    direct_map_range_address(0, 1) == Some(DIRECT_MAP_BASE)
        && direct_map_range_address((INITIAL_IDENTITY_LIMIT - 1) as u64, 1)
            == Some(DIRECT_MAP_BASE + INITIAL_IDENTITY_LIMIT - 1)
        && direct_map_range_address(0, 0).is_none()
        && direct_map_range_address(INITIAL_IDENTITY_LIMIT as u64, 1).is_none()
        && direct_map_range_address((INITIAL_IDENTITY_LIMIT - 1) as u64, 2).is_none()
        && direct_map_range_address(u64::MAX, 1).is_none()
}

pub(crate) fn direct_map_alias_is_valid() -> bool {
    let pml4 = unsafe { core::ptr::addr_of!((*PAGING.pml4.get()).0).cast::<u64>() };
    let identity = unsafe { pml4.read() };
    let direct = unsafe { pml4.add(DIRECT_MAP_PML4_INDEX).read() };
    let expected_identity = PAGING.pdpt.get().cast_const() as u64 | PRESENT | WRITABLE;
    let expected_direct = PAGING.direct_pdpt.get().cast_const() as u64 | PRESENT | WRITABLE;
    if (identity ^ expected_identity) & !ACCESSED != 0
        || (direct ^ expected_direct) & !ACCESSED != 0
        || direct & PRESENT == 0
        || direct & USER != 0
    {
        return false;
    }
    let identity_pdpt = unsafe { core::ptr::addr_of!((*PAGING.pdpt.get()).0).cast::<u64>() };
    let direct_pdpt = unsafe { core::ptr::addr_of!((*PAGING.direct_pdpt.get()).0).cast::<u64>() };
    for index in 0..DIRECTORY_COUNT {
        if unsafe { (identity_pdpt.add(index).read() ^ direct_pdpt.add(index).read()) & !ACCESSED }
            != 0
        {
            return false;
        }
    }
    let physical = PAGING.pml4.get().cast_const() as usize;
    let Some(alias) = direct_map_address(physical) else {
        return false;
    };
    unsafe { (alias as *const u64).read() == (physical as *const u64).read() }
}

pub(crate) fn mmio_mapping_is_valid(mapping: MmioMapping) -> bool {
    let Some(window_offset) = mapping.virtual_page.checked_sub(MMIO_WINDOW_BASE) else {
        return false;
    };
    let Some(window_end) = window_offset.checked_add(mapping.mapped_length) else {
        return false;
    };
    if mapping.physical_page & (PAGE_SIZE as u64 - 1) != 0
        || mapping.physical_start < mapping.physical_page
        || usize::try_from(mapping.physical_start - mapping.physical_page)
            .ok()
            .and_then(|offset| offset.checked_add(mapping.length))
            .is_none_or(|end| end > mapping.mapped_length)
        || !window_offset.is_multiple_of(PAGE_SIZE)
        || window_end > ENTRY_COUNT * PAGE_SIZE
        || mapping.length == 0
        || mapping.mapped_length == 0
        || !mapping.mapped_length.is_multiple_of(PAGE_SIZE)
        || mapping.mapped_length > ENTRY_COUNT * PAGE_SIZE
    {
        return false;
    }
    unsafe {
        let direct_pdpt = core::ptr::addr_of!((*PAGING.direct_pdpt.get()).0).cast::<u64>();
        let directory_entry = direct_pdpt.add(MMIO_PDPT_INDEX).read();
        if directory_entry & (PRESENT | WRITABLE | USER | LARGE_PAGE | NO_EXECUTE)
            != PRESENT | WRITABLE
            || directory_entry & ADDRESS_MASK as u64
                != PAGING.mmio_directory.get().cast_const() as u64
        {
            return false;
        }
        let directory = core::ptr::addr_of!((*PAGING.mmio_directory.get()).0).cast::<u64>();
        let table_entry = directory.read();
        if table_entry & (PRESENT | WRITABLE | USER | LARGE_PAGE | NO_EXECUTE) != PRESENT | WRITABLE
            || table_entry & ADDRESS_MASK as u64 != PAGING.mmio_table.get().cast_const() as u64
            || (1..ENTRY_COUNT).any(|index| directory.add(index).read() != 0)
        {
            return false;
        }
        let table = core::ptr::addr_of!((*PAGING.mmio_table.get()).0).cast::<u64>();
        let page_count = mapping.mapped_length / PAGE_SIZE;
        let first_page = window_offset / PAGE_SIZE;
        let expected_flags =
            PRESENT | CACHE_DISABLE | NO_EXECUTE | if mapping.writable { WRITABLE } else { 0 };
        (0..page_count).all(|index| {
            let entry = table.add(first_page + index).read();
            entry & (ADDRESS_MASK as u64 | PRESENT | WRITABLE | USER | CACHE_DISABLE | NO_EXECUTE)
                == (mapping.physical_page + (index * PAGE_SIZE) as u64) | expected_flags
                && entry & LARGE_PAGE == 0
        })
    }
}

pub(crate) fn mmio_mappings_are_exact<const N: usize>(mappings: [MmioMapping; N]) -> bool {
    if !mappings.into_iter().all(mmio_mapping_is_valid) {
        return false;
    }
    for left in 0..mappings.len() {
        for right in left + 1..mappings.len() {
            let left_index = (mappings[left].virtual_page - MMIO_WINDOW_BASE) / PAGE_SIZE;
            let right_index = (mappings[right].virtual_page - MMIO_WINDOW_BASE) / PAGE_SIZE;
            let left_pages = mappings[left].mapped_length / PAGE_SIZE;
            let right_pages = mappings[right].mapped_length / PAGE_SIZE;
            if ranges_overlap(
                (left_index, left_index + left_pages),
                (right_index, right_index + right_pages),
            ) || ranges_overlap_u64(
                (
                    mappings[left].physical_page,
                    mappings[left].physical_page + mappings[left].mapped_length as u64,
                ),
                (
                    mappings[right].physical_page,
                    mappings[right].physical_page + mappings[right].mapped_length as u64,
                ),
            ) {
                return false;
            }
        }
    }
    let table = unsafe { core::ptr::addr_of!((*PAGING.mmio_table.get()).0).cast::<u64>() };
    (0..ENTRY_COUNT).all(|index| {
        let expected = mappings.into_iter().find_map(|mapping| {
            let first_index = (mapping.virtual_page - MMIO_WINDOW_BASE) / PAGE_SIZE;
            let pages = mapping.mapped_length / PAGE_SIZE;
            (first_index..first_index + pages)
                .contains(&index)
                .then_some(
                    (mapping.physical_page + ((index - first_index) * PAGE_SIZE) as u64)
                        | PRESENT
                        | CACHE_DISABLE
                        | NO_EXECUTE
                        | if mapping.writable { WRITABLE } else { 0 },
                )
        });
        let Some(expected) = expected else {
            return unsafe { table.add(index).read() == 0 };
        };
        let entry = unsafe { table.add(index).read() };
        entry & (ADDRESS_MASK as u64 | PRESENT | WRITABLE | USER | CACHE_DISABLE | NO_EXECUTE)
            == expected
            && entry & LARGE_PAGE == 0
    })
}

pub(crate) fn mmio_identity_alias_is_absent(mapping: MmioMapping) -> bool {
    let Some(end) = mapping
        .physical_page
        .checked_add(mapping.mapped_length as u64)
    else {
        return false;
    };
    if end > INITIAL_IDENTITY_LIMIT as u64 {
        return mapping.physical_page >= INITIAL_IDENTITY_LIMIT as u64;
    }
    (mapping.physical_page..end)
        .step_by(PAGE_SIZE)
        .all(|physical| {
            let physical = physical as usize;
            let region = physical / LARGE_PAGE_SIZE as usize;
            let directory = PAGING
                .page_directories
                .get()
                .cast_const()
                .cast::<PageTable>()
                .wrapping_add(region / ENTRY_COUNT);
            let directory_entries = unsafe { core::ptr::addr_of!((*directory).0).cast::<u64>() };
            let directory_entry = unsafe { directory_entries.add(region % ENTRY_COUNT).read() };
            if directory_entry & (PRESENT | LARGE_PAGE) != PRESENT {
                return false;
            }
            let table = (directory_entry as usize & ADDRESS_MASK) as *const u64;
            let page = (physical % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
            unsafe { table.add(page).read() == 0 }
        })
}

pub(crate) fn mmio_mapping_is_writable(mapping: MmioMapping) -> bool {
    mmio_mapping_is_valid(mapping) && mapping.writable
}

pub(crate) fn mmio_translation_is_bounded(
    mapping: MmioMapping,
    physical: u64,
    length: usize,
) -> bool {
    let Some(virtual_start) = mapping.address(physical, length) else {
        return false;
    };
    let Some(last_physical) = physical.checked_add(length.saturating_sub(1) as u64) else {
        return false;
    };
    let Some(after) = physical.checked_add(length as u64) else {
        return false;
    };
    mapping.address(physical, 0).is_none()
        && physical
            .checked_sub(1)
            .is_none_or(|before| mapping.address(before, 1).is_none())
        && mapping.address(last_physical, 1) == virtual_start.checked_add(length - 1)
        && mapping.address(after, 1).is_none()
        && mapping.address(u64::MAX, 1).is_none()
}

pub(crate) fn static_page_tables_are_read_only() -> bool {
    const TABLE_PAGE_COUNT: usize = 5 + DIRECTORY_COUNT + MAX_FINE_TABLES;
    let mut table_pages = [0_usize; TABLE_PAGE_COUNT];
    table_pages[0] = PAGING.pml4.get().cast_const() as usize;
    table_pages[1] = PAGING.pdpt.get().cast_const() as usize;
    table_pages[2] = PAGING.direct_pdpt.get().cast_const() as usize;
    table_pages[3] = PAGING.mmio_directory.get().cast_const() as usize;
    table_pages[4] = PAGING.mmio_table.get().cast_const() as usize;
    let directories_address = PAGING.page_directories.get().cast_const() as usize;
    let fine_tables_address = PAGING.fine_page_tables.get().cast_const() as usize;
    for index in 0..DIRECTORY_COUNT {
        table_pages[5 + index] = directories_address + index * PAGE_SIZE;
    }
    for index in 0..MAX_FINE_TABLES {
        table_pages[5 + DIRECTORY_COUNT + index] = fine_tables_address + index * PAGE_SIZE;
    }
    table_pages.into_iter().all(|address| unsafe {
        let region = address / LARGE_PAGE_SIZE as usize;
        let directory = PAGING
            .page_directories
            .get()
            .cast_const()
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
        let directories = PAGING
            .page_directories
            .get()
            .cast_const()
            .cast::<PageTable>();
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
                    || ranges_overlap(image.image_range(), (region_start, region_end))
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
                let image_page =
                    ranges_overlap(image.image_range(), (address, address + PAGE_SIZE));
                let image_writable = image
                    .writable_ranges()
                    .iter()
                    .any(|(start, end)| address < *end && *start < address + PAGE_SIZE);
                let expected_writable = image_writable && !is_static_table_page(address);
                let executable = image_code
                    || range_contains_runtime_code(boot_info, address, address + PAGE_SIZE);
                if executable == (leaf & NO_EXECUTE != 0)
                    || (image_page && expected_writable != (leaf & WRITABLE != 0))
                {
                    return false;
                }
            }
        }
    }
    true
}

pub(crate) fn identity_range_is_read_only_data(start: usize, length: usize) -> bool {
    let Some(last) = length
        .checked_sub(1)
        .and_then(|offset| start.checked_add(offset))
    else {
        return false;
    };
    if last >= INITIAL_IDENTITY_LIMIT {
        return false;
    }
    let first_page = start / PAGE_SIZE * PAGE_SIZE;
    let last_page = last / PAGE_SIZE * PAGE_SIZE;
    let mut address = first_page;
    loop {
        let region = address / LARGE_PAGE_SIZE as usize;
        let directory = unsafe {
            PAGING
                .page_directories
                .get()
                .cast_const()
                .cast::<PageTable>()
                .add(region / ENTRY_COUNT)
        };
        let entries = unsafe { core::ptr::addr_of!((*directory).0).cast::<u64>() };
        let directory_entry = unsafe { entries.add(region % ENTRY_COUNT).read() };
        let permissions_valid = if directory_entry & LARGE_PAGE != 0 {
            directory_entry & PRESENT != 0
                && directory_entry & WRITABLE == 0
                && directory_entry & NO_EXECUTE != 0
        } else {
            let table = (directory_entry as usize & ADDRESS_MASK) as *const u64;
            if directory_entry & PRESENT == 0 || table.is_null() {
                false
            } else {
                let page = (address % LARGE_PAGE_SIZE as usize) / PAGE_SIZE;
                let leaf = unsafe { table.add(page).read() };
                leaf & PRESENT != 0 && leaf & WRITABLE == 0 && leaf & NO_EXECUTE != 0
            }
        };
        if !permissions_valid {
            return false;
        }
        if address == last_page {
            return true;
        }
        let Some(next) = address.checked_add(PAGE_SIZE) else {
            return false;
        };
        address = next;
    }
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

const fn ranges_overlap(left: (usize, usize), right: (usize, usize)) -> bool {
    left.0 < right.1 && right.0 < left.1
}

const fn ranges_overlap_u64(left: (u64, u64), right: (u64, u64)) -> bool {
    left.0 < right.1 && right.0 < left.1
}

fn is_static_table_page(address: usize) -> bool {
    if address == PAGING.pml4.get().cast_const() as usize
        || address == PAGING.pdpt.get().cast_const() as usize
        || address == PAGING.direct_pdpt.get().cast_const() as usize
        || address == PAGING.mmio_directory.get().cast_const() as usize
        || address == PAGING.mmio_table.get().cast_const() as usize
    {
        return true;
    }
    let directories = PAGING.page_directories.get().cast_const() as usize;
    let directories_end = directories + DIRECTORY_COUNT * PAGE_SIZE;
    let fine_tables = PAGING.fine_page_tables.get().cast_const() as usize;
    let fine_tables_end = fine_tables + MAX_FINE_TABLES * PAGE_SIZE;
    (directories..directories_end).contains(&address)
        || (fine_tables..fine_tables_end).contains(&address)
}

unsafe fn zero_entries(entries: *mut u64) {
    for index in 0..ENTRY_COUNT {
        entries.add(index).write(0);
    }
}
