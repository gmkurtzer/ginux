#![no_std]

use ginux_native_abi::{
    NATIVE_DOMAIN_ATTACH, NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_CREATE,
    NATIVE_DOMAIN_EXEC, NATIVE_DOMAIN_HOST_REBOOT, NATIVE_DOMAIN_HOST_SHUTDOWN,
    NATIVE_DOMAIN_QUERY, NATIVE_DOMAIN_RESTART, NATIVE_DOMAIN_SET_CPU, NATIVE_DOMAIN_SET_IMAGE,
    NATIVE_DOMAIN_SET_INIT, NATIVE_DOMAIN_SET_MEMORY_MIB, NATIVE_DOMAIN_START_ATTACHED,
    NATIVE_DOMAIN_STOP, NATIVE_HOST_INSPECT_OPERATION, NATIVE_NETWORK_INVENTORY_OPERATION,
    NATIVE_ROOT_OPERATION,
};
use ginux_windowing::{
    paint_border, paint_rect, paint_text_inter_16, Rect, COLOR_ACCENT, COLOR_BORDER,
    COLOR_DARK_TEXT, COLOR_DESKTOP, COLOR_HEALTHY, COLOR_MUTED, COLOR_PANEL, COLOR_SURFACE,
    COLOR_SURFACE_RAISED, COLOR_TEXT,
};

pub const MAX_DOMAINS: usize = 4;
pub const MAX_JOBS: usize = 8;
pub const MAX_NOTICES: usize = 8;
pub const MAX_NAME: usize = 16;
const LOGICAL_WIDTH: u16 = 1024;
const LOGICAL_HEIGHT: u16 = 768;
const DISPLAY_SCALE: u16 = 1;
pub const SHELL_WIDTH: usize = LOGICAL_WIDTH as usize * DISPLAY_SCALE as usize;
pub const SHELL_HEIGHT: usize = LOGICAL_HEIGHT as usize * DISPLAY_SCALE as usize;
pub const SHELL_BACKGROUND_COLOR: u32 = COLOR_DESKTOP;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    pub desktop: u32,
    pub panel: u32,
    pub surface: u32,
    pub raised: u32,
    pub border: u32,
    pub accent: u32,
    pub text: u32,
    pub muted: u32,
    pub healthy: u32,
    pub danger: u32,
}

pub const DEFAULT_THEME: Theme = Theme {
    desktop: COLOR_DESKTOP,
    panel: COLOR_PANEL,
    surface: COLOR_SURFACE,
    raised: COLOR_SURFACE_RAISED,
    border: COLOR_BORDER,
    accent: COLOR_ACCENT,
    text: COLOR_TEXT,
    muted: COLOR_MUTED,
    healthy: COLOR_HEALTHY,
    danger: 0x00e1_5b64,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    pub width: u16,
    pub height: u16,
    pub top_bar_height: u16,
    pub sidebar_width: u16,
    pub content_left: i16,
    pub content_top: i16,
    pub content_width: u16,
    pub content_bottom: i16,
    pub nav_left: i16,
    pub nav_width: u16,
    pub nav_first_top: u16,
    pub nav_step: u16,
}

pub const DEFAULT_LAYOUT: Layout = Layout {
    width: LOGICAL_WIDTH,
    height: LOGICAL_HEIGHT,
    top_bar_height: 52,
    sidebar_width: 220,
    content_left: 252,
    content_top: 82,
    content_width: 744,
    content_bottom: 738,
    nav_left: 16,
    nav_width: 188,
    nav_first_top: 94,
    nav_step: 56,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerAction {
    Reboot,
    Shutdown,
}

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
    pub power_menu: bool,
    pub power_confirmation: Option<PowerAction>,
    pub layout: Layout,
    power_action: Option<PowerAction>,
}

