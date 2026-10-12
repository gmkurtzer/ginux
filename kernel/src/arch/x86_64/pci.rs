use core::arch::{asm, global_asm};
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{compiler_fence, AtomicBool, AtomicU64, AtomicU8, Ordering};

use super::{
    apic::{LocalApic, Mode},
    cpu,
    paging::MmioMapping,
    timer,
};
use crate::{
    control_abi,
    scheduler::{self, UserInterruptFrame},
    volume,
};

const CONFIG_ADDRESS_PORT: u16 = 0x0cf8;
const CONFIG_DATA_PORT: u16 = 0x0cfc;
const CONFIG_ENABLE: u32 = 1 << 31;
const CONFIG_SIZE: usize = 256;
const CONFIG_DWORDS: usize = CONFIG_SIZE / 4;
const VIRTIO_VENDOR: u16 = 0x1af4;
const VIRTIO_NETWORK_DEVICE: u16 = 0x1041;
const VIRTIO_BLOCK_DEVICE: u16 = 0x1042;
const STATUS_CAPABILITIES: u16 = 1 << 4;
const CAPABILITY_VENDOR_SPECIFIC: u8 = 0x09;
const CAPABILITY_MSIX: u8 = 0x11;
const MSIX_CAPABILITY_SIZE: usize = 12;
const MSIX_TABLE_SIZE_MASK: u16 = 0x07ff;
const MSIX_FUNCTION_MASK: u16 = 1 << 14;
const MSIX_ENABLE: u16 = 1 << 15;
const MSIX_BIR_MASK: u32 = 0x7;
const MSIX_OFFSET_MASK: u32 = !MSIX_BIR_MASK;
const MSIX_TABLE_ENTRY_SIZE: u32 = 16;
const MSIX_PBA_BITS_PER_ENTRY: u32 = 64;
const MSIX_PBA_ENTRY_SIZE: u32 = 8;
const MSIX_VECTOR_MASK: u32 = 1;
const MSI_ADDRESS_BASE: u32 = 0xfee0_0000;
const MSI_DESTINATION_SHIFT: u32 = 12;
const FIRST_DEVICE_VECTOR: u8 = 33;
const LAST_DEVICE_VECTOR: u8 = 0xef;
const SYSCALL_VECTOR: u8 = 0x80;
const VIRTIO_INTERRUPT_VECTOR: u8 = FIRST_DEVICE_VECTOR;
const VIRTIO_NETWORK_RX_VECTOR: u8 = FIRST_DEVICE_VECTOR + 1;
const VIRTIO_NETWORK_TX_VECTOR: u8 = FIRST_DEVICE_VECTOR + 2;
const APIC_EOI: u32 = 0xb0;
const VIRTIO_COMMON: u8 = 1;
const VIRTIO_NOTIFY: u8 = 2;
const VIRTIO_ISR: u8 = 3;
const VIRTIO_DEVICE: u8 = 4;
const MAX_CAPABILITIES: usize = 48;
const PCI_COMMAND: u8 = 0x04;
const COMMAND_IO_SPACE: u16 = 1 << 0;
const COMMAND_MEMORY_SPACE: u16 = 1 << 1;
const COMMAND_BUS_MASTER: u16 = 1 << 2;
const COMMON_DEVICE_FEATURE_SELECT: usize = 0x00;
const COMMON_DEVICE_FEATURE: usize = 0x04;
const COMMON_DRIVER_FEATURE_SELECT: usize = 0x08;
const COMMON_DRIVER_FEATURE: usize = 0x0c;
const COMMON_NUM_QUEUES: usize = 0x12;
const COMMON_DEVICE_STATUS: usize = 0x14;
const COMMON_CONFIG_GENERATION: usize = 0x15;
const COMMON_QUEUE_SELECT: usize = 0x16;
const COMMON_QUEUE_SIZE: usize = 0x18;
const COMMON_QUEUE_MSIX_VECTOR: usize = 0x1a;
const COMMON_QUEUE_ENABLE: usize = 0x1c;
const COMMON_QUEUE_NOTIFY_OFF: usize = 0x1e;
const COMMON_QUEUE_DESC: usize = 0x20;
const COMMON_QUEUE_DRIVER: usize = 0x28;
const COMMON_QUEUE_DEVICE: usize = 0x30;
const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FEATURES_OK: u8 = 8;
const STATUS_DEVICE_NEEDS_RESET: u8 = 64;
const STATUS_FAILED: u8 = 128;
const FEATURE_NET_MAC: u64 = 1 << 5;
const FEATURE_NET_MRG_RXBUF: u64 = 1 << 15;
const FEATURE_FLUSH: u64 = 1 << 9;
const FEATURE_VERSION_1: u64 = 1 << 32;
const FEATURE_ACCESS_PLATFORM: u64 = 1 << 33;
const RESET_POLL_LIMIT: usize = 100_000;
const QUEUE_POLL_LIMIT: usize = 10_000_000;
const DMA_PAGE_SIZE: usize = 4096;
const QUEUE_SIZE: u16 = 8;
const DESC_NEXT: u16 = 1;
const DESC_WRITE: u16 = 2;
const DESC_OFFSET: usize = 0x000;
const AVAIL_OFFSET: usize = 0x100;
const USED_OFFSET: usize = 0x200;
const REQUEST_OFFSET: usize = 0x400;
const DATA_OFFSET: usize = 0x600;
const DATA_LENGTH: usize = 512;
const STATUS_OFFSET: usize = 0x900;
const NET_RX_DESC_OFFSET: usize = 0x000;
const NET_RX_AVAIL_OFFSET: usize = 0x080;
const NET_RX_USED_OFFSET: usize = 0x100;
const NET_TX_DESC_OFFSET: usize = 0x180;
const NET_TX_AVAIL_OFFSET: usize = 0x200;
const NET_TX_USED_OFFSET: usize = 0x280;
const NET_RX_BUFFER_OFFSET: usize = 0x400;
const NET_TX_BUFFER_OFFSET: usize = 0xa00;
const NET_BUFFER_LENGTH: usize = 0x600;
const NET_DESCRIPTOR_BYTES: usize = QUEUE_SIZE as usize * 16;
const NET_AVAILABLE_BYTES: usize = 6 + QUEUE_SIZE as usize * 2;
const NET_USED_BYTES: usize = 6 + QUEUE_SIZE as usize * 8;

static VIRTIO_INTERRUPT_FIRED: AtomicBool = AtomicBool::new(false);
static VIRTIO_INTERRUPT_ACTIVE: AtomicBool = AtomicBool::new(false);
static VIRTIO_INTERRUPT_APIC_BASE: AtomicU64 = AtomicU64::new(0);
static VIRTIO_INTERRUPT_APIC_MODE: AtomicU8 = AtomicU8::new(0);
static VIRTIO_INTERRUPT_COUNT: AtomicU8 = AtomicU8::new(0);
static VIRTIO_INTERRUPT_DOMAIN: AtomicU64 = AtomicU64::new(0);
static VIRTIO_INTERRUPT_REQUEST: AtomicU64 = AtomicU64::new(0);
static FORCED_NOTIFIED_TIMEOUT_DOMAIN: AtomicU64 = AtomicU64::new(0);
static FORCED_NOTIFIED_TIMEOUT_REQUEST: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum IsolatedInterruptCompletion {
    Completed,
    TimedOut,
    Failed,
}

global_asm!(
    r#"
    .text
    .global ginux_virtio_interrupt_entry
ginux_virtio_interrupt_entry:
    clac
    cld
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov r12, rsp
    mov rdi, r12
    and rsp, -16
    call {interrupt_handler}
    mov rsp, r12
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    iretq
"#,
    interrupt_handler = sym virtio_interrupt_handler,
);

extern "C" {
    fn ginux_virtio_interrupt_entry();
}

#[derive(Clone, Copy)]
struct ConfigSpace {
    bytes: [u8; CONFIG_SIZE],
}

#[derive(Clone, Copy)]
pub(crate) struct VirtioBlockDevice {
    pub(crate) bus: u8,
    pub(crate) device: u8,
    pub(crate) function: u8,
    pub(crate) common_bar_address: u64,
    pub(crate) common_offset: u32,
    pub(crate) common_length: u32,
    notify_bar_address: u64,
    notify_offset: u32,
    notify_length: u32,
    isr_bar_address: u64,
    isr_offset: u32,
    isr_length: u32,
    device_bar_address: u64,
    device_offset: u32,
    device_length: u32,
    pub(crate) notify_multiplier: u32,
    pub(crate) capability_count: u8,
    pub(crate) msix: MsixCapability,
}

#[derive(Clone, Copy)]
pub(crate) struct VirtioNetworkDevice {
    pub(crate) bus: u8,
    pub(crate) device: u8,
    pub(crate) function: u8,
    pub(crate) common_bar_address: u64,
    pub(crate) common_offset: u32,
    pub(crate) common_length: u32,
    notify_bar_address: u64,
    notify_offset: u32,
    notify_length: u32,
    device_bar_address: u64,
    device_offset: u32,
    device_length: u32,
    pub(crate) notify_multiplier: u32,
    pub(crate) capability_count: u8,
    pub(crate) msix: MsixCapability,
}

#[derive(Clone, Copy)]
pub(crate) struct MsixCapability {
    pub(crate) capability_offset: u8,
    pub(crate) vector_count: u16,
    pub(crate) table_bar_address: u64,
    pub(crate) table_offset: u32,
    pub(crate) table_length: u32,
    pub(crate) pba_bar_address: u64,
    pub(crate) pba_offset: u32,
    pub(crate) pba_length: u32,
    pub(crate) function_masked: bool,
}

#[derive(Clone, Copy)]
struct MsixTarget {
    bus: u8,
    device: u8,
    function: u8,
    capability: MsixCapability,
}

impl VirtioBlockDevice {
    fn msix_target(self) -> MsixTarget {
        MsixTarget {
            bus: self.bus,
            device: self.device,
            function: self.function,
            capability: self.msix,
        }
    }
}

impl VirtioNetworkDevice {
    fn msix_target(self) -> MsixTarget {
        MsixTarget {
            bus: self.bus,
            device: self.device,
            function: self.function,
            capability: self.msix,
        }
    }
}

impl VirtioNetworkDevice {
    pub(crate) fn common_region(self) -> Option<(u64, usize)> {
        let physical = self
            .common_bar_address
            .checked_add(u64::from(self.common_offset))?;
        let length = usize::try_from(self.common_length).ok()?;
        physical.checked_add(length as u64)?;
        (length > COMMON_QUEUE_DEVICE).then_some((physical, length))
    }

    pub(crate) fn device_region(self) -> Option<(u64, usize)> {
        region_bounds(
            self.device_bar_address,
            self.device_offset,
            self.device_length,
        )
    }

    pub(crate) fn notify_region(self) -> Option<(u64, usize)> {
        region_bounds(
            self.notify_bar_address,
            self.notify_offset,
            self.notify_length,
        )
    }

    pub(crate) fn msix_table_region(self) -> Option<(u64, usize)> {
        region_bounds(
            self.msix.table_bar_address,
            self.msix.table_offset,
            self.msix.table_length,
        )
    }

    pub(crate) fn msix_pba_region(self) -> Option<(u64, usize)> {
        region_bounds(
            self.msix.pba_bar_address,
            self.msix.pba_offset,
            self.msix.pba_length,
        )
    }

    pub(crate) fn msix_mmio_span(self) -> Option<(u64, usize)> {
        let (table, table_length) = self.msix_table_region()?;
        let (pba, pba_length) = self.msix_pba_region()?;
        let start = table.min(pba);
        let end = table
            .checked_add(table_length as u64)?
            .max(pba.checked_add(pba_length as u64)?);
        let length = usize::try_from(end.checked_sub(start)?).ok()?;
        (length != 0).then_some((start, length))
    }
}

impl VirtioBlockDevice {
    pub(crate) fn common_region(self) -> Option<(u64, usize)> {
        let physical = self
            .common_bar_address
            .checked_add(u64::from(self.common_offset))?;
        let length = usize::try_from(self.common_length).ok()?;
        physical.checked_add(length as u64)?;
        (length > COMMON_DEVICE_STATUS).then_some((physical, length))
    }

    pub(crate) fn status_register_physical(self) -> Option<u64> {
        let (physical, length) = self.common_region()?;
        (length > COMMON_DEVICE_STATUS).then(|| physical + COMMON_DEVICE_STATUS as u64)
    }

    pub(crate) fn mmio_span(self) -> Option<(u64, usize)> {
        let regions = [
            (
                self.common_bar_address,
                self.common_offset,
                self.common_length,
            ),
            (
                self.notify_bar_address,
                self.notify_offset,
                self.notify_length,
            ),
            (self.isr_bar_address, self.isr_offset, self.isr_length),
            (
                self.device_bar_address,
                self.device_offset,
                self.device_length,
            ),
        ];
        let mut start = u64::MAX;
        let mut end = 0_u64;
        for (bar, offset, length) in regions {
            let region_start = bar.checked_add(u64::from(offset))?;
            let region_end = region_start.checked_add(u64::from(length))?;
            start = start.min(region_start);
            end = end.max(region_end);
        }
        let length = usize::try_from(end.checked_sub(start)?).ok()?;
        (length != 0).then_some((start, length))
    }

    fn notify_region(self) -> Option<(u64, usize)> {
        region_bounds(
            self.notify_bar_address,
            self.notify_offset,
            self.notify_length,
        )
    }

    fn device_region(self) -> Option<(u64, usize)> {
        region_bounds(
            self.device_bar_address,
            self.device_offset,
            self.device_length,
        )
    }

    pub(crate) fn msix_table_region(self) -> Option<(u64, usize)> {
        region_bounds(
            self.msix.table_bar_address,
            self.msix.table_offset,
            self.msix.table_length,
        )
    }

    pub(crate) fn msix_pba_region(self) -> Option<(u64, usize)> {
        region_bounds(
            self.msix.pba_bar_address,
            self.msix.pba_offset,
            self.msix.pba_length,
        )
    }

