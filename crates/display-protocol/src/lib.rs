#![no_std]

pub const VERSION: u16 = 0;
pub const FEATURE_BUFFERS: u32 = 1 << 0;
pub const FEATURE_DAMAGE: u32 = 1 << 1;
pub const FEATURE_INPUT: u32 = 1 << 2;
pub const SUPPORTED_FEATURES: u32 = FEATURE_BUFFERS | FEATURE_DAMAGE | FEATURE_INPUT;
pub const HEADER_SIZE: usize = 16;
pub const MAX_MESSAGE_SIZE: usize = 256;
pub const MAX_CLIENTS: usize = 2;
pub const MAX_OBJECTS: usize = 8;
pub const MAX_OBJECT_ID: u32 = 65_535;
pub const MAX_EVENTS: usize = 16;
pub const MAX_WIDTH: u32 = 4096;
pub const MAX_HEIGHT: u32 = 2160;
pub const MAX_STRIDE: u32 = MAX_WIDTH * 4;

pub const OPCODE_CREATE_SURFACE: u16 = 1;
pub const OPCODE_CREATE_BUFFER: u16 = 2;
pub const OPCODE_DAMAGE: u16 = 3;
pub const OPCODE_COMMIT: u16 = 4;
pub const OPCODE_FRAME: u16 = 5;
pub const OPCODE_DESTROY: u16 = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    Version,
    Feature,
    Capability,
    Capacity,
    Object,
    ObjectType,
    Duplicate,
    Length,
    Opcode,
    Bounds,
    Busy,
    QueueFull,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    pub size: u16,
    pub version: u16,
    pub opcode: u16,
    pub flags: u16,
    pub object_id: u32,
    pub payload_size: u32,
}

