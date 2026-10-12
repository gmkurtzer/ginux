#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    ptr::{read_volatile, write_volatile},
    sync::atomic::{compiler_fence, Ordering},
};

use ginux_native_abi::{
    native_control_abi_is_compatible, NativeServiceBootV0, NATIVE_BLOCK_WAIT_CANCELLED,
    NATIVE_BLOCK_WAIT_FAILED, NATIVE_BLOCK_WAIT_NOTIFIED, NATIVE_BLOCK_WAIT_OPERATION,
    NATIVE_BLOCK_WAIT_PREPARED, NATIVE_CONTROL_DISCOVERY_OPERATION, NATIVE_SERVICE_EXIT_COMPLETED,
    NATIVE_SERVICE_EXIT_OPERATION, NATIVE_SERVICE_EXIT_REJECTED,
    NATIVE_SERVICE_EXIT_RESTART_REQUIRED,
};
use ginux_volume_format::SECTOR_SIZE;
use ginux_volume_service::{commit, recover_with_slot, BlockDevice, CommitError, ScanError};

const QUEUE_SIZE: u16 = 8;
const DESC_NEXT: u16 = 1;
const DESC_WRITE: u16 = 2;
const DESC_OFFSET: usize = 0x000;
const AVAIL_OFFSET: usize = 0x100;
const USED_OFFSET: usize = 0x200;
const REQUEST_OFFSET: usize = 0x400;
const DATA_OFFSET: usize = 0x600;
const STATUS_OFFSET: usize = 0x900;
const COMMON_DEVICE_FEATURE_SELECT: usize = 0x00;
const COMMON_DEVICE_FEATURE: usize = 0x04;
const COMMON_DRIVER_FEATURE_SELECT: usize = 0x08;
const COMMON_DRIVER_FEATURE: usize = 0x0c;
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
const FEATURE_FLUSH: u32 = 1 << 9;
const FEATURE_VERSION_1: u32 = 1;
const VOLUME_SECTOR_BASE: usize = 4;
const PAYLOAD: &[u8] = b"ginux-isolated-volume\n";

global_asm!(
    r#"
    .text
    .global _start
_start:
    mov rdi, rbx
    call ginux_volume_service_main
    int3
"#
);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    fail()
}

#[no_mangle]
extern "C" fn ginux_volume_service_main(info: *const NativeServiceBootV0) -> ! {
    if !native_control_abi_is_compatible(discover_control_abi()) {
        fail();
    }
    let Some(info) = (unsafe { info.as_ref() }) else {
        fail();
    };
    let Some(mut device) = (unsafe { VirtioBlock::start(info) }) else {
        fail();
    };
    match recover_with_slot(&mut device) {
        Ok(Some((_, record))) if record.generation == 1 && record.payload() == PAYLOAD => success(),
        Ok(Some(_)) | Err(ScanError::Ambiguous | ScanError::Corrupt) => rejected(),
        Ok(None) => match commit(&mut device, PAYLOAD) {
            Ok(record) if record.generation == 1 && record.payload() == PAYLOAD => success(),
            Err(CommitError::Corrupt | CommitError::GenerationExhausted) => rejected(),
            _ => fail(),
        },
        Err(ScanError::Device) => fail(),
    }
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

struct VirtioBlock {
    common: usize,
    notification: usize,
    dma: usize,
    dma_physical: u64,
    capability: u64,
    available: u16,
    used: u16,
    request: u64,
}

enum Operation<'a> {
    Read {
        sector: u64,
        output: &'a mut [u8; SECTOR_SIZE],
    },
    Write {
        sector: u64,
        input: &'a [u8; SECTOR_SIZE],
    },
    Flush,
}