    pub(crate) fn msix_mmio_span(self) -> Option<(u64, usize)> {
        let (table, table_length) = self.msix_table_region()?;
        let (pba, pba_length) = self.msix_pba_region()?;
        let start = table.min(pba);
        let end = table
            .checked_add(table_length as u64)?
            .max(pba.checked_add(pba_length as u64)?);
        let length = usize::try_from(end.checked_sub(start)?).ok()?;
        (length != 0).then_some((start, length))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VirtioBlockInitialization {
    pub(crate) offered_features: u64,
    pub(crate) accepted_features: u64,
    pub(crate) queue_count: u16,
}

#[derive(Clone, Copy)]
pub(crate) struct VirtioNetworkInitialization {
    pub(crate) offered_features: u64,
    pub(crate) accepted_features: u64,
    pub(crate) queue_count: u16,
    pub(crate) mac: [u8; 6],
    pub(crate) receive_queue_size: u16,
    pub(crate) transmit_queue_size: u16,
    pub(crate) receive_notification: u64,
    pub(crate) transmit_notification: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct VirtioNetworkTransmit {
    pub(crate) frame_length: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct VirtioNetworkReceive {
    pub(crate) frame_length: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct VirtioBlockRead {
    pub(crate) capacity_sectors: u64,
    pub(crate) used_length: u32,
}

pub(crate) struct UnmappedDmaAttempt {
    completion_published: bool,
    used_index: u16,
    used_id: u32,
    used_length: u32,
    request_status: u8,
    device_status: u8,
    mapped_payload_unchanged: bool,
    descriptor_unchanged: bool,
}

impl UnmappedDmaAttempt {
    pub(crate) fn transport_outcome_is_bounded(&self, prior_used_index: u16) -> bool {
        let completion_is_safe = if self.completion_published {
            self.used_index == prior_used_index.wrapping_add(1)
                && self.used_id == 0
                && self.used_length == (DATA_LENGTH + 1) as u32
                && self.request_status == 0
                && self.device_status
                    == STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK
        } else {
            self.used_index == prior_used_index && self.request_status == 0xff
        };
        completion_is_safe && self.mapped_payload_unchanged && self.descriptor_unchanged
    }
}

pub(crate) struct VirtioBlockBackend {
    device: VirtioBlockDevice,
    common: usize,
    notification: usize,
    dma_physical: u64,
    dma_virtual: usize,
    capacity_sectors: u64,
    queue_size: u16,
    available_index: u16,
    used_index: u16,
    active: bool,
}

enum BlockOperation<'a> {
    Read {
        sector: u64,
        output: &'a mut [u8; DATA_LENGTH],
    },
    Write {
        sector: u64,
        input: &'a [u8; DATA_LENGTH],
    },
    Flush,
}

#[derive(Clone, Copy)]
struct Region {
    bar_address: u64,
    offset: u32,
    length: u32,
}

/// Finds exactly one modern virtio block function on PCI bus zero.
///
/// This first profile intentionally scans function zero only. Configuration
/// mechanism 1 access is serialized by the single bootstrap processor while
/// interrupts are disabled.
pub(crate) fn discover_virtio_block() -> Option<VirtioBlockDevice> {
    discover_virtio_device(VIRTIO_BLOCK_DEVICE, parse_virtio_block)
}

/// Finds exactly one modern virtio network function on PCI bus zero without
/// changing its command register or touching MMIO.
pub(crate) fn discover_virtio_network() -> Option<VirtioNetworkDevice> {
    discover_virtio_device(VIRTIO_NETWORK_DEVICE, parse_virtio_network)
}

fn discover_virtio_device<T: Copy>(
    expected_device: u16,
    parse: fn(&ConfigSpace, u8, u8, u8) -> Option<T>,
) -> Option<T> {
    let mut found = None;
    for device in 0_u8..32 {
        let identity = unsafe { read_config_u32(0, device, 0, 0) };
        if identity as u16 == u16::MAX {
            continue;
        }
        if identity as u16 != VIRTIO_VENDOR || (identity >> 16) as u16 != expected_device {
            continue;
        }
        if found.is_some() {
            return None;
        }
        let config = unsafe { read_config_space(0, device, 0) };
        found = parse(&config, 0, device, 0);
        found?;
    }
    found
}

pub(crate) fn network_function_is_disabled(device: VirtioNetworkDevice) -> bool {
    let command = network_function_command(device);
    command & (COMMAND_IO_SPACE | COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER) == 0
}

pub(crate) fn network_msix_is_disabled(device: VirtioNetworkDevice) -> bool {
    device
        .msix
        .capability_offset
        .checked_add(2)
        .is_some_and(|offset| unsafe {
            read_config_u16(device.bus, device.device, device.function, offset) & MSIX_ENABLE == 0
        })
}

pub(crate) fn network_function_command(device: VirtioNetworkDevice) -> u16 {
    unsafe { read_config_u16(device.bus, device.device, device.function, PCI_COMMAND) }
}

pub(crate) fn revoke_network_function(device: VirtioNetworkDevice) -> bool {
    let command = network_function_command(device);
    unsafe {
        write_config_u16(
            device.bus,
            device.device,
            device.function,
            PCI_COMMAND,
            command & !(COMMAND_IO_SPACE | COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER),
        );
    }
    network_function_is_disabled(device)
}

pub(crate) fn verify_parser_rejections(device: VirtioBlockDevice) -> bool {
    let config = unsafe { read_config_space(device.bus, device.device, device.function) };
    verify_parser_rejections_for(
        &config,
        VIRTIO_BLOCK_DEVICE,
        device.bus,
        device.device,
        device.function,
    )
}

pub(crate) fn verify_network_parser_rejections(device: VirtioNetworkDevice) -> bool {
    let config = unsafe { read_config_space(device.bus, device.device, device.function) };
    verify_parser_rejections_for(
        &config,
        VIRTIO_NETWORK_DEVICE,
        device.bus,
        device.device,
        device.function,
    )
}

fn verify_parser_rejections_for(
    config: &ConfigSpace,
    expected_device: u16,
    bus: u8,
    device: u8,
    function: u8,
) -> bool {
    let mut wrong_identity = *config;
    wrong_identity.bytes[2] ^= 1;

    let mut no_capabilities = *config;
    no_capabilities.bytes[6] &= !(STATUS_CAPABILITIES as u8);

    let mut unaligned_head = *config;
    unaligned_head.bytes[0x34] |= 1;

    let head = usize::from(config.bytes[0x34]);
    if !(0x40..=0xfc).contains(&head) || head % 4 != 0 {
        return false;
    }
    let mut cycle = *config;
    cycle.bytes[head + 1] = head as u8;

    let Some(required) = first_required_capability(config) else {
        return false;
    };
    let mut invalid_length = *config;
    invalid_length.bytes[required + 2] = 0;

    let mut invalid_bar = *config;
    invalid_bar.bytes[required + 4] = 6;

    let Some(msix) = first_capability(config, CAPABILITY_MSIX) else {
        return false;
    };
    let mut missing_msix = *config;
    missing_msix.bytes[msix] = 0;

    let mut enabled_msix = *config;
    let enabled_control = read_u16(&enabled_msix.bytes, msix + 2) | MSIX_ENABLE;
    enabled_msix.bytes[msix + 2..msix + 4].copy_from_slice(&enabled_control.to_le_bytes());

    let mut invalid_msix_table_bar = *config;
    let invalid_table = read_u32(&invalid_msix_table_bar.bytes, msix + 4) | MSIX_BIR_MASK;
    invalid_msix_table_bar.bytes[msix + 4..msix + 8].copy_from_slice(&invalid_table.to_le_bytes());

    let mut invalid_msix_pba_bar = *config;
    let invalid_pba = read_u32(&invalid_msix_pba_bar.bytes, msix + 8) | MSIX_BIR_MASK;
    invalid_msix_pba_bar.bytes[msix + 8..msix + 12].copy_from_slice(&invalid_pba.to_le_bytes());

    parse_virtio_config(&wrong_identity, expected_device, bus, device, function).is_none()
        && parse_virtio_config(&no_capabilities, expected_device, bus, device, function).is_none()
        && parse_virtio_config(&unaligned_head, expected_device, bus, device, function).is_none()
        && parse_virtio_config(&cycle, expected_device, bus, device, function).is_none()
        && parse_virtio_config(&invalid_length, expected_device, bus, device, function).is_none()
        && parse_virtio_config(&invalid_bar, expected_device, bus, device, function).is_none()
        && parse_virtio_config(&missing_msix, expected_device, bus, device, function).is_none()
        && parse_virtio_config(&enabled_msix, expected_device, bus, device, function).is_none()
        && parse_virtio_config(
            &invalid_msix_table_bar,
            expected_device,
            bus,
            device,
            function,
        )
        .is_none()
        && parse_virtio_config(
            &invalid_msix_pba_bar,
            expected_device,
            bus,
            device,
            function,
        )
        .is_none()
}

pub(crate) fn program_masked_msix_vector(
    device: VirtioBlockDevice,
    mapping: MmioMapping,
    index: u16,
    apic_id: u8,
    vector: u8,
) -> bool {
    program_masked_msix_vector_raw(device.msix_target(), mapping, index, apic_id, vector)
}

fn program_masked_msix_vector_raw(
    target: MsixTarget,
    mapping: MmioMapping,
    index: u16,
    apic_id: u8,
    vector: u8,
) -> bool {
    let msix = target.capability;
    if index >= msix.vector_count
        || !(FIRST_DEVICE_VECTOR..=LAST_DEVICE_VECTOR).contains(&vector)
        || vector == SYSCALL_VECTOR
    {
        return false;
    }
    let control_offset = match msix.capability_offset.checked_add(2) {
        Some(offset) => offset,
        None => return false,
    };
    if unsafe { read_config_u16(target.bus, target.device, target.function, control_offset) }
        & MSIX_ENABLE
        != 0
    {
        return false;
    }
    let Some((table_physical, table_length)) =
        region_bounds(msix.table_bar_address, msix.table_offset, msix.table_length)
    else {
        return false;
    };
    let Some(entry_offset) = usize::from(index).checked_mul(MSIX_TABLE_ENTRY_SIZE as usize) else {
        return false;
    };
    let Some(entry_physical) = table_physical.checked_add(entry_offset as u64) else {
        return false;
    };
    if entry_offset
        .checked_add(MSIX_TABLE_ENTRY_SIZE as usize)
        .is_none_or(|end| end > table_length)
    {
        return false;
    }
    let Some(entry) = mapping.address(entry_physical, MSIX_TABLE_ENTRY_SIZE as usize) else {
        return false;
    };
    let message_address = MSI_ADDRESS_BASE | u32::from(apic_id) << MSI_DESTINATION_SHIFT;
    write_mmio_u32(entry + 12, MSIX_VECTOR_MASK);
    compiler_fence(Ordering::SeqCst);
    if read_mmio_u32(entry + 12) & MSIX_VECTOR_MASK == 0 {
        return false;
    }
    write_mmio_u32(entry, message_address);
    write_mmio_u32(entry + 4, 0);
    write_mmio_u32(entry + 8, u32::from(vector));
    compiler_fence(Ordering::SeqCst);
    read_mmio_u32(entry) == message_address
        && read_mmio_u32(entry + 4) == 0
        && read_mmio_u32(entry + 8) == u32::from(vector)
        && read_mmio_u32(entry + 12) == MSIX_VECTOR_MASK
        && unsafe { read_config_u16(target.bus, target.device, target.function, control_offset) }
            & MSIX_ENABLE
            == 0
}

pub(crate) fn verify_msix_programming_rejections(
    device: VirtioBlockDevice,
    mapping: MmioMapping,
) -> bool {
    let Some((table_physical, _)) = device.msix_table_region() else {
        return false;
    };
    let Some(entry) = mapping.address(table_physical, MSIX_TABLE_ENTRY_SIZE as usize) else {
        return false;
    };
    let before = [
        read_mmio_u32(entry),
        read_mmio_u32(entry + 4),
        read_mmio_u32(entry + 8),
        read_mmio_u32(entry + 12),
    ];
    !program_masked_msix_vector(
        device,
        mapping,
        device.msix.vector_count,
        0,
        FIRST_DEVICE_VECTOR,
    ) && !program_masked_msix_vector(device, mapping, 0, 0, 31)
        && !program_masked_msix_vector(device, mapping, 0, 0, SYSCALL_VECTOR)
        && before
            == [
                read_mmio_u32(entry),
                read_mmio_u32(entry + 4),
                read_mmio_u32(entry + 8),
                read_mmio_u32(entry + 12),
            ]
}

pub(crate) fn verify_network_msix_programming_rejections(
    device: VirtioNetworkDevice,
    mapping: MmioMapping,
) -> bool {
    let Some((table_physical, table_length)) = device.msix_table_region() else {
        return false;
    };
    if table_length < 2 * MSIX_TABLE_ENTRY_SIZE as usize {
        return false;
    }
    let Some(table) = mapping.address(table_physical, 2 * MSIX_TABLE_ENTRY_SIZE as usize) else {
        return false;
    };
    let before: [u32; 8] = core::array::from_fn(|word| read_mmio_u32(table + word * 4));
    !program_masked_msix_vector_raw(
        device.msix_target(),
        mapping,
        device.msix.vector_count,
        0,
        VIRTIO_NETWORK_RX_VECTOR,
    ) && !program_masked_msix_vector_raw(device.msix_target(), mapping, 0, 0, 31)
        && !program_masked_msix_vector_raw(device.msix_target(), mapping, 1, 0, SYSCALL_VECTOR)
        && before == core::array::from_fn(|word| read_mmio_u32(table + word * 4))
}

pub(crate) fn program_masked_network_msix_vectors(
    device: VirtioNetworkDevice,
    mapping: MmioMapping,
    apic_id: u8,
) -> bool {
    if !network_function_is_disabled(device) {
        return false;
    }
    unsafe {
        write_config_u16(
            device.bus,
            device.device,
            device.function,
            PCI_COMMAND,
            COMMAND_MEMORY_SPACE,
        );
    }
    let programmed = network_function_command(device) == COMMAND_MEMORY_SPACE
        && device.msix.vector_count >= 2
        && program_masked_msix_vector_raw(
            device.msix_target(),
            mapping,
            0,
            apic_id,
            VIRTIO_NETWORK_RX_VECTOR,
        )
        && program_masked_msix_vector_raw(
            device.msix_target(),
            mapping,
            1,
            apic_id,
            VIRTIO_NETWORK_TX_VECTOR,
        )
        && network_msix_vectors_match(device, mapping, apic_id);
    programmed && revoke_network_function(device)
}

pub(crate) fn network_msix_vectors_are_masked(
    device: VirtioNetworkDevice,
    mapping: MmioMapping,
    apic_id: u8,
) -> bool {
    if !network_function_is_disabled(device) {
        return false;
    }
    unsafe {
        write_config_u16(
            device.bus,
            device.device,
            device.function,
            PCI_COMMAND,
            COMMAND_MEMORY_SPACE,
        );
    }
    let matched = network_function_command(device) == COMMAND_MEMORY_SPACE
        && network_msix_vectors_match(device, mapping, apic_id);
    matched && revoke_network_function(device)
}

fn network_msix_vectors_match(
    device: VirtioNetworkDevice,
    mapping: MmioMapping,
    apic_id: u8,
) -> bool {
    let Some((table_physical, table_length)) = device.msix_table_region() else {
        return false;
    };
    if table_length < 2 * MSIX_TABLE_ENTRY_SIZE as usize || !network_msix_is_disabled(device) {
        return false;
    }
    let message_address = MSI_ADDRESS_BASE | u32::from(apic_id) << MSI_DESTINATION_SHIFT;
    [VIRTIO_NETWORK_RX_VECTOR, VIRTIO_NETWORK_TX_VECTOR]
        .into_iter()
        .enumerate()
        .all(|(index, vector)| {
            let Some(entry) = table_physical
                .checked_add((index as u64) * u64::from(MSIX_TABLE_ENTRY_SIZE))
                .and_then(|physical| mapping.address(physical, MSIX_TABLE_ENTRY_SIZE as usize))
            else {
                return false;
            };
            read_mmio_u32(entry) == message_address
                && read_mmio_u32(entry + 4) == 0
                && read_mmio_u32(entry + 8) == u32::from(vector)
                && read_mmio_u32(entry + 12) == MSIX_VECTOR_MASK
        })
}

fn set_msix_control(device: VirtioBlockDevice, enabled: bool, function_masked: bool) -> bool {
    set_msix_control_raw(device.msix_target(), enabled, function_masked)
}

fn set_msix_control_raw(target: MsixTarget, enabled: bool, function_masked: bool) -> bool {
    let Some(offset) = target.capability.capability_offset.checked_add(2) else {
        return false;
    };
    let current = unsafe { read_config_u16(target.bus, target.device, target.function, offset) };
    let desired = (current & !(MSIX_ENABLE | MSIX_FUNCTION_MASK))
        | if enabled { MSIX_ENABLE } else { 0 }
        | if function_masked {
            MSIX_FUNCTION_MASK
        } else {
            0
        };
    unsafe {
        write_config_u16(target.bus, target.device, target.function, offset, desired);
    }
    (unsafe { read_config_u16(target.bus, target.device, target.function, offset) }) == desired
}

fn set_msix_entry_mask(
    device: VirtioBlockDevice,
    mapping: MmioMapping,
    index: u16,
    masked: bool,
) -> bool {
    set_msix_entry_mask_raw(device.msix_target(), mapping, index, masked)
}

fn set_msix_entry_mask_raw(
    target: MsixTarget,
    mapping: MmioMapping,
    index: u16,
    masked: bool,
) -> bool {
    let msix = target.capability;
    if index >= msix.vector_count {
        return false;
    }
    let Some((table, table_length)) =
        region_bounds(msix.table_bar_address, msix.table_offset, msix.table_length)
    else {
        return false;
    };
    let Some(offset) = usize::from(index).checked_mul(MSIX_TABLE_ENTRY_SIZE as usize) else {
        return false;
    };
    if offset
        .checked_add(MSIX_TABLE_ENTRY_SIZE as usize)
        .is_none_or(|end| end > table_length)
    {
        return false;
    }
    let Some(entry) = table
        .checked_add(offset as u64)
        .and_then(|physical| mapping.address(physical, MSIX_TABLE_ENTRY_SIZE as usize))
    else {
        return false;
    };
    write_mmio_u32(entry + 12, if masked { MSIX_VECTOR_MASK } else { 0 });
    read_mmio_u32(entry + 12) == if masked { MSIX_VECTOR_MASK } else { 0 }
}

fn install_virtio_interrupt(apic: &LocalApic) -> bool {
    install_virtio_interrupt_at(apic, VIRTIO_INTERRUPT_VECTOR)
}

fn install_virtio_interrupt_at(apic: &LocalApic, vector: u8) -> bool {
    VIRTIO_INTERRUPT_APIC_BASE.store(apic.physical_base, Ordering::SeqCst);
    VIRTIO_INTERRUPT_APIC_MODE.store(
        if apic.mode == Mode::X2Apic { 1 } else { 0 },
        Ordering::SeqCst,
    );
    VIRTIO_INTERRUPT_FIRED.store(false, Ordering::SeqCst);
    VIRTIO_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    VIRTIO_INTERRUPT_DOMAIN.store(0, Ordering::SeqCst);
    VIRTIO_INTERRUPT_REQUEST.store(0, Ordering::SeqCst);
    VIRTIO_INTERRUPT_ACTIVE.store(false, Ordering::SeqCst);
    FORCED_NOTIFIED_TIMEOUT_DOMAIN.store(0, Ordering::SeqCst);
    FORCED_NOTIFIED_TIMEOUT_REQUEST.store(0, Ordering::SeqCst);
    cpu::install_interrupt_gate(vector, ginux_virtio_interrupt_entry as *const () as usize)
}

fn retire_virtio_interrupt(apic: &LocalApic) -> bool {
    retire_virtio_interrupt_at(apic, VIRTIO_INTERRUPT_VECTOR)
}

fn retire_virtio_interrupt_at(apic: &LocalApic, vector: u8) -> bool {
    if VIRTIO_INTERRUPT_ACTIVE.load(Ordering::SeqCst) || !apic.vector_is_idle(vector) {
        return false;
    }
    let cleared = cpu::clear_interrupt_gate(vector);
    if cleared {
        VIRTIO_INTERRUPT_APIC_BASE.store(0, Ordering::SeqCst);
        VIRTIO_INTERRUPT_APIC_MODE.store(0, Ordering::SeqCst);
        VIRTIO_INTERRUPT_DOMAIN.store(0, Ordering::SeqCst);
        VIRTIO_INTERRUPT_REQUEST.store(0, Ordering::SeqCst);
        FORCED_NOTIFIED_TIMEOUT_DOMAIN.store(0, Ordering::SeqCst);
        FORCED_NOTIFIED_TIMEOUT_REQUEST.store(0, Ordering::SeqCst);
    }
    cleared
}

extern "sysv64" fn virtio_interrupt_handler(_frame: *mut UserInterruptFrame) {
    VIRTIO_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
    if VIRTIO_INTERRUPT_ACTIVE.load(Ordering::SeqCst) {
        let domain = VIRTIO_INTERRUPT_DOMAIN.load(Ordering::SeqCst);
        let request = VIRTIO_INTERRUPT_REQUEST.load(Ordering::SeqCst);
        if domain == 0
            || scheduler::wake_device_completion(
                domain,
                control_abi::BLOCK_DEVICE_OBJECT,
                request,
                0,
            )
        {
            VIRTIO_INTERRUPT_FIRED.store(true, Ordering::SeqCst);
        }
    }
    let mode = if VIRTIO_INTERRUPT_APIC_MODE.load(Ordering::SeqCst) == 1 {
        Mode::X2Apic
    } else {
        Mode::XApic
    };
    let Some(apic) =
        LocalApic::from_saved(0, mode, VIRTIO_INTERRUPT_APIC_BASE.load(Ordering::SeqCst))
    else {
        return;
    };
    // SAFETY: The interrupt proof owns this vector and stored the validated
    // local APIC before installing the IDT gate.
    unsafe { apic.write(APIC_EOI, 0) };
}

unsafe fn wait_for_one_virtio_interrupt() -> bool {
    asm!("sti", options(nomem, nostack));
    let mut interrupted = false;
    for _ in 0..QUEUE_POLL_LIMIT {
        if VIRTIO_INTERRUPT_FIRED.load(Ordering::SeqCst) {
            interrupted = true;
            break;
        }
        core::hint::spin_loop();
    }
    asm!("cli", options(nomem, nostack));
    compiler_fence(Ordering::SeqCst);
    interrupted && VIRTIO_INTERRUPT_COUNT.load(Ordering::SeqCst) == 1
}

pub(crate) fn initialize_without_dma(
    device: VirtioBlockDevice,
    mapping: MmioMapping,
) -> Option<VirtioBlockInitialization> {
    let (common_physical, common_length) = device.common_region()?;
    let common = mapping.address(common_physical, common_length)?;
    if common % align_of::<u32>() != 0 {
        return None;
    }

    let command =
        unsafe { read_config_u16(device.bus, device.device, device.function, PCI_COMMAND) };
    let safe_command = (command | COMMAND_MEMORY_SPACE) & !COMMAND_BUS_MASTER;
    unsafe {
        write_config_u16(
            device.bus,
            device.device,
            device.function,
            PCI_COMMAND,
            safe_command,
        );
    }
    if unsafe { read_config_u16(device.bus, device.device, device.function, PCI_COMMAND) }
        & (COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER)
        != COMMAND_MEMORY_SPACE
    {
        return None;
    }

    let result = negotiate(common);

    if reset(common).is_none()
        || unsafe { read_config_u16(device.bus, device.device, device.function, PCI_COMMAND) }
            & COMMAND_BUS_MASTER
            != 0
    {
        return None;
    }
    result
}

pub(crate) fn initialize_network_without_dma(
    device: VirtioNetworkDevice,
    common_mapping: MmioMapping,
    device_mapping: MmioMapping,
) -> Option<VirtioNetworkInitialization> {
    let (common_physical, common_length) = device.common_region()?;
    let common = common_mapping.address(common_physical, common_length)?;
    let (device_physical, device_length) = device.device_region()?;
    if device_length < 6 {
        return None;
    }
    let device_configuration = device_mapping.address(device_physical, device_length)?;
    if common % align_of::<u32>() != 0 || !network_function_is_disabled(device) {
        return None;
    }

    let command = network_function_command(device);
    unsafe {
        write_config_u16(
            device.bus,
            device.device,
            device.function,
            PCI_COMMAND,
            (command | COMMAND_MEMORY_SPACE) & !(COMMAND_IO_SPACE | COMMAND_BUS_MASTER),
        );
    }
    let memory_enabled = network_function_command(device)
        & (COMMAND_IO_SPACE | COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER)
        == COMMAND_MEMORY_SPACE;
    let result = memory_enabled
        .then(|| {
            let mut initialization = negotiate_network(common)?;
            initialization.mac = read_network_mac(common, device_configuration)?;
            let (receive, transmit) = network_queue_geometry(common, device)?;
            initialization.receive_queue_size = receive.0;
            initialization.receive_notification = receive.1;
            initialization.transmit_queue_size = transmit.0;
            initialization.transmit_notification = transmit.1;
            Some(initialization)
        })
        .flatten();
    let reset_complete = reset(common).is_some();
    let revoked = revoke_network_function(device);
    if !reset_complete || !revoked {
        return None;
    }
    result
}

/// Activates two empty virtio-net queues and immediately quiesces the device.
///
/// # Safety
///
/// The caller must exclusively own the complete DMA page and must have already
/// activated an IOMMU translation that permits this device to access only that
/// page. The lease must remain live until this function returns.
pub(crate) unsafe fn activate_network_queues_without_io(
    device: VirtioNetworkDevice,
    common_mapping: MmioMapping,
    dma_physical: u64,
    dma_virtual: usize,
) -> bool {
    if !dma_physical.is_multiple_of(DMA_PAGE_SIZE as u64)
        || !dma_virtual.is_multiple_of(DMA_PAGE_SIZE)
        || !network_function_is_disabled(device)
        || !verify_network_dma_layout()
    {
        return false;
    }
    let Some((common_physical, common_length)) = device.common_region() else {
        return false;
    };
    let Some(common) = common_mapping.address(common_physical, common_length) else {
        return false;
    };
    core::ptr::write_bytes(dma_virtual as *mut u8, 0, DMA_PAGE_SIZE);
    let command = network_function_command(device);
    write_config_u16(
        device.bus,
        device.device,
        device.function,
        PCI_COMMAND,
        (command | COMMAND_MEMORY_SPACE) & !(COMMAND_IO_SPACE | COMMAND_BUS_MASTER),
    );
    let activated = (|| {
        let initialization = negotiate_network(common)?;
        if initialization.queue_count < 2 {
            return None;
        }
        for (queue, descriptor, available, used) in [
            (
                0_u16,
                NET_RX_DESC_OFFSET,
                NET_RX_AVAIL_OFFSET,
                NET_RX_USED_OFFSET,
            ),
            (
                1_u16,
                NET_TX_DESC_OFFSET,
                NET_TX_AVAIL_OFFSET,
                NET_TX_USED_OFFSET,
            ),
        ] {
            write_common_u16(common, COMMON_QUEUE_SELECT, queue);
            validate_network_queue_size(read_common_u16(common, COMMON_QUEUE_SIZE))?;
            if read_common_u16(common, COMMON_QUEUE_ENABLE) != 0 {
                return None;
            }
            write_common_u16(common, COMMON_QUEUE_SIZE, QUEUE_SIZE);
            write_common_u64(common, COMMON_QUEUE_DESC, dma_physical + descriptor as u64);
            write_common_u64(common, COMMON_QUEUE_DRIVER, dma_physical + available as u64);
            write_common_u64(common, COMMON_QUEUE_DEVICE, dma_physical + used as u64);
            write_common_u16(common, COMMON_QUEUE_ENABLE, 1);
            if read_common_u16(common, COMMON_QUEUE_SIZE) != QUEUE_SIZE
                || read_common_u16(common, COMMON_QUEUE_ENABLE) != 1
                || read_common_u64(common, COMMON_QUEUE_DESC) != dma_physical + descriptor as u64
                || read_common_u64(common, COMMON_QUEUE_DRIVER) != dma_physical + available as u64
                || read_common_u64(common, COMMON_QUEUE_DEVICE) != dma_physical + used as u64
            {
                return None;
            }
        }
        write_common_u8(
            common,
            COMMON_DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
        );
        if read_common_u8(common, COMMON_DEVICE_STATUS)
            != STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK
        {
            return None;
        }
        write_config_u16(
            device.bus,
            device.device,
            device.function,
            PCI_COMMAND,
            COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER,
        );
        (network_function_command(device)
            & (COMMAND_IO_SPACE | COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER)
            == COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER)
            .then_some(())
    })()
    .is_some();

    let current = network_function_command(device);
    write_config_u16(
        device.bus,
        device.device,
        device.function,
        PCI_COMMAND,
        (current | COMMAND_MEMORY_SPACE) & !(COMMAND_IO_SPACE | COMMAND_BUS_MASTER),
    );
    let bus_master_revoked = network_function_command(device) & COMMAND_BUS_MASTER == 0;
    let reset_complete = reset(common).is_some();
    activated
        && bus_master_revoked
        && reset_complete
        && revoke_network_function(device)
        && network_function_is_disabled(device)
}

/// Transmits one fixed, bounded Ethernet frame through queue 1.
///
/// # Safety
///
/// The caller must exclusively own the DMA page and have an active IOMMU
/// translation restricting this function to that page until return.
pub(crate) unsafe fn transmit_network_probe(
    device: VirtioNetworkDevice,
    common_mapping: MmioMapping,
    notification_mapping: MmioMapping,
    msix_mapping: MmioMapping,
    apic: &LocalApic,
    dma_physical: u64,
    dma_virtual: usize,
) -> Option<VirtioNetworkTransmit> {
    const HEADER_LENGTH: usize = 12;
    const ETHERNET_FRAME: [u8; 19] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x88, 0xb5, b'G',
        b'I', b'N', b'U', b'X',
    ];
    const PAYLOAD_LENGTH: usize = HEADER_LENGTH + ETHERNET_FRAME.len();
    if !dma_physical.is_multiple_of(DMA_PAGE_SIZE as u64)
        || !dma_virtual.is_multiple_of(DMA_PAGE_SIZE)
        || !network_function_is_disabled(device)
        || PAYLOAD_LENGTH >= NET_BUFFER_LENGTH
    {
        return None;
    }
    let (common_physical, common_length) = device.common_region()?;
    let common = common_mapping.address(common_physical, common_length)?;
    let (notify_physical, notify_length) = device.notify_region()?;
    if !install_virtio_interrupt_at(apic, VIRTIO_NETWORK_TX_VECTOR) {
        return None;
    }
    core::ptr::write_bytes(dma_virtual as *mut u8, 0, DMA_PAGE_SIZE);
    write_config_u16(
        device.bus,
        device.device,
        device.function,
        PCI_COMMAND,
        COMMAND_MEMORY_SPACE,
    );
    let result = (|| {
        if network_function_command(device) != COMMAND_MEMORY_SPACE
            || !program_masked_msix_vector_raw(
                device.msix_target(),
                msix_mapping,
                1,
                apic.id,
                VIRTIO_NETWORK_TX_VECTOR,
            )
        {
            return None;
        }
        let initialization = negotiate_network(common)?;
        if initialization.queue_count < 2 || !set_msix_control_raw(device.msix_target(), true, true)
        {
            return None;
        }
        write_common_u16(common, COMMON_QUEUE_SELECT, 1);
        validate_network_queue_size(read_common_u16(common, COMMON_QUEUE_SIZE))?;
        write_common_u16(common, COMMON_QUEUE_SIZE, QUEUE_SIZE);
        write_common_u16(common, COMMON_QUEUE_MSIX_VECTOR, 1);
        let notify_offset = notification_byte_offset(
            read_common_u16(common, COMMON_QUEUE_NOTIFY_OFF),
            device.notify_multiplier,
            notify_length,
        )?;
        let notification = notification_mapping.address(
            notify_physical.checked_add(notify_offset as u64)?,
            size_of::<u16>(),
        )?;
        write_common_u64(
            common,
            COMMON_QUEUE_DESC,
            dma_physical + NET_TX_DESC_OFFSET as u64,
        );
        write_common_u64(
            common,
            COMMON_QUEUE_DRIVER,
            dma_physical + NET_TX_AVAIL_OFFSET as u64,
        );
        write_common_u64(
            common,
            COMMON_QUEUE_DEVICE,
            dma_physical + NET_TX_USED_OFFSET as u64,
        );

        write_dma_u64(
            dma_virtual + NET_TX_DESC_OFFSET,
            dma_physical + NET_TX_BUFFER_OFFSET as u64,
        );
        write_dma_u32(dma_virtual + NET_TX_DESC_OFFSET + 8, PAYLOAD_LENGTH as u32);
        write_dma_u16(dma_virtual + NET_TX_DESC_OFFSET + 12, 0);
        write_dma_u16(dma_virtual + NET_TX_DESC_OFFSET + 14, 0);
        for offset in 0..HEADER_LENGTH {
            write_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET + offset, 0);
        }
        for (offset, byte) in ETHERNET_FRAME.into_iter().enumerate() {
            write_dma_u8(
                dma_virtual + NET_TX_BUFFER_OFFSET + HEADER_LENGTH + offset,
                byte,
            );
        }
        write_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET - 1, 0x3c);
        write_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET + PAYLOAD_LENGTH, 0xc3);
        write_dma_u16(dma_virtual + NET_TX_AVAIL_OFFSET, 0);
        write_dma_u16(dma_virtual + NET_TX_AVAIL_OFFSET + 2, 1);
        write_dma_u16(dma_virtual + NET_TX_AVAIL_OFFSET + 4, 0);
        compiler_fence(Ordering::SeqCst);
        write_common_u16(common, COMMON_QUEUE_ENABLE, 1);
        if read_common_u16(common, COMMON_QUEUE_ENABLE) != 1
            || read_common_u16(common, COMMON_QUEUE_MSIX_VECTOR) != 1
        {
            return None;
        }
        write_common_u8(
            common,
            COMMON_DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
        );
        let device_status = read_common_u8(common, COMMON_DEVICE_STATUS);
        if device_status & STATUS_DRIVER_OK == 0
            || device_status & (STATUS_DEVICE_NEEDS_RESET | STATUS_FAILED) != 0
        {
            return None;
        }
        write_config_u16(
            device.bus,
            device.device,
            device.function,
            PCI_COMMAND,
            COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER,
        );
        if network_function_command(device) != COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER {
            return None;
        }
        VIRTIO_INTERRUPT_FIRED.store(false, Ordering::SeqCst);
        VIRTIO_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
        VIRTIO_INTERRUPT_ACTIVE.store(true, Ordering::SeqCst);
        if !set_msix_entry_mask_raw(device.msix_target(), msix_mapping, 1, false)
            || !set_msix_control_raw(device.msix_target(), true, false)
        {
            return None;
        }
        compiler_fence(Ordering::SeqCst);
        write_mmio_u16(notification, 1);
        let interrupted = wait_for_one_virtio_interrupt();
        let header_unchanged = (0..HEADER_LENGTH)
            .all(|offset| read_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET + offset) == 0);
        let frame_unchanged = ETHERNET_FRAME.iter().enumerate().all(|(offset, expected)| {
            read_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET + HEADER_LENGTH + offset) == *expected
        });
        let device_status = read_common_u8(common, COMMON_DEVICE_STATUS);
        (interrupted
            && read_dma_u16(dma_virtual + NET_TX_USED_OFFSET + 2) == 1
            && read_dma_u32(dma_virtual + NET_TX_USED_OFFSET + 4) == 0
            && device_status & (STATUS_DEVICE_NEEDS_RESET | STATUS_FAILED) == 0
            && header_unchanged
            && frame_unchanged
            && read_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET - 1) == 0x3c
            && read_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET + PAYLOAD_LENGTH) == 0xc3)
            .then_some(VirtioNetworkTransmit {
                frame_length: ETHERNET_FRAME.len() as u32,
            })
    })();

    asm!("cli", options(nomem, nostack));
    let function_masked = set_msix_control_raw(device.msix_target(), true, true);
    let entry_masked = set_msix_entry_mask_raw(device.msix_target(), msix_mapping, 1, true);
    let msix_disabled = set_msix_control_raw(device.msix_target(), false, true);
    VIRTIO_INTERRUPT_ACTIVE.store(false, Ordering::SeqCst);
    let current = network_function_command(device);
    write_config_u16(
        device.bus,
        device.device,
        device.function,
        PCI_COMMAND,
        (current | COMMAND_MEMORY_SPACE) & !(COMMAND_IO_SPACE | COMMAND_BUS_MASTER),
    );
    let bus_master_revoked = network_function_command(device) & COMMAND_BUS_MASTER == 0;
    let reset_complete = reset(common).is_some();
    let revoked = revoke_network_function(device);
    let interrupt_retired = retire_virtio_interrupt_at(apic, VIRTIO_NETWORK_TX_VECTOR);
    (function_masked
        && entry_masked
        && msix_disabled
        && bus_master_revoked
        && reset_complete
        && revoked
        && interrupt_retired)
        .then_some(result)
        .flatten()
}

