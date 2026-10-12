#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    ptr::{read_volatile, write_volatile},
    sync::atomic::{compiler_fence, Ordering},
};

use ginux_native_abi::{
    native_control_abi_is_compatible, NativeControlDescriptorV4, NativeNetworkBootV0,
    NATIVE_CONTROL_DESCRIPTOR_OPERATION, NATIVE_CONTROL_DESCRIPTOR_SIZE,
    NATIVE_CONTROL_DISCOVERY_OPERATION, NATIVE_NETWORK_DEVICE_ACTIVATE_EMPTY_QUEUES,
    NATIVE_NETWORK_DEVICE_DESCRIPTOR_WORD, NATIVE_NETWORK_DEVICE_OPERATION,
    NATIVE_NETWORK_DEVICE_PROBE, NATIVE_SERVICE_EXIT_COMPLETED, NATIVE_SERVICE_EXIT_OPERATION,
};

static SERVICE_IDENTITY: &[u8] = b"GINUX_NETWORK_SERVICE_V0";

const DMA_PAGE_SIZE: usize = 4096;
const QUEUE_SIZE: u16 = 8;
const RX_DESC_OFFSET: u64 = 0x000;
const RX_AVAIL_OFFSET: u64 = 0x080;
const RX_USED_OFFSET: u64 = 0x100;
const TX_DESC_OFFSET: u64 = 0x180;
const TX_AVAIL_OFFSET: u64 = 0x200;
const TX_USED_OFFSET: u64 = 0x280;
const TX_BUFFER_OFFSET: u64 = 0xa00;
const TX_HEADER_LENGTH: usize = 12;
const TX_FRAME: [u8; 19] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x88, 0xb5, b'G', b'I',
    b'N', b'U', b'X',
];
const COMMON_DEVICE_FEATURE_SELECT: usize = 0x00;
const COMMON_DEVICE_FEATURE: usize = 0x04;
const COMMON_DRIVER_FEATURE_SELECT: usize = 0x08;
const COMMON_DRIVER_FEATURE: usize = 0x0c;
const COMMON_NUM_QUEUES: usize = 0x12;
const COMMON_DEVICE_STATUS: usize = 0x14;
const COMMON_QUEUE_SELECT: usize = 0x16;
const COMMON_QUEUE_SIZE: usize = 0x18;
const COMMON_QUEUE_MSIX_VECTOR: usize = 0x1a;
const COMMON_QUEUE_ENABLE: usize = 0x1c;
const COMMON_QUEUE_DESC: usize = 0x20;
const COMMON_QUEUE_DRIVER: usize = 0x28;
const COMMON_QUEUE_DEVICE: usize = 0x30;
const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FEATURES_OK: u8 = 8;
const FEATURE_NET_MAC: u32 = 1 << 5;
const FEATURE_NET_MRG_RXBUF: u32 = 1 << 15;
const FEATURE_VERSION_1: u32 = 1;
const FEATURE_ACCESS_PLATFORM: u32 = 1 << 1;
const RESET_POLL_LIMIT: usize = 100_000;
const COMPLETION_POLL_LIMIT: usize = 10_000_000;

global_asm!(
    r#"
    .text
    .global _start
_start:
    mov rdi, rbx
    call ginux_network_service_main
    int3
"#
);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    fail()
}

#[no_mangle]
extern "C" fn ginux_network_service_main(info: *const NativeNetworkBootV0) -> ! {
    let identity = unsafe { core::ptr::read_volatile(&SERVICE_IDENTITY) };
    if identity != b"GINUX_NETWORK_SERVICE_V0"
        || !native_control_abi_is_compatible(discover_control_abi())
        || !discover_control_descriptor().is_compatible()
    {
        fail();
    }
    if !info.is_null() {
        let Some(info) = (unsafe { info.as_ref() }) else {
            fail();
        };
        if !info.is_valid_virtio_network()
            || info.mac_address != 0x5254_0012_3456
            || probe_network_device(info.device_capability) != NATIVE_NETWORK_DEVICE_DESCRIPTOR_WORD
        {
            fail();
        }
        if info.requests_empty_queues()
            && (!unsafe { configure_empty_queues(info) }
                || activate_empty_queues(info.device_capability) != 0)
        {
            fail();
        }
        if info.requests_transmit_probe() && !unsafe { transmit_probe(info) } {
            fail();
        }
    }
    exit(NATIVE_SERVICE_EXIT_COMPLETED)
}

