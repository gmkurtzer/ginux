#![no_std]

use ginux_domain_display::{Bridge, Endpoint};

pub const TRUSTED_LABEL: &[u8] = b"GINUX DOMAIN";
pub const MAX_RESIZE_TICKS: u64 = 8;

const BACKGROUND: u32 = 0x0018_2028;
const CHROME: u32 = 0x0028_3440;
const ACTIVE: u32 = 0x0028_78c8;
const DOMAIN: u32 = 0x0048_5868;
const FAILED: u32 = 0x0090_3030;
const STOPPED: u32 = 0x0040_4040;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Bounds,
    State,
    Stale,
    Busy,
    Timeout,
    Display,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Windowed,
    Fullscreen,
    Minimized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lifecycle {
    Running,
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResizeRequest {
    pub serial: u64,
    pub width: u16,
    pub height: u16,
    pub deadline: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Status {
    pub domain: u64,
    pub endpoint_generation: u32,
    pub mode: Mode,
    pub lifecycle: Lifecycle,
    pub rect: Rect,
    pub focused: bool,
    pub exclusive_lease: bool,
    pub trusted_chrome: bool,
    pub content: u64,
    pub frame_sequence: u64,
    pub pending_resize: Option<ResizeRequest>,
}

pub struct Presentation {
    bridge: Bridge,
    endpoint: Endpoint,
    domain: u64,
    screen: Rect,
    rect: Rect,
    restore: Rect,
    mode: Mode,
    lifecycle: Lifecycle,
    focused: bool,
    exclusive_lease: bool,
    content: u64,
    frame_sequence: u64,
    resize_serial: u64,
    pending_resize: Option<ResizeRequest>,
}

impl Presentation {
    pub fn start(domain: u64, screen: Rect, window: Rect) -> Result<Self, Error> {
        if !valid_rect(screen, screen) || !valid_rect(window, screen) || window.y < 4 {
            return Err(Error::Bounds);
        }
        let mut bridge = Bridge::new();
        let endpoint = bridge
            .start(domain, window.width, window.height.saturating_sub(4))
            .map_err(|_| Error::Display)?;
        Ok(Self {
            bridge,
            endpoint,
            domain,
            screen,
            rect: window,
            restore: window,
            mode: Mode::Windowed,
            lifecycle: Lifecycle::Running,
            focused: false,
            exclusive_lease: false,
            content: 0,
            frame_sequence: 0,
            resize_serial: 1,
            pending_resize: None,
        })
    }

    pub fn status(&self) -> Status {
        Status {
            domain: self.domain,
            endpoint_generation: self.endpoint_generation(),
            mode: self.mode,
            lifecycle: self.lifecycle,
            rect: self.rect,
            focused: self.focused,
            exclusive_lease: self.exclusive_lease,
            trusted_chrome: true,
            content: self.content,
            frame_sequence: self.frame_sequence,
            pending_resize: self.pending_resize,
        }
    }

    pub fn focus(&mut self) -> Result<(), Error> {
        self.require_running()?;
        self.bridge.focus(self.endpoint).map_err(|_| Error::Stale)?;
        self.focused = true;
        Ok(())
    }

    pub fn publish(&mut self, content: u64) -> Result<u64, Error> {
        self.require_running()?;
        let width = self.rect.width;
        let height = self.rect.height.saturating_sub(4);
        self.bridge
            .publish(self.endpoint, [0, 0, width, height], content)
            .map_err(|_| Error::Display)?;
        let frame = self
            .bridge
            .take_frame(self.endpoint)
            .map_err(|_| Error::Display)?;
        self.content = frame.content;
        self.frame_sequence = frame.sequence;
        Ok(frame.sequence)
    }

    pub fn request_resize(
        &mut self,
        width: u16,
        height: u16,
        now: u64,
    ) -> Result<ResizeRequest, Error> {
        self.require_running()?;
        if self.pending_resize.is_some() {
            return Err(Error::Busy);
        }
        let next = Rect::new(self.rect.x, self.rect.y, width, height);
        if self.mode != Mode::Windowed || !valid_rect(next, self.screen) || height <= 4 {
            return Err(Error::Bounds);
        }
        let request = ResizeRequest {
            serial: self.resize_serial,
            width,
            height: height - 4,
            deadline: now.saturating_add(MAX_RESIZE_TICKS),
        };
        self.resize_serial = self.resize_serial.checked_add(1).ok_or(Error::State)?;
        self.pending_resize = Some(request);
        Ok(request)
    }

    pub fn ack_resize(&mut self, serial: u64, now: u64) -> Result<(), Error> {
        self.require_running()?;
        let request = self.pending_resize.ok_or(Error::State)?;
        if request.serial != serial {
            return Err(Error::Stale);
        }
        if now > request.deadline {
            self.pending_resize = None;
            return Err(Error::Timeout);
        }
        self.endpoint = self
            .bridge
            .resize(self.endpoint, request.width, request.height)
            .map_err(|_| Error::Display)?;
        self.rect.width = request.width;
        self.rect.height = request.height + 4;
        self.restore = self.rect;
        self.pending_resize = None;
        self.focused = false;
        Ok(())
    }

    pub fn minimize(&mut self) -> Result<(), Error> {
        self.require_running()?;
        if self.mode == Mode::Fullscreen {
            return Err(Error::State);
        }
        self.mode = Mode::Minimized;
        self.revoke_ownership();
        Ok(())
    }

    pub fn restore(&mut self) -> Result<(), Error> {
        self.require_running()?;
        self.mode = Mode::Windowed;
        self.rect = self.restore;
        Ok(())
    }

    pub fn enter_fullscreen(&mut self) -> Result<(), Error> {
        self.require_running()?;
        if self.mode != Mode::Windowed || self.pending_resize.is_some() {
            return Err(Error::State);
        }
        self.restore = self.rect;
        self.mode = Mode::Fullscreen;
        self.rect = self.screen;
        self.focus()?;
        self.exclusive_lease = true;
        Ok(())
    }

    pub fn exit_fullscreen(&mut self) -> Result<(), Error> {
        self.require_running()?;
        if self.mode != Mode::Fullscreen {
            return Err(Error::State);
        }
        self.mode = Mode::Windowed;
        self.rect = self.restore;
        self.exclusive_lease = false;
        Ok(())
    }

    pub fn secure_attention(&mut self) -> Result<(), Error> {
        self.require_running()?;
        if self.mode == Mode::Fullscreen {
            self.mode = Mode::Windowed;
            self.rect = self.restore;
        }
        self.revoke_ownership();
        Ok(())
    }

    pub fn send_input(&mut self, kind: u16, first: i32, second: i32) -> Result<u64, Error> {
        self.require_running()?;
        if !self.focused {
            return Err(Error::State);
        }
        self.bridge
            .send_input(self.endpoint, kind, first, second, false)
            .map_err(|_| Error::Display)
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        self.require_running()?;
        self.bridge
            .stop(self.endpoint)
            .map_err(|_| Error::Display)?;
        self.lifecycle = Lifecycle::Stopped;
        self.clear_surface();
        Ok(())
    }

    pub fn fail(&mut self) -> Result<(), Error> {
        self.require_running()?;
        self.bridge
            .stop(self.endpoint)
            .map_err(|_| Error::Display)?;
        self.lifecycle = Lifecycle::Failed;
        self.clear_surface();
        Ok(())
    }

    pub fn restart(&mut self) -> Result<(), Error> {
        if self.lifecycle == Lifecycle::Running {
            return Err(Error::State);
        }
        self.endpoint = self
            .bridge
            .start(
                self.domain,
                self.restore.width,
                self.restore.height.saturating_sub(4),
            )
            .map_err(|_| Error::Display)?;
        self.lifecycle = Lifecycle::Running;
        self.mode = Mode::Windowed;
        self.rect = self.restore;
        self.content = 0;
        self.frame_sequence = 0;
        Ok(())
    }

    pub fn render(&self, pixels: &mut [u32], stride: usize) -> Result<(), Error> {
        if stride < usize::from(self.screen.width)
            || pixels.len() < stride * usize::from(self.screen.height)
        {
            return Err(Error::Bounds);
        }
        fill(pixels, stride, self.screen, BACKGROUND);
        if self.mode == Mode::Minimized {
            return Ok(());
        }
        let color = match self.lifecycle {
            Lifecycle::Running => DOMAIN ^ (self.content as u32 & 0x000f_0f0f),
            Lifecycle::Stopped => STOPPED,
            Lifecycle::Failed => FAILED,
        };
        fill(pixels, stride, self.rect, color);
        let chrome = Rect::new(self.rect.x, self.rect.y, self.rect.width, 4);
        fill(
            pixels,
            stride,
            chrome,
            if self.focused { ACTIVE } else { CHROME },
        );
        // Fixed trusted marks are controller-rendered and cannot be supplied by a domain.
        for offset in 0..TRUSTED_LABEL.len().min(usize::from(self.rect.width)) {
            let x = usize::from(self.rect.x) + offset;
            let y = usize::from(self.rect.y) + 1;
            pixels[y * stride + x] = 0x00ff_ffff;
        }
        Ok(())
    }

    fn endpoint_generation(&self) -> u32 {
        self.endpoint.generation()
    }

    fn require_running(&self) -> Result<(), Error> {
        (self.lifecycle == Lifecycle::Running)
            .then_some(())
            .ok_or(Error::State)
    }

    fn revoke_ownership(&mut self) {
        self.focused = false;
        self.exclusive_lease = false;
    }

    fn clear_surface(&mut self) {
        self.revoke_ownership();
        self.mode = Mode::Windowed;
        self.rect = self.restore;
        self.content = 0;
        self.frame_sequence = 0;
        self.pending_resize = None;
    }
}

fn valid_rect(rect: Rect, bounds: Rect) -> bool {
    rect.width != 0
        && rect.height != 0
        && u32::from(rect.x) + u32::from(rect.width) <= u32::from(bounds.width)
        && u32::from(rect.y) + u32::from(rect.height) <= u32::from(bounds.height)
}

fn fill(pixels: &mut [u32], stride: usize, rect: Rect, color: u32) {
    for y in usize::from(rect.y)..usize::from(rect.y + rect.height) {
        let row = y * stride;
        for x in usize::from(rect.x)..usize::from(rect.x + rect.width) {
            pixels[row + x] = color;
        }
    }
}

pub fn pixel_hash(pixels: &[u32]) -> u64 {
    pixels.iter().fold(0xcbf2_9ce4_8422_2325, |hash, pixel| {
        (hash ^ u64::from(*pixel)).wrapping_mul(0x100_0000_01b3)
    })
}

pub fn contract_probe() -> bool {
    let screen = Rect::new(0, 0, 64, 40);
    let Ok(mut view) = Presentation::start(7, screen, Rect::new(8, 8, 40, 24)) else {
        return false;
    };
    if view.publish(0x1234).is_err() || view.focus().is_err() || view.send_input(1, 30, 1).is_err()
    {
        return false;
    }
    let content = view.status().content;
    let Ok(resize) = view.request_resize(44, 26, 10) else {
        return false;
    };
    if view.ack_resize(resize.serial, 11).is_err()
        || view.status().content != content
        || view.enter_fullscreen().is_err()
        || !view.status().exclusive_lease
        || view.secure_attention().is_err()
        || view.status().mode != Mode::Windowed
        || view.status().focused
        || view.stop().is_err()
        || view.status().lifecycle != Lifecycle::Stopped
        || view.status().content != 0
        || view.restart().is_err()
        || view.status().lifecycle != Lifecycle::Running
    {
        return false;
    }
    let mut pixels = [0_u32; 64 * 40];
    view.render(&mut pixels, 64).is_ok() && pixel_hash(&pixels) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> Presentation {
        Presentation::start(42, Rect::new(0, 0, 64, 40), Rect::new(8, 8, 40, 24)).unwrap()
    }

    #[test]
    fn same_live_surface_moves_windowed_fullscreen_and_back() {
        let mut view = view();
        view.publish(0xabcd).unwrap();
        let sequence = view.status().frame_sequence;
        view.enter_fullscreen().unwrap();
        assert_eq!(view.status().mode, Mode::Fullscreen);
        assert_eq!(view.status().content, 0xabcd);
        assert_eq!(view.status().frame_sequence, sequence);
        assert!(view.status().exclusive_lease);
        view.exit_fullscreen().unwrap();
        assert_eq!(view.status().rect, Rect::new(8, 8, 40, 24));
        assert_eq!(view.status().content, 0xabcd);
    }

    #[test]
    fn resize_is_acknowledged_bounded_and_preserves_last_content() {
        let mut view = view();
        view.publish(7).unwrap();
        let request = view.request_resize(44, 28, 100).unwrap();
        assert_eq!(view.ack_resize(request.serial + 1, 101), Err(Error::Stale));
        view.ack_resize(request.serial, 102).unwrap();
        assert_eq!(view.status().content, 7);
        assert_eq!(view.status().rect, Rect::new(8, 8, 44, 28));
        let late = view.request_resize(40, 24, 200).unwrap();
        assert_eq!(
            view.ack_resize(late.serial, late.deadline + 1),
            Err(Error::Timeout)
        );
    }

    #[test]
    fn secure_attention_revokes_fullscreen_focus_and_input() {
        let mut view = view();
        view.enter_fullscreen().unwrap();
        view.send_input(1, 1, 1).unwrap();
        view.secure_attention().unwrap();
        assert_eq!(view.status().mode, Mode::Windowed);
        assert!(!view.status().exclusive_lease);
        assert!(!view.status().focused);
        assert_eq!(view.send_input(1, 1, 1), Err(Error::State));
    }

    #[test]
    fn stop_failure_and_restart_clear_exclusive_surface() {
        let mut stopped = view();
        stopped.publish(8).unwrap();
        stopped.enter_fullscreen().unwrap();
        stopped.stop().unwrap();
        assert_eq!(stopped.status().lifecycle, Lifecycle::Stopped);
        assert_eq!(stopped.status().content, 0);
        assert!(!stopped.status().exclusive_lease);
        stopped.restart().unwrap();
        assert_eq!(stopped.status().lifecycle, Lifecycle::Running);

        let mut failed = view();
        failed.publish(9).unwrap();
        failed.enter_fullscreen().unwrap();
        failed.fail().unwrap();
        assert_eq!(failed.status().lifecycle, Lifecycle::Failed);
        assert_eq!(failed.status().content, 0);
        assert!(!failed.status().exclusive_lease);
    }

    #[test]
    fn deterministic_screenshots_match_exact_fixtures() {
        let mut view = view();
        let mut pixels = [0_u32; 64 * 40];
        view.publish(0x1234).unwrap();
        view.focus().unwrap();
        view.render(&mut pixels, 64).unwrap();
        let windowed = pixel_hash(&pixels);
        view.enter_fullscreen().unwrap();
        view.render(&mut pixels, 64).unwrap();
        let fullscreen = pixel_hash(&pixels);
        view.secure_attention().unwrap();
        view.fail().unwrap();
        view.render(&mut pixels, 64).unwrap();
        let failed = pixel_hash(&pixels);
        assert_eq!(windowed, 12_456_113_470_754_102_457);
        assert_eq!(fullscreen, 15_720_386_047_632_163_513);
        assert_eq!(failed, 15_890_846_944_293_475_641);
    }

    #[test]
    fn contract_is_complete() {
        assert!(contract_probe());
    }
}