/// Sends one ARP request and receives the bounded gateway reply through queue 0.
///
/// # Safety
///
/// The caller must exclusively own the DMA page and have an active IOMMU
/// translation restricting this function to that page until return.
pub(crate) unsafe fn receive_network_arp_reply(
    device: VirtioNetworkDevice,
    common_mapping: MmioMapping,
    notification_mapping: MmioMapping,
    msix_mapping: MmioMapping,
    apic: &LocalApic,
    dma_physical: u64,
    dma_virtual: usize,
) -> Option<VirtioNetworkReceive> {
    const HEADER_LENGTH: usize = 12;
    const GUEST_MAC: [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
    const GATEWAY_MAC: [u8; 6] = [0x52, 0x55, 0x0a, 0x00, 0x02, 0x02];
    const GUEST_IP: [u8; 4] = [10, 0, 2, 15];
    const GATEWAY_IP: [u8; 4] = [10, 0, 2, 2];
    const ARP_REQUEST: [u8; 60] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x08, 0x06, 0x00,
        0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 10, 0, 2, 15,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 10, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ];
    const TX_LENGTH: usize = HEADER_LENGTH + ARP_REQUEST.len();
    const RX_DESCRIPTOR_LENGTH: usize = NET_BUFFER_LENGTH - 1;

    if !dma_physical.is_multiple_of(DMA_PAGE_SIZE as u64)
        || !dma_virtual.is_multiple_of(DMA_PAGE_SIZE)
        || !network_function_is_disabled(device)
        || TX_LENGTH >= NET_BUFFER_LENGTH
    {
        return None;
    }
    let (common_physical, common_length) = device.common_region()?;
    let common = common_mapping.address(common_physical, common_length)?;
    let (notify_physical, notify_length) = device.notify_region()?;
    if !install_virtio_interrupt_at(apic, VIRTIO_NETWORK_RX_VECTOR) {
        return None;
    }
    if !install_virtio_interrupt_at(apic, VIRTIO_NETWORK_TX_VECTOR) {
        let _ = retire_virtio_interrupt_at(apic, VIRTIO_NETWORK_RX_VECTOR);
        return None;
    }

    core::ptr::write_bytes(dma_virtual as *mut u8, 0, DMA_PAGE_SIZE);
    write_config_u16(
        device.bus,
        device.device,
        device.function,
        PCI_COMMAND,
        COMMAND_MEMORY_SPACE,
    );
    let result = (|| {
        if network_function_command(device) != COMMAND_MEMORY_SPACE
            || !program_masked_msix_vector_raw(
                device.msix_target(),
                msix_mapping,
                0,
                apic.id,
                VIRTIO_NETWORK_RX_VECTOR,
            )
            || !program_masked_msix_vector_raw(
                device.msix_target(),
                msix_mapping,
                1,
                apic.id,
                VIRTIO_NETWORK_TX_VECTOR,
            )
        {
            return None;
        }
        let initialization = negotiate_network(common)?;
        if initialization.queue_count < 2 || !set_msix_control_raw(device.msix_target(), true, true)
        {
            return None;
        }

        write_common_u16(common, COMMON_QUEUE_SELECT, 0);
        validate_network_queue_size(read_common_u16(common, COMMON_QUEUE_SIZE))?;
        write_common_u16(common, COMMON_QUEUE_SIZE, QUEUE_SIZE);
        write_common_u16(common, COMMON_QUEUE_MSIX_VECTOR, 0);
        let receive_notify_offset = notification_byte_offset(
            read_common_u16(common, COMMON_QUEUE_NOTIFY_OFF),
            device.notify_multiplier,
            notify_length,
        )?;
        let receive_notification = notification_mapping.address(
            notify_physical.checked_add(receive_notify_offset as u64)?,
            size_of::<u16>(),
        )?;
        write_common_u64(
            common,
            COMMON_QUEUE_DESC,
            dma_physical + NET_RX_DESC_OFFSET as u64,
        );
        write_common_u64(
            common,
            COMMON_QUEUE_DRIVER,
            dma_physical + NET_RX_AVAIL_OFFSET as u64,
        );
        write_common_u64(
            common,
            COMMON_QUEUE_DEVICE,
            dma_physical + NET_RX_USED_OFFSET as u64,
        );
        write_common_u16(common, COMMON_QUEUE_ENABLE, 1);
        if read_common_u16(common, COMMON_QUEUE_ENABLE) != 1
            || read_common_u16(common, COMMON_QUEUE_MSIX_VECTOR) != 0
        {
            return None;
        }

        write_common_u16(common, COMMON_QUEUE_SELECT, 1);
        validate_network_queue_size(read_common_u16(common, COMMON_QUEUE_SIZE))?;
        write_common_u16(common, COMMON_QUEUE_SIZE, QUEUE_SIZE);
        write_common_u16(common, COMMON_QUEUE_MSIX_VECTOR, 1);
        let transmit_notify_offset = notification_byte_offset(
            read_common_u16(common, COMMON_QUEUE_NOTIFY_OFF),
            device.notify_multiplier,
            notify_length,
        )?;
        let transmit_notification = notification_mapping.address(
            notify_physical.checked_add(transmit_notify_offset as u64)?,
            size_of::<u16>(),
        )?;
        write_common_u64(
            common,
            COMMON_QUEUE_DESC,
            dma_physical + NET_TX_DESC_OFFSET as u64,
        );
        write_common_u64(
            common,
            COMMON_QUEUE_DRIVER,
            dma_physical + NET_TX_AVAIL_OFFSET as u64,
        );
        write_common_u64(
            common,
            COMMON_QUEUE_DEVICE,
            dma_physical + NET_TX_USED_OFFSET as u64,
        );
        write_common_u16(common, COMMON_QUEUE_ENABLE, 1);
        if read_common_u16(common, COMMON_QUEUE_ENABLE) != 1
            || read_common_u16(common, COMMON_QUEUE_MSIX_VECTOR) != 1
        {
            return None;
        }

        write_dma_u64(
            dma_virtual + NET_RX_DESC_OFFSET,
            dma_physical + NET_RX_BUFFER_OFFSET as u64,
        );
        write_dma_u32(
            dma_virtual + NET_RX_DESC_OFFSET + 8,
            RX_DESCRIPTOR_LENGTH as u32,
        );
        write_dma_u16(dma_virtual + NET_RX_DESC_OFFSET + 12, DESC_WRITE);
        write_dma_u16(dma_virtual + NET_RX_DESC_OFFSET + 14, 0);
        write_dma_u8(dma_virtual + NET_RX_BUFFER_OFFSET - 1, 0x3c);
        write_dma_u8(
            dma_virtual + NET_RX_BUFFER_OFFSET + RX_DESCRIPTOR_LENGTH,
            0xc3,
        );
        write_dma_u16(dma_virtual + NET_RX_AVAIL_OFFSET, 0);
        write_dma_u16(dma_virtual + NET_RX_AVAIL_OFFSET + 2, 1);
        write_dma_u16(dma_virtual + NET_RX_AVAIL_OFFSET + 4, 0);

        write_dma_u64(
            dma_virtual + NET_TX_DESC_OFFSET,
            dma_physical + NET_TX_BUFFER_OFFSET as u64,
        );
        write_dma_u32(dma_virtual + NET_TX_DESC_OFFSET + 8, TX_LENGTH as u32);
        write_dma_u16(dma_virtual + NET_TX_DESC_OFFSET + 12, 0);
        write_dma_u16(dma_virtual + NET_TX_DESC_OFFSET + 14, 0);
        for offset in 0..HEADER_LENGTH {
            write_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET + offset, 0);
        }
        for (offset, byte) in ARP_REQUEST.into_iter().enumerate() {
            write_dma_u8(
                dma_virtual + NET_TX_BUFFER_OFFSET + HEADER_LENGTH + offset,
                byte,
            );
        }
        write_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET + TX_LENGTH, 0xa5);
        write_dma_u16(dma_virtual + NET_TX_AVAIL_OFFSET, 0);
        write_dma_u16(dma_virtual + NET_TX_AVAIL_OFFSET + 2, 1);
        write_dma_u16(dma_virtual + NET_TX_AVAIL_OFFSET + 4, 0);

        compiler_fence(Ordering::SeqCst);
        write_common_u8(
            common,
            COMMON_DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
        );
        let device_status = read_common_u8(common, COMMON_DEVICE_STATUS);
        if device_status & STATUS_DRIVER_OK == 0
            || device_status & (STATUS_DEVICE_NEEDS_RESET | STATUS_FAILED) != 0
        {
            return None;
        }
        write_config_u16(
            device.bus,
            device.device,
            device.function,
            PCI_COMMAND,
            COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER,
        );
        if network_function_command(device) != COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER {
            return None;
        }

        VIRTIO_INTERRUPT_FIRED.store(false, Ordering::SeqCst);
        VIRTIO_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
        VIRTIO_INTERRUPT_ACTIVE.store(true, Ordering::SeqCst);
        if !set_msix_entry_mask_raw(device.msix_target(), msix_mapping, 1, false)
            || !set_msix_control_raw(device.msix_target(), true, false)
        {
            return None;
        }
        compiler_fence(Ordering::SeqCst);
        write_mmio_u16(receive_notification, 0);
        write_mmio_u16(transmit_notification, 1);
        let transmit_interrupted = wait_for_one_virtio_interrupt();
        let transmit_completed = transmit_interrupted
            && read_dma_u16(dma_virtual + NET_TX_USED_OFFSET + 2) == 1
            && read_dma_u32(dma_virtual + NET_TX_USED_OFFSET + 4) == 0
            && (0..HEADER_LENGTH)
                .all(|offset| read_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET + offset) == 0)
            && ARP_REQUEST.iter().enumerate().all(|(offset, expected)| {
                read_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET + HEADER_LENGTH + offset)
                    == *expected
            })
            && read_dma_u8(dma_virtual + NET_TX_BUFFER_OFFSET + TX_LENGTH) == 0xa5;
        if !transmit_completed
            || !set_msix_entry_mask_raw(device.msix_target(), msix_mapping, 1, true)
        {
            return None;
        }

        VIRTIO_INTERRUPT_FIRED.store(false, Ordering::SeqCst);
        VIRTIO_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
        if !set_msix_entry_mask_raw(device.msix_target(), msix_mapping, 0, false) {
            return None;
        }
        let receive_interrupted = wait_for_one_virtio_interrupt();
        let receive_length = read_dma_u32(dma_virtual + NET_RX_USED_OFFSET + 8);
        let device_status = read_common_u8(common, COMMON_DEVICE_STATUS);
        let ring_valid = receive_interrupted
            && read_dma_u16(dma_virtual + NET_RX_USED_OFFSET + 2) == 1
            && read_dma_u32(dma_virtual + NET_RX_USED_OFFSET + 4) == 0
            && device_status & (STATUS_DEVICE_NEEDS_RESET | STATUS_FAILED) == 0;
        let guards_valid = read_dma_u8(dma_virtual + NET_RX_BUFFER_OFFSET - 1) == 0x3c
            && read_dma_u8(dma_virtual + NET_RX_BUFFER_OFFSET + RX_DESCRIPTOR_LENGTH) == 0xc3;
        let reply_valid = network_arp_reply_is_valid(
            dma_virtual + NET_RX_BUFFER_OFFSET,
            receive_length,
            GUEST_MAC,
            GATEWAY_MAC,
            GUEST_IP,
            GATEWAY_IP,
        );
        let valid = ring_valid && guards_valid && reply_valid;
        valid.then_some(VirtioNetworkReceive {
            frame_length: receive_length.checked_sub(HEADER_LENGTH as u32)?,
        })
    })();

    asm!("cli", options(nomem, nostack));
    let function_masked = set_msix_control_raw(device.msix_target(), true, true);
    let receive_masked = set_msix_entry_mask_raw(device.msix_target(), msix_mapping, 0, true);
    let transmit_masked = set_msix_entry_mask_raw(device.msix_target(), msix_mapping, 1, true);
    let msix_disabled = set_msix_control_raw(device.msix_target(), false, true);
    VIRTIO_INTERRUPT_ACTIVE.store(false, Ordering::SeqCst);
    let current = network_function_command(device);
    write_config_u16(
        device.bus,
        device.device,
        device.function,
        PCI_COMMAND,
        (current | COMMAND_MEMORY_SPACE) & !(COMMAND_IO_SPACE | COMMAND_BUS_MASTER),
    );
    let bus_master_revoked = network_function_command(device) & COMMAND_BUS_MASTER == 0;
    let reset_complete = reset(common).is_some();
    let revoked = revoke_network_function(device);
    let receive_interrupt_retired = retire_virtio_interrupt_at(apic, VIRTIO_NETWORK_RX_VECTOR);
    let transmit_interrupt_retired = retire_virtio_interrupt_at(apic, VIRTIO_NETWORK_TX_VECTOR);
    let retired = function_masked
        && receive_masked
        && transmit_masked
        && msix_disabled
        && bus_master_revoked
        && reset_complete
        && revoked
        && receive_interrupt_retired
        && transmit_interrupt_retired;
    retired.then_some(result).flatten()
}

