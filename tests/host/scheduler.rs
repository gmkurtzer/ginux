//! Host regression tests execute the real scheduler and OwnedState. Only the
//! platform/domain policy is stubbed: all domains have equal scheduling class.
#![allow(dead_code)]
mod control_abi {
    pub const DOMAIN_LIMIT: usize = 4;
    pub const AUXILIARY_STATE_CREATED: u64 = 0;
    pub const AUXILIARY_STATE_STAGED: u64 = 1;
    pub const AUXILIARY_STATE_READY: u64 = 2;
    pub const AUXILIARY_STATE_RUNNING: u64 = 3;
    pub const AUXILIARY_STATE_BLOCKED: u64 = 4;
    pub const AUXILIARY_FLAG_INITIALIZED: u64 = 1;
    pub const PACKET_OBJECT: u64 = 1;
    pub const PACKET_EGRESS_OBJECT: u64 = 2;
    pub const PACKET_RESULT_DRAINED: u64 = u64::MAX - 1;
    pub const PACKET_RESULT_TIMED_OUT: u64 = u64::MAX - 2;
}
mod domain {
    pub struct FaultMetadata {
        pub vector: u64,
        pub error: u64,
        pub address: u64,
        pub instruction: u64,
    }
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub enum ResourceServiceClass {
        Standard,
        LatencyCritical,
    }
    #[derive(PartialEq, Eq)]
    pub enum ServiceFailurePolicy {
        StopOnly,
    }
    pub fn cpu_service_class(_: u64) -> ResourceServiceClass {
        ResourceServiceClass::Standard
    }
    pub fn container_domain_limits(_: u64) -> Option<(u8, u64)> {
        None
    }
    pub fn domain_service_identity(_: u64) -> Option<u64> {
        Some(1)
    }
    pub fn service_failure_policy(_: u64) -> Option<ServiceFailurePolicy> {
        Some(ServiceFailurePolicy::StopOnly)
    }
    pub fn attach_thread(_: u64) -> bool {
        true
    }
    pub fn detach_thread(_: u64) -> bool {
        true
    }
}
mod arch {
    pub mod x86_64 {
        pub mod cpu {
            pub fn current_cpu_index() -> usize {
                0
            }
            pub fn current_linux_execution_instance() -> usize {
                0
            }
        }
        pub mod smp {
            pub fn online_count() -> usize {
                1
            }
            pub fn execute_runtime_operation(_: usize, _: u64) -> bool {
                false
            }
        }
    }
}
#[path = "../../kernel/src/owned_state.rs"]
mod owned_state;
mod scheduler {
    include!("../../kernel/src/scheduler.rs");
    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::Mutex;
        static LOCK: Mutex<()> = Mutex::new(());
        const CS: u16 = 0x33;
        const SS: u16 = 0x2b;
        const WAIT: WaitReason = WaitReason::IpcReceive {
            object: 3,
            service_side: false,
            peer_domain: 1,
        };
        #[derive(Clone, Copy, Debug)]
        enum Op {
            Yield,
            Preempt,
            Block,
            Fault,
            Exit,
        }
        const OPS: [Op; 5] = [Op::Yield, Op::Preempt, Op::Block, Op::Fault, Op::Exit];

