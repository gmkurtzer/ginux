#![no_std]

pub const COMPONENTS: usize = 7;
pub const MAX_DIAGNOSTICS: usize = 32;
pub const MAX_RESTART_ATTEMPTS: u8 = 3;
pub const RETRY_TICKS: u64 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Component {
    Compositor,
    GraphicalController,
    NativeClient,
    DomainBridge,
    WaylandCompositor,
    LinuxClient,
    Domain,
}

impl Component {
    pub const ALL: [Self; COMPONENTS] = [
        Self::Compositor,
        Self::GraphicalController,
        Self::NativeClient,
        Self::DomainBridge,
        Self::WaylandCompositor,
        Self::LinuxClient,
        Self::Domain,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Compositor => 0,
            Self::GraphicalController => 1,
            Self::NativeClient => 2,
            Self::DomainBridge => 3,
            Self::WaylandCompositor => 4,
            Self::LinuxClient => 5,
            Self::Domain => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryResult {
    FaultContained,
    RestartScheduled,
    Restarted,
    BudgetExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub component: Component,
    pub generation: u32,
    pub result: RecoveryResult,
    pub sequence: u64,
}

const EMPTY_DIAGNOSTIC: Diagnostic = Diagnostic {
    component: Component::Compositor,
    generation: 0,
    result: RecoveryResult::FaultContained,
    sequence: 0,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSnapshot {
    pub live_components: u8,
    pub surfaces: u8,
    pub buffers: u8,
    pub event_queues: u8,
    pub handles: u8,
    pub leases: u8,
    pub trusted_focus: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyFailed,
    Stale,
    RetryPending,
    BudgetExhausted,
    Capacity,
}

#[derive(Clone, Copy)]
struct ComponentState {
    generation: u32,
    live: bool,
    restart_attempts: u8,
    retry_at: u64,
}

impl ComponentState {
    const fn new() -> Self {
        Self {
            generation: 1,
            live: true,
            restart_attempts: 0,
            retry_at: 0,
        }
    }
}

pub struct Supervisor {
    states: [ComponentState; COMPONENTS],
    diagnostics: [Diagnostic; MAX_DIAGNOSTICS],
    diagnostic_head: u8,
    diagnostic_count: u8,
    sequence: u64,
    trusted_focus: bool,
    leases: u8,
    surfaces: u8,
    buffers: u8,
    event_queues: u8,
    handles: u8,
}

impl Supervisor {
    pub const fn new() -> Self {
        Self {
            states: [ComponentState::new(); COMPONENTS],
            diagnostics: [EMPTY_DIAGNOSTIC; MAX_DIAGNOSTICS],
            diagnostic_head: 0,
            diagnostic_count: 0,
            sequence: 0,
            trusted_focus: false,
            leases: 1,
            surfaces: 3,
            buffers: 4,
            event_queues: 3,
            handles: 7,
        }
    }

    pub fn generation(&self, component: Component) -> u32 {
        self.states[component.index()].generation
    }

    pub fn validate_generation(&self, component: Component, generation: u32) -> Result<(), Error> {
        let state = self.states[component.index()];
        if state.live && state.generation == generation {
            Ok(())
        } else {
            Err(Error::Stale)
        }
    }

    pub fn fault(&mut self, component: Component, now: u64) -> Result<(), Error> {
        let index = component.index();
        if !self.states[index].live {
            return Err(Error::AlreadyFailed);
        }
        self.states[index].live = false;
        self.states[index].retry_at = now.saturating_add(RETRY_TICKS);
        self.revoke_all();
        self.record(component, RecoveryResult::FaultContained)?;
        self.record(component, RecoveryResult::RestartScheduled)
    }

    pub fn recover(&mut self, component: Component, now: u64) -> Result<u32, Error> {
        let index = component.index();
        if self.states[index].live {
            return Ok(self.states[index].generation);
        }
        if now < self.states[index].retry_at {
            return Err(Error::RetryPending);
        }
        if self.states[index].restart_attempts == MAX_RESTART_ATTEMPTS {
            self.record(component, RecoveryResult::BudgetExhausted)?;
            return Err(Error::BudgetExhausted);
        }
        let generation = self.states[index]
            .generation
            .checked_add(1)
            .filter(|generation| *generation != 0)
            .ok_or(Error::Capacity)?;
        self.states[index].generation = generation;
        self.states[index].live = true;
        self.states[index].restart_attempts = 0;
        self.record(component, RecoveryResult::Restarted)?;
        Ok(generation)
    }

    pub fn fail_restart(&mut self, component: Component, now: u64) -> Result<(), Error> {
        let index = component.index();
        if self.states[index].live {
            return Err(Error::AlreadyFailed);
        }
        if now < self.states[index].retry_at {
            return Err(Error::RetryPending);
        }
        if self.states[index].restart_attempts == MAX_RESTART_ATTEMPTS {
            self.record(component, RecoveryResult::BudgetExhausted)?;
            return Err(Error::BudgetExhausted);
        }
        self.states[index].restart_attempts += 1;
        self.states[index].retry_at = now.saturating_add(RETRY_TICKS);
        self.record(component, RecoveryResult::RestartScheduled)
    }

    pub fn snapshot(&self) -> ResourceSnapshot {
        ResourceSnapshot {
            live_components: self.states.iter().filter(|state| state.live).count() as u8,
            surfaces: self.surfaces,
            buffers: self.buffers,
            event_queues: self.event_queues,
            handles: self.handles,
            leases: self.leases,
            trusted_focus: self.trusted_focus,
        }
    }

    pub fn diagnostic(&self, index: usize) -> Option<Diagnostic> {
        (index < usize::from(self.diagnostic_count)).then(|| {
            self.diagnostics[(usize::from(self.diagnostic_head) + index) % MAX_DIAGNOSTICS]
        })
    }

    fn revoke_all(&mut self) {
        self.trusted_focus = true;
        self.leases = 0;
        self.surfaces = 0;
        self.buffers = 0;
        self.event_queues = 0;
        self.handles = 0;
    }

    fn record(&mut self, component: Component, result: RecoveryResult) -> Result<(), Error> {
        self.sequence = self.sequence.checked_add(1).ok_or(Error::Capacity)?;
        let slot = if usize::from(self.diagnostic_count) < MAX_DIAGNOSTICS {
            let slot = (usize::from(self.diagnostic_head) + usize::from(self.diagnostic_count))
                % MAX_DIAGNOSTICS;
            self.diagnostic_count += 1;
            slot
        } else {
            let slot = usize::from(self.diagnostic_head);
            self.diagnostic_head = ((slot + 1) % MAX_DIAGNOSTICS) as u8;
            slot
        };
        self.diagnostics[slot] = Diagnostic {
            component,
            generation: self.states[component.index()].generation,
            result,
            sequence: self.sequence,
        };
        Ok(())
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

fn actual_component_recovery_probe() -> bool {
    use ginux_display_protocol::{Resource, Server, LEASE_EXCLUSIVE, VERSION};

    let mut server = Server::new();
    let Ok(client) = server.connect(VERSION, 0) else {
        return false;
    };
    if server.create_surface(client, 10).is_err()
        || server.focus_surface(client, 10).is_err()
        || server
            .grant_lease(client, 10, Resource::Display, LEASE_EXCLUSIVE, 10)
            .is_err()
        || server.disconnect(client).is_err()
        || !server.seat_status().trusted_focus
        || server.seat_status().lease_count != 0
        || server.create_surface(client, 11).is_ok()
    {
        return false;
    }
    let Ok(fresh) = server.connect(VERSION, 0) else {
        return false;
    };
    if fresh == client {
        return false;
    }

    let mut wayland = ginux_wayland_domain::Compositor::new();
    let Ok(old_wayland) = wayland.connect() else {
        return false;
    };
    if wayland.disconnect(old_wayland).is_err()
        || wayland.next_event(old_wayland).is_ok()
        || wayland.connect().is_err()
    {
        return false;
    }

    let Ok(mut presentation) = ginux_domain_presentation::Presentation::start(
        9,
        ginux_domain_presentation::Rect::new(0, 0, 64, 40),
        ginux_domain_presentation::Rect::new(8, 8, 40, 24),
    ) else {
        return false;
    };
    presentation.enter_fullscreen().is_ok()
        && presentation.fail().is_ok()
        && !presentation.status().focused
        && !presentation.status().exclusive_lease
        && presentation.status().content == 0
        && presentation.restart().is_ok()
}

pub fn contract_probe() -> bool {
    if !actual_component_recovery_probe() {
        return false;
    }
    let mut supervisor = Supervisor::new();
    for (index, component) in Component::ALL.into_iter().enumerate() {
        let old = supervisor.generation(component);
        let now = (index as u64 + 1) * 10;
        if supervisor.fault(component, now).is_err()
            || !supervisor.snapshot().trusted_focus
            || supervisor.snapshot().leases != 0
            || supervisor.validate_generation(component, old) != Err(Error::Stale)
            || supervisor.recover(component, now + RETRY_TICKS) != Ok(old + 1)
            || supervisor.validate_generation(component, old).is_ok()
        {
            return false;
        }
    }
    supervisor
        .diagnostic(0)
        .is_some_and(|entry| entry.component == Component::Compositor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_fault_matrix_restores_trusted_state_and_fresh_generations() {
        assert!(contract_probe());
    }

    #[test]
    fn retries_are_bounded_and_diagnostics_name_component_and_result() {
        let mut supervisor = Supervisor::new();
        supervisor.fault(Component::Compositor, 1).unwrap();
        assert_eq!(
            supervisor.recover(Component::Compositor, 2),
            Err(Error::RetryPending)
        );
        for now in [3, 5, 7] {
            supervisor.fail_restart(Component::Compositor, now).unwrap();
        }
        assert_eq!(
            supervisor.fail_restart(Component::Compositor, 9),
            Err(Error::BudgetExhausted)
        );
        assert!((0..MAX_DIAGNOSTICS).any(|index| {
            supervisor.diagnostic(index).is_some_and(|entry| {
                entry.component == Component::Compositor
                    && entry.result == RecoveryResult::BudgetExhausted
            })
        }));
    }

    #[test]
    fn repeated_multi_component_lifecycle_has_constant_resource_bounds() {
        let mut supervisor = Supervisor::new();
        for cycle in 0..512_u64 {
            let component = Component::ALL[cycle as usize % COMPONENTS];
            let old = supervisor.generation(component);
            let now = cycle * 4 + 1;
            supervisor.fault(component, now).unwrap();
            let retired = supervisor.snapshot();
            assert_eq!(retired.surfaces, 0);
            assert_eq!(retired.buffers, 0);
            assert_eq!(retired.event_queues, 0);
            assert_eq!(retired.handles, 0);
            assert_eq!(retired.leases, 0);
            assert!(retired.trusted_focus);
            assert_eq!(
                supervisor.recover(component, now + RETRY_TICKS),
                Ok(old + 1)
            );
        }
    }

    #[test]
    fn two_domain_bridges_do_not_cross_pixels_or_input_during_failure() {
        let mut bridge = ginux_domain_display::Bridge::new();
        let first = bridge.start(1, 64, 64).unwrap();
        let second = bridge.start(2, 64, 64).unwrap();
        bridge.focus(first).unwrap();
        bridge.publish(first, [0, 0, 8, 8], 0x1111).unwrap();
        bridge.publish(second, [0, 0, 8, 8], 0x2222).unwrap();
        bridge.send_input(first, 1, 30, 1, false).unwrap();
        assert!(bridge.send_input(second, 1, 30, 1, false).is_err());
        bridge.stop(first).unwrap();
        assert!(bridge.take_input(first).is_err());
        assert_eq!(bridge.take_frame(second).unwrap().content, 0x2222);
    }
}