fn network_arp_reply_is_valid(
    buffer: usize,
    used_length: u32,
    guest_mac: [u8; 6],
    gateway_mac: [u8; 6],
    guest_ip: [u8; 4],
    gateway_ip: [u8; 4],
) -> bool {
    const HEADER_LENGTH: usize = 12;
    const ARP_LENGTH: usize = 42;
    if used_length < (HEADER_LENGTH + ARP_LENGTH) as u32
        || used_length > (NET_BUFFER_LENGTH - 1) as u32
        || !(0..10).all(|offset| read_dma_u8(buffer + offset) == 0)
        || read_dma_u16(buffer + 10) != 1
    {
        return false;
    }
    let frame = buffer + HEADER_LENGTH;
    bytes_match(frame, &guest_mac)
        && bytes_match(frame + 6, &gateway_mac)
        && bytes_match(frame + 12, &[0x08, 0x06])
        && bytes_match(
            frame + 14,
            &[0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02],
        )
        && bytes_match(frame + 22, &gateway_mac)
        && bytes_match(frame + 28, &gateway_ip)
        && bytes_match(frame + 32, &guest_mac)
        && bytes_match(frame + 38, &guest_ip)
}

fn bytes_match(address: usize, expected: &[u8]) -> bool {
    expected
        .iter()
        .enumerate()
        .all(|(offset, byte)| read_dma_u8(address + offset) == *byte)
}

