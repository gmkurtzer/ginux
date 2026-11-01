#![no_std]

use ginux_native_abi::{
    NATIVE_DOMAIN_ATTACH, NATIVE_DOMAIN_CONTROL_OPERATION, NATIVE_DOMAIN_CREATE,
    NATIVE_DOMAIN_EXEC, NATIVE_DOMAIN_QUERY, NATIVE_DOMAIN_RESTART, NATIVE_DOMAIN_SET_CPU,
    NATIVE_DOMAIN_SET_IMAGE, NATIVE_DOMAIN_SET_INIT, NATIVE_DOMAIN_SET_MEMORY_MIB,
    NATIVE_DOMAIN_START_ATTACHED, NATIVE_DOMAIN_STOP, NATIVE_HOST_INSPECT_OPERATION,
    NATIVE_NETWORK_INVENTORY_OPERATION, NATIVE_ROOT_OPERATION,
};
use ginux_windowing::{
    paint_border, paint_rect, paint_text, Desktop, Rect, COLOR_ACCENT, COLOR_BORDER,
    COLOR_DARK_TEXT, COLOR_HEALTHY, COLOR_MUTED, COLOR_PANEL, COLOR_SURFACE, COLOR_SURFACE_RAISED,
    COLOR_TEXT,
};

pub const MAX_DOMAINS: usize = 4;
pub const MAX_JOBS: usize = 8;
pub const MAX_NOTICES: usize = 8;
pub const MAX_NAME: usize = 16;
pub const SHELL_WIDTH: usize = 160;
pub const SHELL_HEIGHT: usize = 100;

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
        self.render_shell_state(pixels, stride, b"controller terminal", 128, 76)
    }

    pub fn render_shell_state(
        &self,
        pixels: &mut [u32],
        stride: usize,
        terminal: &[u8],
        pointer_x: u16,
        pointer_y: u16,
    ) -> Result<u64, Error> {
        let mut desktop = Desktop::new(SHELL_WIDTH as u16, SHELL_HEIGHT as u16);
        let window = desktop
            .create_window(Rect::new(4, 10, 152, 87), b"Ginux Controller", true)
            .map_err(|_| Error::Failed)?;
        let _ = window;
        desktop.render(pixels, stride).map_err(|_| Error::Failed)?;

        // System bar.
        text(pixels, stride, 4, 2, b"GINUX", COLOR_TEXT)?;
        text(pixels, stride, 61, 2, b"SYSTEM MANAGEMENT", COLOR_MUTED)?;
        rect(pixels, stride, Rect::new(147, 2, 3, 3), COLOR_HEALTHY)?;
        text(pixels, stride, 152, 2, b"OK", COLOR_HEALTHY)?;

        // Navigation rail.
        rect(pixels, stride, Rect::new(5, 20, 37, 76), COLOR_PANEL)?;
        rect(pixels, stride, Rect::new(42, 20, 1, 76), COLOR_BORDER)?;
        rect(pixels, stride, Rect::new(7, 23, 33, 8), COLOR_ACCENT)?;
        text(pixels, stride, 9, 25, b"OVERVIEW", COLOR_TEXT)?;
        for (y, label) in [
            (35, &b"DOMAINS"[..]),
            (45, &b"NETWORK"[..]),
            (55, &b"STORAGE"[..]),
            (65, &b"DEVICES"[..]),
            (75, &b"EVENTS"[..]),
        ] {
            rect(pixels, stride, Rect::new(9, y + 1, 2, 2), COLOR_MUTED)?;
            text(pixels, stride, 14, y, label, COLOR_MUTED)?;
        }

        // Overview heading and summary cards.
        text(pixels, stride, 47, 23, b"SYSTEM OVERVIEW", COLOR_TEXT)?;
        card(
            pixels,
            stride,
            Rect::new(47, 31, 32, 17),
            b"DOMAINS",
            b"1 UP",
        )?;
        card(pixels, stride, Rect::new(82, 31, 32, 17), b"CPU", b"2%")?;
        card(
            pixels,
            stride,
            Rect::new(117, 31, 34, 17),
            b"MEMORY",
            b"12%",
        )?;

        // Domain inventory.
        let table = Rect::new(47, 51, 104, 27);
        rect(pixels, stride, table, COLOR_PANEL)?;
        outline(pixels, stride, table, COLOR_BORDER)?;
        rect(
            pixels,
            stride,
            Rect::new(table.x, table.y, table.width, 8),
            COLOR_SURFACE_RAISED,
        )?;
        text(pixels, stride, 50, 53, b"DOMAINS", COLOR_TEXT)?;
        text(
            pixels,
            stride,
            50,
            62,
            b"NAME      STATE  CPU  MEM",
            COLOR_MUTED,
        )?;
        rect(pixels, stride, Rect::new(47, 68, 104, 1), COLOR_BORDER)?;
        text(
            pixels,
            stride,
            50,
            71,
            b"ROCKY     RUNNING  1  128",
            COLOR_TEXT,
        )?;
        rect(pixels, stride, Rect::new(85, 72, 2, 2), COLOR_HEALTHY)?;

        // Command surface.
        let command = Rect::new(47, 82, 104, 11);
        rect(pixels, stride, command, COLOR_SURFACE)?;
        outline(pixels, stride, command, COLOR_ACCENT)?;
        text(pixels, stride, 50, 85, b"GINUX>", COLOR_MUTED)?;
        text(pixels, stride, 74, 85, terminal, COLOR_TEXT)?;
        rect(pixels, stride, Rect::new(147, 85, 1, 5), COLOR_ACCENT)?;

        draw_pointer(
            pixels,
            stride,
            pointer_x.min((SHELL_WIDTH - 1) as u16) as i16,
            pointer_y.min((SHELL_HEIGHT - 1) as u16) as i16,
        )?;
        Ok(ginux_windowing::pixel_hash(pixels))
    }
}

