# Feedback-Driven Kernel Hardening

## Purpose

This secondary Ginux research experiment tests whether concrete external kernel
engineering feedback can be converted into a disciplined, evidence-driven
hardening iteration using Human-Directed Agentic Engineering (HDAE).

The published initial experiment remains identified by the
`research-v1-baseline` tag. Work for this experiment is performed on
`codex/feedback-hardening` and stops for human review at every gate.

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

## Non-goals

- Do not rewrite history or alter the published baseline tag.
- Do not claim production security from passing the planned tests.
- Do not begin SMP before ownership and synchronization gates pass.
- Do not combine structural refactoring with security fixes in a way that
  obscures their evidence.
- Do not weaken existing Linux ABI, Rocky boot, domain, storage, or performance
  gates to make a hardening gate pass.

## Human direction

The human approves the gate order, scope, acceptance criteria, architectural
tradeoffs, and every transition between gates. The agent performs bounded
implementation and verification loops inside the approved gate and reports
failed approaches and limitations as evidence.