/// Executes one interrupt-driven read-only DMA request using one caller-owned
/// page and one kernel-owned MSI-X vector.
///
/// # Safety
///
/// `dma_virtual` must be a writable direct-map alias of the complete physical
/// page at `dma_physical`, and that page must remain exclusively owned until
/// this function returns.
pub(crate) unsafe fn read_blank_sector_interrupt(
    device: VirtioBlockDevice,
    mapping: MmioMapping,
    msix_mapping: MmioMapping,
    apic: &LocalApic,
    dma_physical: u64,
    dma_virtual: usize,
    sector: u64,
) -> Option<VirtioBlockRead> {
    if !dma_physical.is_multiple_of(DMA_PAGE_SIZE as u64)
        || !dma_virtual.is_multiple_of(DMA_PAGE_SIZE)
        || dma_physical.checked_add(DMA_PAGE_SIZE as u64).is_none()
    {
        return None;
    }
    let (common_physical, common_length) = device.common_region()?;
    if common_length < COMMON_QUEUE_DEVICE + size_of::<u64>() {
        return None;
    }
    let common = mapping.address(common_physical, common_length)?;
    let (notify_physical, notify_length) = device.notify_region()?;
    let (device_physical, device_length) = device.device_region()?;
    if device_length < size_of::<u64>() {
        return None;
    }
    let device_configuration = mapping.address(device_physical, device_length)?;
    if common % align_of::<u64>() != 0 || device_configuration % align_of::<u64>() != 0 {
        return None;
    }

    if !program_masked_msix_vector(device, msix_mapping, 0, apic.id, VIRTIO_INTERRUPT_VECTOR)
        || !install_virtio_interrupt(apic)
    {
        return None;
    }
    let result = (|| {
        if !set_pci_command(device, false) {
            return None;
        }
        let initialization = negotiate(common)?;
        if initialization.queue_count == 0 {
            return None;
        }
        if !set_msix_control(device, true, true) {
            return None;
        }
        let capacity_sectors = read_capacity(common, device_configuration)?;
        if capacity_sectors == 0 || sector >= capacity_sectors {
            return None;
        }

        write_common_u16(common, COMMON_QUEUE_SELECT, 0);
        let maximum_size = read_common_u16(common, COMMON_QUEUE_SIZE);
        if maximum_size < 3 {
            return None;
        }
        let queue_size = select_queue_size(maximum_size)?;
        write_common_u16(common, COMMON_QUEUE_SIZE, queue_size);
        if read_common_u16(common, COMMON_QUEUE_SIZE) != queue_size {
            return None;
        }
        write_common_u16(common, COMMON_QUEUE_MSIX_VECTOR, 0);
        if read_common_u16(common, COMMON_QUEUE_MSIX_VECTOR) != 0 {
            return None;
        }
        let notify_offset = notification_byte_offset(
            read_common_u16(common, COMMON_QUEUE_NOTIFY_OFF),
            device.notify_multiplier,
            notify_length,
        )?;
        let notification = mapping.address(
            notify_physical.checked_add(notify_offset as u64)?,
            size_of::<u16>(),
        )?;
        if notification % align_of::<u16>() != 0 {
            return None;
        }

        prepare_read_request(dma_physical, dma_virtual, sector);
        write_common_u64(common, COMMON_QUEUE_DESC, dma_physical + DESC_OFFSET as u64);
        write_common_u64(
            common,
            COMMON_QUEUE_DRIVER,
            dma_physical + AVAIL_OFFSET as u64,
        );
        write_common_u64(
            common,
            COMMON_QUEUE_DEVICE,
            dma_physical + USED_OFFSET as u64,
        );
        write_common_u16(common, COMMON_QUEUE_ENABLE, 1);
        if read_common_u16(common, COMMON_QUEUE_ENABLE) != 1 {
            return None;
        }
        write_common_u8(
            common,
            COMMON_DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
        );
        if read_common_u8(common, COMMON_DEVICE_STATUS) & STATUS_DRIVER_OK == 0
            || !set_pci_command(device, true)
        {
            return None;
        }

        VIRTIO_INTERRUPT_FIRED.store(false, Ordering::SeqCst);
        VIRTIO_INTERRUPT_ACTIVE.store(true, Ordering::SeqCst);
        if !set_msix_entry_mask(device, msix_mapping, 0, false)
            || !set_msix_control(device, true, false)
        {
            return None;
        }

        compiler_fence(Ordering::SeqCst);
        write_mmio_u16(notification, 0);
        asm!("sti", options(nomem, nostack));
        let mut interrupted = false;
        for _ in 0..QUEUE_POLL_LIMIT {
            if VIRTIO_INTERRUPT_FIRED.load(Ordering::SeqCst) {
                interrupted = true;
                break;
            }
            core::hint::spin_loop();
        }
        asm!("cli", options(nomem, nostack));
        compiler_fence(Ordering::SeqCst);
        let device_status = read_common_u8(common, COMMON_DEVICE_STATUS);
        if !interrupted
            || device_status & (STATUS_DEVICE_NEEDS_RESET | STATUS_FAILED) != 0
            || read_dma_u16(dma_virtual + USED_OFFSET + 2) != 1
            || read_dma_u32(dma_virtual + USED_OFFSET + 4) != 0
            || read_dma_u8(dma_virtual + STATUS_OFFSET) != 0
            || read_dma_u8(dma_virtual + DATA_OFFSET - 1) != 0x3c
            || read_dma_u8(dma_virtual + DATA_OFFSET + DATA_LENGTH) != 0xc3
            || (0..DATA_LENGTH).any(|offset| read_dma_u8(dma_virtual + DATA_OFFSET + offset) != 0)
        {
            return None;
        }
        let used_length = read_dma_u32(dma_virtual + USED_OFFSET + 8);
        (used_length == (DATA_LENGTH + 1) as u32).then_some(VirtioBlockRead {
            capacity_sectors,
            used_length,
        })
    })();

    asm!("cli", options(nomem, nostack));
    let function_masked = set_msix_control(device, true, true);
    let entry_masked = set_msix_entry_mask(device, msix_mapping, 0, true);
    let msix_disabled = set_msix_control(device, false, true);
    let bus_master_disabled = set_pci_command(device, false);
    let reset_complete = reset(common).is_some();
    VIRTIO_INTERRUPT_ACTIVE.store(false, Ordering::SeqCst);
    let interrupt_retired = retire_virtio_interrupt(apic);
    if !function_masked
        || !entry_masked
        || !msix_disabled
        || !bus_master_disabled
        || !reset_complete
        || !bus_master_is_disabled(device)
        || !interrupt_retired
        || !VIRTIO_INTERRUPT_FIRED.load(Ordering::SeqCst)
        || VIRTIO_INTERRUPT_COUNT.load(Ordering::SeqCst) != 1
    {
        return None;
    }
    result
}

impl VirtioBlockBackend {
    /// Activates queue zero over one exclusively owned DMA page.
    ///
    /// # Safety
    ///
    /// The DMA addresses must satisfy the same ownership and alias contract as
    /// [`read_blank_sector_interrupt`] and remain live until `finish`
    /// succeeds.
    pub(crate) unsafe fn start(
        device: VirtioBlockDevice,
        mapping: MmioMapping,
        dma_physical: u64,
        dma_virtual: usize,
    ) -> Option<Self> {
        if !dma_physical.is_multiple_of(DMA_PAGE_SIZE as u64)
            || !dma_virtual.is_multiple_of(DMA_PAGE_SIZE)
            || dma_physical.checked_add(DMA_PAGE_SIZE as u64).is_none()
        {
            return None;
        }
        let (common_physical, common_length) = device.common_region()?;
        if common_length < COMMON_QUEUE_DEVICE + size_of::<u64>() {
            return None;
        }
        let common = mapping.address(common_physical, common_length)?;
        let (notify_physical, notify_length) = device.notify_region()?;
        let (device_physical, device_length) = device.device_region()?;
        if device_length < size_of::<u64>() {
            return None;
        }
        let device_configuration = mapping.address(device_physical, device_length)?;
        if common % align_of::<u64>() != 0 || device_configuration % align_of::<u64>() != 0 {
            return None;
        }
        if !set_pci_command(device, false) {
            return None;
        }

        let started = (|| {
            negotiate(common)?;
            let capacity_sectors = read_capacity(common, device_configuration)?;
            if capacity_sectors == 0 {
                return None;
            }
            write_common_u16(common, COMMON_QUEUE_SELECT, 0);
            let queue_size = select_queue_size(read_common_u16(common, COMMON_QUEUE_SIZE))?;
            write_common_u16(common, COMMON_QUEUE_SIZE, queue_size);
            if read_common_u16(common, COMMON_QUEUE_SIZE) != queue_size {
                return None;
            }
            let notify_offset = notification_byte_offset(
                read_common_u16(common, COMMON_QUEUE_NOTIFY_OFF),
                device.notify_multiplier,
                notify_length,
            )?;
            let notification = mapping.address(
                notify_physical.checked_add(notify_offset as u64)?,
                size_of::<u16>(),
            )?;
            if notification % align_of::<u16>() != 0 {
                return None;
            }

            unsafe { core::ptr::write_bytes(dma_virtual as *mut u8, 0, DMA_PAGE_SIZE) };
            write_common_u64(common, COMMON_QUEUE_DESC, dma_physical + DESC_OFFSET as u64);
            write_common_u64(
                common,
                COMMON_QUEUE_DRIVER,
                dma_physical + AVAIL_OFFSET as u64,
            );
            write_common_u64(
                common,
                COMMON_QUEUE_DEVICE,
                dma_physical + USED_OFFSET as u64,
            );
            write_common_u16(common, COMMON_QUEUE_ENABLE, 1);
            if read_common_u16(common, COMMON_QUEUE_ENABLE) != 1 {
                return None;
            }
            write_common_u8(
                common,
                COMMON_DEVICE_STATUS,
                STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
            );
            if read_common_u8(common, COMMON_DEVICE_STATUS) & STATUS_DRIVER_OK == 0
                || !set_pci_command(device, true)
            {
                return None;
            }
            Some(Self {
                device,
                common,
                notification,
                dma_physical,
                dma_virtual,
                capacity_sectors,
                queue_size,
                available_index: 0,
                used_index: 0,
                active: true,
            })
        })();

        if started.is_none() {
            let _ = set_pci_command(device, false);
            let _ = reset(common);
        }
        started
    }

    pub(crate) fn capacity_sectors(&self) -> u64 {
        self.capacity_sectors
    }

    pub(crate) fn finish(&mut self) -> bool {
        if !self.active {
            return false;
        }
        let disabled = set_pci_command(self.device, false);
        let reset_complete = reset(self.common).is_some();
        let safe = disabled && reset_complete && bus_master_is_disabled(self.device);
        if safe {
            self.active = false;
        }
        safe
    }

    /// Submits one read whose data descriptor names a page outside the active
    /// DMA domain. QEMU may publish an ordinary successful virtio completion
    /// even when VT-d drops the payload write, so this records transport state
    /// while the caller separately requires an unchanged target and fault log.
    pub(crate) fn attempt_unmapped_read(
        &mut self,
        sector: u64,
        unmapped_physical: u64,
    ) -> Option<UnmappedDmaAttempt> {
        if !self.active
            || sector >= self.capacity_sectors
            || !unmapped_physical.is_multiple_of(DMA_PAGE_SIZE as u64)
            || unmapped_physical == self.dma_physical
            || unmapped_physical.checked_add(DATA_LENGTH as u64).is_none()
        {
            return None;
        }
        prepare_operation(
            self.dma_physical,
            self.dma_virtual,
            0,
            sector,
            DATA_LENGTH,
            true,
            None,
        );
        write_dma_u64(self.dma_virtual + DESC_OFFSET + 16, unmapped_physical);

        let available_slot = usize::from(self.available_index % self.queue_size);
        let used_slot = usize::from(self.used_index % self.queue_size);
        let expected_used_index = self.used_index.wrapping_add(1);
        if read_dma_u16(self.dma_virtual + USED_OFFSET + 2) != self.used_index {
            return None;
        }
        write_dma_u16(
            self.dma_virtual + AVAIL_OFFSET + 4 + available_slot * size_of::<u16>(),
            0,
        );
        compiler_fence(Ordering::SeqCst);
        let expected_available = self.available_index.wrapping_add(1);
        write_dma_u16(self.dma_virtual + AVAIL_OFFSET + 2, expected_available);
        self.available_index = expected_available;
        write_mmio_u16(self.notification, 0);

        let mut completed = false;
        for _ in 0..QUEUE_POLL_LIMIT {
            compiler_fence(Ordering::SeqCst);
            if read_dma_u16(self.dma_virtual + USED_OFFSET + 2) == expected_used_index {
                completed = true;
                break;
            }
            if read_common_u8(self.common, COMMON_DEVICE_STATUS)
                & (STATUS_DEVICE_NEEDS_RESET | STATUS_FAILED)
                != 0
            {
                break;
            }
            core::hint::spin_loop();
        }
        compiler_fence(Ordering::SeqCst);
        let used_element = self.dma_virtual + USED_OFFSET + 4 + used_slot * 8;
        Some(UnmappedDmaAttempt {
            completion_published: completed,
            used_index: read_dma_u16(self.dma_virtual + USED_OFFSET + 2),
            used_id: read_dma_u32(used_element),
            used_length: read_dma_u32(used_element + 4),
            request_status: read_dma_u8(self.dma_virtual + STATUS_OFFSET),
            device_status: read_common_u8(self.common, COMMON_DEVICE_STATUS),
            mapped_payload_unchanged: read_dma_u8(self.dma_virtual + DATA_OFFSET - 1) == 0x3c
                && read_dma_u8(self.dma_virtual + DATA_OFFSET + DATA_LENGTH) == 0xc3
                && (0..DATA_LENGTH)
                    .all(|offset| read_dma_u8(self.dma_virtual + DATA_OFFSET + offset) == 0xa5),
            descriptor_unchanged: read_dma_u64(self.dma_virtual + DESC_OFFSET + 16)
                == unmapped_physical,
        })
    }

