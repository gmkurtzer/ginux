use core::ptr::read_volatile;

use crate::memory::{self, FrameLease, FrameOwner};

use super::paging::MmioMapping;

const VERSION_OFFSET: u64 = 0x00;
const CAPABILITY_OFFSET: u64 = 0x08;
const EXTENDED_CAPABILITY_OFFSET: u64 = 0x10;
const GLOBAL_STATUS_OFFSET: u64 = 0x1c;
const ROOT_TABLE_ADDRESS_OFFSET: u64 = 0x20;
const TRANSLATION_ENABLE_STATUS: u32 = 1 << 31;
const SUPPORTED_ADDRESS_WIDTHS_SHIFT: u64 = 8;
const SUPPORTED_ADDRESS_WIDTHS_MASK: u64 = 0x1f;
const SUPPORTED_48_BIT_WIDTH: u64 = 1 << 2;
const PAGE_SIZE: u64 = 4096;
const TABLE_ENTRIES: usize = 512;
const TRANSLATION_FRAME_COUNT: usize = 6;
const ROOT_FRAME: usize = 0;
const CONTEXT_FRAME: usize = 1;
const PML4_FRAME: usize = 2;
const PDPT_FRAME: usize = 3;
const PAGE_DIRECTORY_FRAME: usize = 4;
const PAGE_TABLE_FRAME: usize = 5;
const PRESENT: u64 = 1;
const READ_WRITE: u64 = 3;
const ADDRESS_MASK_48: u64 = 0x0000_ffff_ffff_f000;
const CONTEXT_ADDRESS_WIDTH_48: u64 = 2;
const DOMAIN_ID: u16 = 1;

#[derive(Clone, Copy)]
pub(crate) struct Capabilities {
    pub(crate) major_version: u8,
    pub(crate) minor_version: u8,
    pub(crate) capability: u64,
    pub(crate) extended_capability: u64,
}

pub(crate) struct InactiveTranslation {
    frames: [FrameLease; TRANSLATION_FRAME_COUNT],
    bus: u8,
    device: u8,
    function: u8,
    dma_page: u64,
}

impl InactiveTranslation {
    pub(crate) fn root_table_address(&self) -> u64 {
        self.frames[ROOT_FRAME].address()
    }

    pub(crate) fn is_exact(&self, dma_frame: FrameLease) -> bool {
        if dma_frame.address() != self.dma_page
            || !memory::frames_owned_by(FrameOwner::KERNEL, &self.frames)
        {
            return false;
        }
        let devfn = usize::from(self.device) * 8 + usize::from(self.function);
        let indices = translation_indices(self.dma_page);
        frame_words_are_exact(
            self.frames[ROOT_FRAME],
            &[(
                usize::from(self.bus) * 2,
                self.frames[CONTEXT_FRAME].address() | PRESENT,
            )],
        ) && frame_words_are_exact(
            self.frames[CONTEXT_FRAME],
            &[
                (devfn * 2, self.frames[PML4_FRAME].address() | PRESENT),
                (
                    devfn * 2 + 1,
                    (u64::from(DOMAIN_ID) << 8) | CONTEXT_ADDRESS_WIDTH_48,
                ),
            ],
        ) && frame_words_are_exact(
            self.frames[PML4_FRAME],
            &[(indices[0], self.frames[PDPT_FRAME].address() | READ_WRITE)],
        ) && frame_words_are_exact(
            self.frames[PDPT_FRAME],
            &[(
                indices[1],
                self.frames[PAGE_DIRECTORY_FRAME].address() | READ_WRITE,
            )],
        ) && frame_words_are_exact(
            self.frames[PAGE_DIRECTORY_FRAME],
            &[(
                indices[2],
                self.frames[PAGE_TABLE_FRAME].address() | READ_WRITE,
            )],
        ) && frame_words_are_exact(
            self.frames[PAGE_TABLE_FRAME],
            &[(indices[3], self.dma_page | READ_WRITE)],
        )
    }

    pub(crate) fn retire(self) -> bool {
        memory::scrub_and_release_frames(FrameOwner::KERNEL, &self.frames)
            && !memory::any_frame_lease_live(&self.frames)
    }
}

pub(crate) fn probe(register_base: u64, mapping: MmioMapping) -> Option<Capabilities> {
    let version = read_u32(mapping, register_base.checked_add(VERSION_OFFSET)?)?;
    let capability = read_u64(mapping, register_base.checked_add(CAPABILITY_OFFSET)?)?;
    let extended_capability = read_u64(
        mapping,
        register_base.checked_add(EXTENDED_CAPABILITY_OFFSET)?,
    )?;
    let global_status = read_u32(mapping, register_base.checked_add(GLOBAL_STATUS_OFFSET)?)?;
    let root_table_address = read_u64(
        mapping,
        register_base.checked_add(ROOT_TABLE_ADDRESS_OFFSET)?,
    )?;
    let major_version = ((version >> 4) & 0x0f) as u8;
    let minor_version = (version & 0x0f) as u8;
    if major_version == 0
        || capability == 0
        || capability >> SUPPORTED_ADDRESS_WIDTHS_SHIFT & SUPPORTED_ADDRESS_WIDTHS_MASK == 0
        || global_status & TRANSLATION_ENABLE_STATUS != 0
        || root_table_address != 0
    {
        return None;
    }
    Some(Capabilities {
        major_version,
        minor_version,
        capability,
        extended_capability,
    })
}