impl ShellState {
    pub const fn new() -> Self {
        Self {
            page: Page::Overview,
            pointer_x: (SHELL_WIDTH / 2) as u16,
            pointer_y: (SHELL_HEIGHT / 2) as u16,
            buttons: 0,
            power_menu: false,
            power_confirmation: None,
            layout: DEFAULT_LAYOUT,
            power_action: None,
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
        let (x, y) = (
            self.pointer_x / DISPLAY_SCALE,
            self.pointer_y / DISPLAY_SCALE,
        );
        let power_left = self.layout.width.saturating_sub(112);
        if (power_left..self.layout.width).contains(&x)
            && (0..self.layout.top_bar_height).contains(&y)
        {
            self.power_menu = !self.power_menu;
            self.power_confirmation = None;
            return true;
        }
        if let Some(action) = self.power_confirmation {
            if (376..488).contains(&x) && (410..448).contains(&y) {
                self.power_confirmation = None;
                return true;
            }
            if (536..648).contains(&x) && (410..448).contains(&y) {
                self.power_action = Some(action);
                self.power_confirmation = None;
                self.power_menu = false;
                return true;
            }
            return false;
        }
        if self.power_menu {
            let action = if (840..1008).contains(&x) && (54..98).contains(&y) {
                Some(PowerAction::Reboot)
            } else if (840..1008).contains(&x) && (98..142).contains(&y) {
                Some(PowerAction::Shutdown)
            } else {
                None
            };
            if let Some(action) = action {
                self.power_confirmation = Some(action);
                return true;
            }
            self.power_menu = false;
        }
        let Some(page) = page_at(x, y, &self.layout) else {
            return false;
        };
        let changed = self.page != page;
        self.page = page;
        changed
    }

    pub fn take_power_action(&mut self) -> Option<PowerAction> {
        self.power_action.take()
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
    Reboot,
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
            Self::Reboot => (NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_HOST_REBOOT),
            Self::Shutdown => (NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_HOST_SHUTDOWN),
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
        self.render_shell_view(pixels, stride, &state, &SystemView::model())
    }

    pub fn render_shell_state(
        &self,
        pixels: &mut [u32],
        stride: usize,
        pointer_x: u16,
        pointer_y: u16,
    ) -> Result<u64, Error> {
        let mut state = ShellState::new();
        state.pointer_x = pointer_x;
        state.pointer_y = pointer_y;
        self.render_shell_view(pixels, stride, &state, &SystemView::model())
    }

    pub fn render_shell_view(
        &self,
        pixels: &mut [u32],
        stride: usize,
        state: &ShellState,
        view: &SystemView,
    ) -> Result<u64, Error> {
        self.render_shell_view_with_theme(pixels, stride, state, view, &DEFAULT_THEME)
    }

    pub fn render_shell_view_without_pointer(
        &self,
        pixels: &mut [u32],
        stride: usize,
        state: &ShellState,
        view: &SystemView,
    ) -> Result<u64, Error> {
        self.render_shell_view_with_theme_inner(pixels, stride, state, view, &DEFAULT_THEME, false)
    }

    pub fn render_shell_view_with_theme(
        &self,
        pixels: &mut [u32],
        stride: usize,
        state: &ShellState,
        view: &SystemView,
        theme: &Theme,
    ) -> Result<u64, Error> {
        self.render_shell_view_with_theme_inner(pixels, stride, state, view, theme, true)
    }

    fn render_shell_view_with_theme_inner(
        &self,
        pixels: &mut [u32],
        stride: usize,
        state: &ShellState,
        view: &SystemView,
        theme: &Theme,
        paint_pointer: bool,
    ) -> Result<u64, Error> {
        let layout = state.layout;
        if layout.width != LOGICAL_WIDTH
            || layout.height != LOGICAL_HEIGHT
            || layout.top_bar_height >= layout.height
            || layout.sidebar_width >= layout.width
            || layout.content_left < i16::try_from(layout.sidebar_width).unwrap_or(i16::MAX)
            || layout.content_width == 0
            || i32::from(layout.content_left) + i32::from(layout.content_width)
                > i32::from(layout.width)
        {
            return Err(Error::Failed);
        }
        rect(
            pixels,
            stride,
            Rect::new(0, 0, layout.width, layout.height),
            theme.desktop,
        )?;

        // Trusted system bar.
        rect(
            pixels,
            stride,
            Rect::new(0, 0, layout.width, layout.top_bar_height),
            theme.panel,
        )?;
        rect(
            pixels,
            stride,
            Rect::new(0, layout.top_bar_height as i16 - 1, layout.width, 1),
            theme.border,
        )?;
        text(pixels, stride, 24, 20, b"GINUX", theme.text)?;
        text(pixels, stride, 424, 18, b"System Management", theme.muted)?;
        let mut time = [b'0'; 5];
        time[0] = b'0' + view.hour / 10;
        time[1] = b'0' + view.hour % 10;
        time[2] = b':';
        time[3] = b'0' + view.minute / 10;
        time[4] = b'0' + view.minute % 10;
        text(pixels, stride, 850, 18, &time, theme.muted)?;
        rect(
            pixels,
            stride,
            Rect::new(908, 21, 8, 8),
            if view.healthy {
                theme.healthy
            } else {
                theme.danger
            },
        )?;
        draw_power_icon(pixels, stride, layout.width as i16 - 48, 16, theme.text)?;

        // Navigation rail.
        rect(
            pixels,
            stride,
            Rect::new(
                0,
                layout.top_bar_height as i16,
                layout.sidebar_width,
                layout.height - layout.top_bar_height,
            ),
            theme.panel,
        )?;
        rect(
            pixels,
            stride,
            Rect::new(
                layout.sidebar_width as i16 - 1,
                layout.top_bar_height as i16,
                1,
                layout.height - layout.top_bar_height,
            ),
            theme.border,
        )?;
        text(pixels, stride, 24, 78, b"Controller", theme.muted)?;
        for (index, (page, icon, label)) in [
            (Page::Overview, 0, &b"Overview"[..]),
            (Page::Domains, 1, &b"Domains"[..]),
            (Page::Network, 2, &b"Network"[..]),
            (Page::Storage, 3, &b"Storage"[..]),
            (Page::Devices, 4, &b"Devices"[..]),
            (Page::Events, 5, &b"Events"[..]),
            (Page::Settings, 6, &b"Settings"[..]),
        ]
        .into_iter()
        .enumerate()
        {
            let y = layout.nav_first_top as i16 + 18 + index as i16 * layout.nav_step as i16;
            let selected = state.page == page;
            if selected {
                rect(
                    pixels,
                    stride,
                    Rect::new(layout.nav_left, y - 18, layout.nav_width, 42),
                    theme.accent,
                )?;
            }
            let color = if selected { theme.text } else { theme.muted };
            nav_icon(pixels, stride, 28, y - 7, icon, color)?;
            text(pixels, stride, 60, y - 5, label, color)?;
        }

        if state.page == Page::Overview {
            self.render_overview(pixels, stride, view, theme, &layout)?;
        } else {
            render_page(pixels, stride, state.page, view, theme, &layout)?;
        }

        if state.power_menu {
            render_power_menu(pixels, stride, theme)?;
        }
        if let Some(action) = state.power_confirmation {
            render_power_confirmation(pixels, stride, action, theme)?;
        }

        if paint_pointer {
            draw_pointer(
                pixels,
                stride,
                state.pointer_x.min((SHELL_WIDTH - 1) as u16) as i16,
                state.pointer_y.min((SHELL_HEIGHT - 1) as u16) as i16,
            )?;
        }
        Ok(if paint_pointer {
            ginux_windowing::pixel_hash(pixels)
        } else {
            0
        })
    }

    fn render_overview(
        &self,
        pixels: &mut [u32],
        stride: usize,
        view: &SystemView,
        theme: &Theme,
        layout: &Layout,
    ) -> Result<(), Error> {
        let left = layout.content_left;
        let card_width = (layout.content_width - 32) / 3;
        let card_top = layout.content_top + 44;
        text(
            pixels,
            stride,
            left,
            layout.content_top,
            b"System Overview",
            theme.text,
        )?;
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
            Rect::new(left, card_top, card_width, 116),
            1,
            (b"Domains", domain_value, domain_detail),
            theme,
        )?;
        let mut cpu = [0_u8; 6];
        let cpu = decimal_suffix(&mut cpu, u64::from(view.cpu_allocated), b" CPU");
        card(
            pixels,
            stride,
            Rect::new(left + card_width as i16 + 16, card_top, card_width, 116),
            4,
            (b"CPU", cpu, b"Configured"),
            theme,
        )?;
        let mut memory = [0_u8; 8];
        let memory = decimal_suffix(&mut memory, u64::from(view.memory_used_mib), b"M");
        let mut memory_detail = [0_u8; 16];
        let memory_detail = total_detail(&mut memory_detail, u64::from(view.memory_total_mib));
        card(
            pixels,
            stride,
            Rect::new(
                left + (card_width as i16 + 16) * 2,
                card_top,
                card_width,
                116,
            ),
            3,
            (b"Memory", memory, memory_detail),
            theme,
        )?;

        // Domain inventory.
        let table_top = card_top + 140;
        let table = Rect::new(
            left,
            table_top,
            layout.content_width,
            (layout.content_bottom - table_top) as u16,
        );
        rect(pixels, stride, table, theme.panel)?;
        outline(pixels, stride, table, theme.border)?;
        rect(
            pixels,
            stride,
            Rect::new(left + 1, table_top + 1, layout.content_width - 2, 40),
            theme.raised,
        )?;
        text(
            pixels,
            stride,
            left + 16,
            table_top + 14,
            b"Domains",
            theme.text,
        )?;
        rect(
            pixels,
            stride,
            Rect::new(left + 1, table_top + 41, layout.content_width - 2, 42),
            theme.surface,
        )?;
        for offset in [230, 410, 530] {
            rect(
                pixels,
                stride,
                Rect::new(left + offset, table_top + 41, 1, 98),
                theme.border,
            )?;
        }
        text(
            pixels,
            stride,
            left + 16,
            table_top + 55,
            b"Name",
            theme.muted,
        )?;
        text(
            pixels,
            stride,
            left + 246,
            table_top + 55,
            b"State",
            theme.muted,
        )?;
        text(
            pixels,
            stride,
            left + 426,
            table_top + 55,
            b"CPU",
            theme.muted,
        )?;
        text(
            pixels,
            stride,
            left + 546,
            table_top + 55,
            b"Memory",
            theme.muted,
        )?;
        rect(
            pixels,
            stride,
            Rect::new(left + 1, table_top + 82, layout.content_width - 2, 1),
            theme.border,
        )?;
        for (index, domain) in view.domains[..usize::from(view.domain_count.min(MAX_DOMAINS as u8))]
            .iter()
            .enumerate()
        {
            let y = table_top + 102 + index as i16 * 42;
            text(pixels, stride, left + 16, y, domain.name(), theme.text)?;
            let state = domain_state_text(domain.state);
            rect(
                pixels,
                stride,
                Rect::new(left + 246, y + 3, 8, 8),
                if domain.state == DomainState::Running {
                    theme.healthy
                } else {
                    theme.muted
                },
            )?;
            text(pixels, stride, left + 264, y, state, theme.text)?;
            let mut cpu = [0_u8; 5];
            let cpu = decimal(&mut cpu, u64::from(domain.cpu));
            text(pixels, stride, left + 426, y, cpu, theme.text)?;
            let mut memory = [0_u8; 8];
            let memory = decimal_suffix(&mut memory, u64::from(domain.memory_mib), b"M");
            text(pixels, stride, left + 546, y, memory, theme.text)?;
        }
        Ok(())
    }
}

