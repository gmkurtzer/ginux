use crate::sync::{IrqSpinLock, LockRank};
use ginux_native_abi::{
    NativeDisplayRequestV0, NativeInputEventV0, NATIVE_DISPLAY_QUERY, NATIVE_DISPLAY_RESET,
    NATIVE_DISPLAY_WRITE_PIXEL, NATIVE_INPUT_QUEUE_CAPACITY,
};

const MAX_WIDTH: u32 = 4096;
const MAX_HEIGHT: u32 = 2160;
const BYTES_PER_PIXEL: usize = 4;

#[derive(Clone, Copy)]
pub(crate) struct FramebufferInfo {
    pub(crate) base: u64,
    pub(crate) size: usize,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) stride: u32,
    pub(crate) format: u32,
}

struct ControllerDevices {
    framebuffer: Option<FramebufferInfo>,
    input: [NativeInputEventV0; NATIVE_INPUT_QUEUE_CAPACITY],
    head: usize,
    count: usize,
    next_sequence: u64,
    overflow_count: u64,
}

impl ControllerDevices {
    const fn new() -> Self {
        Self {
            framebuffer: None,
            input: [NativeInputEventV0::EMPTY; NATIVE_INPUT_QUEUE_CAPACITY],
            head: 0,
            count: 0,
            next_sequence: 1,
            overflow_count: 0,
        }
    }

    fn install(&mut self, framebuffer: FramebufferInfo) -> bool {
        if self.framebuffer.is_some() || !valid_framebuffer(framebuffer) {
            return false;
        }
        self.framebuffer = Some(framebuffer);
        self.reset_input();
        true
    }

    fn display(&mut self, request: &mut NativeDisplayRequestV0) -> bool {
        let Some(framebuffer) = self.framebuffer else {
            return false;
        };
        if request.size != NativeDisplayRequestV0::SIZE || request.version != 0 {
            return false;
        }
        match request.command {
            NATIVE_DISPLAY_QUERY => {
                request.width = framebuffer.width;
                request.height = framebuffer.height;
                request.stride = framebuffer.stride;
                request.format = framebuffer.format;
                true
            }
            NATIVE_DISPLAY_WRITE_PIXEL => write_pixel(framebuffer, request),
            NATIVE_DISPLAY_RESET => {
                request.x = 0;
                request.y = 0;
                request.color = 0;
                true
            }
            _ => false,
        }
    }

    fn inject(&mut self, kind: u16, code: u32, value: i32) -> bool {
        if self.count == self.input.len() || kind == 0 || self.next_sequence == u64::MAX {
            self.overflow_count = self.overflow_count.saturating_add(1);
            return false;
        }
        let tail = (self.head + self.count) % self.input.len();
        self.input[tail] = NativeInputEventV0 {
            size: NativeInputEventV0::SIZE,
            version: 0,
            kind,
            code,
            value,
            sequence: self.next_sequence,
        };
        self.next_sequence += 1;
        self.count += 1;
        true
    }

    fn pop(&mut self) -> Option<NativeInputEventV0> {
        if self.count == 0 {
            return None;
        }
        let event = self.input[self.head];
        self.input[self.head] = NativeInputEventV0::EMPTY;
        self.head = (self.head + 1) % self.input.len();
        self.count -= 1;
        Some(event)
    }

    fn reset_input(&mut self) {
        self.input.fill(NativeInputEventV0::EMPTY);
        self.head = 0;
        self.count = 0;
        self.next_sequence = 1;
        self.overflow_count = 0;
    }
}

static DEVICES: IrqSpinLock<ControllerDevices> =
    IrqSpinLock::new(LockRank::DEVICE_NETWORK, ControllerDevices::new());

