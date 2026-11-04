#![no_std]

pub const VERSION: u16 = 0;
pub const MAX_DOMAINS: usize = 4;
pub const MAX_INPUT: usize = 16;
pub const MAX_FRAMES: usize = 2;
pub const MAX_WIDTH: u16 = 4096;
pub const MAX_HEIGHT: u16 = 2160;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Capacity,
    Capability,
    Stale,
    Bounds,
    Busy,
    NotFocused,
    Empty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Endpoint {
    slot: u8,
    generation: u32,
    domain: u64,
}

impl Endpoint {
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinuxDisplayRequestV0 {
    pub version: u16,
    pub size: u16,
    pub command: u16,
    pub flags: u16,
    pub generation: u32,
    pub width: u16,
    pub height: u16,
    pub damage_x: u16,
    pub damage_y: u16,
    pub damage_width: u16,
    pub damage_height: u16,
    pub content: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinuxInputEventV0 {
    pub version: u16,
    pub size: u16,
    pub kind: u16,
    pub flags: u16,
    pub value0: i32,
    pub value1: i32,
    pub sequence: u64,
}

pub const INPUT_SYNTHETIC: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frame {
    pub generation: u32,
    pub sequence: u64,
    pub width: u16,
    pub height: u16,
    pub damage: [u16; 4],
    pub content: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Input {
    pub kind: u16,
    pub flags: u16,
    pub value0: i32,
    pub value1: i32,
    pub sequence: u64,
}

#[derive(Clone, Copy)]
struct Domain {
    id: u64,
    generation: u32,
    width: u16,
    height: u16,
    running: bool,
    focused: bool,
    frames: [Option<Frame>; MAX_FRAMES],
    frame_head: u8,
    frame_count: u8,
    next_frame: u64,
    input: [Option<Input>; MAX_INPUT],
    input_head: u8,
    input_count: u8,
    next_input: u64,
}

impl Domain {
    const fn empty() -> Self {
        Self {
            id: 0,
            generation: 0,
            width: 0,
            height: 0,
            running: false,
            focused: false,
            frames: [None; MAX_FRAMES],
            frame_head: 0,
            frame_count: 0,
            next_frame: 1,
            input: [None; MAX_INPUT],
            input_head: 0,
            input_count: 0,
            next_input: 1,
        }
    }
    fn endpoint(&self, slot: usize) -> Endpoint {
        Endpoint {
            slot: slot as u8,
            generation: self.generation,
            domain: self.id,
        }
    }
    fn reset_queues(&mut self) {
        self.frames = [None; MAX_FRAMES];
        self.frame_head = 0;
        self.frame_count = 0;
        self.input = [None; MAX_INPUT];
        self.input_head = 0;
        self.input_count = 0;
    }
}

pub struct Bridge {
    domains: [Domain; MAX_DOMAINS],
    generations: [u32; MAX_DOMAINS],
    focused: Option<Endpoint>,
}

impl Bridge {
    pub const fn new() -> Self {
        Self {
            domains: [Domain::empty(); MAX_DOMAINS],
            generations: [0; MAX_DOMAINS],
            focused: None,
        }
    }
    fn domain(&self, endpoint: Endpoint) -> Result<&Domain, Error> {
        self.domains
            .get(endpoint.slot as usize)
            .filter(|domain| {
                domain.running
                    && domain.id == endpoint.domain
                    && domain.generation == endpoint.generation
                    && endpoint.generation != 0
            })
            .ok_or(Error::Stale)
    }
    fn domain_mut(&mut self, endpoint: Endpoint) -> Result<&mut Domain, Error> {
        self.domains
            .get_mut(endpoint.slot as usize)
            .filter(|domain| {
                domain.running
                    && domain.id == endpoint.domain
                    && domain.generation == endpoint.generation
                    && endpoint.generation != 0
            })
            .ok_or(Error::Stale)
    }
    pub fn start(&mut self, id: u64, width: u16, height: u16) -> Result<Endpoint, Error> {
        if id == 0 || width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
            return Err(Error::Bounds);
        }
        if self
            .domains
            .iter()
            .any(|domain| domain.running && domain.id == id)
        {
            return Err(Error::Busy);
        }
        let slot = self
            .domains
            .iter()
            .position(|domain| !domain.running)
            .ok_or(Error::Capacity)?;
        let generation = self.generations[slot]
            .checked_add(1)
            .filter(|value| *value != 0)
            .ok_or(Error::Capacity)?;
        self.generations[slot] = generation;
        let mut domain = Domain::empty();
        domain.id = id;
        domain.generation = generation;
        domain.width = width;
        domain.height = height;
        domain.running = true;
        self.domains[slot] = domain;
        Ok(domain.endpoint(slot))
    }
    pub fn focus(&mut self, endpoint: Endpoint) -> Result<(), Error> {
        self.domain(endpoint)?;
        for domain in &mut self.domains {
            domain.focused = false;
        }
        self.domain_mut(endpoint)?.focused = true;
        self.focused = Some(endpoint);
        Ok(())
    }
    pub fn publish(
        &mut self,
        endpoint: Endpoint,
        damage: [u16; 4],
        content: u64,
    ) -> Result<u64, Error> {
        let domain = self.domain_mut(endpoint)?;
        if content == 0
            || damage[2] == 0
            || damage[3] == 0
            || u32::from(damage[0]) + u32::from(damage[2]) > u32::from(domain.width)
            || u32::from(damage[1]) + u32::from(damage[3]) > u32::from(domain.height)
        {
            return Err(Error::Bounds);
        }
        if usize::from(domain.frame_count) == MAX_FRAMES {
            return Err(Error::Busy);
        }
        let sequence = domain.next_frame;
        domain.next_frame = sequence.checked_add(1).ok_or(Error::Capacity)?;
        let tail = (usize::from(domain.frame_head) + usize::from(domain.frame_count)) % MAX_FRAMES;
        domain.frames[tail] = Some(Frame {
            generation: domain.generation,
            sequence,
            width: domain.width,
            height: domain.height,
            damage,
            content,
        });
        domain.frame_count += 1;
        Ok(sequence)
    }
    pub fn take_frame(&mut self, endpoint: Endpoint) -> Result<Frame, Error> {
        let domain = self.domain_mut(endpoint)?;
        if domain.frame_count == 0 {
            return Err(Error::Empty);
        }
        let slot = usize::from(domain.frame_head);
        let frame = domain.frames[slot].take().ok_or(Error::Empty)?;
        domain.frame_head = ((slot + 1) % MAX_FRAMES) as u8;
        domain.frame_count -= 1;
        Ok(frame)
    }
    pub fn send_input(
        &mut self,
        endpoint: Endpoint,
        kind: u16,
        value0: i32,
        value1: i32,
        synthetic: bool,
    ) -> Result<u64, Error> {
        if !synthetic && self.focused != Some(endpoint) {
            return Err(Error::NotFocused);
        }
        let domain = self.domain_mut(endpoint)?;
        if !synthetic && !domain.focused {
            return Err(Error::NotFocused);
        }
        if usize::from(domain.input_count) == MAX_INPUT {
            return Err(Error::Busy);
        }
        let sequence = domain.next_input;
        domain.next_input = sequence.checked_add(1).ok_or(Error::Capacity)?;
        let tail = (usize::from(domain.input_head) + usize::from(domain.input_count)) % MAX_INPUT;
        domain.input[tail] = Some(Input {
            kind,
            flags: if synthetic { INPUT_SYNTHETIC } else { 0 },
            value0,
            value1,
            sequence,
        });
        domain.input_count += 1;
        Ok(sequence)
    }
    pub fn take_input(&mut self, endpoint: Endpoint) -> Result<Input, Error> {
        let domain = self.domain_mut(endpoint)?;
        if domain.input_count == 0 {
            return Err(Error::Empty);
        }
        let slot = usize::from(domain.input_head);
        let event = domain.input[slot].take().ok_or(Error::Empty)?;
        domain.input_head = ((slot + 1) % MAX_INPUT) as u8;
        domain.input_count -= 1;
        Ok(event)
    }
    pub fn resize(
        &mut self,
        endpoint: Endpoint,
        width: u16,
        height: u16,
    ) -> Result<Endpoint, Error> {
        if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
            return Err(Error::Bounds);
        }
        let slot = endpoint.slot as usize;
        self.domain(endpoint)?;
        let generation = self.generations[slot]
            .checked_add(1)
            .ok_or(Error::Capacity)?;
        self.generations[slot] = generation;
        let domain = &mut self.domains[slot];
        domain.generation = generation;
        domain.width = width;
        domain.height = height;
        domain.focused = false;
        domain.reset_queues();
        self.focused = None;
        Ok(domain.endpoint(slot))
    }
    pub fn stop(&mut self, endpoint: Endpoint) -> Result<(), Error> {
        self.domain(endpoint)?;
        let domain = &mut self.domains[endpoint.slot as usize];
        domain.running = false;
        domain.focused = false;
        domain.reset_queues();
        if self.focused == Some(endpoint) {
            self.focused = None;
        }
        Ok(())
    }
    pub fn pending_frames(&self, endpoint: Endpoint) -> Result<u8, Error> {
        Ok(self.domain(endpoint)?.frame_count)
    }
    pub fn pending_input(&self, endpoint: Endpoint) -> Result<u8, Error> {
        Ok(self.domain(endpoint)?.input_count)
    }
}

impl Default for Bridge {
    fn default() -> Self {
        Self::new()
    }
}

pub fn contract_probe() -> bool {
    let mut bridge = Bridge::new();
    let Ok(first) = bridge.start(1, 800, 600) else {
        return false;
    };
    let Ok(second) = bridge.start(2, 640, 480) else {
        return false;
    };
    if bridge.focus(first).is_err()
        || bridge.publish(first, [0, 0, 100, 100], 11).is_err()
        || bridge.publish(second, [0, 0, 64, 64], 22).is_err()
        || bridge.send_input(second, 1, 30, 1, false) != Err(Error::NotFocused)
        || bridge.send_input(first, 1, 30, 1, false).is_err()
        || bridge.send_input(second, 2, 1, 1, true).is_err()
    {
        return false;
    }
    let Ok(next) = bridge.resize(first, 1024, 768) else {
        return false;
    };
    bridge.take_frame(first) == Err(Error::Stale)
        && bridge.pending_frames(next) == Ok(0)
        && bridge.take_frame(second).is_ok()
        && bridge.stop(second).is_ok()
        && bridge.take_input(second) == Err(Error::Stale)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolation_focus_and_synthetic_labeling() {
        let mut bridge = Bridge::new();
        let a = bridge.start(1, 100, 100).unwrap();
        let b = bridge.start(2, 100, 100).unwrap();
        bridge.focus(a).unwrap();
        bridge.publish(a, [0, 0, 10, 10], 1).unwrap();
        bridge.publish(b, [1, 1, 10, 10], 2).unwrap();
        assert_eq!(bridge.take_frame(a).unwrap().content, 1);
        assert_eq!(bridge.take_frame(b).unwrap().content, 2);
        assert_eq!(bridge.send_input(b, 1, 0, 0, false), Err(Error::NotFocused));
        bridge.send_input(b, 1, 0, 0, true).unwrap();
        assert_eq!(bridge.take_input(b).unwrap().flags, INPUT_SYNTHETIC);
        bridge.send_input(a, 1, 0, 0, false).unwrap();
        assert_eq!(bridge.take_input(a).unwrap().flags, 0);
    }
    #[test]
    fn backpressure_resize_stop_restart_and_stale_generations() {
        let mut bridge = Bridge::new();
        let old = bridge.start(7, 100, 100).unwrap();
        bridge.publish(old, [0, 0, 1, 1], 1).unwrap();
        bridge.publish(old, [0, 0, 1, 1], 2).unwrap();
        assert_eq!(bridge.publish(old, [0, 0, 1, 1], 3), Err(Error::Busy));
        let resized = bridge.resize(old, 200, 200).unwrap();
        assert_eq!(bridge.take_frame(old), Err(Error::Stale));
        assert_eq!(bridge.pending_frames(resized), Ok(0));
        bridge.stop(resized).unwrap();
        assert_eq!(bridge.publish(resized, [0, 0, 1, 1], 1), Err(Error::Stale));
        let restarted = bridge.start(7, 200, 200).unwrap();
        assert_ne!(resized, restarted);
    }
    #[test]
    fn linux_transport_layout_is_stable() {
        assert_eq!(core::mem::size_of::<LinuxDisplayRequestV0>(), 32);
        assert_eq!(core::mem::size_of::<LinuxInputEventV0>(), 24);
        assert!(contract_probe());
    }
}