unsafe fn transmit_probe(info: &NativeNetworkBootV0) -> bool {
    let Ok(dma) = usize::try_from(info.dma_virtual_address) else {
        return false;
    };
    let Ok(notification) = usize::try_from(info.transmit_notification_address) else {
        return false;
    };
    let Ok(common) = usize::try_from(info.common_config_address) else {
        return false;
    };
    let payload_length = TX_HEADER_LENGTH + TX_FRAME.len();
    write_u64(
        dma + TX_DESC_OFFSET as usize,
        info.dma_device_address + TX_BUFFER_OFFSET,
    );
    write_u32(dma + TX_DESC_OFFSET as usize + 8, payload_length as u32);
    write_u16(dma + TX_DESC_OFFSET as usize + 12, 0);
    write_u16(dma + TX_DESC_OFFSET as usize + 14, 0);
    for offset in 0..TX_HEADER_LENGTH {
        write_u8(dma + TX_BUFFER_OFFSET as usize + offset, 0);
    }
    for (offset, byte) in TX_FRAME.into_iter().enumerate() {
        write_u8(
            dma + TX_BUFFER_OFFSET as usize + TX_HEADER_LENGTH + offset,
            byte,
        );
    }
    write_u8(dma + TX_BUFFER_OFFSET as usize - 1, 0x3c);
    write_u8(dma + TX_BUFFER_OFFSET as usize + payload_length, 0xc3);
    write_u16(dma + TX_AVAIL_OFFSET as usize, 0);
    write_u16(dma + TX_AVAIL_OFFSET as usize + 4, 0);
    compiler_fence(Ordering::SeqCst);
    write_u16(dma + TX_AVAIL_OFFSET as usize + 2, 1);
    compiler_fence(Ordering::SeqCst);
    write_u16(notification, 1);
    for _ in 0..COMPLETION_POLL_LIMIT {
        if read_u16(dma + TX_USED_OFFSET as usize + 2) == 1 {
            compiler_fence(Ordering::SeqCst);
            return read_u32(dma + TX_USED_OFFSET as usize + 4) == 0
                && read_u8(common + COMMON_DEVICE_STATUS)
                    == STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK;
        }
        core::hint::spin_loop();
    }
    false
}

unsafe fn configure_empty_queues(info: &NativeNetworkBootV0) -> bool {
    let Ok(common) = usize::try_from(info.common_config_address) else {
        return false;
    };
    let Ok(dma) = usize::try_from(info.dma_virtual_address) else {
        return false;
    };
    if !dma.is_multiple_of(DMA_PAGE_SIZE)
        || !info.dma_device_address.is_multiple_of(DMA_PAGE_SIZE as u64)
    {
        return false;
    }
    for offset in 0..DMA_PAGE_SIZE {
        write_u8(dma + offset, 0);
    }
    write_u8(common + COMMON_DEVICE_STATUS, 0);
    let mut reset = false;
    for _ in 0..RESET_POLL_LIMIT {
        if read_u8(common + COMMON_DEVICE_STATUS) == 0 {
            reset = true;
            break;
        }
        core::hint::spin_loop();
    }
    if !reset {
        return false;
    }
    write_u8(common + COMMON_DEVICE_STATUS, STATUS_ACKNOWLEDGE);
    write_u8(
        common + COMMON_DEVICE_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER,
    );
    write_u32(common + COMMON_DEVICE_FEATURE_SELECT, 0);
    if read_u32(common + COMMON_DEVICE_FEATURE) & (FEATURE_NET_MAC | FEATURE_NET_MRG_RXBUF)
        != FEATURE_NET_MAC | FEATURE_NET_MRG_RXBUF
    {
        return false;
    }
    write_u32(common + COMMON_DEVICE_FEATURE_SELECT, 1);
    if read_u32(common + COMMON_DEVICE_FEATURE) & (FEATURE_VERSION_1 | FEATURE_ACCESS_PLATFORM)
        != FEATURE_VERSION_1 | FEATURE_ACCESS_PLATFORM
    {
        return false;
    }
    write_u32(common + COMMON_DRIVER_FEATURE_SELECT, 0);
    write_u32(
        common + COMMON_DRIVER_FEATURE,
        FEATURE_NET_MAC | FEATURE_NET_MRG_RXBUF,
    );
    write_u32(common + COMMON_DRIVER_FEATURE_SELECT, 1);
    write_u32(
        common + COMMON_DRIVER_FEATURE,
        FEATURE_VERSION_1 | FEATURE_ACCESS_PLATFORM,
    );
    write_u8(
        common + COMMON_DEVICE_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK,
    );
    if read_u8(common + COMMON_DEVICE_STATUS)
        != STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK
        || read_u16(common + COMMON_NUM_QUEUES) < 2
    {
        return false;
    }
    for (queue, descriptor, available, used) in [
        (0_u16, RX_DESC_OFFSET, RX_AVAIL_OFFSET, RX_USED_OFFSET),
        (1_u16, TX_DESC_OFFSET, TX_AVAIL_OFFSET, TX_USED_OFFSET),
    ] {
        write_u16(common + COMMON_QUEUE_SELECT, queue);
        let maximum = read_u16(common + COMMON_QUEUE_SIZE);
        if maximum < QUEUE_SIZE
            || !maximum.is_power_of_two()
            || read_u16(common + COMMON_QUEUE_ENABLE) != 0
            || read_u16(common + COMMON_QUEUE_MSIX_VECTOR) != u16::MAX
        {
            return false;
        }
        write_u16(common + COMMON_QUEUE_SIZE, QUEUE_SIZE);
        write_u64(
            common + COMMON_QUEUE_DESC,
            info.dma_device_address + descriptor,
        );
        write_u64(
            common + COMMON_QUEUE_DRIVER,
            info.dma_device_address + available,
        );
        write_u64(common + COMMON_QUEUE_DEVICE, info.dma_device_address + used);
        write_u16(common + COMMON_QUEUE_ENABLE, 1);
        if read_u16(common + COMMON_QUEUE_SIZE) != QUEUE_SIZE
            || read_u16(common + COMMON_QUEUE_ENABLE) != 1
            || read_u64(common + COMMON_QUEUE_DESC) != info.dma_device_address + descriptor
            || read_u64(common + COMMON_QUEUE_DRIVER) != info.dma_device_address + available
            || read_u64(common + COMMON_QUEUE_DEVICE) != info.dma_device_address + used
        {
            return false;
        }
    }
    write_u8(
        common + COMMON_DEVICE_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
    );
    read_u8(common + COMMON_DEVICE_STATUS)
        == STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK
}