        fn setup(second: bool, same_root: bool, blocked_peer: bool) {
            SCHEDULER_RUNTIME.reset();
            assert!(initialize_control_runtime(1));
            let r = runtime_mut().unwrap();
            let first = r
                .scheduler
                .create(1, test_context(11, 0x1000, 0x3000, 0x100000, 0x1110))
                .unwrap();
            if second {
                let next = r
                    .scheduler
                    .create(
                        2,
                        test_context(
                            22,
                            0x5000,
                            0x7000,
                            if same_root { 0x100000 } else { 0x200000 },
                            0x2220,
                        ),
                    )
                    .unwrap();
                if blocked_peer {
                    let peer = r.scheduler.thread_mut(next).unwrap();
                    peer.state = State::Blocked;
                    peer.wait = Some(WAIT);
                }
            }
            assert_eq!(r.scheduler.schedule_next().unwrap().id, first);
        }
        fn counter(r: &mut Runtime, op: Op) -> &mut u8 {
            match op {
                Op::Yield => &mut r.yields,
                Op::Preempt => &mut r.preemptions,
                Op::Block => &mut r.blocks,
                Op::Fault => &mut r.faults,
                Op::Exit => &mut r.exits,
            }
        }
        fn snapshot() -> String {
            let r = runtime_mut().unwrap();
            let threads: Vec<_> = r
                .scheduler
                .threads
                .iter()
                .map(|t| {
                    t.map(|t| {
                        (
                            t.id,
                            t.domain,
                            t.state,
                            t.initialized,
                            t.wait,
                            t.context,
                            t.priority,
                        )
                    })
                })
                .collect();
            format!(
                "{:?}/{:?}/{:?}/{:?}/{:?}",
                threads,
                r.scheduler.generations,
                r.scheduler.current,
                r.scheduler.next_slot,
                (
                    r.yields,
                    r.preemptions,
                    r.blocks,
                    r.faults,
                    r.exits,
                    r.address_space_switches,
                    r.expected_fault_observed
                )
            )
        }
        fn trap() -> UserTrapFrame {
            UserTrapFrame::from_linux_syscall([0; 12], 0x1000, 0x3000, 0x202, 0, 0)
        }
        fn interrupt() -> UserInterruptFrame {
            UserInterruptFrame {
                r15: 15,
                r14: 14,
                r13: 13,
                r12: 12,
                r11: 11,
                r10: 10,
                r9: 9,
                r8: 8,
                rbp: 7,
                rdi: 6,
                rsi: 5,
                rdx: 4,
                rcx: 3,
                rbx: 2,
                rax: 1,
                rip: 0x1000,
                cs: CS.into(),
                rflags: 0x202,
                rsp: 0x3000,
                ss: SS.into(),
            }
        }
        fn fault() -> UserFaultFrame {
            UserFaultFrame {
                r15: 15,
                r14: 14,
                r13: 13,
                r12: 12,
                r11: 11,
                r10: 10,
                r9: 9,
                r8: 8,
                rbp: 7,
                rdi: 6,
                rsi: 5,
                rdx: 4,
                rcx: 3,
                rbx: 2,
                rax: 1,
                vector: 14,
                error: 6,
                rip: 0x1000,
                cs: CS.into(),
                rflags: 0x202,
                rsp: 0x3000,
                ss: SS.into(),
            }
        }
        fn run(op: Op, peers: bool, invalid_context: bool) -> (RuntimeAction, bool) {
            let mut t = trap();
            let mut i = interrupt();
            let mut f = fault();
            if invalid_context {
                t.rflags = 2;
                i.rflags = 2;
            }
            let ts = t.saved_context(0, 0x100000, 0x1110);
            let is = i.saved_context(0x100000, 0x1110);
            let fs = (f.rip, f.rsp, f.rcx, f.r11, f.rax);
            let action = match op {
                Op::Yield => yield_runtime(&mut t, CS, SS, 0x1110),
                Op::Preempt => preempt_runtime(&mut i, CS, SS, 0x1110),
                Op::Block => block_runtime(&mut t, CS, SS, WAIT, 0x1110),
                Op::Fault => fault_runtime(&mut f, 0, CS, SS, peers.then_some(WAIT)),
                Op::Exit => exit_runtime(&mut t, CS, SS, peers.then_some(WAIT)),
            };
            let unchanged = ts == t.saved_context(0, 0x100000, 0x1110)
                && is == i.saved_context(0x100000, 0x1110)
                && fs == (f.rip, f.rsp, f.rcx, f.r11, f.rax);
            (action, unchanged)
        }
        #[test]
        fn operation_overflow_is_atomic() {
            let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
            for op in OPS {
                setup(true, false, false);
                *counter(runtime_mut().unwrap(), op) = 255;
                let before = snapshot();
                assert_eq!(
                    run(op, false, false),
                    (RuntimeAction::Invalid, true),
                    "{op:?}"
                );
                assert_eq!(snapshot(), before, "{op:?}");
            }
        }
        #[test]
        fn address_space_overflow_is_atomic() {
            let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
            for op in OPS {
                setup(true, false, false);
                runtime_mut().unwrap().address_space_switches = 255;
                let before = snapshot();
                assert_eq!(
                    run(op, false, false),
                    (RuntimeAction::Invalid, true),
                    "{op:?}"
                );
                assert_eq!(snapshot(), before, "{op:?}");
            }
        }
        #[test]
        fn peer_wakeups_are_atomic_on_both_overflows() {
            let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
            for op in [Op::Fault, Op::Exit] {
                for switch_overflow in [false, true] {
                    setup(true, false, true);
                    if switch_overflow {
                        runtime_mut().unwrap().address_space_switches = 255;
                    } else {
                        *counter(runtime_mut().unwrap(), op) = 255;
                    }
                    let before = snapshot();
                    assert_eq!(
                        run(op, true, false),
                        (RuntimeAction::Invalid, true),
                        "{op:?}"
                    );
                    assert_eq!(
                        snapshot(),
                        before,
                        "{op:?} switch_overflow={switch_overflow}"
                    );
                }
            }
        }
        #[test]
        fn invalid_context_does_not_consume_a_counter() {
            let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
            for op in [Op::Yield, Op::Preempt, Op::Block] {
                setup(true, false, false);
                let before = snapshot();
                assert_eq!(run(op, false, true), (RuntimeAction::Invalid, true));
                assert_eq!(snapshot(), before, "{op:?}");
            }
        }
        #[test]
        fn failed_block_does_not_consume_a_counter() {
            let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
            setup(false, false, false);
            let before = snapshot();
            assert_eq!(run(Op::Block, false, false), (RuntimeAction::Invalid, true));
            assert_eq!(snapshot(), before);
        }
        #[test]
        fn successful_switch_preserves_selected_context_at_boundary() {
            let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
            for op in OPS {
                setup(true, false, false);
                *counter(runtime_mut().unwrap(), op) = 254;
                runtime_mut().unwrap().address_space_switches = 254;
                assert_eq!(
                    run(op, false, false).0,
                    RuntimeAction::Resume {
                        result: 22,
                        address_space_root: 0x200000,
                        fs_base: 0x2220
                    },
                    "{op:?}"
                );
                assert_eq!(current_domain(), Some(2));
                let r = runtime_mut().unwrap();
                assert_eq!(*counter(r, op), 255);
                assert_eq!(r.address_space_switches, 255);
            }
        }
        #[test]
        fn same_root_does_not_require_switch_counter_capacity() {
            let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
            for op in OPS {
                setup(true, true, false);
                runtime_mut().unwrap().address_space_switches = 255;
                assert!(
                    matches!(
                        run(op, false, false).0,
                        RuntimeAction::Resume {
                            address_space_root: 0x100000,
                            ..
                        }
                    ),
                    "{op:?}"
                );
                assert_eq!(current_domain(), Some(2));
            }
        }
        #[test]
        fn last_thread_exit_and_fault_complete_without_a_switch() {
            let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
            for op in [Op::Fault, Op::Exit] {
                setup(false, false, false);
                runtime_mut().unwrap().address_space_switches = 255;
                assert_eq!(run(op, false, false).0, RuntimeAction::Complete);
                assert_eq!(current_domain(), None);
                assert!(runtime_mut()
                    .unwrap()
                    .scheduler
                    .threads
                    .iter()
                    .all(Option::is_none));
            }
        }
        #[test]
        fn successful_peer_cancellation_dispatches_peer() {
            let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
            for op in [Op::Fault, Op::Exit] {
                setup(true, false, true);
                assert_eq!(
                    run(op, true, false).0,
                    RuntimeAction::Resume {
                        result: IPC_PEER_CLOSED,
                        address_space_root: 0x200000,
                        fs_base: 0x2220
                    }
                );
                assert_eq!(current_domain(), Some(2));
            }
        }
        #[test]
        fn existing_invariants_hold() {
            let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
            assert!(verify_scheduler_invariants());
        }
    }
}
