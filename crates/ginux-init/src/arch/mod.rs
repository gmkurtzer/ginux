//! Architecture-specific daemon entry and host syscall ABI.
//!
//! One binary targets one architecture. x86_64 and aarch64 each own their
//! startup sequence, syscall instruction, host syscall numbers, and ptrace
//! register file. Domain decisions stay in x86_64 syscall numbers. The
//! aarch64 file translates `user_pt_regs` and the generic syscall table at
//! the ptrace boundary.

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