impl VirtioBlock {
    unsafe fn start(info: &NativeServiceBootV0) -> Option<Self> {
        let common = usize::try_from(info.common_config_address).ok()?;
        let notification = usize::try_from(info.queue_notification_address).ok()?;
        let dma = usize::try_from(info.dma_virtual_address).ok()?;
        if !info.is_valid_virtio_block()
            || !dma.is_multiple_of(4096)
            || !info.dma_device_address.is_multiple_of(4096)
        {
            return None;
        }
        write_u8(common + COMMON_DEVICE_STATUS, 0);
        write_u8(common + COMMON_DEVICE_STATUS, STATUS_ACKNOWLEDGE);
        write_u8(
            common + COMMON_DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER,
        );
        write_u32(common + COMMON_DEVICE_FEATURE_SELECT, 0);
        if read_u32(common + COMMON_DEVICE_FEATURE) & FEATURE_FLUSH == 0 {
            return None;
        }
        write_u32(common + COMMON_DEVICE_FEATURE_SELECT, 1);
        if read_u32(common + COMMON_DEVICE_FEATURE) & FEATURE_VERSION_1 == 0 {
            return None;
        }
        write_u32(common + COMMON_DRIVER_FEATURE_SELECT, 0);
        write_u32(common + COMMON_DRIVER_FEATURE, FEATURE_FLUSH);
        write_u32(common + COMMON_DRIVER_FEATURE_SELECT, 1);
        write_u32(common + COMMON_DRIVER_FEATURE, FEATURE_VERSION_1);
        write_u8(
            common + COMMON_DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK,
        );
        if read_u8(common + COMMON_DEVICE_STATUS) & STATUS_FEATURES_OK == 0 {
            return None;
        }
        write_u16(common + COMMON_QUEUE_SELECT, 0);
        if read_u16(common + COMMON_QUEUE_SIZE) < QUEUE_SIZE {
            return None;
        }
        write_u16(common + COMMON_QUEUE_SIZE, QUEUE_SIZE);
        write_u16(common + COMMON_QUEUE_MSIX_VECTOR, 0);
        write_u64(
            common + COMMON_QUEUE_DESC,
            info.dma_device_address + DESC_OFFSET as u64,
        );
        write_u64(
            common + COMMON_QUEUE_DRIVER,
            info.dma_device_address + AVAIL_OFFSET as u64,
        );
        write_u64(
            common + COMMON_QUEUE_DEVICE,
            info.dma_device_address + USED_OFFSET as u64,
        );
        write_u16(common + COMMON_QUEUE_ENABLE, 1);
        write_u8(
            common + COMMON_DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
        );
        if read_u16(common + COMMON_QUEUE_ENABLE) != 1
            || read_u8(common + COMMON_DEVICE_STATUS) & STATUS_DRIVER_OK == 0
        {
            return None;
        }
        Some(Self {
            common,
            notification,
            dma,
            dma_physical: info.dma_device_address,
            capability: info.device_capability,
            available: 0,
            used: 0,
            request: 1,
        })
    }