pub(crate) fn build_inactive_translation(
    capabilities: Capabilities,
    bus: u8,
    device: u8,
    function: u8,
    dma_frame: FrameLease,
) -> Option<InactiveTranslation> {
    if capabilities.capability >> SUPPORTED_ADDRESS_WIDTHS_SHIFT
        & SUPPORTED_ADDRESS_WIDTHS_MASK
        & SUPPORTED_48_BIT_WIDTH
        == 0
        || device >= 32
        || function >= 8
        || !page_fits_48_bits(dma_frame.address())
    {
        return None;
    }
    let frames = memory::allocate_frames::<TRANSLATION_FRAME_COUNT>(FrameOwner::KERNEL)?;
    if frames
        .iter()
        .any(|frame| !page_fits_48_bits(frame.address()))
    {
        let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &frames);
        return None;
    }
    let translation = InactiveTranslation {
        frames,
        bus,
        device,
        function,
        dma_page: dma_frame.address(),
    };
    let devfn = usize::from(device) * 8 + usize::from(function);
    let indices = translation_indices(dma_frame.address());
    let initialized = write_word(
        frames[ROOT_FRAME],
        usize::from(bus) * 2,
        frames[CONTEXT_FRAME].address() | PRESENT,
    ) && write_word(
        frames[CONTEXT_FRAME],
        devfn * 2,
        frames[PML4_FRAME].address() | PRESENT,
    ) && write_word(
        frames[CONTEXT_FRAME],
        devfn * 2 + 1,
        (u64::from(DOMAIN_ID) << 8) | CONTEXT_ADDRESS_WIDTH_48,
    ) && write_word(
        frames[PML4_FRAME],
        indices[0],
        frames[PDPT_FRAME].address() | READ_WRITE,
    ) && write_word(
        frames[PDPT_FRAME],
        indices[1],
        frames[PAGE_DIRECTORY_FRAME].address() | READ_WRITE,
    ) && write_word(
        frames[PAGE_DIRECTORY_FRAME],
        indices[2],
        frames[PAGE_TABLE_FRAME].address() | READ_WRITE,
    ) && write_word(
        frames[PAGE_TABLE_FRAME],
        indices[3],
        dma_frame.address() | READ_WRITE,
    );
    if !initialized || !translation.is_exact(dma_frame) {
        let _ = translation.retire();
        return None;
    }
    Some(translation)
}

pub(crate) fn verify_inactive_translation_rejections(
    capabilities: Capabilities,
    dma_frame: FrameLease,
) -> bool {
    let mut unsupported = capabilities;
    unsupported.capability &= !(SUPPORTED_48_BIT_WIDTH << SUPPORTED_ADDRESS_WIDTHS_SHIFT);
    build_inactive_translation(unsupported, 0, 2, 0, dma_frame).is_none()
        && build_inactive_translation(capabilities, 0, 32, 0, dma_frame).is_none()
        && build_inactive_translation(capabilities, 0, 2, 8, dma_frame).is_none()
}

fn translation_indices(address: u64) -> [usize; 4] {
    [
        ((address >> 39) & 0x1ff) as usize,
        ((address >> 30) & 0x1ff) as usize,
        ((address >> 21) & 0x1ff) as usize,
        ((address >> 12) & 0x1ff) as usize,
    ]
}

fn page_fits_48_bits(address: u64) -> bool {
    address.is_multiple_of(PAGE_SIZE)
        && address
            .checked_add(PAGE_SIZE - 1)
            .is_some_and(|end| end <= ADDRESS_MASK_48 | (PAGE_SIZE - 1))
}

fn write_word(frame: FrameLease, index: usize, value: u64) -> bool {
    let Some(address) = frame.direct_map_address() else {
        return false;
    };
    if index >= TABLE_ENTRIES {
        return false;
    }
    unsafe {
        (address as *mut u64).add(index).write(value.to_le());
    }
    true
}

fn frame_words_are_exact(frame: FrameLease, expected: &[(usize, u64)]) -> bool {
    let Some(address) = frame.direct_map_address() else {
        return false;
    };
    if expected.iter().enumerate().any(|(index, (word, _))| {
        *word >= TABLE_ENTRIES
            || expected[..index]
                .iter()
                .any(|(prior_word, _)| prior_word == word)
    }) {
        return false;
    }
    (0..TABLE_ENTRIES).all(|index| {
        let actual = u64::from_le(unsafe { (address as *const u64).add(index).read() });
        let wanted = expected
            .iter()
            .find_map(|(word, value)| (*word == index).then_some(*value))
            .unwrap_or(0);
        actual == wanted
    })
}

fn read_u32(mapping: MmioMapping, physical: u64) -> Option<u32> {
    let address = mapping.address(physical, size_of::<u32>())?;
    Some(u32::from_le(unsafe {
        read_volatile(address as *const u32)
    }))
}

fn read_u64(mapping: MmioMapping, physical: u64) -> Option<u64> {
    let address = mapping.address(physical, size_of::<u64>())?;
    Some(u64::from_le(unsafe {
        read_volatile(address as *const u64)
    }))
}
