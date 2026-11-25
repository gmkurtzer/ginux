//! Release-only regression: `[profile.release]` must keep overflow checks on.
//!
//! `cargo test` builds test targets with `panic = "unwind"` even when the
//! profile sets `panic = "abort"` (MEASURED: `#[should_panic]` passes under
//! `cargo test --release` with rustc 1.99.0), so a plain `#[should_panic]`
//! discriminates:
//!
//! - release with `overflow-checks = true` -> the add panics -> PASS
//! - release without the flag              -> the add wraps silently -> FAIL
//! - debug (overflow checks on by default) -> PASS either way
//!
//! Only `cargo test --release` discriminates.

#[test]
#[should_panic(expected = "attempt to add with overflow")]
fn release_profile_keeps_overflow_checks_active() {
    let a: u64 = std::hint::black_box(u64::MAX);
    let b: u64 = std::hint::black_box(1);
    let _ = std::hint::black_box(a + b);
}
