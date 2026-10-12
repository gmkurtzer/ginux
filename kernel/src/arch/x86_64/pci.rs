use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{compiler_fence, Ordering};

use super::paging::MmioMapping;

const CONFIG_ADDRESS_PORT: u16 = 0x0cf8;
const CONFIG_DATA_PORT: u16 = 0x0cfc;
const CONFIG_ENABLE: u32 = 1 << 31;
const CONFIG_SIZE: usize = 256;
const CONFIG_DWORDS: usize = CONFIG_SIZE / 4;
const VIRTIO_VENDOR: u16 = 0x1af4;
const VIRTIO_BLOCK_DEVICE: u16 = 0x1042;
const STATUS_CAPABILITIES: u16 = 1 << 4;
const CAPABILITY_VENDOR_SPECIFIC: u8 = 0x09;
const VIRTIO_COMMON: u8 = 1;
const VIRTIO_NOTIFY: u8 = 2;
const VIRTIO_ISR: u8 = 3;
const VIRTIO_DEVICE: u8 = 4;
const MAX_CAPABILITIES: usize = 48;
const PCI_COMMAND: u8 = 0x04;
const COMMAND_MEMORY_SPACE: u16 = 1 << 1;
const COMMAND_BUS_MASTER: u16 = 1 << 2;
const COMMON_DEVICE_FEATURE_SELECT: usize = 0x00;
const COMMON_DEVICE_FEATURE: usize = 0x04;
const COMMON_DRIVER_FEATURE_SELECT: usize = 0x08;
const COMMON_DRIVER_FEATURE: usize = 0x0c;
const COMMON_NUM_QUEUES: usize = 0x12;
const COMMON_DEVICE_STATUS: usize = 0x14;
const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_FEATURES_OK: u8 = 8;
const FEATURE_FLUSH: u64 = 1 << 9;
const FEATURE_VERSION_1: u64 = 1 << 32;
const RESET_POLL_LIMIT: usize = 100_000;

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
    pub(crate) notify_multiplier: u32,
    pub(crate) capability_count: u8,
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
}

#[derive(Clone, Copy)]
pub(crate) struct VirtioBlockInitialization {
    pub(crate) offered_features: u64,
    pub(crate) accepted_features: u64,
    pub(crate) queue_count: u16,
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
    let mut found = None;
    for device in 0_u8..32 {
        let identity = unsafe { read_config_u32(0, device, 0, 0) };
        if identity as u16 == u16::MAX {
            continue;
        }
        if identity as u16 != VIRTIO_VENDOR || (identity >> 16) as u16 != VIRTIO_BLOCK_DEVICE {
            continue;
        }
        if found.is_some() {
            return None;
        }
        let config = unsafe { read_config_space(0, device, 0) };
        found = parse_virtio_block(&config, 0, device, 0);
        found?;
    }
    found
}

pub(crate) fn verify_parser_rejections(device: VirtioBlockDevice) -> bool {
    let config = unsafe { read_config_space(device.bus, device.device, device.function) };

    let mut wrong_identity = config;
    wrong_identity.bytes[2] ^= 1;

    let mut no_capabilities = config;
    no_capabilities.bytes[6] &= !(STATUS_CAPABILITIES as u8);

    let mut unaligned_head = config;
    unaligned_head.bytes[0x34] |= 1;

    let head = usize::from(config.bytes[0x34]);
    if !(0x40..=0xfc).contains(&head) || head % 4 != 0 {
        return false;
    }
    let mut cycle = config;
    cycle.bytes[head + 1] = head as u8;

    let Some(required) = first_required_capability(&config) else {
        return false;
    };
    let mut invalid_length = config;
    invalid_length.bytes[required + 2] = 0;

    let mut invalid_bar = config;
    invalid_bar.bytes[required + 4] = 6;

    parse_virtio_block(&wrong_identity, device.bus, device.device, device.function).is_none()
        && parse_virtio_block(&no_capabilities, device.bus, device.device, device.function)
            .is_none()
        && parse_virtio_block(&unaligned_head, device.bus, device.device, device.function).is_none()
        && parse_virtio_block(&cycle, device.bus, device.device, device.function).is_none()
        && parse_virtio_block(&invalid_length, device.bus, device.device, device.function).is_none()
        && parse_virtio_block(&invalid_bar, device.bus, device.device, device.function).is_none()
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

    let result = (|| {
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
    })();

    if reset(common).is_none()
        || unsafe { read_config_u16(device.bus, device.device, device.function, PCI_COMMAND) }
            & COMMAND_BUS_MASTER
            != 0
    {
        return None;
    }
    result
}

pub(crate) fn verify_feature_rejections(offered: u64, accepted: u64) -> bool {
    accepted == (FEATURE_VERSION_1 | FEATURE_FLUSH)
        && select_features(offered) == Some(accepted)
        && select_features(offered & !FEATURE_VERSION_1).is_none()
        && select_features(offered & !FEATURE_FLUSH).is_none()
        && select_features(0).is_none()
}

fn select_features(offered: u64) -> Option<u64> {
    let required = FEATURE_VERSION_1 | FEATURE_FLUSH;
    (offered & required == required).then_some(required)
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

fn parse_virtio_block(
    config: &ConfigSpace,
    bus: u8,
    device: u8,
    function: u8,
) -> Option<VirtioBlockDevice> {
    if read_u16(&config.bytes, 0) != VIRTIO_VENDOR
        || read_u16(&config.bytes, 2) != VIRTIO_BLOCK_DEVICE
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
        }
        if next != 0 && (!(0x40..=0xfc).contains(&next) || next % 4 != 0) {
            return None;
        }
        offset = next;
    }

    let common = common?;
    notify?;
    isr?;
    device_region?;
    Some(VirtioBlockDevice {
        bus,
        device,
        function,
        common_bar_address: common.bar_address,
        common_offset: common.offset,
        common_length: common.length,
        notify_multiplier: notify_multiplier?,
        capability_count: u8::try_from(capability_count).ok()?,
    })
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

fn memory_bar(config: &ConfigSpace, bar: u8) -> Option<u64> {
    if bar >= 6 {
        return None;
    }
    let offset = 0x10 + usize::from(bar) * 4;
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