fn activate_empty_queues(capability: u64) -> u64 {
    let mut result = NATIVE_NETWORK_DEVICE_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") NATIVE_NETWORK_DEVICE_ACTIVATE_EMPTY_QUEUES,
            in("rdx") 0_u64,
            options(nostack),
        );
    }
    result
}

unsafe fn read_u8(address: usize) -> u8 {
    compiler_fence(Ordering::SeqCst);
    let value = read_volatile(address as *const u8);
    compiler_fence(Ordering::SeqCst);
    value
}

unsafe fn read_u16(address: usize) -> u16 {
    compiler_fence(Ordering::SeqCst);
    let value = read_volatile(address as *const u16);
    compiler_fence(Ordering::SeqCst);
    value
}

unsafe fn read_u32(address: usize) -> u32 {
    compiler_fence(Ordering::SeqCst);
    let value = read_volatile(address as *const u32);
    compiler_fence(Ordering::SeqCst);
    value
}

unsafe fn read_u64(address: usize) -> u64 {
    compiler_fence(Ordering::SeqCst);
    let value = read_volatile(address as *const u64);
    compiler_fence(Ordering::SeqCst);
    value
}

unsafe fn write_u8(address: usize, value: u8) {
    compiler_fence(Ordering::SeqCst);
    write_volatile(address as *mut u8, value);
    compiler_fence(Ordering::SeqCst);
}

unsafe fn write_u16(address: usize, value: u16) {
    compiler_fence(Ordering::SeqCst);
    write_volatile(address as *mut u16, value);
    compiler_fence(Ordering::SeqCst);
}

unsafe fn write_u32(address: usize, value: u32) {
    compiler_fence(Ordering::SeqCst);
    write_volatile(address as *mut u32, value);
    compiler_fence(Ordering::SeqCst);
}

unsafe fn write_u64(address: usize, value: u64) {
    compiler_fence(Ordering::SeqCst);
    write_volatile(address as *mut u64, value);
    compiler_fence(Ordering::SeqCst);
}

fn probe_network_device(capability: u64) -> u64 {
    let mut result = NATIVE_NETWORK_DEVICE_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") NATIVE_NETWORK_DEVICE_PROBE,
            in("rdx") 0_u64,
            options(nostack),
        );
    }
    result
}

fn discover_control_abi() -> u64 {
    let mut result = NATIVE_CONTROL_DISCOVERY_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") 0_u64,
            in("rsi") 0_u64,
            in("rdx") 0_u64,
            options(nostack),
        );
    }
    result
}

fn discover_control_descriptor() -> NativeControlDescriptorV4 {
    let mut descriptor = NativeControlDescriptorV4::zeroed();
    let mut result = NATIVE_CONTROL_DESCRIPTOR_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") 0_u64,
            in("rsi") &mut descriptor as *mut NativeControlDescriptorV4 as u64,
            in("rdx") u64::from(NATIVE_CONTROL_DESCRIPTOR_SIZE),
            options(nostack),
        );
    }
    if result == u64::from(NATIVE_CONTROL_DESCRIPTOR_SIZE) {
        descriptor
    } else {
        NativeControlDescriptorV4::zeroed()
    }
}

fn exit(status: u8) -> ! {
    unsafe {
        asm!(
            "int 0x80",
            in("rax") NATIVE_SERVICE_EXIT_OPERATION,
            in("rdi") 0_u64,
            in("rsi") u64::from(status),
            options(noreturn),
        )
    }
}

fn fail() -> ! {
    unsafe { asm!("hlt", options(noreturn)) }
}
