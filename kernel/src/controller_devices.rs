use core::arch::asm;

use crate::sync::{IrqSpinLock, LockRank};
use ginux_native_abi::{
    NativeDisplayRequestV0, NativeInputEventV0, NATIVE_DISPLAY_FILL_RECT, NATIVE_DISPLAY_QUERY,
    NATIVE_DISPLAY_RESET, NATIVE_DISPLAY_WRITE_PIXEL, NATIVE_INPUT_QUEUE_CAPACITY,
};

const MAX_WIDTH: u32 = 4096;
const MAX_HEIGHT: u32 = 2160;
const BYTES_PER_PIXEL: usize = 4;
const MAX_BLIT_WIDTH: usize = 256;
const MAX_BLIT_HEIGHT: u32 = 256;
const MAX_BLIT_SCALE: u32 = 16;

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
    shift: bool,
    mouse_packet: [u8; 3],
    mouse_length: u8,
    pointer_x: i16,
    pointer_y: i16,
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
            shift: false,
            mouse_packet: [0; 3],
            mouse_length: 0,
            pointer_x: 48,
            pointer_y: 32,
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
            NATIVE_DISPLAY_FILL_RECT => fill_rect(framebuffer, request),
            NATIVE_DISPLAY_RESET => {
                clear(framebuffer, 0);
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
        self.mouse_length = 0;
    }

    fn keyboard(&mut self, scancode: u8) -> Option<u8> {
        let released = scancode & 0x80 != 0;
        let code = scancode & 0x7f;
        if matches!(code, 0x2a | 0x36) {
            self.shift = !released;
            return None;
        }
        let _ = self.inject(
            ginux_native_abi::NATIVE_INPUT_KIND_KEY,
            u32::from(code),
            if released { 0 } else { 1 },
        );
        (!released).then(|| set1_ascii(code, self.shift)).flatten()
    }

    fn mouse(&mut self, byte: u8) -> bool {
        if self.mouse_length == 0 && byte & 0x08 == 0 {
            return false;
        }
        self.mouse_packet[usize::from(self.mouse_length)] = byte;
        self.mouse_length += 1;
        if self.mouse_length != 3 {
            return false;
        }
        self.mouse_length = 0;
        let dx = i16::from(self.mouse_packet[1] as i8);
        let dy = -i16::from(self.mouse_packet[2] as i8);
        self.pointer_x = (self.pointer_x + dx).clamp(0, 159);
        self.pointer_y = (self.pointer_y + dy).clamp(0, 99);
        let _ = self.inject(
            ginux_native_abi::NATIVE_INPUT_KIND_POINTER_MOTION,
            u32::from(self.pointer_x as u16) | (u32::from(self.pointer_y as u16) << 16),
            (i32::from(dx) & 0xffff) | (i32::from(dy) << 16),
        );
        let _ = self.inject(
            ginux_native_abi::NATIVE_INPUT_KIND_POINTER_BUTTON,
            u32::from(self.mouse_packet[0] & 0x07),
            1,
        );
        true
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
    let color = native_color(info.format, request.color);
    crate::arch::x86_64::pci::write_mmio_u32(address as usize, color);
    crate::arch::x86_64::pci::read_mmio_u32(address as usize) == color
}

fn fill_rect(info: FramebufferInfo, request: &NativeDisplayRequestV0) -> bool {
    let Some(right) = request.x.checked_add(request.width) else {
        return false;
    };
    let Some(bottom) = request.y.checked_add(request.height) else {
        return false;
    };
    if request.width == 0 || request.height == 0 || right > info.width || bottom > info.height {
        return false;
    }
    let color = native_color(info.format, request.color);
    for y in request.y..bottom {
        let row = y as usize * info.stride as usize;
        for x in request.x..right {
            let offset = (row + x as usize) * BYTES_PER_PIXEL;
            if offset + BYTES_PER_PIXEL > info.size {
                return false;
            }
            crate::arch::x86_64::pci::write_mmio_u32(info.base as usize + offset, color);
        }
    }
    true
}

fn clear(info: FramebufferInfo, color: u32) {
    let color = native_color(info.format, color);
    for pixel in 0..(info.stride as usize * info.height as usize) {
        crate::arch::x86_64::pci::write_mmio_u32(
            info.base as usize + pixel * BYTES_PER_PIXEL,
            color,
        );
    }
}

fn native_color(format: u32, color: u32) -> u32 {
    if format == 1 {
        color
    } else {
        let red = (color >> 16) & 0xff;
        let green = color & 0x00ff00;
        let blue = color & 0xff;
        (blue << 16) | green | red
    }
}

pub(crate) fn install(info: FramebufferInfo) -> bool {
    DEVICES.lock().install(info)
}

pub(crate) fn display(request: &mut NativeDisplayRequestV0) -> bool {
    DEVICES.lock().display(request)
}

