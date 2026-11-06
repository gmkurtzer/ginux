//! Architecture-specific daemon entry and host syscall ABI.
//!
//! One binary targets one architecture. x86_64 and aarch64 each own their
//! startup sequence, syscall instruction, and host syscall numbers. Domain
//! mediation of a traced workload stays in the supervisor and uses the
//! x86_64 Linux register and syscall vocabulary until the arm64 mediation
//! stage replaces it.

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
pub use aarch64::*;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!("the Ginux userspace daemon builds for x86_64 or aarch64 only");
