#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    ptr::{read_volatile, write_volatile},
    sync::atomic::{compiler_fence, Ordering},
};

use ginux_volume_format::SECTOR_SIZE;
use ginux_volume_service::{commit, recover, BlockDevice};

const BLOCK_WAIT_OPERATION: u64 = 19;
const BLOCK_WAIT_COMMAND: u64 = 1;
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

#[repr(C)]
pub struct BootInfo {
    common: usize,
    notification: usize,
    dma_virtual: usize,
    dma_physical: u64,
    capability: u64,
}

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
extern "C" fn ginux_volume_service_main(info: *const BootInfo) -> ! {
    let Some(info) = (unsafe { info.as_ref() }) else {
        fail();
    };
    let Some(mut device) = (unsafe { VirtioBlock::start(info) }) else {
        fail();
    };
    match recover(&mut device) {
        Some(record) if record.generation == 1 && record.payload() == PAYLOAD => success(),
        None => match commit(&mut device, PAYLOAD) {
            Ok(record) if record.generation == 1 && record.payload() == PAYLOAD => success(),
            _ => fail(),
        },
        _ => fail(),
    }
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
    unsafe fn start(info: &BootInfo) -> Option<Self> {
        if info.common == 0
            || info.notification == 0
            || info.dma_virtual == 0
            || !info.dma_virtual.is_multiple_of(4096)
            || !info.dma_physical.is_multiple_of(4096)
            || info.capability == 0
        {
            return None;
        }
        write_u8(info.common + COMMON_DEVICE_STATUS, 0);
        write_u8(info.common + COMMON_DEVICE_STATUS, STATUS_ACKNOWLEDGE);
        write_u8(
            info.common + COMMON_DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER,
        );
        write_u32(info.common + COMMON_DEVICE_FEATURE_SELECT, 0);
        if read_u32(info.common + COMMON_DEVICE_FEATURE) & FEATURE_FLUSH == 0 {
            return None;
        }
        write_u32(info.common + COMMON_DEVICE_FEATURE_SELECT, 1);
        if read_u32(info.common + COMMON_DEVICE_FEATURE) & FEATURE_VERSION_1 == 0 {
            return None;
        }
        write_u32(info.common + COMMON_DRIVER_FEATURE_SELECT, 0);
        write_u32(info.common + COMMON_DRIVER_FEATURE, FEATURE_FLUSH);
        write_u32(info.common + COMMON_DRIVER_FEATURE_SELECT, 1);
        write_u32(info.common + COMMON_DRIVER_FEATURE, FEATURE_VERSION_1);
        write_u8(
            info.common + COMMON_DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK,
        );
        if read_u8(info.common + COMMON_DEVICE_STATUS) & STATUS_FEATURES_OK == 0 {
            return None;
        }
        write_u16(info.common + COMMON_QUEUE_SELECT, 0);
        if read_u16(info.common + COMMON_QUEUE_SIZE) < QUEUE_SIZE {
            return None;
        }
        write_u16(info.common + COMMON_QUEUE_SIZE, QUEUE_SIZE);
        write_u16(info.common + COMMON_QUEUE_MSIX_VECTOR, 0);
        write_u64(
            info.common + COMMON_QUEUE_DESC,
            info.dma_physical + DESC_OFFSET as u64,
        );
        write_u64(
            info.common + COMMON_QUEUE_DRIVER,
            info.dma_physical + AVAIL_OFFSET as u64,
        );
        write_u64(
            info.common + COMMON_QUEUE_DEVICE,
            info.dma_physical + USED_OFFSET as u64,
        );
        write_u16(info.common + COMMON_QUEUE_ENABLE, 1);
        write_u8(
            info.common + COMMON_DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
        );
        if read_u16(info.common + COMMON_QUEUE_ENABLE) != 1
            || read_u8(info.common + COMMON_DEVICE_STATUS) & STATUS_DRIVER_OK == 0
        {
            return None;
        }
        Some(Self {
            common: info.common,
            notification: info.notification,
            dma: info.dma_virtual,
            dma_physical: info.dma_physical,
            capability: info.capability,
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
            write_u16(self.notification, 0);
        }
        if wait(self.capability, self.request) != 0 {
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

fn wait(capability: u64, request: u64) -> u64 {
    let mut result = BLOCK_WAIT_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") BLOCK_WAIT_COMMAND,
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
    unsafe { asm!("ud2", options(noreturn)) }
}

fn fail() -> ! {
    unsafe { asm!("int3", options(noreturn)) }
}