    fn submit(&mut self, operation: BlockOperation<'_>) -> bool {
        if !self.active {
            return false;
        }
        let (request_type, sector, data_length, device_writes, expected_used) = match &operation {
            BlockOperation::Read { sector, .. } => (0_u32, *sector, DATA_LENGTH, true, 513_u32),
            BlockOperation::Write { sector, .. } => (1_u32, *sector, DATA_LENGTH, false, 1_u32),
            BlockOperation::Flush => (4_u32, 0, 0, false, 1_u32),
        };
        if data_length != 0 && sector >= self.capacity_sectors {
            return false;
        }
        prepare_operation(
            self.dma_physical,
            self.dma_virtual,
            request_type,
            sector,
            data_length,
            device_writes,
            match &operation {
                BlockOperation::Write { input, .. } => Some(*input),
                _ => None,
            },
        );

        let available_slot = usize::from(self.available_index % self.queue_size);
        let used_slot = usize::from(self.used_index % self.queue_size);
        if read_dma_u16(self.dma_virtual + USED_OFFSET + 2) != self.used_index {
            return false;
        }
        write_dma_u16(
            self.dma_virtual + AVAIL_OFFSET + 4 + available_slot * size_of::<u16>(),
            0,
        );
        compiler_fence(Ordering::SeqCst);
        let expected_available = self.available_index.wrapping_add(1);
        write_dma_u16(self.dma_virtual + AVAIL_OFFSET + 2, expected_available);
        self.available_index = expected_available;
        write_mmio_u16(self.notification, 0);

        let expected_used_index = self.used_index.wrapping_add(1);
        let mut completed = false;
        for _ in 0..QUEUE_POLL_LIMIT {
            compiler_fence(Ordering::SeqCst);
            if read_common_u8(self.common, COMMON_DEVICE_STATUS)
                & (STATUS_DEVICE_NEEDS_RESET | STATUS_FAILED)
                != 0
            {
                return false;
            }
            if read_dma_u16(self.dma_virtual + USED_OFFSET + 2) == expected_used_index {
                completed = true;
                break;
            }
            core::hint::spin_loop();
        }
        compiler_fence(Ordering::SeqCst);
        let used_element = self.dma_virtual + USED_OFFSET + 4 + used_slot * 8;
        if !completed
            || read_dma_u32(used_element) != 0
            || read_dma_u32(used_element + 4) != expected_used
            || read_dma_u8(self.dma_virtual + STATUS_OFFSET) != 0
            || (data_length != 0
                && (read_dma_u8(self.dma_virtual + DATA_OFFSET - 1) != 0x3c
                    || read_dma_u8(self.dma_virtual + DATA_OFFSET + DATA_LENGTH) != 0xc3))
        {
            return false;
        }
        self.used_index = expected_used_index;
        if let BlockOperation::Read { output, .. } = operation {
            for (offset, byte) in output.iter_mut().enumerate() {
                *byte = read_dma_u8(self.dma_virtual + DATA_OFFSET + offset);
            }
        }
        true
    }
}

impl volume::BlockDevice for VirtioBlockBackend {
    fn read_sector(&mut self, sector: usize, output: &mut [u8; DATA_LENGTH]) -> bool {
        let Ok(sector) = u64::try_from(sector) else {
            return false;
        };
        self.submit(BlockOperation::Read { sector, output })
    }

    fn write_sector(&mut self, sector: usize, input: &[u8; DATA_LENGTH]) -> bool {
        let Ok(sector) = u64::try_from(sector) else {
            return false;
        };
        self.submit(BlockOperation::Write { sector, input })
    }

    fn flush(&mut self) -> bool {
        self.submit(BlockOperation::Flush)
    }
}

fn negotiate(common: usize) -> Option<VirtioBlockInitialization> {
    reset(common)?;
    write_common_u8(common, COMMON_DEVICE_STATUS, STATUS_ACKNOWLEDGE);
    write_common_u8(
        common,
        COMMON_DEVICE_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER,
    );

    write_common_u32(common, COMMON_DEVICE_FEATURE_SELECT, 0);
    let low = read_common_u32(common, COMMON_DEVICE_FEATURE);
    write_common_u32(common, COMMON_DEVICE_FEATURE_SELECT, 1);
    let high = read_common_u32(common, COMMON_DEVICE_FEATURE);
    let offered_features = u64::from(low) | (u64::from(high) << 32);
    let accepted_features = select_features(offered_features)?;

    write_common_u32(common, COMMON_DRIVER_FEATURE_SELECT, 0);
    write_common_u32(common, COMMON_DRIVER_FEATURE, accepted_features as u32);
    write_common_u32(common, COMMON_DRIVER_FEATURE_SELECT, 1);
    write_common_u32(
        common,
        COMMON_DRIVER_FEATURE,
        (accepted_features >> 32) as u32,
    );
    write_common_u8(
        common,
        COMMON_DEVICE_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK,
    );
    if read_common_u8(common, COMMON_DEVICE_STATUS) & STATUS_FEATURES_OK == 0 {
        return None;
    }
    let queue_count = read_common_u16(common, COMMON_NUM_QUEUES);
    (queue_count != 0).then_some(VirtioBlockInitialization {
        offered_features,
        accepted_features,
        queue_count,
    })
}

fn negotiate_network(common: usize) -> Option<VirtioNetworkInitialization> {
    reset(common)?;
    write_common_u8(common, COMMON_DEVICE_STATUS, STATUS_ACKNOWLEDGE);
    write_common_u8(
        common,
        COMMON_DEVICE_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER,
    );

    write_common_u32(common, COMMON_DEVICE_FEATURE_SELECT, 0);
    let low = read_common_u32(common, COMMON_DEVICE_FEATURE);
    write_common_u32(common, COMMON_DEVICE_FEATURE_SELECT, 1);
    let high = read_common_u32(common, COMMON_DEVICE_FEATURE);
    let offered_features = u64::from(low) | (u64::from(high) << 32);
    let accepted_features = select_network_features(offered_features)?;

    write_common_u32(common, COMMON_DRIVER_FEATURE_SELECT, 0);
    write_common_u32(common, COMMON_DRIVER_FEATURE, accepted_features as u32);
    write_common_u32(common, COMMON_DRIVER_FEATURE_SELECT, 1);
    write_common_u32(
        common,
        COMMON_DRIVER_FEATURE,
        (accepted_features >> 32) as u32,
    );
    write_common_u8(
        common,
        COMMON_DEVICE_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK,
    );
    if read_common_u8(common, COMMON_DEVICE_STATUS) & STATUS_FEATURES_OK == 0 {
        return None;
    }
    let queue_count = read_common_u16(common, COMMON_NUM_QUEUES);
    (queue_count >= 2).then_some(VirtioNetworkInitialization {
        offered_features,
        accepted_features,
        queue_count,
        mac: [0; 6],
        receive_queue_size: 0,
        transmit_queue_size: 0,
        receive_notification: 0,
        transmit_notification: 0,
    })
}

fn network_queue_geometry(
    common: usize,
    device: VirtioNetworkDevice,
) -> Option<((u16, u64), (u16, u64))> {
    let (notification_physical, notification_length) = device.notify_region()?;
    let mut geometry = [(0_u16, 0_u64); 2];
    for (queue, entry) in geometry.iter_mut().enumerate() {
        write_common_u16(common, COMMON_QUEUE_SELECT, u16::try_from(queue).ok()?);
        let size = read_common_u16(common, COMMON_QUEUE_SIZE);
        let notification_offset = notification_byte_offset(
            read_common_u16(common, COMMON_QUEUE_NOTIFY_OFF),
            device.notify_multiplier,
            notification_length,
        )?;
        let size = validate_network_queue_size(size)?;
        if read_common_u16(common, COMMON_QUEUE_ENABLE) != 0
            || read_common_u16(common, COMMON_QUEUE_MSIX_VECTOR) != u16::MAX
            || read_common_u64(common, COMMON_QUEUE_DESC) != 0
            || read_common_u64(common, COMMON_QUEUE_DRIVER) != 0
            || read_common_u64(common, COMMON_QUEUE_DEVICE) != 0
        {
            return None;
        }
        *entry = (
            size,
            notification_physical.checked_add(notification_offset as u64)?,
        );
    }
    (geometry[0].1 != geometry[1].1).then_some((geometry[0], geometry[1]))
}

fn validate_network_queue_size(size: u16) -> Option<u16> {
    (size >= QUEUE_SIZE && size.is_power_of_two()).then_some(size)
}

pub(crate) fn verify_network_queue_rejections(initialization: VirtioNetworkInitialization) -> bool {
    initialization.receive_queue_size == 256
        && initialization.transmit_queue_size == 256
        && initialization
            .transmit_notification
            .checked_sub(initialization.receive_notification)
            == Some(4)
        && validate_network_queue_size(0).is_none()
        && validate_network_queue_size(QUEUE_SIZE - 1).is_none()
        && validate_network_queue_size(QUEUE_SIZE) == Some(QUEUE_SIZE)
        && validate_network_queue_size(255).is_none()
        && validate_network_queue_size(256) == Some(256)
        && notification_byte_offset(1, 4, 4).is_none()
        && notification_byte_offset(u16::MAX, u32::MAX, 4096).is_none()
}

pub(crate) fn verify_network_dma_layout() -> bool {
    network_dma_layout_is_valid(&network_dma_regions())
        && NET_BUFFER_LENGTH >= 12 + 1514
        && NET_TX_BUFFER_OFFSET + NET_BUFFER_LENGTH == DMA_PAGE_SIZE
}

pub(crate) fn verify_network_dma_layout_rejections() -> bool {
    let mut zero_length = network_dma_regions();
    zero_length[0].1 = 0;
    let mut overlap = network_dma_regions();
    overlap[3].0 = NET_RX_DESC_OFFSET;
    let mut misaligned = network_dma_regions();
    misaligned[2].0 += 1;
    let mut out_of_page = network_dma_regions();
    out_of_page[7].1 += 1;
    !network_dma_layout_is_valid(&zero_length)
        && !network_dma_layout_is_valid(&overlap)
        && !network_dma_layout_is_valid(&misaligned)
        && !network_dma_layout_is_valid(&out_of_page)
}

fn network_dma_regions() -> [(usize, usize, usize); 8] {
    [
        (NET_RX_DESC_OFFSET, NET_DESCRIPTOR_BYTES, 16),
        (NET_RX_AVAIL_OFFSET, NET_AVAILABLE_BYTES, 2),
        (NET_RX_USED_OFFSET, NET_USED_BYTES, 4),
        (NET_TX_DESC_OFFSET, NET_DESCRIPTOR_BYTES, 16),
        (NET_TX_AVAIL_OFFSET, NET_AVAILABLE_BYTES, 2),
        (NET_TX_USED_OFFSET, NET_USED_BYTES, 4),
        (NET_RX_BUFFER_OFFSET, NET_BUFFER_LENGTH, 2),
        (NET_TX_BUFFER_OFFSET, NET_BUFFER_LENGTH, 2),
    ]
}

fn network_dma_layout_is_valid(regions: &[(usize, usize, usize); 8]) -> bool {
    for (index, (offset, length, alignment)) in regions.iter().copied().enumerate() {
        if length == 0
            || alignment == 0
            || !offset.is_multiple_of(alignment)
            || offset
                .checked_add(length)
                .is_none_or(|end| end > DMA_PAGE_SIZE)
            || regions[index + 1..]
                .iter()
                .any(|(other_offset, other_length, _)| {
                    let end = offset + length;
                    let other_end = other_offset.saturating_add(*other_length);
                    offset < other_end && *other_offset < end
                })
        {
            return false;
        }
    }
    true
}

fn read_network_mac(common: usize, device_configuration: usize) -> Option<[u8; 6]> {
    for _ in 0..8 {
        let before = read_common_u8(common, COMMON_CONFIG_GENERATION);
        let mac = core::array::from_fn(|offset| read_common_u8(device_configuration, offset));
        let after = read_common_u8(common, COMMON_CONFIG_GENERATION);
        if before == after && mac != [0; 6] && mac != [0xff; 6] && mac[0] & 1 == 0 {
            return Some(mac);
        }
    }
    None
}

pub(crate) fn verify_feature_rejections(offered: u64, accepted: u64) -> bool {
    accepted == (FEATURE_VERSION_1 | FEATURE_ACCESS_PLATFORM | FEATURE_FLUSH)
        && select_features(offered) == Some(accepted)
        && select_features(offered & !FEATURE_VERSION_1).is_none()
        && select_features(offered & !FEATURE_ACCESS_PLATFORM).is_none()
        && select_features(offered & !FEATURE_FLUSH).is_none()
        && select_features(0).is_none()
}

pub(crate) fn verify_network_feature_rejections(offered: u64, accepted: u64) -> bool {
    accepted
        == (FEATURE_VERSION_1 | FEATURE_ACCESS_PLATFORM | FEATURE_NET_MAC | FEATURE_NET_MRG_RXBUF)
        && select_network_features(offered) == Some(accepted)
        && select_network_features(offered & !FEATURE_VERSION_1).is_none()
        && select_network_features(offered & !FEATURE_ACCESS_PLATFORM).is_none()
        && select_network_features(offered & !FEATURE_NET_MAC).is_none()
        && select_network_features(offered & !FEATURE_NET_MRG_RXBUF).is_none()
        && select_network_features(0).is_none()
}

pub(crate) fn verify_queue_rejections() -> bool {
    select_queue_size(0).is_none()
        && select_queue_size(2).is_none()
        && select_queue_size(3) == Some(3)
        && select_queue_size(QUEUE_SIZE) == Some(QUEUE_SIZE)
        && select_queue_size(u16::MAX) == Some(QUEUE_SIZE)
        && notification_byte_offset(0, 4, 2) == Some(0)
        && notification_byte_offset(1, 4, 6) == Some(4)
        && notification_byte_offset(0, 0, 2).is_none()
        && notification_byte_offset(1, 4, 5).is_none()
        && notification_byte_offset(u16::MAX, u32::MAX, 4096).is_none()
        && DESC_OFFSET + 3 * 16 <= AVAIL_OFFSET
        && AVAIL_OFFSET + 4 + usize::from(QUEUE_SIZE) * 2 + 2 <= USED_OFFSET
        && USED_OFFSET + 4 + usize::from(QUEUE_SIZE) * 8 + 2 <= REQUEST_OFFSET
        && REQUEST_OFFSET + 16 < DATA_OFFSET
        && DATA_OFFSET + DATA_LENGTH < STATUS_OFFSET
        && STATUS_OFFSET < DMA_PAGE_SIZE
}

fn select_features(offered: u64) -> Option<u64> {
    let required = FEATURE_VERSION_1 | FEATURE_ACCESS_PLATFORM | FEATURE_FLUSH;
    (offered & required == required).then_some(required)
}

fn select_network_features(offered: u64) -> Option<u64> {
    let required =
        FEATURE_VERSION_1 | FEATURE_ACCESS_PLATFORM | FEATURE_NET_MAC | FEATURE_NET_MRG_RXBUF;
    (offered & required == required).then_some(required)
}

fn select_queue_size(maximum: u16) -> Option<u16> {
    (maximum >= 3).then_some(maximum.min(QUEUE_SIZE))
}

fn notification_byte_offset(offset: u16, multiplier: u32, length: usize) -> Option<usize> {
    if multiplier == 0 {
        return None;
    }
    let offset = usize::from(offset).checked_mul(usize::try_from(multiplier).ok()?)?;
    (offset.checked_add(size_of::<u16>())? <= length).then_some(offset)
}

fn set_pci_command(device: VirtioBlockDevice, bus_master: bool) -> bool {
    let command =
        unsafe { read_config_u16(device.bus, device.device, device.function, PCI_COMMAND) };
    let mut requested = command | COMMAND_MEMORY_SPACE;
    if bus_master {
        requested |= COMMAND_BUS_MASTER;
    } else {
        requested &= !COMMAND_BUS_MASTER;
    }
    unsafe {
        write_config_u16(
            device.bus,
            device.device,
            device.function,
            PCI_COMMAND,
            requested,
        );
    }
    let observed =
        unsafe { read_config_u16(device.bus, device.device, device.function, PCI_COMMAND) };
    observed & (COMMAND_MEMORY_SPACE | COMMAND_BUS_MASTER)
        == COMMAND_MEMORY_SPACE | if bus_master { COMMAND_BUS_MASTER } else { 0 }
}

pub(crate) fn bus_master_is_disabled(device: VirtioBlockDevice) -> bool {
    (unsafe { read_config_u16(device.bus, device.device, device.function, PCI_COMMAND) })
        & COMMAND_BUS_MASTER
        == 0
}

fn bus_master_is_enabled(device: VirtioBlockDevice) -> bool {
    (unsafe { read_config_u16(device.bus, device.device, device.function, PCI_COMMAND) })
        & COMMAND_BUS_MASTER
        != 0
}

pub(crate) fn device_is_reset(device: VirtioBlockDevice, mapping: MmioMapping) -> bool {
    let Some((physical, length)) = device.common_region() else {
        return false;
    };
    let Some(common) = mapping.address(physical, length) else {
        return false;
    };
    read_common_u8(common, COMMON_DEVICE_STATUS) == 0
}

pub(crate) fn network_device_is_reset(device: VirtioNetworkDevice, mapping: MmioMapping) -> bool {
    let Some((physical, length)) = device.common_region() else {
        return false;
    };
    let Some(common) = mapping.address(physical, length) else {
        return false;
    };
    read_common_u8(common, COMMON_DEVICE_STATUS) == 0
}

