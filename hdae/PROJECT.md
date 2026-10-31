# Feedback-Driven Kernel Hardening

## Purpose

This secondary Ginux research experiment tests whether concrete external kernel
engineering feedback can be converted into a disciplined, evidence-driven
hardening iteration using Human-Directed Agentic Engineering (HDAE).

The published initial experiment remains identified by the
`research-v1-baseline` tag. Work for this experiment is performed on
`codex/feedback-hardening`; the initial ten-gate batch used delegated closure
after complete fresh evidence. Follow-up topology measurement and its inserted
correctness gates retain explicit review policies in their gate records.
Gate 12 is a separate follow-up experiment on `codex/performance-parity`: it
tests whether profiling-guided implementation work can bring every matched UP
and SMP userspace benchmark to Linux parity without weakening correctness.

## Intended outcome

Produce a better structured and more defensible kernel implementation by:

- reproducing each reported weakness before changing it;
- defining explicit security, ownership, and concurrency invariants;
- retaining each reproduction as a regression test;
- hardening privilege entry and userspace memory access;
- replacing ambient mutable state with explicit ownership;
- establishing synchronization before enabling SMP;
- decomposing the x86-64 implementation without changing behavior; and
- qualifying the result against the original compatibility and performance
  evidence.

## Scope

The experiment covers the five review themes recorded in
[`docs/FEEDBACK-HARDENING-RESEARCH.md`](../docs/FEEDBACK-HARDENING-RESEARCH.md):
privilege entry, recoverable userspace access, mutable global state,
synchronization/SMP, and x86-64 code structure.

SMP is a required delivered capability, not only a readiness exercise. After
the SMP gate passes, ordinary QEMU regression commands default to two virtual
CPUs, while dedicated topology qualification continues to cover one, two, and
four virtual CPUs.

Gate 12 covers CPU, memory, thread-yield, and mutex performance at fixed one-
thread and scaled one-, two-, and four-vCPU topologies. Its target is to beat
the matched Rocky-on-Linux median in every cell; passage requires every
favorable-direction ratio to reach at least 0.96x.

## Non-goals

- Do not rewrite history or alter the published baseline tag.
- Do not claim production security from passing the planned tests.
- Do not expand this experiment into a general security audit; its security
  scope is the reported privilege-entry and userspace-access defects.
- Do not begin SMP before ownership and synchronization gates pass.
- Do not combine structural refactoring with security fixes in a way that
  obscures their evidence.
- Do not weaken existing Linux ABI, Rocky boot, domain, storage, or performance
  gates to make a hardening gate pass.
- Do not special-case sysbench, change the workload, or give Linux and Ginux
  different benchmark resources to make the performance gate pass.

## Human direction

The human approves the gate order, scope, acceptance criteria, architectural
tradeoffs, and review policy. The agent performs bounded implementation and
verification loops inside the approved batch and reports failed approaches and
limitations as evidence.
