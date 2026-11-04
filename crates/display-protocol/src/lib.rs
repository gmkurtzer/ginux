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
pub const MAX_LEASE_TICKS: u64 = 60_000;
pub const LEASE_OBSERVE: u16 = 1 << 0;
pub const LEASE_EXCLUSIVE: u16 = 1 << 1;
pub const LEASE_RIGHTS: u16 = LEASE_OBSERVE | LEASE_EXCLUSIVE;

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
pub enum Resource {
    Display,
    Keyboard,
    Pointer,
}

impl Resource {
    const fn index(self) -> usize {
        match self {
            Self::Display => 0,
            Self::Keyboard => 1,
            Self::Pointer => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Revoker {
    TrustedController,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LeaseStatus {
    pub resource: Resource,
    pub rights: u16,
    pub generation: u32,
    pub owner_slot: u16,
    pub owner_generation: u32,
    pub surface_id: u32,
    pub expiry_tick: u64,
    pub revoker: Revoker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeatStatus {
    pub trusted_focus: bool,
    pub focus_owner_slot: u16,
    pub focus_owner_generation: u32,
    pub focus_surface_id: u32,
    pub pointer_captured: bool,
    pub lease_count: u8,
    pub transition_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionKind {
    Focus,
    Capture,
    Lease,
    Revoke,
    SecureAttention,
    OwnerDeath,
    Timeout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Transition {
    pub kind: TransitionKind,
    pub owner_slot: u16,
    pub owner_generation: u32,
    pub surface_id: u32,
    pub sequence: u64,
}

const EMPTY_TRANSITION: Transition = Transition {
    kind: TransitionKind::Revoke,
    owner_slot: 0,
    owner_generation: 0,
    surface_id: 0,
    sequence: 0,
};

#[derive(Clone, Copy)]
struct Focus {
    owner: ClientHandle,
    surface_id: u32,
}

#[derive(Clone, Copy)]
struct Lease {
    status: LeaseStatus,
    owner: ClientHandle,
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
    focus: Option<Focus>,
    pointer_capture: Option<Focus>,
    leases: [Option<Lease>; 3],
    lease_generations: [u32; 3],
    now: u64,
    history: [Transition; MAX_EVENTS],
    history_head: usize,
    history_count: usize,
    transition_sequence: u64,
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
            focus: None,
            pointer_capture: None,
            leases: [None; 3],
            lease_generations: [0; 3],
            now: 0,
            history: [EMPTY_TRANSITION; MAX_EVENTS],
            history_head: 0,
            history_count: 0,
            transition_sequence: 0,
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
        self.client(handle)?;
        self.revoke_owner(handle, TransitionKind::OwnerDeath);
        *self.client_mut(handle)? = Client::empty();
        Ok(())
    }

    fn validate_surface(&self, handle: ClientHandle, surface_id: u32) -> Result<(), ProtocolError> {
        let client = self.client(handle)?;
        let index = client
            .object_index(surface_id)
            .ok_or(ProtocolError::Object)?;
        if matches!(
            client.objects[index],
            Some(Object {
                kind: ObjectKind::Surface(_),
                ..
            })
        ) {
            Ok(())
        } else {
            Err(ProtocolError::ObjectType)
        }
    }

    fn record(&mut self, kind: TransitionKind, focus: Option<Focus>) {
        self.transition_sequence = self.transition_sequence.saturating_add(1);
        let focus = focus.unwrap_or(Focus {
            owner: ClientHandle {
                slot: 0,
                generation: 0,
            },
            surface_id: 0,
        });
        let transition = Transition {
            kind,
            owner_slot: focus.owner.slot,
            owner_generation: focus.owner.generation,
            surface_id: focus.surface_id,
            sequence: self.transition_sequence,
        };
        let slot = if self.history_count < MAX_EVENTS {
            let slot = (self.history_head + self.history_count) % MAX_EVENTS;
            self.history_count += 1;
            slot
        } else {
            let slot = self.history_head;
            self.history_head = (self.history_head + 1) % MAX_EVENTS;
            slot
        };
        self.history[slot] = transition;
    }

    fn lease_owned_by_other(&self, resource: Resource, owner: ClientHandle) -> bool {
        self.leases[resource.index()].is_some_and(|lease| lease.owner != owner)
    }

    pub fn focus_surface(
        &mut self,
        handle: ClientHandle,
        surface_id: u32,
    ) -> Result<(), ProtocolError> {
        self.validate_surface(handle, surface_id)?;
        if self.client(handle)?.event_count == MAX_EVENTS {
            return Err(ProtocolError::QueueFull);
        }
        if self.lease_owned_by_other(Resource::Display, handle)
            || self.lease_owned_by_other(Resource::Keyboard, handle)
        {
            return Err(ProtocolError::Busy);
        }
        let focus = Focus {
            owner: handle,
            surface_id,
        };
        self.focus = Some(focus);
        if self
            .pointer_capture
            .is_some_and(|capture| capture.owner != handle || capture.surface_id != surface_id)
        {
            self.pointer_capture = None;
        }
        self.record(TransitionKind::Focus, Some(focus));
        self.send_input(handle, surface_id, EventKind::Focus, 1, 0)
    }

    pub fn capture_pointer(
        &mut self,
        handle: ClientHandle,
        surface_id: u32,
    ) -> Result<(), ProtocolError> {
        self.validate_surface(handle, surface_id)?;
        if self
            .focus
            .is_none_or(|focus| focus.owner != handle || focus.surface_id != surface_id)
            || self.lease_owned_by_other(Resource::Pointer, handle)
        {
            return Err(ProtocolError::Capability);
        }
        let capture = Focus {
            owner: handle,
            surface_id,
        };
        self.pointer_capture = Some(capture);
        self.record(TransitionKind::Capture, Some(capture));
        Ok(())
    }

    pub fn grant_lease(
        &mut self,
        handle: ClientHandle,
        surface_id: u32,
        resource: Resource,
        rights: u16,
        duration_ticks: u64,
    ) -> Result<LeaseStatus, ProtocolError> {
        self.validate_surface(handle, surface_id)?;
        if rights == 0
            || rights & !LEASE_RIGHTS != 0
            || duration_ticks == 0
            || duration_ticks > MAX_LEASE_TICKS
        {
            return Err(ProtocolError::Bounds);
        }
        if self.leases[resource.index()].is_some() {
            return Err(ProtocolError::Busy);
        }
        if self
            .focus
            .is_none_or(|focus| focus.owner != handle || focus.surface_id != surface_id)
        {
            return Err(ProtocolError::Capability);
        }
        let generation = self.lease_generations[resource.index()]
            .checked_add(1)
            .filter(|generation| *generation != 0)
            .ok_or(ProtocolError::Capacity)?;
        let expiry_tick = self
            .now
            .checked_add(duration_ticks)
            .ok_or(ProtocolError::Bounds)?;
        self.lease_generations[resource.index()] = generation;
        let status = LeaseStatus {
            resource,
            rights,
            generation,
            owner_slot: handle.slot,
            owner_generation: handle.generation,
            surface_id,
            expiry_tick,
            revoker: Revoker::TrustedController,
        };
        self.leases[resource.index()] = Some(Lease {
            status,
            owner: handle,
        });
        self.record(
            TransitionKind::Lease,
            Some(Focus {
                owner: handle,
                surface_id,
            }),
        );
        Ok(status)
    }

    pub fn revoke_lease(
        &mut self,
        resource: Resource,
        generation: u32,
    ) -> Result<(), ProtocolError> {
        let lease = self.leases[resource.index()].ok_or(ProtocolError::Object)?;
        if lease.status.generation != generation {
            return Err(ProtocolError::Capability);
        }
        self.leases[resource.index()] = None;
        self.record(
            TransitionKind::Revoke,
            Some(Focus {
                owner: lease.owner,
                surface_id: lease.status.surface_id,
            }),
        );
        Ok(())
    }

    fn revoke_owner(&mut self, owner: ClientHandle, reason: TransitionKind) {
        let focus = self.focus.filter(|focus| focus.owner == owner);
        for lease in &mut self.leases {
            if lease.is_some_and(|lease| lease.owner == owner) {
                *lease = None;
            }
        }
        if focus.is_some() {
            self.focus = None;
        }
        if self
            .pointer_capture
            .is_some_and(|capture| capture.owner == owner)
        {
            self.pointer_capture = None;
        }
        self.record(reason, focus);
    }

    pub fn secure_attention(&mut self) {
        self.focus = None;
        self.pointer_capture = None;
        self.leases.fill(None);
        self.record(TransitionKind::SecureAttention, None);
    }

    pub fn advance_time(&mut self, now: u64) -> Result<(), ProtocolError> {
        if now < self.now {
            return Err(ProtocolError::Bounds);
        }
        self.now = now;
        let focused_owner = self.focus.map(|focus| focus.owner);
        let mut focused_lease_expired = false;
        for lease in &mut self.leases {
            if lease.is_some_and(|lease| lease.status.expiry_tick <= now) {
                focused_lease_expired |=
                    lease.is_some_and(|lease| Some(lease.owner) == focused_owner);
                *lease = None;
            }
        }
        if let (true, Some(owner)) = (focused_lease_expired, focused_owner) {
            self.revoke_owner(owner, TransitionKind::Timeout);
        }
        Ok(())
    }

    pub fn route_physical_input(
        &mut self,
        kind: EventKind,
        value0: i32,
        value1: i32,
    ) -> Result<bool, ProtocolError> {
        let target = match kind {
            EventKind::Pointer => self.pointer_capture.or(self.focus),
            EventKind::Keyboard => self.focus,
            _ => return Err(ProtocolError::ObjectType),
        };
        let Some(target) = target else {
            return Ok(false);
        };
        self.send_input(target.owner, target.surface_id, kind, value0, value1)?;
        Ok(true)
    }

    pub fn seat_status(&self) -> SeatStatus {
        let focus = self.focus;
        SeatStatus {
            trusted_focus: focus.is_none(),
            focus_owner_slot: focus.map_or(0, |focus| focus.owner.slot),
            focus_owner_generation: focus.map_or(0, |focus| focus.owner.generation),
            focus_surface_id: focus.map_or(0, |focus| focus.surface_id),
            pointer_captured: self.pointer_capture.is_some(),
            lease_count: self.leases.iter().filter(|lease| lease.is_some()).count() as u8,
            transition_sequence: self.transition_sequence,
        }
    }

    pub fn lease_status(&self, resource: Resource) -> Option<LeaseStatus> {
        self.leases[resource.index()].map(|lease| lease.status)
    }

    pub fn transition(&self, index: usize) -> Option<Transition> {
        (index < self.history_count).then(|| self.history[(self.history_head + index) % MAX_EVENTS])
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
        let was_focused_surface = {
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
            let was_surface = matches!(
                client.objects[index],
                Some(Object {
                    kind: ObjectKind::Surface(_),
                    ..
                })
            );
            client.objects[index] = None;
            was_surface
                && self
                    .focus
                    .is_some_and(|focus| focus.owner == handle && focus.surface_id == object_id)
        };
        if was_focused_surface {
            self.revoke_owner(handle, TransitionKind::OwnerDeath);
        }
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

pub fn seat_contract_probe() -> bool {
    let mut server = Server::new();
    let Ok(first) = server.connect(VERSION, SUPPORTED_FEATURES) else {
        return false;
    };
    let Ok(second) = server.connect(VERSION, SUPPORTED_FEATURES) else {
        return false;
    };
    if server.create_surface(first, 3).is_err()
        || server.create_surface(second, 3).is_err()
        || server.focus_surface(first, 3).is_err()
        || server.capture_pointer(first, 3).is_err()
    {
        return false;
    }
    let Ok(keyboard) = server.grant_lease(first, 3, Resource::Keyboard, LEASE_EXCLUSIVE, 10) else {
        return false;
    };
    if keyboard.owner_generation == 0
        || keyboard.expiry_tick != 10
        || server.focus_surface(second, 3) != Err(ProtocolError::Busy)
        || server.route_physical_input(EventKind::Keyboard, 30, 1) != Ok(true)
    {
        return false;
    }
    server.secure_attention();
    if !server.seat_status().trusted_focus
        || server.seat_status().lease_count != 0
        || server.route_physical_input(EventKind::Keyboard, 30, 1) != Ok(false)
        || server.revoke_lease(Resource::Keyboard, keyboard.generation)
            != Err(ProtocolError::Object)
        || server.focus_surface(second, 3).is_err()
        || server
            .grant_lease(second, 3, Resource::Pointer, LEASE_EXCLUSIVE, 5)
            .is_err()
        || server.advance_time(5).is_err()
        || !server.seat_status().trusted_focus
        || server.disconnect(second).is_err()
        || server.focus_surface(second, 3) != Err(ProtocolError::Capability)
    {
        return false;
    }
    server.focus_surface(first, 3).is_ok()
        && server.seat_status().focus_owner_generation != 0
        && server.transition(0).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_probe_covers_two_isolated_clients() {
        assert!(contract_probe());
        assert!(seat_contract_probe());
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

    #[test]
    fn secure_attention_timeout_owner_death_and_stale_handles_recover() {
        let mut server = Server::new();
        let first = server.connect(VERSION, SUPPORTED_FEATURES).unwrap();
        let second = server.connect(VERSION, SUPPORTED_FEATURES).unwrap();
        server.create_surface(first, 3).unwrap();
        server.create_surface(second, 3).unwrap();
        server.focus_surface(first, 3).unwrap();
        server.capture_pointer(first, 3).unwrap();
        let display = server
            .grant_lease(first, 3, Resource::Display, LEASE_EXCLUSIVE, 10)
            .unwrap();
        assert_eq!(server.focus_surface(second, 3), Err(ProtocolError::Busy));
        assert_eq!(
            server.route_physical_input(EventKind::Keyboard, 30, 1),
            Ok(true)
        );
        server.secure_attention();
        assert!(server.seat_status().trusted_focus);
        assert_eq!(server.seat_status().lease_count, 0);
        assert_eq!(
            server.revoke_lease(Resource::Display, display.generation),
            Err(ProtocolError::Object)
        );

        server.focus_surface(second, 3).unwrap();
        let pointer = server
            .grant_lease(second, 3, Resource::Pointer, LEASE_EXCLUSIVE, 5)
            .unwrap();
        assert_eq!(pointer.expiry_tick, 5);
        server.advance_time(5).unwrap();
        assert!(server.seat_status().trusted_focus);
        assert_eq!(server.seat_status().lease_count, 0);

        server.focus_surface(first, 3).unwrap();
        server.disconnect(first).unwrap();
        assert!(server.seat_status().trusted_focus);
        assert_eq!(
            server.route_physical_input(EventKind::Pointer, 1, 1),
            Ok(false)
        );
        assert_eq!(
            server.focus_surface(first, 3),
            Err(ProtocolError::Capability)
        );
        let mut event_count = 0;
        while let Some(event) = server.next_event(second).unwrap() {
            assert!(!matches!(
                event.kind,
                EventKind::Keyboard | EventKind::Pointer
            ));
            event_count += 1;
        }
        assert_eq!(event_count, 2);
    }

    #[test]
    fn concurrent_lease_requests_have_one_winner() {
        extern crate std;
        use std::sync::{Arc, Mutex};
        use std::thread;

        let mut server = Server::new();
        let first = server.connect(VERSION, SUPPORTED_FEATURES).unwrap();
        let second = server.connect(VERSION, SUPPORTED_FEATURES).unwrap();
        server.create_surface(first, 3).unwrap();
        server.create_surface(second, 3).unwrap();
        let server = Arc::new(Mutex::new(server));
        let contenders = [(server.clone(), first), (server.clone(), second)];
        let mut threads = std::vec::Vec::new();
        for (server, client) in contenders {
            threads.push(thread::spawn(move || {
                let mut server = server.lock().unwrap();
                server.focus_surface(client, 3).is_ok()
                    && server
                        .grant_lease(client, 3, Resource::Keyboard, LEASE_EXCLUSIVE, 10)
                        .is_ok()
            }));
        }
        let winners = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .filter(|won| *won)
            .count();
        assert_eq!(winners, 1);
        assert_eq!(server.lock().unwrap().seat_status().lease_count, 1);
    }
}