pub(crate) fn blit<F>(request: &NativeDisplayRequestV0, source: u64, mut copy: F) -> bool
where
    F: FnMut(u64, &mut [u8]) -> bool,
{
    let Some(info) = DEVICES.lock().framebuffer else {
        return false;
    };
    let Ok(width) = usize::try_from(request.width) else {
        return false;
    };
    let scale = request.reserved;
    let Some(rendered_width) = request.width.checked_mul(scale) else {
        return false;
    };
    let Some(rendered_height) = request.height.checked_mul(scale) else {
        return false;
    };
    if request.size != NativeDisplayRequestV0::SIZE
        || request.version != 0
        || request.command != ginux_native_abi::NATIVE_DISPLAY_BLIT
        || width == 0
        || width > MAX_BLIT_WIDTH
        || request.height == 0
        || request.height > MAX_BLIT_HEIGHT
        || request.stride < request.width
        || request.format != 0
        || !(1..=MAX_BLIT_SCALE).contains(&scale)
        || request
            .x
            .checked_add(rendered_width)
            .is_none_or(|right| right > info.width)
        || request
            .y
            .checked_add(rendered_height)
            .is_none_or(|bottom| bottom > info.height)
    {
        return false;
    }
    let mut row = [0_u8; MAX_BLIT_WIDTH * BYTES_PER_PIXEL];
    let row_bytes = width * BYTES_PER_PIXEL;
    for source_y in 0..request.height {
        let Some(source_offset) = (source_y as u64)
            .checked_mul(u64::from(request.stride))
            .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL as u64))
        else {
            return false;
        };
        let Some(row_address) = source.checked_add(source_offset) else {
            return false;
        };
        if !copy(row_address, &mut row[..row_bytes]) {
            return false;
        }
        for source_x in 0..width {
            let offset = source_x * BYTES_PER_PIXEL;
            let color = u32::from_ne_bytes(row[offset..offset + 4].try_into().unwrap());
            let rect = NativeDisplayRequestV0 {
                size: NativeDisplayRequestV0::SIZE,
                version: 0,
                command: ginux_native_abi::NATIVE_DISPLAY_FILL_RECT,
                x: request.x + source_x as u32 * scale,
                y: request.y + source_y * scale,
                color,
                width: scale,
                height: scale,
                stride: 0,
                format: 0,
                reserved: 0,
            };
            if !fill_rect(info, &rect) {
                return false;
            }
        }
    }
    true
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

pub(crate) fn hardware_input_ready() -> bool {
    in_u8(0x64) & 0x01 != 0
}

pub(crate) fn read_hardware_input() -> Option<u8> {
    if !hardware_input_ready() {
        return None;
    }
    let status = in_u8(0x64);
    let byte = in_u8(0x60);
    let mut devices = DEVICES.lock();
    if status & 0x20 != 0 {
        devices.mouse(byte).then_some(0)
    } else {
        devices.keyboard(byte)
    }
}

pub(crate) fn initialize_hardware_input() -> bool {
    for _ in 0..32 {
        if !hardware_input_ready() {
            break;
        }
        let _ = in_u8(0x60);
    }
    if wait_input_clear() {
        out_u8(0x64, 0x20);
    }
    let configuration = wait_output_ready().map(|value| (value | 0x40) & !0x30);
    if let Some(configuration) = configuration {
        if wait_input_clear() {
            out_u8(0x64, 0x60);
        }
        if wait_input_clear() {
            out_u8(0x60, configuration);
        }
    }
    if wait_input_clear() {
        out_u8(0x64, 0xae);
    }
    let keyboard_reset = write_keyboard(0xff) && wait_output_ready() == Some(0xaa);
    let keyboard_enabled =
        keyboard_reset && write_keyboard(0xf0) && write_keyboard(0x02) && write_keyboard(0xf4);
    if wait_input_clear() {
        out_u8(0x64, 0xa8);
    }
    if wait_input_clear() {
        out_u8(0x64, 0xd4);
    }
    let mouse_command = if wait_input_clear() {
        out_u8(0x60, 0xf4);
        true
    } else {
        false
    };
    let mouse_enabled = mouse_command && wait_output_ready() == Some(0xfa);
    keyboard_enabled && mouse_enabled
}

fn write_keyboard(command: u8) -> bool {
    if !wait_input_clear() {
        return false;
    }
    out_u8(0x60, command);
    wait_output_ready() == Some(0xfa)
}

fn wait_output_ready() -> Option<u8> {
    for _ in 0..100_000 {
        if in_u8(0x64) & 0x01 != 0 {
            return Some(in_u8(0x60));
        }
        core::hint::spin_loop();
    }
    None
}

fn wait_input_clear() -> bool {
    for _ in 0..100_000 {
        if in_u8(0x64) & 0x02 == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn set1_ascii(code: u8, shift: bool) -> Option<u8> {
    let plain = match code {
        0x02..=0x0a => b'1' + code - 0x02,
        0x0b => b'0',
        0x0c => b'-',
        0x0e => 0x08,
        0x0f => b'\t',
        0x10 => b'q',
        0x11 => b'w',
        0x12 => b'e',
        0x13 => b'r',
        0x14 => b't',
        0x15 => b'y',
        0x16 => b'u',
        0x17 => b'i',
        0x18 => b'o',
        0x19 => b'p',
        0x1c => b'\n',
        0x1e => b'a',
        0x1f => b's',
        0x20 => b'd',
        0x21 => b'f',
        0x22 => b'g',
        0x23 => b'h',
        0x24 => b'j',
        0x25 => b'k',
        0x26 => b'l',
        0x2c => b'z',
        0x2d => b'x',
        0x2e => b'c',
        0x2f => b'v',
        0x30 => b'b',
        0x31 => b'n',
        0x32 => b'm',
        0x39 => b' ',
        _ => return None,
    };
    Some(if shift && plain.is_ascii_lowercase() {
        plain.to_ascii_uppercase()
    } else {
        plain
    })
}

fn in_u8(port: u16) -> u8 {
    let value: u8;
    // SAFETY: this private device boundary is called only for the legacy PS/2
    // controller's data and status ports after platform I/O is initialized.
    unsafe {
        asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn out_u8(port: u16, value: u8) {
    // SAFETY: callers use only the PS/2 controller's command and data ports
    // with protocol bytes bounded by the initialization sequence above.
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
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