pub(crate) fn device_features_are_selected(
    device: VirtioBlockDevice,
    mapping: MmioMapping,
) -> bool {
    let Some((common_physical, common_length)) = device.common_region() else {
        return false;
    };
    let Some(common) = mapping.address(common_physical, common_length) else {
        return false;
    };
    let status = read_common_u8(common, COMMON_DEVICE_STATUS);
    if status & (STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK)
        != STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK
        || status & (STATUS_DEVICE_NEEDS_RESET | STATUS_FAILED) != 0
    {
        return false;
    }
    write_common_u32(common, COMMON_DRIVER_FEATURE_SELECT, 0);
    let low = read_common_u32(common, COMMON_DRIVER_FEATURE);
    write_common_u32(common, COMMON_DRIVER_FEATURE_SELECT, 1);
    let high = read_common_u32(common, COMMON_DRIVER_FEATURE);
    u64::from(low) | (u64::from(high) << 32)
        == FEATURE_VERSION_1 | FEATURE_ACCESS_PLATFORM | FEATURE_FLUSH
}

pub(crate) fn device_active_queue_is_bound(
    device: VirtioBlockDevice,
    mapping: MmioMapping,
    dma_physical: u64,
) -> bool {
    let Some((common_physical, common_length)) = device.common_region() else {
        return false;
    };
    let Some(common) = mapping.address(common_physical, common_length) else {
        return false;
    };
    device_features_are_selected(device, mapping)
        && read_common_u8(common, COMMON_DEVICE_STATUS)
            == STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK
        && bus_master_is_enabled(device)
        && read_common_u16(common, COMMON_QUEUE_SELECT) == 0
        && read_common_u16(common, COMMON_QUEUE_SIZE) == QUEUE_SIZE
        && read_common_u16(common, COMMON_QUEUE_ENABLE) == 1
        && read_common_u64(common, COMMON_QUEUE_DESC) == dma_physical
        && read_common_u64(common, COMMON_QUEUE_DRIVER) == dma_physical + AVAIL_OFFSET as u64
        && read_common_u64(common, COMMON_QUEUE_DEVICE) == dma_physical + USED_OFFSET as u64
}

pub(crate) fn arm_isolated_device_interrupt(
    device: VirtioBlockDevice,
    mapping: MmioMapping,
    msix_mapping: MmioMapping,
    apic: &LocalApic,
    domain: u64,
    request: u64,
) -> bool {
    if domain == 0
        || request == 0
        || !device_is_reset(device, mapping)
        || !bus_master_is_disabled(device)
        || !program_masked_msix_vector(device, msix_mapping, 0, apic.id, VIRTIO_INTERRUPT_VECTOR)
        || !install_virtio_interrupt(apic)
    {
        return false;
    }
    let armed = set_msix_control(device, true, true)
        && set_pci_command(device, true)
        && bus_master_is_enabled(device);
    if armed {
        VIRTIO_INTERRUPT_DOMAIN.store(domain, Ordering::SeqCst);
        VIRTIO_INTERRUPT_REQUEST.store(request, Ordering::SeqCst);
        return true;
    }
    let _ = set_msix_control(device, true, true);
    let _ = set_msix_entry_mask(device, msix_mapping, 0, true);
    let _ = set_msix_control(device, false, true);
    let _ = set_pci_command(device, false);
    VIRTIO_INTERRUPT_ACTIVE.store(false, Ordering::SeqCst);
    let _ = retire_virtio_interrupt(apic);
    false
}

pub(crate) fn isolated_interrupt_wait_ready(
    device: VirtioBlockDevice,
    mapping: MmioMapping,
    domain: u64,
    request: u64,
    dma_physical: u64,
) -> bool {
    let Some((common_physical, common_length)) = device.common_region() else {
        return false;
    };
    let Some(common) = mapping.address(common_physical, common_length) else {
        return false;
    };
    domain != 0
        && request != 0
        && VIRTIO_INTERRUPT_DOMAIN.load(Ordering::SeqCst) == domain
        && VIRTIO_INTERRUPT_REQUEST.load(Ordering::SeqCst) == request
        && !VIRTIO_INTERRUPT_ACTIVE.load(Ordering::SeqCst)
        && !VIRTIO_INTERRUPT_FIRED.load(Ordering::SeqCst)
        && VIRTIO_INTERRUPT_COUNT.load(Ordering::SeqCst) == 0
        && read_common_u16(common, COMMON_QUEUE_MSIX_VECTOR) == 0
        && device_active_queue_is_bound(device, mapping, dma_physical)
}

pub(crate) fn complete_isolated_interrupt_wait(
    device: VirtioBlockDevice,
    msix_mapping: MmioMapping,
    domain: u64,
    request: u64,
    notified: bool,
) -> IsolatedInterruptCompletion {
    if VIRTIO_INTERRUPT_DOMAIN.load(Ordering::SeqCst) != domain
        || VIRTIO_INTERRUPT_REQUEST.load(Ordering::SeqCst) != request
    {
        return IsolatedInterruptCompletion::Failed;
    }
    let force_timeout = notified
        && FORCED_NOTIFIED_TIMEOUT_DOMAIN.load(Ordering::SeqCst) == domain
        && FORCED_NOTIFIED_TIMEOUT_REQUEST.load(Ordering::SeqCst) == request;
    if force_timeout {
        FORCED_NOTIFIED_TIMEOUT_DOMAIN.store(0, Ordering::SeqCst);
        FORCED_NOTIFIED_TIMEOUT_REQUEST.store(0, Ordering::SeqCst);
    }
    VIRTIO_INTERRUPT_ACTIVE.store(true, Ordering::SeqCst);
    let interrupt_configured = if force_timeout {
        set_msix_entry_mask(device, msix_mapping, 0, true) && set_msix_control(device, true, true)
    } else {
        set_msix_entry_mask(device, msix_mapping, 0, false) && set_msix_control(device, true, false)
    };
    if !timer::arm_device_wait_timer(domain, request) || !interrupt_configured {
        VIRTIO_INTERRUPT_ACTIVE.store(false, Ordering::SeqCst);
        let _ = timer::finish_device_wait_timer(domain, request);
        let _ = scheduler::wake_device_completion(
            domain,
            control_abi::BLOCK_DEVICE_OBJECT,
            request,
            u64::MAX - 2,
        );
        return IsolatedInterruptCompletion::Failed;
    }
    while !VIRTIO_INTERRUPT_FIRED.load(Ordering::SeqCst)
        && !timer::device_wait_timed_out(domain, request)
    {
        // STI's interrupt shadow covers the following HLT, so an interrupt
        // cannot be lost between the readiness test and sleeping.
        unsafe { asm!("sti", "hlt", "cli", options(nomem, nostack)) };
    }
    let function_masked = set_msix_control(device, true, true);
    let entry_masked = set_msix_entry_mask(device, msix_mapping, 0, true);
    VIRTIO_INTERRUPT_ACTIVE.store(false, Ordering::SeqCst);
    let Some(timed_out) = timer::finish_device_wait_timer(domain, request) else {
        return IsolatedInterruptCompletion::Failed;
    };
    if !function_masked || !entry_masked || scheduler::current_domain() != Some(domain) {
        IsolatedInterruptCompletion::Failed
    } else if timed_out {
        IsolatedInterruptCompletion::TimedOut
    } else if VIRTIO_INTERRUPT_COUNT.load(Ordering::SeqCst) == 1 {
        IsolatedInterruptCompletion::Completed
    } else {
        IsolatedInterruptCompletion::Failed
    }
}

pub(crate) fn force_next_notified_wait_timeout(domain: u64, request: u64) -> bool {
    if domain == 0
        || request == 0
        || VIRTIO_INTERRUPT_DOMAIN.load(Ordering::SeqCst) != domain
        || VIRTIO_INTERRUPT_REQUEST.load(Ordering::SeqCst) != request
        || FORCED_NOTIFIED_TIMEOUT_DOMAIN.load(Ordering::SeqCst) != 0
        || FORCED_NOTIFIED_TIMEOUT_REQUEST.load(Ordering::SeqCst) != 0
    {
        return false;
    }
    FORCED_NOTIFIED_TIMEOUT_REQUEST.store(request, Ordering::SeqCst);
    FORCED_NOTIFIED_TIMEOUT_DOMAIN.store(domain, Ordering::SeqCst);
    true
}

pub(crate) fn advance_isolated_interrupt_wait(
    domain: u64,
    completed_request: u64,
    next_request: u64,
) -> bool {
    if domain == 0
        || next_request != completed_request.checked_add(1).unwrap_or(0)
        || VIRTIO_INTERRUPT_DOMAIN.load(Ordering::SeqCst) != domain
        || VIRTIO_INTERRUPT_REQUEST.load(Ordering::SeqCst) != completed_request
        || VIRTIO_INTERRUPT_ACTIVE.load(Ordering::SeqCst)
        || !VIRTIO_INTERRUPT_FIRED.load(Ordering::SeqCst)
        || VIRTIO_INTERRUPT_COUNT.load(Ordering::SeqCst) != 1
    {
        return false;
    }
    VIRTIO_INTERRUPT_FIRED.store(false, Ordering::SeqCst);
    VIRTIO_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    VIRTIO_INTERRUPT_REQUEST.store(next_request, Ordering::SeqCst);
    true
}

pub(crate) fn quiesce_isolated_interrupt(
    device: VirtioBlockDevice,
    mapping: MmioMapping,
    msix_mapping: MmioMapping,
    domain: u64,
    request: u64,
) -> bool {
    if VIRTIO_INTERRUPT_DOMAIN.load(Ordering::SeqCst) != domain
        || VIRTIO_INTERRUPT_REQUEST.load(Ordering::SeqCst) != request
    {
        return false;
    }
    let mode = if VIRTIO_INTERRUPT_APIC_MODE.load(Ordering::SeqCst) == 1 {
        Mode::X2Apic
    } else {
        Mode::XApic
    };
    let Some(apic) =
        LocalApic::from_saved(0, mode, VIRTIO_INTERRUPT_APIC_BASE.load(Ordering::SeqCst))
    else {
        return false;
    };
    let function_masked = set_msix_control(device, true, true);
    let entry_masked = set_msix_entry_mask(device, msix_mapping, 0, true);
    let msix_disabled = set_msix_control(device, false, true);
    let device_quiesced = quiesce(device, mapping);
    VIRTIO_INTERRUPT_ACTIVE.store(false, Ordering::SeqCst);
    function_masked
        && entry_masked
        && msix_disabled
        && device_quiesced
        && retire_virtio_interrupt(&apic)
}

pub(crate) fn queue_zero_notification_register(
    device: VirtioBlockDevice,
    mapping: MmioMapping,
) -> Option<u64> {
    let (common_physical, common_length) = device.common_region()?;
    let common = mapping.address(common_physical, common_length)?;
    let (notify_physical, notify_length) = device.notify_region()?;
    write_common_u16(common, COMMON_QUEUE_SELECT, 0);
    let offset = notification_byte_offset(
        read_common_u16(common, COMMON_QUEUE_NOTIFY_OFF),
        device.notify_multiplier,
        notify_length,
    )?;
    let register = notify_physical.checked_add(offset as u64)?;
    (register.is_multiple_of(size_of::<u16>() as u64)
        && mapping.address(register, size_of::<u16>()).is_some())
    .then_some(register)
}

pub(crate) fn quiesce(device: VirtioBlockDevice, mapping: MmioMapping) -> bool {
    let Some((common_physical, common_length)) = device.common_region() else {
        return false;
    };
    let Some(common) = mapping.address(common_physical, common_length) else {
        return false;
    };
    set_pci_command(device, false)
        && reset(common).is_some()
        && bus_master_is_disabled(device)
        && device_is_reset(device, mapping)
}

fn read_capacity(common: usize, device_configuration: usize) -> Option<u64> {
    for _ in 0..8 {
        let before = read_common_u8(common, COMMON_CONFIG_GENERATION);
        let capacity = read_mmio_u64(device_configuration);
        let after = read_common_u8(common, COMMON_CONFIG_GENERATION);
        if before == after {
            return Some(capacity);
        }
    }
    None
}

fn prepare_read_request(dma_physical: u64, dma_virtual: usize, sector: u64) {
    unsafe { core::ptr::write_bytes(dma_virtual as *mut u8, 0, DMA_PAGE_SIZE) };
    write_dma_u64(
        dma_virtual + DESC_OFFSET,
        dma_physical + REQUEST_OFFSET as u64,
    );
    write_dma_u32(dma_virtual + DESC_OFFSET + 8, 16);
    write_dma_u16(dma_virtual + DESC_OFFSET + 12, DESC_NEXT);
    write_dma_u16(dma_virtual + DESC_OFFSET + 14, 1);

    write_dma_u64(
        dma_virtual + DESC_OFFSET + 16,
        dma_physical + DATA_OFFSET as u64,
    );
    write_dma_u32(dma_virtual + DESC_OFFSET + 24, DATA_LENGTH as u32);
    write_dma_u16(dma_virtual + DESC_OFFSET + 28, DESC_NEXT | DESC_WRITE);
    write_dma_u16(dma_virtual + DESC_OFFSET + 30, 2);

    write_dma_u64(
        dma_virtual + DESC_OFFSET + 32,
        dma_physical + STATUS_OFFSET as u64,
    );
    write_dma_u32(dma_virtual + DESC_OFFSET + 40, 1);
    write_dma_u16(dma_virtual + DESC_OFFSET + 44, DESC_WRITE);
    write_dma_u16(dma_virtual + DESC_OFFSET + 46, 0);

    write_dma_u32(dma_virtual + REQUEST_OFFSET, 0);
    write_dma_u32(dma_virtual + REQUEST_OFFSET + 4, 0);
    write_dma_u64(dma_virtual + REQUEST_OFFSET + 8, sector);
    unsafe {
        core::ptr::write_bytes((dma_virtual + DATA_OFFSET) as *mut u8, 0xa5, DATA_LENGTH);
    }
    write_dma_u8(dma_virtual + DATA_OFFSET - 1, 0x3c);
    write_dma_u8(dma_virtual + DATA_OFFSET + DATA_LENGTH, 0xc3);
    write_dma_u8(dma_virtual + STATUS_OFFSET, 0xff);

    write_dma_u16(dma_virtual + AVAIL_OFFSET, 0);
    write_dma_u16(dma_virtual + AVAIL_OFFSET + 2, 1);
    write_dma_u16(dma_virtual + AVAIL_OFFSET + 4, 0);
    compiler_fence(Ordering::SeqCst);
}

fn prepare_operation(
    dma_physical: u64,
    dma_virtual: usize,
    request_type: u32,
    sector: u64,
    data_length: usize,
    device_writes: bool,
    input: Option<&[u8; DATA_LENGTH]>,
) {
    unsafe {
        core::ptr::write_bytes((dma_virtual + DESC_OFFSET) as *mut u8, 0, 3 * 16);
        core::ptr::write_bytes((dma_virtual + REQUEST_OFFSET) as *mut u8, 0, 16);
    }
    write_dma_u32(dma_virtual + REQUEST_OFFSET, request_type);
    write_dma_u64(dma_virtual + REQUEST_OFFSET + 8, sector);
    write_dma_u64(
        dma_virtual + DESC_OFFSET,
        dma_physical + REQUEST_OFFSET as u64,
    );
    write_dma_u32(dma_virtual + DESC_OFFSET + 8, 16);
    write_dma_u16(dma_virtual + DESC_OFFSET + 12, DESC_NEXT);
    write_dma_u16(dma_virtual + DESC_OFFSET + 14, 1);

    if data_length != 0 {
        write_dma_u64(
            dma_virtual + DESC_OFFSET + 16,
            dma_physical + DATA_OFFSET as u64,
        );
        write_dma_u32(dma_virtual + DESC_OFFSET + 24, data_length as u32);
        write_dma_u16(
            dma_virtual + DESC_OFFSET + 28,
            DESC_NEXT | if device_writes { DESC_WRITE } else { 0 },
        );
        write_dma_u16(dma_virtual + DESC_OFFSET + 30, 2);
        write_dma_u64(
            dma_virtual + DESC_OFFSET + 32,
            dma_physical + STATUS_OFFSET as u64,
        );
        write_dma_u32(dma_virtual + DESC_OFFSET + 40, 1);
        write_dma_u16(dma_virtual + DESC_OFFSET + 44, DESC_WRITE);
        if let Some(input) = input {
            for (offset, byte) in input.iter().copied().enumerate() {
                write_dma_u8(dma_virtual + DATA_OFFSET + offset, byte);
            }
        } else {
            unsafe {
                core::ptr::write_bytes((dma_virtual + DATA_OFFSET) as *mut u8, 0xa5, DATA_LENGTH);
            }
        }
        write_dma_u8(dma_virtual + DATA_OFFSET - 1, 0x3c);
        write_dma_u8(dma_virtual + DATA_OFFSET + DATA_LENGTH, 0xc3);
        write_dma_u8(dma_virtual + STATUS_OFFSET, 0xff);
    } else {
        write_dma_u64(
            dma_virtual + DESC_OFFSET + 16,
            dma_physical + STATUS_OFFSET as u64,
        );
        write_dma_u32(dma_virtual + DESC_OFFSET + 24, 1);
        write_dma_u16(dma_virtual + DESC_OFFSET + 28, DESC_WRITE);
        write_dma_u8(dma_virtual + STATUS_OFFSET, 0xff);
    }
    compiler_fence(Ordering::SeqCst);
}