pub fn paint_shell_pointer(pixels: &mut [u32], stride: usize, x: u16, y: u16) -> Result<(), Error> {
    draw_pointer(
        pixels,
        stride,
        x.min((SHELL_WIDTH - 1) as u16) as i16,
        y.min((SHELL_HEIGHT - 1) as u16) as i16,
    )
}

fn page_at(x: u16, y: u16, layout: &Layout) -> Option<Page> {
    let content_left = layout.content_left.max(0) as u16;
    let card_top = (layout.content_top + 44).max(0) as u16;
    let first_card_right = content_left + (layout.content_width - 32) / 3;
    if (content_left..first_card_right).contains(&x) && (card_top..card_top + 104).contains(&y) {
        return Some(Page::Domains);
    }
    let nav_left = layout.nav_left.max(0) as u16;
    if !(nav_left..nav_left + layout.nav_width).contains(&x) {
        return None;
    }
    [
        Page::Overview,
        Page::Domains,
        Page::Network,
        Page::Storage,
        Page::Devices,
        Page::Events,
        Page::Settings,
    ]
    .into_iter()
    .enumerate()
    .find_map(|(index, page)| {
        let top = layout.nav_first_top + index as u16 * layout.nav_step;
        (y >= top && y < top + 42).then_some(page)
    })
}