fn rect(pixels: &mut [u32], stride: usize, area: Rect, color: u32) -> Result<(), Error> {
    paint_rect(pixels, stride, area, color).map_err(|_| Error::Failed)
}

fn outline(pixels: &mut [u32], stride: usize, area: Rect, color: u32) -> Result<(), Error> {
    paint_border(pixels, stride, area, color).map_err(|_| Error::Failed)
}

fn text(
    pixels: &mut [u32],
    stride: usize,
    x: i16,
    y: i16,
    value: &[u8],
    color: u32,
) -> Result<(), Error> {
    paint_text(pixels, stride, x, y, value, color).map_err(|_| Error::Failed)
}

fn card(
    pixels: &mut [u32],
    stride: usize,
    area: Rect,
    label: &[u8],
    value: &[u8],
) -> Result<(), Error> {
    rect(pixels, stride, area, COLOR_SURFACE_RAISED)?;
    outline(pixels, stride, area, COLOR_BORDER)?;
    text(pixels, stride, area.x + 3, area.y + 3, label, COLOR_MUTED)?;
    text(pixels, stride, area.x + 3, area.y + 10, value, COLOR_TEXT)?;
    rect(
        pixels,
        stride,
        Rect::new(area.x + area.width as i16 - 5, area.y + 3, 2, 2),
        COLOR_HEALTHY,
    )
}

fn draw_pointer(pixels: &mut [u32], stride: usize, x: i16, y: i16) -> Result<(), Error> {
    for (dx, dy) in [
        (0, 0),
        (0, 1),
        (0, 2),
        (0, 3),
        (0, 4),
        (0, 5),
        (1, 1),
        (1, 2),
        (1, 3),
        (1, 4),
        (2, 2),
        (2, 3),
        (2, 4),
        (3, 3),
        (3, 4),
        (4, 4),
    ] {
        rect(
            pixels,
            stride,
            Rect::new(x + dx, y + dy, 1, 1),
            COLOR_DARK_TEXT,
        )?;
    }
    for (dx, dy) in [(1, 2), (1, 3), (2, 3)] {
        rect(pixels, stride, Rect::new(x + dx, y + dy, 1, 1), COLOR_TEXT)?;
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
        assert_eq!(hash, app.render_shell(&mut pixels, SHELL_WIDTH).unwrap());
    }
}