fn reset(common: usize) -> Option<()> {
    write_common_u8(common, COMMON_DEVICE_STATUS, 0);
    for _ in 0..RESET_POLL_LIMIT {
        if read_common_u8(common, COMMON_DEVICE_STATUS) == 0 {
            return Some(());
        }
        core::hint::spin_loop();
    }
    None
}

fn read_common_u8(common: usize, offset: usize) -> u8 {
    compiler_fence(Ordering::SeqCst);
    let value = unsafe { read_volatile((common + offset) as *const u8) };
    compiler_fence(Ordering::SeqCst);
    value
}

fn read_common_u16(common: usize, offset: usize) -> u16 {
    compiler_fence(Ordering::SeqCst);
    let value = unsafe { read_volatile((common + offset) as *const u16) };
    compiler_fence(Ordering::SeqCst);
    u16::from_le(value)
}

fn read_common_u32(common: usize, offset: usize) -> u32 {
    compiler_fence(Ordering::SeqCst);
    let value = unsafe { read_volatile((common + offset) as *const u32) };
    compiler_fence(Ordering::SeqCst);
    u32::from_le(value)
}

fn read_common_u64(common: usize, offset: usize) -> u64 {
    let low = read_common_u32(common, offset);
    let high = read_common_u32(common, offset + size_of::<u32>());
    u64::from(low) | (u64::from(high) << 32)
}

fn write_common_u8(common: usize, offset: usize, value: u8) {
    compiler_fence(Ordering::SeqCst);
    unsafe { write_volatile((common + offset) as *mut u8, value) };
    compiler_fence(Ordering::SeqCst);
}

fn write_common_u32(common: usize, offset: usize, value: u32) {
    compiler_fence(Ordering::SeqCst);
    unsafe { write_volatile((common + offset) as *mut u32, value.to_le()) };
    compiler_fence(Ordering::SeqCst);
}

fn write_common_u16(common: usize, offset: usize, value: u16) {
    compiler_fence(Ordering::SeqCst);
    unsafe { write_volatile((common + offset) as *mut u16, value.to_le()) };
    compiler_fence(Ordering::SeqCst);
}

fn write_common_u64(common: usize, offset: usize, value: u64) {
    compiler_fence(Ordering::SeqCst);
    unsafe { write_volatile((common + offset) as *mut u64, value.to_le()) };
    compiler_fence(Ordering::SeqCst);
}

fn read_mmio_u64(address: usize) -> u64 {
    compiler_fence(Ordering::SeqCst);
    let value = unsafe { read_volatile(address as *const u64) };
    compiler_fence(Ordering::SeqCst);
    u64::from_le(value)
}

fn read_mmio_u32(address: usize) -> u32 {
    compiler_fence(Ordering::SeqCst);
    let value = unsafe { read_volatile(address as *const u32) };
    compiler_fence(Ordering::SeqCst);
    u32::from_le(value)
}

fn write_mmio_u32(address: usize, value: u32) {
    compiler_fence(Ordering::SeqCst);
    unsafe { write_volatile(address as *mut u32, value.to_le()) };
    compiler_fence(Ordering::SeqCst);
}

fn write_mmio_u16(address: usize, value: u16) {
    compiler_fence(Ordering::SeqCst);
    unsafe { write_volatile(address as *mut u16, value.to_le()) };
    compiler_fence(Ordering::SeqCst);
}

fn read_dma_u8(address: usize) -> u8 {
    unsafe { read_volatile(address as *const u8) }
}

fn read_dma_u16(address: usize) -> u16 {
    u16::from_le(unsafe { read_volatile(address as *const u16) })
}

fn read_dma_u32(address: usize) -> u32 {
    u32::from_le(unsafe { read_volatile(address as *const u32) })
}

fn read_dma_u64(address: usize) -> u64 {
    u64::from_le(unsafe { read_volatile(address as *const u64) })
}

fn write_dma_u8(address: usize, value: u8) {
    unsafe { write_volatile(address as *mut u8, value) };
}

fn write_dma_u16(address: usize, value: u16) {
    unsafe { write_volatile(address as *mut u16, value.to_le()) };
}

fn write_dma_u32(address: usize, value: u32) {
    unsafe { write_volatile(address as *mut u32, value.to_le()) };
}

fn write_dma_u64(address: usize, value: u64) {
    unsafe { write_volatile(address as *mut u64, value.to_le()) };
}

fn parse_virtio_block(
    config: &ConfigSpace,
    bus: u8,
    device: u8,
    function: u8,
) -> Option<VirtioBlockDevice> {
    parse_virtio_config(config, VIRTIO_BLOCK_DEVICE, bus, device, function)
}

fn parse_virtio_network(
    config: &ConfigSpace,
    bus: u8,
    device: u8,
    function: u8,
) -> Option<VirtioNetworkDevice> {
    let parsed = parse_virtio_config(config, VIRTIO_NETWORK_DEVICE, bus, device, function)?;
    Some(VirtioNetworkDevice {
        bus: parsed.bus,
        device: parsed.device,
        function: parsed.function,
        common_bar_address: parsed.common_bar_address,
        common_offset: parsed.common_offset,
        common_length: parsed.common_length,
        notify_bar_address: parsed.notify_bar_address,
        notify_offset: parsed.notify_offset,
        notify_length: parsed.notify_length,
        device_bar_address: parsed.device_bar_address,
        device_offset: parsed.device_offset,
        device_length: parsed.device_length,
        notify_multiplier: parsed.notify_multiplier,
        capability_count: parsed.capability_count,
        msix: parsed.msix,
    })
}

fn parse_virtio_config(
    config: &ConfigSpace,
    expected_device: u16,
    bus: u8,
    device: u8,
    function: u8,
) -> Option<VirtioBlockDevice> {
    if read_u16(&config.bytes, 0) != VIRTIO_VENDOR
        || read_u16(&config.bytes, 2) != expected_device
        || config.bytes[0x0e] & 0x7f != 0
        || read_u16(&config.bytes, 6) & STATUS_CAPABILITIES == 0
    {
        return None;
    }

    let mut offset = usize::from(config.bytes[0x34]);
    let mut visited = [false; CONFIG_DWORDS];
    let mut capability_count = 0_usize;
    let mut common = None;
    let mut notify = None;
    let mut isr = None;
    let mut device_region = None;
    let mut notify_multiplier = None;
    let mut msix = None;

    while offset != 0 {
        if !(0x40..=0xfc).contains(&offset)
            || offset % 4 != 0
            || visited[offset / 4]
            || capability_count == MAX_CAPABILITIES
        {
            return None;
        }
        visited[offset / 4] = true;
        capability_count += 1;
        let id = config.bytes[offset];
        let next = usize::from(config.bytes[offset + 1]);
        if id == CAPABILITY_VENDOR_SPECIFIC {
            let length = usize::from(config.bytes[offset + 2]);
            if length < 16 || offset.checked_add(length)? > CONFIG_SIZE {
                return None;
            }
            let configuration_type = config.bytes[offset + 3];
            if matches!(
                configuration_type,
                VIRTIO_COMMON | VIRTIO_NOTIFY | VIRTIO_ISR | VIRTIO_DEVICE
            ) {
                let bar = config.bytes[offset + 4];
                let region = Region {
                    bar_address: memory_bar(config, bar)?,
                    offset: read_u32(&config.bytes, offset + 8),
                    length: read_u32(&config.bytes, offset + 12),
                };
                if region.length == 0 || region.offset.checked_add(region.length).is_none() {
                    return None;
                }
                match configuration_type {
                    VIRTIO_COMMON if common.replace(region).is_none() => {}
                    VIRTIO_NOTIFY if notify.replace(region).is_none() => {
                        if length < 20 {
                            return None;
                        }
                        let multiplier = read_u32(&config.bytes, offset + 16);
                        if multiplier == 0 {
                            return None;
                        }
                        notify_multiplier = Some(multiplier);
                    }
                    VIRTIO_ISR if isr.replace(region).is_none() => {}
                    VIRTIO_DEVICE if device_region.replace(region).is_none() => {}
                    _ => return None,
                }
            }
        } else if id == CAPABILITY_MSIX {
            if offset.checked_add(MSIX_CAPABILITY_SIZE)? > CONFIG_SIZE || msix.is_some() {
                return None;
            }
            let control = read_u16(&config.bytes, offset + 2);
            if control & MSIX_ENABLE != 0 {
                return None;
            }
            let vector_count = (control & MSIX_TABLE_SIZE_MASK).checked_add(1)?;
            let table = read_u32(&config.bytes, offset + 4);
            let pba = read_u32(&config.bytes, offset + 8);
            let table_length = u32::from(vector_count).checked_mul(MSIX_TABLE_ENTRY_SIZE)?;
            let pba_length = u32::from(vector_count)
                .checked_add(MSIX_PBA_BITS_PER_ENTRY - 1)?
                .checked_div(MSIX_PBA_BITS_PER_ENTRY)?
                .checked_mul(MSIX_PBA_ENTRY_SIZE)?;
            let table_offset = table & MSIX_OFFSET_MASK;
            let pba_offset = pba & MSIX_OFFSET_MASK;
            table_offset.checked_add(table_length)?;
            pba_offset.checked_add(pba_length)?;
            msix = Some(MsixCapability {
                capability_offset: u8::try_from(offset).ok()?,
                vector_count,
                table_bar_address: memory_bar(config, (table & MSIX_BIR_MASK) as u8)?,
                table_offset,
                table_length,
                pba_bar_address: memory_bar(config, (pba & MSIX_BIR_MASK) as u8)?,
                pba_offset,
                pba_length,
                function_masked: control & MSIX_FUNCTION_MASK != 0,
            });
        }
        if next != 0 && (!(0x40..=0xfc).contains(&next) || next % 4 != 0) {
            return None;
        }
        offset = next;
    }

    let common = common?;
    let notify = notify?;
    let isr = isr?;
    let device_region = device_region?;
    Some(VirtioBlockDevice {
        bus,
        device,
        function,
        common_bar_address: common.bar_address,
        common_offset: common.offset,
        common_length: common.length,
        notify_bar_address: notify.bar_address,
        notify_offset: notify.offset,
        notify_length: notify.length,
        isr_bar_address: isr.bar_address,
        isr_offset: isr.offset,
        isr_length: isr.length,
        device_bar_address: device_region.bar_address,
        device_offset: device_region.offset,
        device_length: device_region.length,
        notify_multiplier: notify_multiplier?,
        capability_count: u8::try_from(capability_count).ok()?,
        msix: msix?,
    })
}

fn region_bounds(bar: u64, offset: u32, length: u32) -> Option<(u64, usize)> {
    let physical = bar.checked_add(u64::from(offset))?;
    let length = usize::try_from(length).ok()?;
    physical.checked_add(length as u64)?;
    (length != 0).then_some((physical, length))
}

fn first_required_capability(config: &ConfigSpace) -> Option<usize> {
    let mut offset = usize::from(config.bytes[0x34]);
    let mut visited = [false; CONFIG_DWORDS];
    while offset != 0 {
        if !(0x40..=0xfc).contains(&offset) || offset % 4 != 0 || visited[offset / 4] {
            return None;
        }
        visited[offset / 4] = true;
        if config.bytes[offset] == CAPABILITY_VENDOR_SPECIFIC
            && matches!(config.bytes[offset + 3], VIRTIO_COMMON..=VIRTIO_DEVICE)
        {
            return Some(offset);
        }
        offset = usize::from(config.bytes[offset + 1]);
    }
    None
}

fn first_capability(config: &ConfigSpace, id: u8) -> Option<usize> {
    let mut offset = usize::from(config.bytes[0x34]);
    let mut visited = [false; CONFIG_DWORDS];
    while offset != 0 {
        if !(0x40..=0xfc).contains(&offset) || offset % 4 != 0 || visited[offset / 4] {
            return None;
        }
        visited[offset / 4] = true;
        if config.bytes[offset] == id {
            return Some(offset);
        }
        offset = usize::from(config.bytes[offset + 1]);
    }
    None
}

fn memory_bar(config: &ConfigSpace, bar: u8) -> Option<u64> {
    if bar >= 6 {
        return None;
    }
    let offset = 0x10 + usize::from(bar) * 4;
    if bar > 0 {
        let previous = read_u32(&config.bytes, offset - 4);
        if previous & 1 == 0 && (previous >> 1) & 3 == 2 {
            return None;
        }
    }
    let low = read_u32(&config.bytes, offset);
    if low & 1 != 0 {
        return None;
    }
    let address = match (low >> 1) & 3 {
        0 => u64::from(low & !0xf),
        2 if bar < 5 => {
            u64::from(low & !0xf) | (u64::from(read_u32(&config.bytes, offset + 4)) << 32)
        }
        _ => return None,
    };
    (address != 0).then_some(address)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap_or([0; 2]))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap_or([0; 4]))
}

unsafe fn read_config_space(bus: u8, device: u8, function: u8) -> ConfigSpace {
    let mut bytes = [0_u8; CONFIG_SIZE];
    for index in 0..CONFIG_DWORDS {
        let value = read_config_u32(bus, device, function, (index * 4) as u8);
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    ConfigSpace { bytes }
}

unsafe fn read_config_u32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address = CONFIG_ENABLE
        | (u32::from(bus) << 16)
        | (u32::from(device) << 11)
        | (u32::from(function) << 8)
        | u32::from(offset & 0xfc);
    asm!(
        "out dx, eax",
        in("dx") CONFIG_ADDRESS_PORT,
        in("eax") address,
        options(nomem, nostack, preserves_flags)
    );
    let value: u32;
    asm!(
        "in eax, dx",
        in("dx") CONFIG_DATA_PORT,
        out("eax") value,
        options(nomem, nostack, preserves_flags)
    );
    value
}

unsafe fn read_config_u16(bus: u8, device: u8, function: u8, offset: u8) -> u16 {
    let address = CONFIG_ENABLE
        | (u32::from(bus) << 16)
        | (u32::from(device) << 11)
        | (u32::from(function) << 8)
        | u32::from(offset & 0xfc);
    asm!(
        "out dx, eax",
        in("dx") CONFIG_ADDRESS_PORT,
        in("eax") address,
        options(nomem, nostack, preserves_flags)
    );
    let value: u16;
    asm!(
        "in ax, dx",
        in("dx") CONFIG_DATA_PORT + u16::from(offset & 2),
        out("ax") value,
        options(nomem, nostack, preserves_flags)
    );
    value
}

unsafe fn write_config_u16(bus: u8, device: u8, function: u8, offset: u8, value: u16) {
    let address = CONFIG_ENABLE
        | (u32::from(bus) << 16)
        | (u32::from(device) << 11)
        | (u32::from(function) << 8)
        | u32::from(offset & 0xfc);
    asm!(
        "out dx, eax",
        in("dx") CONFIG_ADDRESS_PORT,
        in("eax") address,
        options(nomem, nostack, preserves_flags)
    );
    asm!(
        "out dx, ax",
        in("dx") CONFIG_DATA_PORT + u16::from(offset & 2),
        in("ax") value,
        options(nomem, nostack, preserves_flags)
    );
}
