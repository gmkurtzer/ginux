#![no_std]

pub const SOCKET_PATH: &[u8] = b"/run/user/0/wayland-0";
pub const MAX_CLIENTS: usize = 4;
pub const MAX_WINDOWS: usize = 8;
pub const MAX_EVENTS: usize = 32;
pub const VERSION: u16 = 0;

pub const WL_DISPLAY: u16 = 1;
pub const WL_REGISTRY: u16 = 2;
pub const WL_COMPOSITOR: u16 = 3;
pub const WL_SHM: u16 = 4;
pub const WL_SEAT: u16 = 5;
pub const XDG_WM_BASE: u16 = 6;

pub const EVENT_CONFIGURE: u16 = 1;
pub const EVENT_FRAME_DONE: u16 = 2;
pub const EVENT_POINTER: u16 = 3;
pub const EVENT_KEYBOARD: u16 = 4;
pub const EVENT_CLOSE: u16 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Capacity,
    InvalidObject,
    InvalidMessage,
    UnsupportedInterface,
    UnsupportedRequest,
    Bounds,
    Stale,
    Empty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Client {
    slot: u8,
    generation: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Event {
    pub kind: u16,
    pub object: u32,
    pub first: i32,
    pub second: i32,
    pub serial: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportedDesktop {
    pub generation: u32,
    pub sequence: u64,
    pub windows: u8,
    pub damage: [u16; 4],
    pub content_hash: u64,
}

#[derive(Clone, Copy)]
struct ClientState {
    live: bool,
    generation: u16,
    registry: bool,
    compositor: bool,
    shm: bool,
    seat: bool,
    shell: bool,
    events: [Option<Event>; MAX_EVENTS],
    event_head: u8,
    event_count: u8,
}

impl ClientState {
    const fn empty() -> Self {
        Self {
            live: false,
            generation: 0,
            registry: false,
            compositor: false,
            shm: false,
            seat: false,
            shell: false,
            events: [None; MAX_EVENTS],
            event_head: 0,
            event_count: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct Window {
    live: bool,
    client: Client,
    surface: u32,
    buffer: u32,
    callback: u32,
    configured: bool,
    width: u16,
    height: u16,
    damage: [u16; 4],
    content: u64,
}

impl Window {
    const fn empty() -> Self {
        Self {
            live: false,
            client: Client {
                slot: 0,
                generation: 0,
            },
            surface: 0,
            buffer: 0,
            callback: 0,
            configured: false,
            width: 0,
            height: 0,
            damage: [0; 4],
            content: 0,
        }
    }
}

pub struct Compositor {
    clients: [ClientState; MAX_CLIENTS],
    generations: [u16; MAX_CLIENTS],
    windows: [Window; MAX_WINDOWS],
    desktop_generation: u32,
    frame_sequence: u64,
    serial: u32,
}

impl Compositor {
    pub const fn new() -> Self {
        Self {
            clients: [ClientState::empty(); MAX_CLIENTS],
            generations: [0; MAX_CLIENTS],
            windows: [Window::empty(); MAX_WINDOWS],
            desktop_generation: 1,
            frame_sequence: 1,
            serial: 1,
        }
    }

    pub fn socket_path(&self) -> &'static [u8] {
        SOCKET_PATH
    }

    pub fn connect(&mut self) -> Result<Client, Error> {
        let slot = self
            .clients
            .iter()
            .position(|client| !client.live)
            .ok_or(Error::Capacity)?;
        let generation = self.generations[slot]
            .checked_add(1)
            .filter(|value| *value != 0)
            .ok_or(Error::Capacity)?;
        self.generations[slot] = generation;
        self.clients[slot] = ClientState {
            live: true,
            generation,
            ..ClientState::empty()
        };
        Ok(Client {
            slot: slot as u8,
            generation,
        })
    }

    fn client(&self, client: Client) -> Result<&ClientState, Error> {
        self.clients
            .get(client.slot as usize)
            .filter(|state| state.live && state.generation == client.generation)
            .ok_or(Error::Stale)
    }

    fn client_mut(&mut self, client: Client) -> Result<&mut ClientState, Error> {
        self.clients
            .get_mut(client.slot as usize)
            .filter(|state| state.live && state.generation == client.generation)
            .ok_or(Error::Stale)
    }

    pub fn bind(&mut self, client: Client, interface: u16) -> Result<(), Error> {
        let state = self.client_mut(client)?;
        match interface {
            WL_DISPLAY | WL_REGISTRY => state.registry = true,
            WL_COMPOSITOR => state.compositor = true,
            WL_SHM => state.shm = true,
            WL_SEAT => state.seat = true,
            XDG_WM_BASE => state.shell = true,
            _ => return Err(Error::UnsupportedInterface),
        }
        Ok(())
    }

    pub fn create_window(
        &mut self,
        client: Client,
        surface: u32,
        buffer: u32,
        width: u16,
        height: u16,
    ) -> Result<(), Error> {
        let state = self.client(client)?;
        if !state.compositor || !state.shm || !state.shell {
            return Err(Error::InvalidObject);
        }
        if surface == 0 || buffer == 0 || width == 0 || height == 0 || width > 4096 || height > 2160
        {
            return Err(Error::Bounds);
        }
        if self
            .windows
            .iter()
            .any(|window| window.live && window.client == client && window.surface == surface)
        {
            return Err(Error::InvalidObject);
        }
        let slot = self
            .windows
            .iter()
            .position(|window| !window.live)
            .ok_or(Error::Capacity)?;
        self.windows[slot] = Window {
            live: true,
            client,
            surface,
            buffer,
            callback: 0,
            configured: false,
            width,
            height,
            damage: [0, 0, width, height],
            content: 0,
        };
        let serial = self.next_serial()?;
        self.push_event(
            client,
            Event {
                kind: EVENT_CONFIGURE,
                object: surface,
                first: i32::from(width),
                second: i32::from(height),
                serial,
            },
        )
    }

    pub fn ack_configure(
        &mut self,
        client: Client,
        surface: u32,
        serial: u32,
    ) -> Result<(), Error> {
        if serial == 0 {
            return Err(Error::InvalidMessage);
        }
        let window = self.window_mut(client, surface)?;
        window.configured = true;
        Ok(())
    }

    pub fn commit(
        &mut self,
        client: Client,
        surface: u32,
        damage: [u16; 4],
        content: u64,
        callback: u32,
    ) -> Result<(), Error> {
        let window = self.window_mut(client, surface)?;
        if !window.configured
            || content == 0
            || callback == 0
            || damage[2] == 0
            || damage[3] == 0
            || u32::from(damage[0]) + u32::from(damage[2]) > u32::from(window.width)
            || u32::from(damage[1]) + u32::from(damage[3]) > u32::from(window.height)
        {
            return Err(Error::Bounds);
        }
        window.damage = damage;
        window.content = content;
        window.callback = callback;
        Ok(())
    }

    pub fn compose(&mut self) -> Result<ExportedDesktop, Error> {
        let mut windows = 0_u8;
        let mut damage = [u16::MAX, u16::MAX, 0, 0];
        let mut content_hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut callbacks = [(
            Client {
                slot: 0,
                generation: 0,
            },
            0_u32,
        ); MAX_WINDOWS];
        let mut callback_count = 0;
        for window in self
            .windows
            .iter_mut()
            .filter(|window| window.live && window.configured && window.content != 0)
        {
            windows = windows.checked_add(1).ok_or(Error::Capacity)?;
            damage[0] = damage[0].min(window.damage[0]);
            damage[1] = damage[1].min(window.damage[1]);
            damage[2] = damage[2].max(window.damage[0].saturating_add(window.damage[2]));
            damage[3] = damage[3].max(window.damage[1].saturating_add(window.damage[3]));
            content_hash = content_hash.wrapping_mul(0x100_0000_01b3)
                ^ window.content
                ^ u64::from(window.buffer);
            if window.callback != 0 {
                callbacks[callback_count] = (window.client, window.callback);
                callback_count += 1;
                window.callback = 0;
            }
        }
        if windows == 0 {
            return Err(Error::Empty);
        }
        damage[2] = damage[2].saturating_sub(damage[0]);
        damage[3] = damage[3].saturating_sub(damage[1]);
        let sequence = self.frame_sequence;
        self.frame_sequence = sequence.checked_add(1).ok_or(Error::Capacity)?;
        for (client, callback) in callbacks.into_iter().take(callback_count) {
            let serial = self.next_serial()?;
            self.push_event(
                client,
                Event {
                    kind: EVENT_FRAME_DONE,
                    object: callback,
                    first: sequence as i32,
                    second: 0,
                    serial,
                },
            )?;
        }
        Ok(ExportedDesktop {
            generation: self.desktop_generation,
            sequence,
            windows,
            damage,
            content_hash,
        })
    }

    pub fn input(
        &mut self,
        client: Client,
        pointer: bool,
        first: i32,
        second: i32,
    ) -> Result<(), Error> {
        if !self.client(client)?.seat {
            return Err(Error::InvalidObject);
        }
        let kind = if pointer {
            EVENT_POINTER
        } else {
            EVENT_KEYBOARD
        };
        let serial = self.next_serial()?;
        self.push_event(
            client,
            Event {
                kind,
                object: WL_SEAT as u32,
                first,
                second,
                serial,
            },
        )
    }

    pub fn next_event(&mut self, client: Client) -> Result<Event, Error> {
        let state = self.client_mut(client)?;
        if state.event_count == 0 {
            return Err(Error::Empty);
        }
        let slot = usize::from(state.event_head);
        let event = state.events[slot].take().ok_or(Error::Empty)?;
        state.event_head = ((slot + 1) % MAX_EVENTS) as u8;
        state.event_count -= 1;
        Ok(event)
    }

    pub fn disconnect(&mut self, client: Client) -> Result<(), Error> {
        self.client(client)?;
        for window in self
            .windows
            .iter_mut()
            .filter(|window| window.live && window.client == client)
        {
            *window = Window::empty();
        }
        self.clients[client.slot as usize] = ClientState::empty();
        self.desktop_generation = self
            .desktop_generation
            .checked_add(1)
            .ok_or(Error::Capacity)?;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        self.clients = [ClientState::empty(); MAX_CLIENTS];
        self.windows = [Window::empty(); MAX_WINDOWS];
        self.desktop_generation = self
            .desktop_generation
            .checked_add(1)
            .ok_or(Error::Capacity)?;
        Ok(())
    }

    fn window_mut(&mut self, client: Client, surface: u32) -> Result<&mut Window, Error> {
        self.client(client)?;
        self.windows
            .iter_mut()
            .find(|window| window.live && window.client == client && window.surface == surface)
            .ok_or(Error::InvalidObject)
    }

    fn push_event(&mut self, client: Client, event: Event) -> Result<(), Error> {
        let state = self.client_mut(client)?;
        if usize::from(state.event_count) == MAX_EVENTS {
            return Err(Error::Capacity);
        }
        let tail = (usize::from(state.event_head) + usize::from(state.event_count)) % MAX_EVENTS;
        state.events[tail] = Some(event);
        state.event_count += 1;
        Ok(())
    }

    fn next_serial(&mut self) -> Result<u32, Error> {
        let serial = self.serial;
        self.serial = serial
            .checked_add(1)
            .filter(|value| *value != 0)
            .ok_or(Error::Capacity)?;
        Ok(serial)
    }
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new()
    }
}

fn bind_standard_client(compositor: &mut Compositor, client: Client) -> Result<(), Error> {
    for interface in [
        WL_DISPLAY,
        WL_REGISTRY,
        WL_COMPOSITOR,
        WL_SHM,
        WL_SEAT,
        XDG_WM_BASE,
    ] {
        compositor.bind(client, interface)?;
    }
    Ok(())
}

pub fn contract_probe() -> bool {
    let mut compositor = Compositor::new();
    let Ok(first) = compositor.connect() else {
        return false;
    };
    let Ok(second) = compositor.connect() else {
        return false;
    };
    if compositor.socket_path() != SOCKET_PATH
        || bind_standard_client(&mut compositor, first).is_err()
        || bind_standard_client(&mut compositor, second).is_err()
        || compositor.bind(first, 99) != Err(Error::UnsupportedInterface)
        || compositor.create_window(first, 10, 20, 320, 200).is_err()
        || compositor.create_window(second, 11, 21, 400, 240).is_err()
    {
        return false;
    }
    let Ok(first_configure) = compositor.next_event(first) else {
        return false;
    };
    let Ok(second_configure) = compositor.next_event(second) else {
        return false;
    };
    if compositor
        .ack_configure(first, 10, first_configure.serial)
        .is_err()
        || compositor
            .ack_configure(second, 11, second_configure.serial)
            .is_err()
        || compositor
            .commit(first, 10, [0, 0, 320, 200], 0x1111, 30)
            .is_err()
        || compositor
            .commit(second, 11, [0, 0, 400, 240], 0x2222, 31)
            .is_err()
    {
        return false;
    }
    let Ok(frame) = compositor.compose() else {
        return false;
    };
    frame.windows == 2
        && compositor
            .next_event(first)
            .is_ok_and(|event| event.kind == EVENT_FRAME_DONE)
        && compositor
            .next_event(second)
            .is_ok_and(|event| event.kind == EVENT_FRAME_DONE)
        && compositor.input(first, true, 5, 7).is_ok()
        && compositor.input(second, false, 30, 1).is_ok()
        && compositor
            .next_event(first)
            .is_ok_and(|event| event.kind == EVENT_POINTER)
        && compositor
            .next_event(second)
            .is_ok_and(|event| event.kind == EVENT_KEYBOARD)
        && compositor.disconnect(first).is_ok()
        && compositor
            .compose()
            .is_ok_and(|desktop| desktop.windows == 1)
        && compositor.reset().is_ok()
        && compositor.compose() == Err(Error::Empty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_standard_clients_render_receive_input_and_close() {
        assert!(contract_probe());
    }

    #[test]
    fn client_crash_isolated_and_reconnect_generation_is_fresh() {
        let mut compositor = Compositor::new();
        let old = compositor.connect().unwrap();
        bind_standard_client(&mut compositor, old).unwrap();
        compositor.create_window(old, 10, 20, 64, 64).unwrap();
        let configure = compositor.next_event(old).unwrap();
        compositor.ack_configure(old, 10, configure.serial).unwrap();
        compositor.commit(old, 10, [0, 0, 64, 64], 1, 30).unwrap();
        compositor.disconnect(old).unwrap();
        assert_eq!(compositor.next_event(old), Err(Error::Stale));
        let fresh = compositor.connect().unwrap();
        assert_ne!(old, fresh);
    }

    #[test]
    fn unsupported_and_malformed_requests_fail_explicitly() {
        let mut compositor = Compositor::new();
        let client = compositor.connect().unwrap();
        assert_eq!(
            compositor.bind(client, 99),
            Err(Error::UnsupportedInterface)
        );
        assert_eq!(
            compositor.create_window(client, 1, 2, 10, 10),
            Err(Error::InvalidObject)
        );
        bind_standard_client(&mut compositor, client).unwrap();
        assert_eq!(
            compositor.create_window(client, 1, 2, 0, 10),
            Err(Error::Bounds)
        );
    }
}
