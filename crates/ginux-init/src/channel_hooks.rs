//! Native channel-lane dispatch hooks.
//!
//! Owner: G-files/channel adapter lane (serialized with G-files changes).
//! Balanced bootstrap polling validates the tracee's descriptor array before
//! a bounded console-readiness query. The generic opaque transport the
//! worker-protocol frames ride on is still awaiting integration.

use crate::supervisor::{ptrace, PTRACE_PEEKDATA};

/// Only the three assigned bootstrap descriptors (and ignored negative slots)
/// may appear in a bounded poll. The kernel supplies readiness and
/// POLLNVAL for closed descriptors; no descriptor is created or transferred.
pub fn bootstrap_poll_allowed(pid: usize, address: u64, count: u64) -> bool {
    if count > ginux_core::MAX_BOOTSTRAP_POLL_FDS {
        return false;
    }
    for index in 0..count {
        let Some(address) = address.checked_add(index * 8) else {
            return false;
        };
        let mut entry = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address as usize,
            &mut entry as *mut _ as usize,
        ) < 0
        {
            return false;
        }
        let descriptor = entry as u32 as i32;
        if descriptor > 2 {
            return false;
        }
    }
    true
}

/// Lane membership by syscall number. Reserved for G14.
pub fn claims(syscall: u64) -> bool {
    let _ = syscall;
    false
}

/// Emulated result when this lane handles the entry. `None` falls through to
/// the shared balanced-contract decision. Reserved for G14.
pub fn claim(syscall: u64, rdi: u64, rsi: u64, rdx: u64) -> Option<i64> {
    let _ = (syscall, rdi, rsi, rdx);
    None
}
