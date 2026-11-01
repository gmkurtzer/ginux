#![no_std]

use ginux_native_abi::{
    NATIVE_DOMAIN_ATTACH, NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_CREATE,
    NATIVE_DOMAIN_EXEC, NATIVE_DOMAIN_QUERY, NATIVE_DOMAIN_RESTART, NATIVE_DOMAIN_SET_CPU,
    NATIVE_DOMAIN_SET_IMAGE, NATIVE_DOMAIN_SET_INIT, NATIVE_DOMAIN_SET_MEMORY_MIB,
    NATIVE_DOMAIN_START_ATTACHED, NATIVE_DOMAIN_STOP, NATIVE_HOST_INSPECT_OPERATION,
    NATIVE_NETWORK_INVENTORY_OPERATION, NATIVE_ROOT_OPERATION,
};
use ginux_windowing::{
    paint_border, paint_rect, paint_text_5x7_scaled, Rect, COLOR_ACCENT, COLOR_BORDER,
    COLOR_DARK_TEXT, COLOR_DESKTOP, COLOR_HEALTHY, COLOR_MUTED, COLOR_PANEL, COLOR_SURFACE,
    COLOR_SURFACE_RAISED, COLOR_TEXT,
};

pub const MAX_DOMAINS: usize = 4;
pub const MAX_JOBS: usize = 8;
pub const MAX_NOTICES: usize = 8;
pub const MAX_NAME: usize = 16;
const UI_SCALE: i16 = 1;
const LOGICAL_WIDTH: u16 = 400;
const LOGICAL_HEIGHT: u16 = 300;
pub const SHELL_WIDTH: usize = LOGICAL_WIDTH as usize * UI_SCALE as usize;
pub const SHELL_HEIGHT: usize = LOGICAL_HEIGHT as usize * UI_SCALE as usize;
pub const SHELL_BACKGROUND_COLOR: u32 = COLOR_DESKTOP;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Page {
    Overview,
    Domains,
    Network,
    Storage,
    Devices,
    Events,
    Settings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellState {
    pub page: Page,
    pub pointer_x: u16,
    pub pointer_y: u16,
    pub buttons: u8,
}

impl ShellState {
    pub const fn new() -> Self {
        Self {
            page: Page::Overview,
            pointer_x: 200,
            pointer_y: 150,
            buttons: 0,
        }
    }

    pub fn pointer_motion(&mut self, x: u16, y: u16) -> bool {
        let next = (
            x.min((SHELL_WIDTH - 1) as u16),
            y.min((SHELL_HEIGHT - 1) as u16),
        );
        if (self.pointer_x, self.pointer_y) == next {
            return false;
        }
        (self.pointer_x, self.pointer_y) = next;
        true
    }

    pub fn pointer_buttons(&mut self, changed: u8, buttons: u8) -> bool {
        self.buttons = buttons;
        if changed & 1 == 0 || buttons & 1 == 0 {
            return false;
        }
        let Some(page) = page_at(
            self.pointer_x / UI_SCALE as u16,
            self.pointer_y / UI_SCALE as u16,
        ) else {
            return false;
        };
        let changed = self.page != page;
        self.page = page;
        changed
    }
}

impl Default for ShellState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DomainView {
    pub name: [u8; MAX_NAME],
    pub name_len: u8,
    pub state: DomainState,
    pub cpu: u16,
    pub memory_mib: u32,
}

impl DomainView {
    pub const EMPTY: Self = Self {
        name: [0; MAX_NAME],
        name_len: 0,
        state: DomainState::Empty,
        cpu: 0,
        memory_mib: 0,
    };

    pub fn name(&self) -> &[u8] {
        &self.name[..usize::from(self.name_len)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemView {
    pub domains: [DomainView; MAX_DOMAINS],
    pub domain_count: u8,
    pub running_count: u8,
    pub cpu_allocated: u16,
    pub memory_used_mib: u32,
    pub memory_total_mib: u32,
    pub hour: u8,
    pub minute: u8,
    pub healthy: bool,
}

impl SystemView {
    pub const fn empty() -> Self {
        Self {
            domains: [DomainView::EMPTY; MAX_DOMAINS],
            domain_count: 0,
            running_count: 0,
            cpu_allocated: 0,
            memory_used_mib: 0,
            memory_total_mib: 0,
            hour: 0,
            minute: 0,
            healthy: false,
        }
    }

    fn model() -> Self {
        let mut view = Self::empty();
        view.domain_count = 1;
        view.running_count = 1;
        view.cpu_allocated = 2;
        view.memory_used_mib = 3_789;
        view.memory_total_mib = 32_666;
        view.hour = 10;
        view.minute = 24;
        view.healthy = true;
        view.domains[0] = DomainView {
            name: *b"rocky\0\0\0\0\0\0\0\0\0\0\0",
            name_len: 5,
            state: DomainState::Running,
            cpu: 2,
            memory_mib: 3_789,
        };
        view
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Capacity,
    NotFound,
    Invalid,
    Busy,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainState {
    Empty,
    Created,
    Configured,
    Starting,
    Running,
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    List,
    Create,
    SetImage,
    SetInit,
    SetCpu,
    SetMemory,
    Start,
    Stop,
    Restart,
    Attach,
    Exec,
    SystemStatus,
    DeviceStatus,
    NetworkStatus,
    StorageStatus,
    FocusStatus,
    LeaseStatus,
    Shutdown,
}

impl Operation {
    pub const fn abi(self) -> (u64, u64) {
        match self {
            Self::List => (NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_QUERY),
            Self::Create => (NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_CREATE),
            Self::SetImage => (NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_SET_IMAGE),
            Self::SetInit => (NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_SET_INIT),
            Self::SetCpu => (NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_SET_CPU),
            Self::SetMemory => (
                NATIVE_DOMAIN_CONTROL_OPERATION,
                NATIVE_DOMAIN_SET_MEMORY_MIB,
            ),
            Self::Start => (
                NATIVE_DOMAIN_CONTROL_OPERATION,
                NATIVE_DOMAIN_START_ATTACHED,
            ),
            Self::Stop => (NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_STOP),
            Self::Restart => (NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_RESTART),
            Self::Attach => (NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_ATTACH),
            Self::Exec => (NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_EXEC),
            Self::SystemStatus | Self::DeviceStatus => (NATIVE_HOST_INSPECT_OPERATION, 0),
            Self::NetworkStatus => (NATIVE_NETWORK_INVENTORY_OPERATION, 0),
            Self::StorageStatus => (NATIVE_ROOT_OPERATION, 0),
            Self::FocusStatus | Self::LeaseStatus => (NATIVE_HOST_INSPECT_OPERATION, 0),
            Self::Shutdown => (NATIVE_DOMAIN_CONTROL_OPERATION, 4),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    pub operation: Operation,
    pub name: [u8; MAX_NAME],
    pub name_len: u8,
    pub value: u64,
}

impl Request {
    pub fn new(operation: Operation, name: &[u8], value: u64) -> Result<Self, Error> {
        if name.len() > MAX_NAME {
            return Err(Error::Invalid);
        }
        let mut request = Self {
            operation,
            name: [0; MAX_NAME],
            name_len: name.len() as u8,
            value,
        };
        request.name[..name.len()].copy_from_slice(name);
        Ok(request)
    }
    fn name(&self) -> &[u8] {
        &self.name[..usize::from(self.name_len)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reply {
    pub changed: bool,
    pub domains: u8,
    pub state: DomainState,
}

pub trait ManagementBackend {
    fn execute(&mut self, request: Request) -> Result<Reply, Error>;
}

#[derive(Clone, Copy)]
struct Domain {
    name: [u8; MAX_NAME],
    name_len: u8,
    state: DomainState,
    image: bool,
    init: bool,
    cpu: u16,
    memory: u32,
}

impl Domain {
    const fn empty() -> Self {
        Self {
            name: [0; MAX_NAME],
            name_len: 0,
            state: DomainState::Empty,
            image: false,
            init: false,
            cpu: 1,
            memory: 128,
        }
    }
    fn named(&self, name: &[u8]) -> bool {
        self.state != DomainState::Empty && &self.name[..usize::from(self.name_len)] == name
    }
}

pub struct ModelBackend {
    domains: [Domain; MAX_DOMAINS],
    shutdown: bool,
}

impl ModelBackend {
    pub const fn new() -> Self {
        Self {
            domains: [Domain::empty(); MAX_DOMAINS],
            shutdown: false,
        }
    }
    fn index(&self, name: &[u8]) -> Option<usize> {
        self.domains.iter().position(|domain| domain.named(name))
    }
    pub fn state(&self, name: &[u8]) -> Option<DomainState> {
        self.index(name).map(|index| self.domains[index].state)
    }
    pub const fn shutdown_requested(&self) -> bool {
        self.shutdown
    }
}

impl Default for ModelBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagementBackend for ModelBackend {
    fn execute(&mut self, request: Request) -> Result<Reply, Error> {
        let count = self
            .domains
            .iter()
            .filter(|domain| domain.state != DomainState::Empty)
            .count() as u8;
        if matches!(
            request.operation,
            Operation::SystemStatus
                | Operation::DeviceStatus
                | Operation::NetworkStatus
                | Operation::StorageStatus
                | Operation::FocusStatus
                | Operation::LeaseStatus
                | Operation::List
        ) {
            return Ok(Reply {
                changed: false,
                domains: count,
                state: DomainState::Empty,
            });
        }
        if request.operation == Operation::Shutdown {
            self.shutdown = true;
            return Ok(Reply {
                changed: true,
                domains: count,
                state: DomainState::Empty,
            });
        }
        if request.operation == Operation::Create {
            if request.name().is_empty() || self.index(request.name()).is_some() {
                return Err(Error::Invalid);
            }
            let slot = self
                .domains
                .iter()
                .position(|domain| domain.state == DomainState::Empty)
                .ok_or(Error::Capacity)?;
            let mut domain = Domain::empty();
            domain.name_len = request.name_len;
            domain.name[..usize::from(request.name_len)].copy_from_slice(request.name());
            domain.state = DomainState::Created;
            self.domains[slot] = domain;
            return Ok(Reply {
                changed: true,
                domains: count + 1,
                state: DomainState::Created,
            });
        }
        let index = self.index(request.name()).ok_or(Error::NotFound)?;
        let domain = &mut self.domains[index];
        match request.operation {
            Operation::SetImage => domain.image = true,
            Operation::SetInit => domain.init = true,
            Operation::SetCpu if request.value > 0 && request.value <= u16::MAX as u64 => {
                domain.cpu = request.value as u16
            }
            Operation::SetMemory if request.value > 0 && request.value <= u32::MAX as u64 => {
                domain.memory = request.value as u32
            }
            Operation::Start if domain.image && domain.init => domain.state = DomainState::Running,
            Operation::Stop if domain.state == DomainState::Running => {
                domain.state = DomainState::Stopped
            }
            Operation::Restart
                if matches!(domain.state, DomainState::Running | DomainState::Stopped) =>
            {
                domain.state = DomainState::Running
            }
            Operation::Attach | Operation::Exec if domain.state == DomainState::Running => {}
            Operation::SetCpu | Operation::SetMemory => return Err(Error::Invalid),
            _ => return Err(Error::Busy),
        }
        if domain.image && domain.init && domain.state == DomainState::Created {
            domain.state = DomainState::Configured;
        }
        Ok(Reply {
            changed: true,
            domains: count,
            state: domain.state,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobState {
    Empty,
    Pending,
    Succeeded,
    Failed,
}

#[derive(Clone, Copy)]
struct Job {
    request: Request,
    state: JobState,
    error: Option<Error>,
}
impl Job {
    const fn empty() -> Self {
        Self {
            request: Request {
                operation: Operation::List,
                name: [0; MAX_NAME],
                name_len: 0,
                value: 0,
            },
            state: JobState::Empty,
            error: None,
        }
    }
}

pub struct Application<'a, B: ManagementBackend> {
    backend: &'a mut B,
    jobs: [Job; MAX_JOBS],
    head: u8,
    count: u8,
    notices: [JobState; MAX_NOTICES],
    notice_count: u8,
}

impl<'a, B: ManagementBackend> Application<'a, B> {
    pub fn new(backend: &'a mut B) -> Self {
        Self {
            backend,
            jobs: [Job::empty(); MAX_JOBS],
            head: 0,
            count: 0,
            notices: [JobState::Empty; MAX_NOTICES],
            notice_count: 0,
        }
    }
    pub fn submit(&mut self, request: Request) -> Result<u8, Error> {
        if usize::from(self.count) == MAX_JOBS {
            return Err(Error::Capacity);
        }
        let slot = (usize::from(self.head) + usize::from(self.count)) % MAX_JOBS;
        self.jobs[slot] = Job {
            request,
            state: JobState::Pending,
            error: None,
        };
        self.count += 1;
        Ok(slot as u8)
    }
    pub fn process_one(&mut self) -> bool {
        if self.count == 0 {
            return false;
        }
        let slot = usize::from(self.head);
        let result = self.backend.execute(self.jobs[slot].request);
        self.jobs[slot].state = if result.is_ok() {
            JobState::Succeeded
        } else {
            JobState::Failed
        };
        self.jobs[slot].error = result.err();
        if usize::from(self.notice_count) < MAX_NOTICES {
            self.notices[usize::from(self.notice_count)] = self.jobs[slot].state;
            self.notice_count += 1;
        }
        self.head = ((slot + 1) % MAX_JOBS) as u8;
        self.count -= 1;
        true
    }
    pub fn job_state(&self, slot: u8) -> Option<JobState> {
        self.jobs
            .get(usize::from(slot))
            .map(|job| job.state)
            .filter(|state| *state != JobState::Empty)
    }
    pub fn pending(&self) -> u8 {
        self.count
    }
    pub fn render_shell(&self, pixels: &mut [u32], stride: usize) -> Result<u64, Error> {
        let mut state = ShellState::new();
        state.pointer_x = 350;
        state.pointer_y = 230;
        self.render_shell_view(
            pixels,
            stride,
            b"controller terminal",
            &state,
            &SystemView::model(),
        )
    }

    pub fn render_shell_state(
        &self,
        pixels: &mut [u32],
        stride: usize,
        terminal: &[u8],
        pointer_x: u16,
        pointer_y: u16,
    ) -> Result<u64, Error> {
        let mut state = ShellState::new();
        state.pointer_x = pointer_x;
        state.pointer_y = pointer_y;
        self.render_shell_view(pixels, stride, terminal, &state, &SystemView::model())
    }

    pub fn render_shell_view(
        &self,
        pixels: &mut [u32],
        stride: usize,
        terminal: &[u8],
        state: &ShellState,
        view: &SystemView,
    ) -> Result<u64, Error> {
        rect(
            pixels,
            stride,
            Rect::new(0, 0, LOGICAL_WIDTH, LOGICAL_HEIGHT),
            COLOR_DESKTOP,
        )?;

        // Trusted system bar.
        rect(pixels, stride, Rect::new(0, 0, 400, 15), COLOR_PANEL)?;
        rect(pixels, stride, Rect::new(0, 14, 400, 1), COLOR_BORDER)?;
        text(pixels, stride, 7, 5, b"GINUX", COLOR_TEXT)?;
        text(
            pixels,
            stride,
            90,
            5,
            b"SYSTEM MANAGEMENT | HOST: GINUX",
            COLOR_MUTED,
        )?;
        let mut time = [b'0'; 5];
        time[0] = b'0' + view.hour / 10;
        time[1] = b'0' + view.hour % 10;
        time[2] = b':';
        time[3] = b'0' + view.minute / 10;
        time[4] = b'0' + view.minute % 10;
        text(pixels, stride, 310, 5, &time, COLOR_MUTED)?;
        rect(pixels, stride, Rect::new(341, 5, 5, 5), COLOR_HEALTHY)?;
        text(
            pixels,
            stride,
            350,
            5,
            if view.healthy {
                b"HEALTHY"
            } else {
                b"DEGRADED"
            },
            if view.healthy {
                COLOR_HEALTHY
            } else {
                COLOR_MUTED
            },
        )?;

        // Controller window and chrome.
        let shell = Rect::new(11, 29, 378, 247);
        rect(pixels, stride, shell, COLOR_PANEL)?;
        outline(pixels, stride, shell, COLOR_BORDER)?;
        rect(
            pixels,
            stride,
            Rect::new(12, 30, 376, 14),
            COLOR_SURFACE_RAISED,
        )?;
        text(pixels, stride, 17, 35, b"GINUX CONTROLLER", COLOR_TEXT)?;
        text(pixels, stride, 347, 35, b"-  O  X", COLOR_MUTED)?;

        // Navigation rail.
        rect(pixels, stride, Rect::new(12, 44, 68, 206), COLOR_PANEL)?;
        rect(pixels, stride, Rect::new(79, 44, 1, 206), COLOR_BORDER)?;
        for (page, y, icon, label) in [
            (Page::Overview, 55, 0, &b"OVERVIEW"[..]),
            (Page::Domains, 77, 1, &b"DOMAINS"[..]),
            (Page::Network, 101, 2, &b"NETWORK"[..]),
            (Page::Storage, 125, 3, &b"STORAGE"[..]),
            (Page::Devices, 149, 4, &b"DEVICES"[..]),
            (Page::Events, 173, 5, &b"EVENTS"[..]),
            (Page::Settings, 197, 6, &b"SETTINGS"[..]),
        ] {
            let selected = state.page == page;
            if selected {
                rect(pixels, stride, Rect::new(13, y - 7, 66, 18), COLOR_ACCENT)?;
            }
            let color = if selected { COLOR_TEXT } else { COLOR_MUTED };
            nav_icon(pixels, stride, 19, y - 2, icon, color)?;
            text(pixels, stride, 33, y, label, color)?;
        }

        if state.page == Page::Overview {
            self.render_overview(pixels, stride, view)?;
        } else {
            render_page(pixels, stride, state.page, view)?;
        }

        // Command surface.
        let command = Rect::new(16, 257, 368, 14);
        rect(pixels, stride, command, COLOR_SURFACE)?;
        outline(pixels, stride, command, COLOR_ACCENT)?;
        text(pixels, stride, 22, 262, b"GINUX>", COLOR_MUTED)?;
        text(pixels, stride, 50, 262, terminal, COLOR_TEXT)?;
        rect(
            pixels,
            stride,
            Rect::new(50 + terminal.len() as i16 * 6, 261, 2, 7),
            COLOR_TEXT,
        )?;

        draw_pointer(
            pixels,
            stride,
            state.pointer_x.min((SHELL_WIDTH - 1) as u16) as i16,
            state.pointer_y.min((SHELL_HEIGHT - 1) as u16) as i16,
        )?;
        Ok(ginux_windowing::pixel_hash(pixels))
    }

    fn render_overview(
        &self,
        pixels: &mut [u32],
        stride: usize,
        view: &SystemView,
    ) -> Result<(), Error> {
        text(pixels, stride, 91, 51, b"SYSTEM OVERVIEW", COLOR_TEXT)?;
        let mut domain_value = [0_u8; 3];
        let domain_value = decimal(&mut domain_value, u64::from(view.domain_count));
        let mut domain_detail = [0_u8; 20];
        let domain_detail = count_detail(
            &mut domain_detail,
            u64::from(view.running_count),
            u64::from(view.domain_count.saturating_sub(view.running_count)),
        );
        card(
            pixels,
            stride,
            Rect::new(89, 71, 94, 40),
            1,
            b"DOMAINS",
            domain_value,
            domain_detail,
        )?;
        let mut cpu = [0_u8; 6];
        let cpu = decimal_suffix(&mut cpu, u64::from(view.cpu_allocated), b" CPU");
        card(
            pixels,
            stride,
            Rect::new(190, 71, 93, 40),
            4,
            b"ALLOCATED",
            cpu,
            b"CONFIGURED",
        )?;
        let mut memory = [0_u8; 8];
        let memory = decimal_suffix(&mut memory, u64::from(view.memory_used_mib), b"M");
        let mut memory_detail = [0_u8; 16];
        let memory_detail = total_detail(&mut memory_detail, u64::from(view.memory_total_mib));
        card(
            pixels,
            stride,
            Rect::new(290, 71, 94, 40),
            3,
            b"MEMORY",
            memory,
            memory_detail,
        )?;

        // Domain inventory.
        let table = Rect::new(89, 119, 295, 132);
        rect(pixels, stride, table, COLOR_PANEL)?;
        outline(pixels, stride, table, COLOR_BORDER)?;
        rect(
            pixels,
            stride,
            Rect::new(90, 120, 293, 13),
            COLOR_SURFACE_RAISED,
        )?;
        text(pixels, stride, 96, 124, b"DOMAINS", COLOR_TEXT)?;
        rect(pixels, stride, Rect::new(90, 133, 293, 15), COLOR_SURFACE)?;
        for x in [181, 264, 314] {
            rect(pixels, stride, Rect::new(x, 133, 1, 30), COLOR_BORDER)?;
        }
        text(pixels, stride, 96, 138, b"NAME", COLOR_MUTED)?;
        text(pixels, stride, 187, 138, b"STATE", COLOR_MUTED)?;
        text(pixels, stride, 270, 138, b"CPU", COLOR_MUTED)?;
        text(pixels, stride, 320, 138, b"MEMORY", COLOR_MUTED)?;
        rect(pixels, stride, Rect::new(90, 148, 293, 1), COLOR_BORDER)?;
        for (index, domain) in view.domains[..usize::from(view.domain_count.min(MAX_DOMAINS as u8))]
            .iter()
            .enumerate()
        {
            let y = 154 + index as i16 * 14;
            text(pixels, stride, 96, y, domain.name(), COLOR_TEXT)?;
            let state = domain_state_text(domain.state);
            rect(
                pixels,
                stride,
                Rect::new(187, y, 5, 5),
                if domain.state == DomainState::Running {
                    COLOR_HEALTHY
                } else {
                    COLOR_MUTED
                },
            )?;
            text(pixels, stride, 197, y, state, COLOR_TEXT)?;
            let mut cpu = [0_u8; 5];
            let cpu = decimal(&mut cpu, u64::from(domain.cpu));
            text(pixels, stride, 270, y, cpu, COLOR_TEXT)?;
            let mut memory = [0_u8; 8];
            let memory = decimal_suffix(&mut memory, u64::from(domain.memory_mib), b"M");
            text(pixels, stride, 320, y, memory, COLOR_TEXT)?;
        }
        Ok(())
    }
}

fn page_at(x: u16, y: u16) -> Option<Page> {
    if (89..183).contains(&x) && (71..111).contains(&y) {
        return Some(Page::Domains);
    }
    if !(13..79).contains(&x) {
        return None;
    }
    [
        (Page::Overview, 48_u16),
        (Page::Domains, 70),
        (Page::Network, 94),
        (Page::Storage, 118),
        (Page::Devices, 142),
        (Page::Events, 166),
        (Page::Settings, 190),
    ]
    .into_iter()
    .find_map(|(page, top)| (y >= top && y < top + 18).then_some(page))
}

fn page_title(page: Page) -> &'static [u8] {
    match page {
        Page::Overview => b"SYSTEM OVERVIEW",
        Page::Domains => b"DOMAINS",
        Page::Network => b"NETWORK",
        Page::Storage => b"STORAGE",
        Page::Devices => b"DEVICES",
        Page::Events => b"EVENTS",
        Page::Settings => b"SETTINGS",
    }
}

fn render_page(
    pixels: &mut [u32],
    stride: usize,
    page: Page,
    view: &SystemView,
) -> Result<(), Error> {
    text(pixels, stride, 91, 51, page_title(page), COLOR_TEXT)?;
    let panel = Rect::new(89, 71, 295, 180);
    rect(pixels, stride, panel, COLOR_PANEL)?;
    outline(pixels, stride, panel, COLOR_BORDER)?;
    text(
        pixels,
        stride,
        98,
        82,
        b"LIVE CONTROLLER VIEW",
        COLOR_HEALTHY,
    )?;
    let (label, value) = match page {
        Page::Domains => (b"CONFIGURED DOMAINS".as_slice(), view.domain_count),
        Page::Network => (b"HOST NETWORK STATUS".as_slice(), u8::from(view.healthy)),
        Page::Storage => (b"LOCAL STORAGE STATUS".as_slice(), u8::from(view.healthy)),
        Page::Devices => (b"DEVICE SERVICE STATUS".as_slice(), u8::from(view.healthy)),
        Page::Events => (b"ACTIVE EVENT COUNT".as_slice(), 0),
        Page::Settings => (b"SYSTEM SETTINGS".as_slice(), 1),
        Page::Overview => (b"CONFIGURED DOMAINS".as_slice(), view.domain_count),
    };
    text(pixels, stride, 98, 101, label, COLOR_MUTED)?;
    let mut numeric = [0_u8; 3];
    let numeric = decimal(&mut numeric, u64::from(value));
    text(pixels, stride, 98, 113, numeric, COLOR_TEXT)?;
    if page == Page::Settings {
        text(
            pixels,
            stride,
            98,
            142,
            b"DATE, TIME, AND TIMEZONE",
            COLOR_TEXT,
        )?;
        text(
            pixels,
            stride,
            98,
            154,
            b"CONFIGURATION INTERFACE PENDING",
            COLOR_MUTED,
        )?;
    } else {
        text(
            pixels,
            stride,
            98,
            142,
            b"STATE REFRESHES FROM THE KERNEL",
            COLOR_MUTED,
        )?;
    }
    Ok(())
}

fn decimal(output: &mut [u8], mut value: u64) -> &[u8] {
    let mut reversed = [0_u8; 20];
    let mut count = 0;
    loop {
        reversed[count] = b'0' + (value % 10) as u8;
        count += 1;
        value /= 10;
        if value == 0 || count == reversed.len() {
            break;
        }
    }
    let count = count.min(output.len());
    for index in 0..count {
        output[index] = reversed[count - index - 1];
    }
    &output[..count]
}

fn decimal_suffix<'a>(output: &'a mut [u8], value: u64, suffix: &[u8]) -> &'a [u8] {
    let digits = decimal(output, value).len();
    let suffix_len = suffix.len().min(output.len().saturating_sub(digits));
    output[digits..digits + suffix_len].copy_from_slice(&suffix[..suffix_len]);
    &output[..digits + suffix_len]
}

fn count_detail(output: &mut [u8], running: u64, stopped: u64) -> &[u8] {
    let mut length = decimal(output, running).len();
    let separator = b" RUN ";
    let count = separator.len().min(output.len().saturating_sub(length));
    output[length..length + count].copy_from_slice(&separator[..count]);
    length += count;
    length += decimal(&mut output[length..], stopped).len();
    let suffix = b" STOP";
    let count = suffix.len().min(output.len().saturating_sub(length));
    output[length..length + count].copy_from_slice(&suffix[..count]);
    &output[..length + count]
}

fn total_detail(output: &mut [u8], total: u64) -> &[u8] {
    output[..3].copy_from_slice(b"OF ");
    let mut length = 3 + decimal(&mut output[3..], total).len();
    let suffix = b" MIB";
    let count = suffix.len().min(output.len().saturating_sub(length));
    output[length..length + count].copy_from_slice(&suffix[..count]);
    length += count;
    &output[..length]
}

fn domain_state_text(state: DomainState) -> &'static [u8] {
    match state {
        DomainState::Created => b"CREATED",
        DomainState::Configured => b"CONFIGURED",
        DomainState::Starting => b"STARTING",
        DomainState::Running => b"RUNNING",
        DomainState::Stopped => b"STOPPED",
        DomainState::Failed => b"FAILED",
        DomainState::Empty => b"EMPTY",
    }
}

fn rect(pixels: &mut [u32], stride: usize, area: Rect, color: u32) -> Result<(), Error> {
    paint_rect(pixels, stride, scaled_rect(area), color).map_err(|_| Error::Failed)
}

fn outline(pixels: &mut [u32], stride: usize, area: Rect, color: u32) -> Result<(), Error> {
    paint_border(pixels, stride, scaled_rect(area), color).map_err(|_| Error::Failed)
}

fn scaled_rect(area: Rect) -> Rect {
    Rect::new(
        area.x * UI_SCALE,
        area.y * UI_SCALE,
        area.width * UI_SCALE as u16,
        area.height * UI_SCALE as u16,
    )
}

fn text(
    pixels: &mut [u32],
    stride: usize,
    x: i16,
    y: i16,
    value: &[u8],
    color: u32,
) -> Result<(), Error> {
    paint_text_5x7_scaled(pixels, stride, x * UI_SCALE, y * UI_SCALE, value, color, 1)
        .map_err(|_| Error::Failed)
}

fn card(
    pixels: &mut [u32],
    stride: usize,
    area: Rect,
    icon: u8,
    label: &[u8],
    value: &[u8],
    detail: &[u8],
) -> Result<(), Error> {
    rect(pixels, stride, area, COLOR_SURFACE_RAISED)?;
    outline(pixels, stride, area, COLOR_BORDER)?;
    nav_icon(pixels, stride, area.x + 7, area.y + 13, icon, COLOR_TEXT)?;
    text(pixels, stride, area.x + 27, area.y + 9, label, COLOR_MUTED)?;
    text(pixels, stride, area.x + 27, area.y + 17, value, COLOR_TEXT)?;
    text(pixels, stride, area.x + 7, area.y + 32, detail, COLOR_MUTED)
}

fn nav_icon(
    pixels: &mut [u32],
    stride: usize,
    x: i16,
    y: i16,
    kind: u8,
    color: u32,
) -> Result<(), Error> {
    match kind {
        0 => {
            for (dx, dy) in [(0, 0), (5, 0), (0, 5), (5, 5)] {
                outline(pixels, stride, Rect::new(x + dx, y + dy, 4, 4), color)?;
            }
        }
        1 => {
            outline(pixels, stride, Rect::new(x + 1, y, 8, 10), color)?;
            rect(pixels, stride, Rect::new(x, y + 3, 10, 1), color)?;
        }
        2 => {
            for (dx, dy) in [(0, 7), (5, 0), (10, 7)] {
                outline(pixels, stride, Rect::new(x + dx, y + dy, 3, 3), color)?;
            }
            rect(pixels, stride, Rect::new(x + 6, y + 3, 1, 5), color)?;
            rect(pixels, stride, Rect::new(x + 2, y + 7, 9, 1), color)?;
        }
        3 => {
            outline(pixels, stride, Rect::new(x, y + 1, 11, 8), color)?;
            rect(pixels, stride, Rect::new(x + 2, y + 4, 7, 1), color)?;
        }
        4 => {
            outline(pixels, stride, Rect::new(x + 2, y + 2, 7, 7), color)?;
            for (dx, dy) in [(0, 4), (10, 4), (5, 0), (5, 10)] {
                rect(pixels, stride, Rect::new(x + dx, y + dy, 2, 2), color)?;
            }
        }
        5 => {
            outline(pixels, stride, Rect::new(x, y, 10, 11), color)?;
            rect(pixels, stride, Rect::new(x + 3, y + 3, 1, 1), color)?;
            rect(pixels, stride, Rect::new(x + 3, y + 7, 1, 1), color)?;
            rect(pixels, stride, Rect::new(x + 6, y + 3, 2, 1), color)?;
            rect(pixels, stride, Rect::new(x + 6, y + 7, 2, 1), color)?;
        }
        _ => {
            outline(pixels, stride, Rect::new(x + 1, y + 1, 9, 9), color)?;
            rect(pixels, stride, Rect::new(x + 5, y + 3, 1, 5), color)?;
            rect(pixels, stride, Rect::new(x + 3, y + 5, 5, 1), color)?;
        }
    }
    Ok(())
}

fn draw_pointer(pixels: &mut [u32], stride: usize, x: i16, y: i16) -> Result<(), Error> {
    for (dx, dy) in [
        (0, 0),
        (0, 1),
        (0, 2),
        (0, 3),
        (0, 4),
        (0, 5),
        (0, 6),
        (0, 7),
        (0, 8),
        (1, 1),
        (1, 2),
        (1, 3),
        (1, 4),
        (1, 5),
        (1, 6),
        (1, 7),
        (2, 2),
        (2, 3),
        (2, 4),
        (2, 5),
        (2, 6),
        (3, 3),
        (3, 4),
        (3, 5),
        (4, 4),
        (4, 5),
        (5, 5),
    ] {
        paint_rect(
            pixels,
            stride,
            Rect::new(x + dx * 2, y + dy * 2, 2, 2),
            COLOR_DARK_TEXT,
        )
        .map_err(|_| Error::Failed)?;
    }
    for (dx, dy) in [(1, 2), (1, 3), (1, 4), (2, 3), (2, 4), (3, 4)] {
        paint_rect(
            pixels,
            stride,
            Rect::new(x + dx * 2, y + dy * 2, 2, 2),
            COLOR_TEXT,
        )
        .map_err(|_| Error::Failed)?;
    }
    Ok(())
}

pub fn cli_execute<B: ManagementBackend>(
    backend: &mut B,
    request: Request,
) -> Result<Reply, Error> {
    backend.execute(request)
}

pub fn contract_probe() -> bool {
    let mut backend = ModelBackend::new();
    {
        let mut app = Application::new(&mut backend);
        for request in [
            Request::new(Operation::Create, b"rocky", 0).unwrap(),
            Request::new(Operation::SetImage, b"rocky", 0).unwrap(),
            Request::new(Operation::SetInit, b"rocky", 0).unwrap(),
            Request::new(Operation::SetCpu, b"rocky", 2).unwrap(),
            Request::new(Operation::SetMemory, b"rocky", 2048).unwrap(),
            Request::new(Operation::Start, b"rocky", 0).unwrap(),
            Request::new(Operation::Exec, b"rocky", 0).unwrap(),
        ] {
            if app.submit(request).is_err() || !app.process_one() {
                return false;
            }
        }
        let failed = app
            .submit(Request::new(Operation::Start, b"missing", 0).unwrap())
            .unwrap();
        if !app.process_one()
            || app.job_state(failed) != Some(JobState::Failed)
            || app.pending() != 0
        {
            return false;
        }
    }
    if backend.state(b"rocky") != Some(DomainState::Running) {
        return false;
    }
    cli_execute(
        &mut backend,
        Request::new(Operation::Stop, b"rocky", 0).unwrap(),
    )
    .is_ok()
        && backend.state(b"rocky") == Some(DomainState::Stopped)
        && Operation::NetworkStatus.abi().0 == NATIVE_NETWORK_INVENTORY_OPERATION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_workflow_is_nonblocking_and_reports_each_result() {
        let mut backend = ModelBackend::new();
        let mut app = Application::new(&mut backend);
        let create = app
            .submit(Request::new(Operation::Create, b"rocky", 0).unwrap())
            .unwrap();
        let failure = app
            .submit(Request::new(Operation::Start, b"missing", 0).unwrap())
            .unwrap();
        assert_eq!(app.pending(), 2);
        assert_eq!(app.job_state(create), Some(JobState::Pending));
        assert!(app.process_one());
        assert_eq!(app.job_state(create), Some(JobState::Succeeded));
        assert!(app.process_one());
        assert_eq!(app.job_state(failure), Some(JobState::Failed));
    }

    #[test]
    fn cli_and_gui_share_backend_and_gui_death_preserves_state() {
        let mut backend = ModelBackend::new();
        {
            let mut gui = Application::new(&mut backend);
            for operation in [
                Operation::Create,
                Operation::SetImage,
                Operation::SetInit,
                Operation::Start,
            ] {
                gui.submit(Request::new(operation, b"rocky", 0).unwrap())
                    .unwrap();
                assert!(gui.process_one());
            }
        }
        assert_eq!(backend.state(b"rocky"), Some(DomainState::Running));
        cli_execute(
            &mut backend,
            Request::new(Operation::Stop, b"rocky", 0).unwrap(),
        )
        .unwrap();
        assert_eq!(backend.state(b"rocky"), Some(DomainState::Stopped));
        cli_execute(
            &mut backend,
            Request::new(Operation::Restart, b"rocky", 0).unwrap(),
        )
        .unwrap();
        assert_eq!(backend.state(b"rocky"), Some(DomainState::Running));
    }

    #[test]
    fn all_observation_and_domain_operations_map_to_versioned_native_abi() {
        for operation in [
            Operation::List,
            Operation::Create,
            Operation::SetImage,
            Operation::SetInit,
            Operation::SetCpu,
            Operation::SetMemory,
            Operation::Start,
            Operation::Stop,
            Operation::Restart,
            Operation::Attach,
            Operation::Exec,
            Operation::SystemStatus,
            Operation::DeviceStatus,
            Operation::NetworkStatus,
            Operation::StorageStatus,
            Operation::FocusStatus,
            Operation::LeaseStatus,
            Operation::Shutdown,
        ] {
            assert_ne!(operation.abi().0, 0);
        }
        assert!(contract_probe());
    }

    #[test]
    fn graphical_shell_has_stable_exact_pixels() {
        let mut backend = ModelBackend::new();
        let app = Application::new(&mut backend);
        let mut pixels = [0_u32; SHELL_WIDTH * SHELL_HEIGHT];
        let hash = app.render_shell(&mut pixels, SHELL_WIDTH).unwrap();
        assert_ne!(hash, 0);
        assert_eq!(pixels[29 * SHELL_WIDTH + 11], COLOR_BORDER);
        assert_eq!(pixels[30 * SHELL_WIDTH + 12], COLOR_SURFACE_RAISED);
        assert_eq!(pixels[48 * SHELL_WIDTH + 13], COLOR_ACCENT);
        assert_eq!(pixels[71 * SHELL_WIDTH + 89], COLOR_BORDER);
        assert_eq!(pixels[72 * SHELL_WIDTH + 90], COLOR_SURFACE_RAISED);
        assert_eq!(hash, app.render_shell(&mut pixels, SHELL_WIDTH).unwrap());
    }

    #[test]
    fn pointer_click_selects_navigation_once() {
        let mut state = ShellState::new();
        assert!(state.pointer_motion(45, 197));
        assert!(state.pointer_buttons(1, 1));
        assert_eq!(state.page, Page::Settings);
        assert!(!state.pointer_buttons(1, 0));
        assert!(!state.pointer_buttons(0, 0));
        assert_eq!(state.page, Page::Settings);
    }

    #[test]
    fn live_view_changes_rendered_frame() {
        let mut backend = ModelBackend::new();
        let app = Application::new(&mut backend);
        let state = ShellState::new();
        let mut pixels = [0_u32; SHELL_WIDTH * SHELL_HEIGHT];
        let empty = SystemView::empty();
        let empty_hash = app
            .render_shell_view(&mut pixels, SHELL_WIDTH, b"", &state, &empty)
            .unwrap();
        let mut live = empty;
        live.healthy = true;
        live.domain_count = 1;
        live.running_count = 1;
        live.cpu_allocated = 2;
        live.memory_used_mib = 512;
        live.memory_total_mib = 2_048;
        live.domains[0] = DomainView {
            name: *b"rocky\0\0\0\0\0\0\0\0\0\0\0",
            name_len: 5,
            state: DomainState::Running,
            cpu: 2,
            memory_mib: 512,
        };
        let live_hash = app
            .render_shell_view(&mut pixels, SHELL_WIDTH, b"", &state, &live)
            .unwrap();
        assert_ne!(empty_hash, live_hash);
    }
}