fn page_title(page: Page) -> &'static [u8] {
    match page {
        Page::Overview => b"System Overview",
        Page::Domains => b"Domains",
        Page::Network => b"Network",
        Page::Storage => b"Storage",
        Page::Devices => b"Devices",
        Page::Events => b"Events",
        Page::Settings => b"Settings",
    }
}

fn render_page(
    pixels: &mut [u32],
    stride: usize,
    page: Page,
    view: &SystemView,
    theme: &Theme,
    layout: &Layout,
) -> Result<(), Error> {
    text(
        pixels,
        stride,
        layout.content_left,
        layout.content_top,
        page_title(page),
        theme.text,
    )?;
    let panel_top = layout.content_top + 44;
    let panel = Rect::new(
        layout.content_left,
        panel_top,
        layout.content_width,
        (layout.content_bottom - panel_top) as u16,
    );
    rect(pixels, stride, panel, theme.panel)?;
    outline(pixels, stride, panel, theme.border)?;
    text(
        pixels,
        stride,
        layout.content_left + 24,
        panel_top + 24,
        b"Live Controller View",
        theme.healthy,
    )?;
    let (label, value) = match page {
        Page::Domains => (b"Configured domains".as_slice(), view.domain_count),
        Page::Network => (b"Host network status".as_slice(), u8::from(view.healthy)),
        Page::Storage => (b"Local storage status".as_slice(), u8::from(view.healthy)),
        Page::Devices => (b"Device service status".as_slice(), u8::from(view.healthy)),
        Page::Events => (b"Active event count".as_slice(), 0),
        Page::Settings => (b"System settings".as_slice(), 1),
        Page::Overview => (b"Configured domains".as_slice(), view.domain_count),
    };
    text(
        pixels,
        stride,
        layout.content_left + 24,
        panel_top + 72,
        label,
        theme.muted,
    )?;
    let mut numeric = [0_u8; 3];
    let numeric = decimal(&mut numeric, u64::from(value));
    text(
        pixels,
        stride,
        layout.content_left + 24,
        panel_top + 100,
        numeric,
        theme.text,
    )?;
    if page == Page::Settings {
        text(
            pixels,
            stride,
            layout.content_left + 24,
            panel_top + 172,
            b"Date, time, and timezone",
            theme.text,
        )?;
        text(
            pixels,
            stride,
            layout.content_left + 24,
            panel_top + 204,
            b"Configuration interface pending",
            theme.muted,
        )?;
    } else {
        text(
            pixels,
            stride,
            layout.content_left + 24,
            panel_top + 172,
            b"State refreshes from the kernel",
            theme.muted,
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
    let separator = b" run ";
    let count = separator.len().min(output.len().saturating_sub(length));
    output[length..length + count].copy_from_slice(&separator[..count]);
    length += count;
    length += decimal(&mut output[length..], stopped).len();
    let suffix = b" stopped";
    let count = suffix.len().min(output.len().saturating_sub(length));
    output[length..length + count].copy_from_slice(&suffix[..count]);
    &output[..length + count]
}

fn total_detail(output: &mut [u8], total: u64) -> &[u8] {
    output[..3].copy_from_slice(b"of ");
    let mut length = 3 + decimal(&mut output[3..], total).len();
    let suffix = b" MiB";
    let count = suffix.len().min(output.len().saturating_sub(length));
    output[length..length + count].copy_from_slice(&suffix[..count]);
    length += count;
    &output[..length]
}

fn domain_state_text(state: DomainState) -> &'static [u8] {
    match state {
        DomainState::Created => b"Created",
        DomainState::Configured => b"Configured",
        DomainState::Starting => b"Starting",
        DomainState::Running => b"Running",
        DomainState::Stopped => b"Stopped",
        DomainState::Failed => b"Failed",
        DomainState::Empty => b"Empty",
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
        area.x.saturating_mul(DISPLAY_SCALE as i16),
        area.y.saturating_mul(DISPLAY_SCALE as i16),
        area.width.saturating_mul(DISPLAY_SCALE),
        area.height.saturating_mul(DISPLAY_SCALE),
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
    paint_text_inter_16(
        pixels,
        stride,
        x.saturating_mul(DISPLAY_SCALE as i16),
        y.saturating_mul(DISPLAY_SCALE as i16),
        value,
        color,
    )
    .map_err(|_| Error::Failed)
}

fn card(
    pixels: &mut [u32],
    stride: usize,
    area: Rect,
    icon: u8,
    content: (&[u8], &[u8], &[u8]),
    theme: &Theme,
) -> Result<(), Error> {
    let (label, value, detail) = content;
    rect(pixels, stride, area, theme.raised)?;
    outline(pixels, stride, area, theme.border)?;
    nav_icon(pixels, stride, area.x + 16, area.y + 34, icon, theme.text)?;
    text(pixels, stride, area.x + 58, area.y + 18, label, theme.muted)?;
    text(pixels, stride, area.x + 58, area.y + 46, value, theme.text)?;
    text(
        pixels,
        stride,
        area.x + 16,
        area.y + 78,
        detail,
        theme.muted,
    )
}

fn nav_icon(
    pixels: &mut [u32],
    stride: usize,
    x: i16,
    y: i16,
    kind: u8,
    color: u32,
) -> Result<(), Error> {
    const S: i16 = 2;
    match kind {
        0 => {
            for (dx, dy) in [(0, 0), (5, 0), (0, 5), (5, 5)] {
                outline(
                    pixels,
                    stride,
                    Rect::new(x + dx * S, y + dy * S, 8, 8),
                    color,
                )?;
            }
        }
        1 => {
            outline(pixels, stride, Rect::new(x + 2, y, 16, 20), color)?;
            rect(pixels, stride, Rect::new(x, y + 6, 20, 2), color)?;
        }
        2 => {
            for (dx, dy) in [(0, 7), (5, 0), (10, 7)] {
                outline(
                    pixels,
                    stride,
                    Rect::new(x + dx * S, y + dy * S, 6, 6),
                    color,
                )?;
            }
            rect(pixels, stride, Rect::new(x + 12, y + 6, 2, 10), color)?;
            rect(pixels, stride, Rect::new(x + 4, y + 14, 18, 2), color)?;
        }
        3 => {
            outline(pixels, stride, Rect::new(x, y + 2, 22, 16), color)?;
            rect(pixels, stride, Rect::new(x + 4, y + 8, 14, 2), color)?;
        }
        4 => {
            outline(pixels, stride, Rect::new(x + 4, y + 4, 14, 14), color)?;
            for (dx, dy) in [(0, 4), (10, 4), (5, 0), (5, 10)] {
                rect(
                    pixels,
                    stride,
                    Rect::new(x + dx * S, y + dy * S, 4, 4),
                    color,
                )?;
            }
        }
        5 => {
            outline(pixels, stride, Rect::new(x, y, 20, 22), color)?;
            rect(pixels, stride, Rect::new(x + 6, y + 6, 2, 2), color)?;
            rect(pixels, stride, Rect::new(x + 6, y + 14, 2, 2), color)?;
            rect(pixels, stride, Rect::new(x + 12, y + 6, 4, 2), color)?;
            rect(pixels, stride, Rect::new(x + 12, y + 14, 4, 2), color)?;
        }
        _ => {
            outline(pixels, stride, Rect::new(x + 2, y + 2, 18, 18), color)?;
            rect(pixels, stride, Rect::new(x + 10, y + 6, 2, 10), color)?;
            rect(pixels, stride, Rect::new(x + 6, y + 10, 10, 2), color)?;
        }
    }
    Ok(())
}

fn draw_pointer(pixels: &mut [u32], stride: usize, x: i16, y: i16) -> Result<(), Error> {
    // A compact arrow with a dark outline, light face, and accent hotspot.
    let rows: [u16; 18] = [
        0b1,
        0b11,
        0b111,
        0b1111,
        0b1_1111,
        0b11_1111,
        0b111_1111,
        0b1111_1111,
        0b1_1111_1111,
        0b11_1111_1111,
        0b111_1111_1111,
        0b1_1111_1111_1111,
        0b111_1111,
        0b110_0111,
        0b1100_0011,
        0b1100_0001,
        0b1100_0000,
        0b1000_0000,
    ];
    for (row, bits) in rows.into_iter().enumerate() {
        let width = 16 - bits.leading_zeros() as i16;
        for column in 0..width {
            if bits & (1 << column) == 0 {
                continue;
            }
            let edge = column == 0
                || row == 0
                || column == width - 1
                || row + 1 == rows.len()
                || rows[row + 1] & (1 << column) == 0;
            paint_rect(
                pixels,
                stride,
                Rect::new(x + column, y + row as i16, 1, 1),
                if edge { COLOR_DARK_TEXT } else { COLOR_TEXT },
            )
            .map_err(|_| Error::Failed)?;
        }
    }
    paint_rect(pixels, stride, Rect::new(x + 2, y + 3, 2, 2), COLOR_ACCENT)
        .map_err(|_| Error::Failed)?;
    Ok(())
}

fn draw_power_icon(
    pixels: &mut [u32],
    stride: usize,
    x: i16,
    y: i16,
    color: u32,
) -> Result<(), Error> {
    rect(pixels, stride, Rect::new(x + 11, y, 3, 13), color)?;
    rect(pixels, stride, Rect::new(x + 5, y + 6, 5, 2), color)?;
    rect(pixels, stride, Rect::new(x + 15, y + 6, 5, 2), color)?;
    rect(pixels, stride, Rect::new(x + 2, y + 9, 3, 12), color)?;
    rect(pixels, stride, Rect::new(x + 20, y + 9, 3, 12), color)?;
    rect(pixels, stride, Rect::new(x + 5, y + 21, 4, 3), color)?;
    rect(pixels, stride, Rect::new(x + 16, y + 21, 4, 3), color)?;
    rect(pixels, stride, Rect::new(x + 9, y + 24, 7, 2), color)
}

fn render_power_menu(pixels: &mut [u32], stride: usize, theme: &Theme) -> Result<(), Error> {
    let panel = Rect::new(840, 52, 168, 96);
    rect(pixels, stride, panel, theme.raised)?;
    outline(pixels, stride, panel, theme.border)?;
    text(pixels, stride, 860, 68, b"Reboot", theme.text)?;
    rect(pixels, stride, Rect::new(848, 97, 152, 1), theme.border)?;
    text(pixels, stride, 860, 112, b"Shut down", theme.danger)
}

fn render_power_confirmation(
    pixels: &mut [u32],
    stride: usize,
    action: PowerAction,
    theme: &Theme,
) -> Result<(), Error> {
    rect(
        pixels,
        stride,
        Rect::new(0, 0, LOGICAL_WIDTH, LOGICAL_HEIGHT),
        0x0006_0c16,
    )?;
    let panel = Rect::new(344, 294, 336, 180);
    rect(pixels, stride, panel, theme.raised)?;
    outline(pixels, stride, panel, theme.border)?;
    text(
        pixels,
        stride,
        376,
        326,
        if action == PowerAction::Reboot {
            b"Reboot Ginux?"
        } else {
            b"Shut down Ginux?"
        },
        theme.text,
    )?;
    text(
        pixels,
        stride,
        376,
        362,
        b"Running domains will stop.",
        theme.muted,
    )?;
    rect(pixels, stride, Rect::new(376, 410, 112, 38), theme.surface)?;
    outline(pixels, stride, Rect::new(376, 410, 112, 38), theme.border)?;
    text(pixels, stride, 394, 422, b"Cancel", theme.text)?;
    rect(pixels, stride, Rect::new(536, 410, 112, 38), theme.danger)?;
    text(pixels, stride, 552, 422, b"Confirm", theme.text)
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
    extern crate std;

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
            Operation::Reboot,
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
        let mut pixels = std::vec![0_u32; SHELL_WIDTH * SHELL_HEIGHT];
        let hash = app.render_shell(&mut pixels, SHELL_WIDTH).unwrap();
        assert_ne!(hash, 0);
        assert_eq!(pixels[0], COLOR_PANEL);
        assert_eq!(pixels[51 * SHELL_WIDTH], COLOR_BORDER);
        assert_eq!(pixels[94 * SHELL_WIDTH + 16], COLOR_ACCENT);
        assert_eq!(pixels[126 * SHELL_WIDTH + 252], COLOR_BORDER);
        assert_eq!(pixels[127 * SHELL_WIDTH + 253], COLOR_SURFACE_RAISED);
        assert_eq!(hash, app.render_shell(&mut pixels, SHELL_WIDTH).unwrap());
    }

    #[test]
    fn pointer_click_selects_navigation_once() {
        let mut state = ShellState::new();
        assert!(state.pointer_motion(80, 440));
        assert!(state.pointer_buttons(1, 1));
        assert_eq!(state.page, Page::Settings);
        assert!(!state.pointer_buttons(1, 0));
        assert!(!state.pointer_buttons(0, 0));
        assert_eq!(state.page, Page::Settings);
    }

    #[test]
    fn power_menu_requires_confirmation_and_routes_reboot() {
        let mut state = ShellState::new();
        assert!(state.pointer_motion(980, 24));
        assert!(state.pointer_buttons(1, 1));
        assert!(state.power_menu);
        assert!(!state.pointer_buttons(1, 0));
        assert!(state.pointer_motion(860, 76));
        assert!(state.pointer_buttons(1, 1));
        assert_eq!(state.power_confirmation, Some(PowerAction::Reboot));
        assert!(!state.pointer_buttons(1, 0));
        assert!(state.pointer_motion(560, 430));
        assert!(state.pointer_buttons(1, 1));
        assert_eq!(state.take_power_action(), Some(PowerAction::Reboot));
        assert_eq!(state.take_power_action(), None);
    }

    #[test]
    fn theme_override_changes_rendered_frame() {
        let mut backend = ModelBackend::new();
        let app = Application::new(&mut backend);
        let state = ShellState::new();
        let view = SystemView::model();
        let mut pixels = std::vec![0_u32; SHELL_WIDTH * SHELL_HEIGHT];
        let default = app
            .render_shell_view_with_theme(&mut pixels, SHELL_WIDTH, &state, &view, &DEFAULT_THEME)
            .unwrap();
        let mut alternate = DEFAULT_THEME;
        alternate.accent = 0x00ff_8800;
        let changed = app
            .render_shell_view_with_theme(&mut pixels, SHELL_WIDTH, &state, &view, &alternate)
            .unwrap();
        assert_ne!(default, changed);
        assert_eq!(pixels[94 * SHELL_WIDTH + 16], alternate.accent);
    }

    #[test]
    fn layout_override_moves_rendering_and_hit_testing_together() {
        let mut backend = ModelBackend::new();
        let app = Application::new(&mut backend);
        let mut state = ShellState::new();
        state.layout.sidebar_width = 200;
        state.layout.content_left = 232;
        state.layout.content_width = 544;
        state.layout.nav_left = 16;
        state.layout.nav_width = 172;
        state.layout.nav_first_top = 112;
        let view = SystemView::model();
        let mut pixels = std::vec![0_u32; SHELL_WIDTH * SHELL_HEIGHT];
        app.render_shell_view(&mut pixels, SHELL_WIDTH, &state, &view)
            .unwrap();
        assert_eq!(pixels[112 * SHELL_WIDTH + 16], DEFAULT_THEME.accent);
        assert!(state.pointer_motion(80, 112 + 6 * 56 + 10));
        assert!(state.pointer_buttons(1, 1));
        assert_eq!(state.page, Page::Settings);
    }

    #[test]
    fn live_view_changes_rendered_frame() {
        let mut backend = ModelBackend::new();
        let app = Application::new(&mut backend);
        let state = ShellState::new();
        let mut pixels = std::vec![0_u32; SHELL_WIDTH * SHELL_HEIGHT];
        let empty = SystemView::empty();
        let empty_hash = app
            .render_shell_view(&mut pixels, SHELL_WIDTH, &state, &empty)
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
            .render_shell_view(&mut pixels, SHELL_WIDTH, &state, &live)
            .unwrap();
        assert_ne!(empty_hash, live_hash);
    }
}