fn valid_framebuffer(info: FramebufferInfo) -> bool {
    let pixels = usize::try_from(info.stride)
        .ok()
        .and_then(|stride| stride.checked_mul(info.height as usize))
        .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL));
    info.base != 0
        && info.width != 0
        && info.width <= MAX_WIDTH
        && info.height != 0
        && info.height <= MAX_HEIGHT
        && info.stride >= info.width
        && info.format <= 1
        && pixels.is_some_and(|needed| needed <= info.size)
        && info
            .base
            .checked_add(info.size as u64)
            .is_some_and(|end| end <= crate::arch::x86_64::paging::INITIAL_IDENTITY_LIMIT as u64)
}

fn write_pixel(info: FramebufferInfo, request: &NativeDisplayRequestV0) -> bool {
    if request.x >= info.width || request.y >= info.height {
        return false;
    }
    let Some(offset) = (request.y as usize)
        .checked_mul(info.stride as usize)
        .and_then(|row| row.checked_add(request.x as usize))
        .and_then(|pixel| pixel.checked_mul(BYTES_PER_PIXEL))
    else {
        return false;
    };
    if offset
        .checked_add(BYTES_PER_PIXEL)
        .is_none_or(|end| end > info.size)
    {
        return false;
    }
    let Some(address) = info.base.checked_add(offset as u64) else {
        return false;
    };
    crate::arch::x86_64::pci::write_mmio_u32(address as usize, request.color);
    true
}

pub(crate) fn install(info: FramebufferInfo) -> bool {
    DEVICES.lock().install(info)
}

pub(crate) fn display(request: &mut NativeDisplayRequestV0) -> bool {
    DEVICES.lock().display(request)
}

pub(crate) fn input() -> Option<NativeInputEventV0> {
    DEVICES.lock().pop()
}

pub(crate) fn reset_input() {
    DEVICES.lock().reset_input();
}

pub(crate) fn inject_input(kind: u16, code: u32, value: i32) -> bool {
    DEVICES.lock().inject(kind, code, value)
}

pub(crate) fn verify_contract() -> bool {
    let info = DEVICES.lock().framebuffer;
    let Some(info) = info else {
        return false;
    };
    let mut query = NativeDisplayRequestV0 {
        size: NativeDisplayRequestV0::SIZE,
        version: 0,
        command: NATIVE_DISPLAY_QUERY,
        x: 0,
        y: 0,
        color: 0,
        width: 0,
        height: 0,
        stride: 0,
        format: 0,
        reserved: 0,
    };
    if !display(&mut query)
        || query.width != info.width
        || query.height != info.height
        || query.stride != info.stride
        || query.format != info.format
    {
        return false;
    }
    let mut pixel = query;
    pixel.command = NATIVE_DISPLAY_WRITE_PIXEL;
    pixel.x = info.width - 1;
    pixel.y = info.height - 1;
    pixel.color = 0x005a_a55a;
    let mut invalid = pixel;
    invalid.x = info.width;
    if !display(&mut pixel) || display(&mut invalid) {
        return false;
    }
    reset_input();
    if !inject_input(ginux_native_abi::NATIVE_INPUT_KIND_KEY, 30, 1)
        || !inject_input(ginux_native_abi::NATIVE_INPUT_KIND_POINTER_MOTION, 0, 7)
        || !inject_input(ginux_native_abi::NATIVE_INPUT_KIND_POINTER_BUTTON, 1, 1)
    {
        return false;
    }
    let ordered = [input(), input(), input()];
    if ordered
        .iter()
        .enumerate()
        .any(|(index, event)| event.is_none_or(|event| event.sequence != index as u64 + 1))
        || input().is_some()
    {
        return false;
    }
    reset_input();
    for index in 0..NATIVE_INPUT_QUEUE_CAPACITY {
        if !inject_input(ginux_native_abi::NATIVE_INPUT_KIND_KEY, index as u32, 1) {
            return false;
        }
    }
    let overflow_rejected = !inject_input(ginux_native_abi::NATIVE_INPUT_KIND_KEY, u32::MAX, 1);
    reset_input();
    overflow_rejected && input().is_none()
}