impl Header {
    pub fn decode(message: &[u8]) -> Result<Self, ProtocolError> {
        if message.len() < HEADER_SIZE || message.len() > MAX_MESSAGE_SIZE {
            return Err(ProtocolError::Length);
        }
        let header = Self {
            size: u16::from_le_bytes([message[0], message[1]]),
            version: u16::from_le_bytes([message[2], message[3]]),
            opcode: u16::from_le_bytes([message[4], message[5]]),
            flags: u16::from_le_bytes([message[6], message[7]]),
            object_id: u32::from_le_bytes(message[8..12].try_into().unwrap()),
            payload_size: u32::from_le_bytes(message[12..16].try_into().unwrap()),
        };
        if usize::from(header.size) != message.len()
            || usize::try_from(header.payload_size).ok() != Some(message.len() - HEADER_SIZE)
            || header.version != VERSION
            || header.flags != 0
            || header.object_id == 0
            || header.object_id > MAX_OBJECT_ID
        {
            return Err(ProtocolError::Length);
        }
        if !(OPCODE_CREATE_SURFACE..=OPCODE_DESTROY).contains(&header.opcode) {
            return Err(ProtocolError::Opcode);
        }
        Ok(header)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientHandle {
    slot: u16,
    generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Damage {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventKind {
    Configure,
    FrameDone,
    BufferRelease,
    Focus,
    Pointer,
    Keyboard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Event {
    pub kind: EventKind,
    pub object_id: u32,
    pub value0: i32,
    pub value1: i32,
    pub sequence: u64,
}

const EMPTY_EVENT: Event = Event {
    kind: EventKind::Configure,
    object_id: 0,
    value0: 0,
    value1: 0,
    sequence: 0,
};

#[derive(Clone, Copy)]
struct Buffer {
    width: u32,
    height: u32,
    stride: u32,
    published: bool,
    content_generation: u64,
}

#[derive(Clone, Copy)]
struct Surface {
    buffer_id: u32,
    callback_id: u32,
    damage: Damage,
}

#[derive(Clone, Copy)]
enum ObjectKind {
    Registry,
    Seat,
    Surface(Surface),
    Buffer(Buffer),
    Callback,
}

#[derive(Clone, Copy)]
struct Object {
    id: u32,
    kind: ObjectKind,
}

struct Client {
    generation: u32,
    objects: [Option<Object>; MAX_OBJECTS],
    events: [Event; MAX_EVENTS],
    event_head: usize,
    event_count: usize,
    next_sequence: u64,
}

impl Client {
    const fn empty() -> Self {
        Self {
            generation: 0,
            objects: [None; MAX_OBJECTS],
            events: [EMPTY_EVENT; MAX_EVENTS],
            event_head: 0,
            event_count: 0,
            next_sequence: 1,
        }
    }

    fn reset(&mut self, generation: u32) {
        *self = Self::empty();
        self.generation = generation;
        self.objects[0] = Some(Object {
            id: 1,
            kind: ObjectKind::Registry,
        });
        self.objects[1] = Some(Object {
            id: 2,
            kind: ObjectKind::Seat,
        });
    }

    fn object_index(&self, id: u32) -> Option<usize> {
        self.objects
            .iter()
            .position(|object| object.is_some_and(|object| object.id == id))
    }

    fn insert(&mut self, object: Object) -> Result<(), ProtocolError> {
        if object.id == 0 || object.id > MAX_OBJECT_ID {
            return Err(ProtocolError::Object);
        }
        if self.object_index(object.id).is_some() {
            return Err(ProtocolError::Duplicate);
        }
        let slot = self
            .objects
            .iter()
            .position(Option::is_none)
            .ok_or(ProtocolError::Capacity)?;
        self.objects[slot] = Some(object);
        Ok(())
    }

    fn push_event(
        &mut self,
        kind: EventKind,
        object_id: u32,
        value0: i32,
        value1: i32,
    ) -> Result<(), ProtocolError> {
        if self.event_count == MAX_EVENTS || self.next_sequence == u64::MAX {
            return Err(ProtocolError::QueueFull);
        }
        let tail = (self.event_head + self.event_count) % MAX_EVENTS;
        self.events[tail] = Event {
            kind,
            object_id,
            value0,
            value1,
            sequence: self.next_sequence,
        };
        self.next_sequence += 1;
        self.event_count += 1;
        Ok(())
    }

    fn pop_event(&mut self) -> Option<Event> {
        if self.event_count == 0 {
            return None;
        }
        let event = self.events[self.event_head];
        self.events[self.event_head] = EMPTY_EVENT;
        self.event_head = (self.event_head + 1) % MAX_EVENTS;
        self.event_count -= 1;
        Some(event)
    }
}

pub struct Server {
    clients: [Client; MAX_CLIENTS],
    generations: [u32; MAX_CLIENTS],
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Server {
    pub const fn new() -> Self {
        Self {
            clients: [Client::empty(), Client::empty()],
            generations: [0; MAX_CLIENTS],
        }
    }

    pub fn connect(
        &mut self,
        version: u16,
        required_features: u32,
    ) -> Result<ClientHandle, ProtocolError> {
        if version != VERSION {
            return Err(ProtocolError::Version);
        }
        if required_features & !SUPPORTED_FEATURES != 0 {
            return Err(ProtocolError::Feature);
        }
        let slot = self
            .clients
            .iter()
            .position(|client| client.generation == 0)
            .ok_or(ProtocolError::Capacity)?;
        let generation = self.generations[slot]
            .checked_add(1)
            .filter(|generation| *generation != 0)
            .ok_or(ProtocolError::Capacity)?;
        self.generations[slot] = generation;
        self.clients[slot].reset(generation);
        Ok(ClientHandle {
            slot: slot as u16,
            generation,
        })
    }

    fn client(&self, handle: ClientHandle) -> Result<&Client, ProtocolError> {
        self.clients
            .get(handle.slot as usize)
            .filter(|client| client.generation == handle.generation && handle.generation != 0)
            .ok_or(ProtocolError::Capability)
    }

    fn client_mut(&mut self, handle: ClientHandle) -> Result<&mut Client, ProtocolError> {
        self.clients
            .get_mut(handle.slot as usize)
            .filter(|client| client.generation == handle.generation && handle.generation != 0)
            .ok_or(ProtocolError::Capability)
    }

    pub fn disconnect(&mut self, handle: ClientHandle) -> Result<(), ProtocolError> {
        let client = self.client_mut(handle)?;
        *client = Client::empty();
        Ok(())
    }

    pub fn create_surface(&mut self, handle: ClientHandle, id: u32) -> Result<(), ProtocolError> {
        let client = self.client_mut(handle)?;
        if client.event_count == MAX_EVENTS {
            return Err(ProtocolError::QueueFull);
        }
        client.insert(Object {
            id,
            kind: ObjectKind::Surface(Surface {
                buffer_id: 0,
                callback_id: 0,
                damage: Damage {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
            }),
        })?;
        client.push_event(EventKind::Configure, id, 0, 0)
    }

    pub fn create_buffer(
        &mut self,
        handle: ClientHandle,
        id: u32,
        width: u32,
        height: u32,
        stride: u32,
    ) -> Result<(), ProtocolError> {
        if width == 0
            || width > MAX_WIDTH
            || height == 0
            || height > MAX_HEIGHT
            || stride < width.saturating_mul(4)
            || stride > MAX_STRIDE
            || !stride.is_multiple_of(4)
            || usize::try_from(stride)
                .ok()
                .and_then(|stride| stride.checked_mul(height as usize))
                .is_none()
        {
            return Err(ProtocolError::Bounds);
        }
        self.client_mut(handle)?.insert(Object {
            id,
            kind: ObjectKind::Buffer(Buffer {
                width,
                height,
                stride,
                published: false,
                content_generation: 0,
            }),
        })
    }

    pub fn request_frame(
        &mut self,
        handle: ClientHandle,
        surface_id: u32,
        callback_id: u32,
    ) -> Result<(), ProtocolError> {
        let client = self.client_mut(handle)?;
        let index = client
            .object_index(surface_id)
            .ok_or(ProtocolError::Object)?;
        if !matches!(
            client.objects[index],
            Some(Object {
                kind: ObjectKind::Surface(_),
                ..
            })
        ) {
            return Err(ProtocolError::ObjectType);
        }
        client.insert(Object {
            id: callback_id,
            kind: ObjectKind::Callback,
        })?;
        let Some(Object {
            kind: ObjectKind::Surface(mut surface),
            ..
        }) = client.objects[index]
        else {
            return Err(ProtocolError::ObjectType);
        };
        surface.callback_id = callback_id;
        client.objects[index] = Some(Object {
            id: surface_id,
            kind: ObjectKind::Surface(surface),
        });
        Ok(())
    }

    pub fn mutate_buffer(
        &mut self,
        handle: ClientHandle,
        buffer_id: u32,
    ) -> Result<u64, ProtocolError> {
        let client = self.client_mut(handle)?;
        let index = client
            .object_index(buffer_id)
            .ok_or(ProtocolError::Object)?;
        let Some(Object {
            kind: ObjectKind::Buffer(mut buffer),
            ..
        }) = client.objects[index]
        else {
            return Err(ProtocolError::ObjectType);
        };
        if buffer.published {
            return Err(ProtocolError::Busy);
        }
        buffer.content_generation = buffer
            .content_generation
            .checked_add(1)
            .ok_or(ProtocolError::Capacity)?;
        client.objects[index] = Some(Object {
            id: buffer_id,
            kind: ObjectKind::Buffer(buffer),
        });
        Ok(buffer.content_generation)
    }

    pub fn commit(
        &mut self,
        handle: ClientHandle,
        surface_id: u32,
        buffer_id: u32,
        damage: Damage,
    ) -> Result<(), ProtocolError> {
        let client = self.client_mut(handle)?;
        let surface_index = client
            .object_index(surface_id)
            .ok_or(ProtocolError::Object)?;
        let buffer_index = client
            .object_index(buffer_id)
            .ok_or(ProtocolError::Object)?;
        let Some(Object {
            kind: ObjectKind::Surface(mut surface),
            ..
        }) = client.objects[surface_index]
        else {
            return Err(ProtocolError::ObjectType);
        };
        let Some(Object {
            kind: ObjectKind::Buffer(mut buffer),
            ..
        }) = client.objects[buffer_index]
        else {
            return Err(ProtocolError::ObjectType);
        };
        let x_end = damage
            .x
            .checked_add(damage.width)
            .ok_or(ProtocolError::Bounds)?;
        let y_end = damage
            .y
            .checked_add(damage.height)
            .ok_or(ProtocolError::Bounds)?;
        if damage.width == 0
            || damage.height == 0
            || x_end > buffer.width
            || y_end > buffer.height
            || buffer.published
        {
            return Err(if buffer.published {
                ProtocolError::Busy
            } else {
                ProtocolError::Bounds
            });
        }
        buffer.published = true;
        surface.buffer_id = buffer_id;
        surface.damage = damage;
        client.objects[buffer_index] = Some(Object {
            id: buffer_id,
            kind: ObjectKind::Buffer(buffer),
        });
        client.objects[surface_index] = Some(Object {
            id: surface_id,
            kind: ObjectKind::Surface(surface),
        });
        Ok(())
    }

    pub fn compositor_buffer(
        &self,
        handle: ClientHandle,
        surface_id: u32,
    ) -> Result<(u32, u32, u32, Damage, u64), ProtocolError> {
        let client = self.client(handle)?;
        let surface = match client.objects[client
            .object_index(surface_id)
            .ok_or(ProtocolError::Object)?]
        {
            Some(Object {
                kind: ObjectKind::Surface(surface),
                ..
            }) => surface,
            _ => return Err(ProtocolError::ObjectType),
        };
        let buffer = match client.objects[client
            .object_index(surface.buffer_id)
            .ok_or(ProtocolError::Object)?]
        {
            Some(Object {
                kind: ObjectKind::Buffer(buffer),
                ..
            }) if buffer.published => buffer,
            _ => return Err(ProtocolError::Busy),
        };
        Ok((
            buffer.width,
            buffer.height,
            buffer.stride,
            surface.damage,
            buffer.content_generation,
        ))
    }

    pub fn complete_frame(
        &mut self,
        handle: ClientHandle,
        surface_id: u32,
    ) -> Result<(), ProtocolError> {
        let client = self.client_mut(handle)?;
        let surface_index = client
            .object_index(surface_id)
            .ok_or(ProtocolError::Object)?;
        let Some(Object {
            kind: ObjectKind::Surface(mut surface),
            ..
        }) = client.objects[surface_index]
        else {
            return Err(ProtocolError::ObjectType);
        };
        let buffer_index = client
            .object_index(surface.buffer_id)
            .ok_or(ProtocolError::Object)?;
        let Some(Object {
            kind: ObjectKind::Buffer(mut buffer),
            ..
        }) = client.objects[buffer_index]
        else {
            return Err(ProtocolError::ObjectType);
        };
        if !buffer.published {
            return Err(ProtocolError::Busy);
        }
        let required_events = if surface.callback_id == 0 { 1 } else { 2 };
        if MAX_EVENTS - client.event_count < required_events {
            return Err(ProtocolError::QueueFull);
        }
        buffer.published = false;
        client.objects[buffer_index] = Some(Object {
            id: surface.buffer_id,
            kind: ObjectKind::Buffer(buffer),
        });
        if surface.callback_id != 0 {
            client.push_event(EventKind::FrameDone, surface.callback_id, 0, 0)?;
            let callback_index = client
                .object_index(surface.callback_id)
                .ok_or(ProtocolError::Object)?;
            client.objects[callback_index] = None;
            surface.callback_id = 0;
        }
        client.push_event(EventKind::BufferRelease, surface.buffer_id, 0, 0)?;
        client.objects[surface_index] = Some(Object {
            id: surface_id,
            kind: ObjectKind::Surface(surface),
        });
        Ok(())
    }

    pub fn send_input(
        &mut self,
        handle: ClientHandle,
        surface_id: u32,
        kind: EventKind,
        value0: i32,
        value1: i32,
    ) -> Result<(), ProtocolError> {
        if !matches!(
            kind,
            EventKind::Focus | EventKind::Pointer | EventKind::Keyboard
        ) {
            return Err(ProtocolError::ObjectType);
        }
        let client = self.client_mut(handle)?;
        let index = client
            .object_index(surface_id)
            .ok_or(ProtocolError::Object)?;
        if !matches!(
            client.objects[index],
            Some(Object {
                kind: ObjectKind::Surface(_),
                ..
            })
        ) {
            return Err(ProtocolError::ObjectType);
        }
        client.push_event(kind, surface_id, value0, value1)
    }

    pub fn next_event(&mut self, handle: ClientHandle) -> Result<Option<Event>, ProtocolError> {
        Ok(self.client_mut(handle)?.pop_event())
    }

    pub fn destroy(&mut self, handle: ClientHandle, object_id: u32) -> Result<(), ProtocolError> {
        if object_id <= 2 {
            return Err(ProtocolError::Object);
        }
        let client = self.client_mut(handle)?;
        let index = client
            .object_index(object_id)
            .ok_or(ProtocolError::Object)?;
        if matches!(
            client.objects[index],
            Some(Object {
                kind: ObjectKind::Buffer(Buffer {
                    published: true,
                    ..
                }),
                ..
            })
        ) {
            return Err(ProtocolError::Busy);
        }
        client.objects[index] = None;
        Ok(())
    }
}

pub fn contract_probe() -> bool {
    let mut server = Server::new();
    let Ok(first) = server.connect(VERSION, SUPPORTED_FEATURES) else {
        return false;
    };
    let Ok(second) = server.connect(VERSION, FEATURE_BUFFERS) else {
        return false;
    };
    if server.create_surface(first, 3).is_err()
        || server.create_buffer(first, 4, 64, 32, 256).is_err()
        || server.request_frame(first, 3, 5).is_err()
        || server.mutate_buffer(first, 4) != Ok(1)
        || server
            .commit(
                first,
                3,
                4,
                Damage {
                    x: 0,
                    y: 0,
                    width: 64,
                    height: 32,
                },
            )
            .is_err()
        || server.mutate_buffer(first, 4) != Err(ProtocolError::Busy)
        || server.compositor_buffer(first, 3).is_err()
        || server
            .send_input(first, 3, EventKind::Keyboard, 30, 1)
            .is_err()
        || server.create_surface(second, 3).is_err()
        || server
            .send_input(second, 3, EventKind::Pointer, 1, 2)
            .is_err()
        || server.complete_frame(first, 3).is_err()
        || server.mutate_buffer(first, 4) != Ok(2)
    {
        return false;
    }
    let mut first_kinds = [EventKind::Configure; 4];
    for kind in &mut first_kinds {
        let Ok(Some(event)) = server.next_event(first) else {
            return false;
        };
        *kind = event.kind;
    }
    let Ok(Some(second_configure)) = server.next_event(second) else {
        return false;
    };
    let Ok(Some(second_pointer)) = server.next_event(second) else {
        return false;
    };
    first_kinds
        == [
            EventKind::Configure,
            EventKind::Keyboard,
            EventKind::FrameDone,
            EventKind::BufferRelease,
        ]
        && second_configure.kind == EventKind::Configure
        && second_pointer.kind == EventKind::Pointer
        && server.next_event(second) == Ok(None)
        && server.disconnect(first).is_ok()
        && server.next_event(first) == Err(ProtocolError::Capability)
        && server.next_event(second) == Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_probe_covers_two_isolated_clients() {
        assert!(contract_probe());
    }

    #[test]
    fn negotiation_and_wire_validation_fail_closed() {
        let mut server = Server::new();
        assert_eq!(server.connect(1, 0), Err(ProtocolError::Version));
        assert_eq!(
            server.connect(VERSION, 1 << 31),
            Err(ProtocolError::Feature)
        );
        let mut valid = [0_u8; HEADER_SIZE];
        valid[0..2].copy_from_slice(&(HEADER_SIZE as u16).to_le_bytes());
        valid[4..6].copy_from_slice(&OPCODE_CREATE_SURFACE.to_le_bytes());
        valid[8..12].copy_from_slice(&3_u32.to_le_bytes());
        assert!(Header::decode(&valid).is_ok());
        for mutation in [
            (0, 0_u8),
            (2, 1_u8),
            (4, 0xff_u8),
            (6, 1_u8),
            (8, 0_u8),
            (12, 1_u8),
        ] {
            let mut malformed = valid;
            malformed[mutation.0] = mutation.1;
            assert!(Header::decode(&malformed).is_err());
        }
        assert_eq!(Header::decode(&[]), Err(ProtocolError::Length));
    }

    #[test]
    fn dimensions_damage_lifetime_and_disconnect_are_bounded() {
        let mut server = Server::new();
        let first = server.connect(VERSION, SUPPORTED_FEATURES).unwrap();
        let second = server.connect(VERSION, SUPPORTED_FEATURES).unwrap();
        assert_eq!(
            server.create_buffer(first, 3, 0, 1, 4),
            Err(ProtocolError::Bounds)
        );
        assert_eq!(
            server.create_buffer(first, 3, MAX_WIDTH + 1, 1, 4),
            Err(ProtocolError::Bounds)
        );
        assert_eq!(
            server.create_surface(first, MAX_OBJECT_ID + 1),
            Err(ProtocolError::Object)
        );
        server.create_surface(first, 3).unwrap();
        server.create_buffer(first, 4, 16, 16, 64).unwrap();
        assert_eq!(
            server.create_buffer(first, 4, 16, 16, 64),
            Err(ProtocolError::Duplicate)
        );
        assert_eq!(
            server.commit(
                first,
                3,
                4,
                Damage {
                    x: 15,
                    y: 0,
                    width: 2,
                    height: 1
                }
            ),
            Err(ProtocolError::Bounds)
        );
        server
            .commit(
                first,
                3,
                4,
                Damage {
                    x: 0,
                    y: 0,
                    width: 16,
                    height: 16,
                },
            )
            .unwrap();
        assert_eq!(server.destroy(first, 4), Err(ProtocolError::Busy));
        assert_eq!(
            server.compositor_buffer(second, 3),
            Err(ProtocolError::Object)
        );
        server.disconnect(first).unwrap();
        assert_eq!(server.next_event(first), Err(ProtocolError::Capability));
        assert_eq!(server.next_event(second), Ok(None));
    }

    #[test]
    fn event_queue_is_bounded() {
        let mut server = Server::new();
        let client = server.connect(VERSION, SUPPORTED_FEATURES).unwrap();
        server.create_surface(client, 3).unwrap();
        for _ in 1..MAX_EVENTS {
            server
                .send_input(client, 3, EventKind::Keyboard, 30, 1)
                .unwrap();
        }
        assert_eq!(
            server.send_input(client, 3, EventKind::Keyboard, 30, 1),
            Err(ProtocolError::QueueFull)
        );
    }
}