    fn submit(&mut self, operation: Operation<'_>) -> bool {
        let (request_type, sector, data_length, device_writes, expected_length) = match &operation {
            Operation::Read { sector, .. } => (0_u32, *sector, SECTOR_SIZE, true, 513_u32),
            Operation::Write { sector, .. } => (1_u32, *sector, SECTOR_SIZE, false, 1_u32),
            Operation::Flush => (4_u32, 0, 0, false, 1_u32),
        };
        unsafe {
            for offset in 0..48 {
                write_u8(self.dma + DESC_OFFSET + offset, 0);
            }
            for offset in 0..16 {
                write_u8(self.dma + REQUEST_OFFSET + offset, 0);
            }
            write_u32(self.dma + REQUEST_OFFSET, request_type);
            write_u64(self.dma + REQUEST_OFFSET + 8, sector);
            write_u64(
                self.dma + DESC_OFFSET,
                self.dma_physical + REQUEST_OFFSET as u64,
            );
            write_u32(self.dma + DESC_OFFSET + 8, 16);
            write_u16(self.dma + DESC_OFFSET + 12, DESC_NEXT);
            write_u16(
                self.dma + DESC_OFFSET + 14,
                if data_length == 0 { 2 } else { 1 },
            );
            if data_length != 0 {
                write_u64(
                    self.dma + DESC_OFFSET + 16,
                    self.dma_physical + DATA_OFFSET as u64,
                );
                write_u32(self.dma + DESC_OFFSET + 24, SECTOR_SIZE as u32);
                write_u16(
                    self.dma + DESC_OFFSET + 28,
                    DESC_NEXT | if device_writes { DESC_WRITE } else { 0 },
                );
                write_u16(self.dma + DESC_OFFSET + 30, 2);
            }
            write_u64(
                self.dma + DESC_OFFSET + 32,
                self.dma_physical + STATUS_OFFSET as u64,
            );
            write_u32(self.dma + DESC_OFFSET + 40, 1);
            write_u16(self.dma + DESC_OFFSET + 44, DESC_WRITE);
            write_u8(self.dma + STATUS_OFFSET, 0xff);
            match &operation {
                Operation::Write { input, .. } => {
                    for (offset, byte) in input.iter().enumerate() {
                        write_u8(self.dma + DATA_OFFSET + offset, *byte);
                    }
                }
                Operation::Read { .. } => {
                    for offset in 0..SECTOR_SIZE {
                        write_u8(self.dma + DATA_OFFSET + offset, 0xa5);
                    }
                }
                Operation::Flush => {}
            }
            let available_slot = usize::from(self.available % QUEUE_SIZE);
            write_u16(self.dma + AVAIL_OFFSET + 4 + available_slot * 2, 0);
            compiler_fence(Ordering::SeqCst);
            self.available = self.available.wrapping_add(1);
            write_u16(self.dma + AVAIL_OFFSET + 2, self.available);
        }
        if self.request == 1
            && (wait(self.capability, NATIVE_BLOCK_WAIT_PREPARED, 2) != NATIVE_BLOCK_WAIT_FAILED
                || wait(self.capability, NATIVE_BLOCK_WAIT_PREPARED, self.request)
                    != NATIVE_BLOCK_WAIT_FAILED)
        {
            return false;
        }
        unsafe { write_u16(self.notification, 0) };
        let wait_result = wait(self.capability, NATIVE_BLOCK_WAIT_NOTIFIED, self.request);
        if wait_result == NATIVE_BLOCK_WAIT_CANCELLED {
            exit(NATIVE_SERVICE_EXIT_RESTART_REQUIRED);
        }
        if wait_result != 0 {
            return false;
        }
        if self.request == 1
            && wait(self.capability, NATIVE_BLOCK_WAIT_NOTIFIED, self.request)
                != NATIVE_BLOCK_WAIT_FAILED
        {
            return false;
        }
        self.request = self.request.checked_add(1).unwrap_or(0);
        let expected_used = self.used.wrapping_add(1);
        let used_slot = usize::from(self.used % QUEUE_SIZE);
        self.used = expected_used;
        compiler_fence(Ordering::SeqCst);
        if self.request == 0
            || unsafe { read_u16(self.dma + USED_OFFSET + 2) } != expected_used
            || unsafe { read_u32(self.dma + USED_OFFSET + 4 + used_slot * 8) } != 0
            || unsafe { read_u32(self.dma + USED_OFFSET + 8 + used_slot * 8) } != expected_length
            || unsafe { read_u8(self.dma + STATUS_OFFSET) } != 0
            || unsafe { read_u8(self.common + COMMON_DEVICE_STATUS) } & 0xc0 != 0
        {
            return false;
        }
        if let Operation::Read { output, .. } = operation {
            for (offset, byte) in output.iter_mut().enumerate() {
                *byte = unsafe { read_u8(self.dma + DATA_OFFSET + offset) };
            }
        }
        true
    }
}

impl BlockDevice for VirtioBlock {
    fn read_sector(&mut self, sector: usize, output: &mut [u8; SECTOR_SIZE]) -> bool {
        let Some(sector) = sector.checked_add(VOLUME_SECTOR_BASE) else {
            return false;
        };
        self.submit(Operation::Read {
            sector: sector as u64,
            output,
        })
    }

    fn write_sector(&mut self, sector: usize, input: &[u8; SECTOR_SIZE]) -> bool {
        let Some(sector) = sector.checked_add(VOLUME_SECTOR_BASE) else {
            return false;
        };
        self.submit(Operation::Write {
            sector: sector as u64,
            input,
        })
    }

    fn flush(&mut self) -> bool {
        self.submit(Operation::Flush)
    }
}

fn wait(capability: u64, command: u64, request: u64) -> u64 {
    let mut result = NATIVE_BLOCK_WAIT_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") command,
            in("rdx") request,
            options(nostack),
        );
    }
    result
}

unsafe fn read_u8(address: usize) -> u8 {
    read_volatile(address as *const u8)
}

unsafe fn read_u16(address: usize) -> u16 {
    read_volatile(address as *const u16)
}

unsafe fn read_u32(address: usize) -> u32 {
    read_volatile(address as *const u32)
}

unsafe fn write_u8(address: usize, value: u8) {
    write_volatile(address as *mut u8, value);
}

unsafe fn write_u16(address: usize, value: u16) {
    write_volatile(address as *mut u16, value);
}

unsafe fn write_u32(address: usize, value: u32) {
    write_volatile(address as *mut u32, value);
}

unsafe fn write_u64(address: usize, value: u64) {
    write_volatile(address as *mut u64, value);
}

fn success() -> ! {
    exit(NATIVE_SERVICE_EXIT_COMPLETED)
}

fn rejected() -> ! {
    exit(NATIVE_SERVICE_EXIT_REJECTED)
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
