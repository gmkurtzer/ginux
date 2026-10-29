# Repository Skills

This directory is the canonical source for Ginux Agent Skills. The same skill
content is exposed to supported coding agents through Git-tracked relative
symlinks:

```text
.agents/skills/<name> -> ../../skills/<name>
.claude/skills/<name> -> ../../skills/<name>
```

- Codex discovers the `.agents/skills` links and invokes a skill as
  `$skill-name`.
- Claude Code discovers the `.claude/skills` links and invokes a skill as
  `/skill-name`.

Edit only the canonical directory under `skills/`. Do not copy or independently
modify agent-specific versions.

Available skills:

- `hdae-development`: teaches and applies the complete HDAE method for new or
  existing projects: human direction, architecture, milestones, executable
  gates, autonomous implementation loops, evidence, and stopping points.
- `pr-issue-review`: discovers repository access, asks for cadence and scope,
  creates a recurring review schedule, and performs evidence-based PR and issue
  review runs without repository mutation by default.

Both skills are repository-agnostic and may be copied with their complete
directories into another project's skill discovery path.

## Copy-paste prompts

Start a new agent session from the repository after checking out the branch
containing the discovery links.

### HDAE development

Codex:

```text
Use $hdae-development for this project.

Project brief:
- Outcome: <Describe the concrete system behavior or result to create.>
- Purpose and users: <Explain who needs it and why it matters.>
- Constraints and non-goals: <State architectural, operational, schedule, or
  scope boundaries.>
- Existing behavior to preserve: <Name compatibility, interfaces, data,
  workflows, or performance that must not regress.>
- Success evidence: <Describe the tests, demonstrations, measurements, or
  review results that would prove the outcome.>

Begin with focused Q&A and repository inspection. Help me correct or complete
the project brief before planning. Propose a complete gate map with acceptance
evidence and stopping points, perform the gate coverage review, and ask me what
is missing or incorrectly scoped. Do not implement anything until I approve a
specific gate, and stop for my review at every gate boundary.
```

Claude Code:

```text
/hdae-development Use this project brief:

- Outcome: <Describe the concrete system behavior or result to create.>
- Purpose and users: <Explain who needs it and why it matters.>
- Constraints and non-goals: <State architectural, operational, schedule, or
  scope boundaries.>
- Existing behavior to preserve: <Name compatibility, interfaces, data,
  workflows, or performance that must not regress.>
- Success evidence: <Describe the tests, demonstrations, measurements, or
  review results that would prove the outcome.>

Begin with focused Q&A and repository inspection. Help me correct or complete
the project brief before planning. Propose a complete gate map with acceptance
evidence and stopping points, perform the gate coverage review, and ask me what
is missing or incorrectly scoped. Do not implement anything until I approve a
specific gate, and stop for my review at every gate boundary.
```

The brief is a structure, not a single keyword. Use complete statements or mark
unknown fields explicitly so the agent resolves them through Q&A. A Ginux
hardening example is:

```text
- Outcome: Turn external technical feedback into an evidence-driven kernel
  hardening iteration covering syscall-entry stack safety, recoverable
  userspace memory access, mutable global state, synchronization, SMP readiness,
  and code decomposition.
- Purpose and users: Determine whether HDAE can convert expert review into a
  coherent second research iteration rather than isolated patches.
- Constraints and non-goals: Preserve the native-domain architecture; do not
  claim production security or complete Linux compatibility.
- Existing behavior to preserve: Rocky Linux systemd boot, Linux userspace ABI
  gates, multi-domain behavior, and recorded performance checkpoints.
- Success evidence: Each approved hardening gate passes its positive, negative,
  regression, documentation, and end-to-end QEMU acceptance checks.
```

### Recurring PR and issue review

Codex:

```text
Use $pr-issue-review to set up recurring, read-only reviews of new or updated
issues and pull requests for this repository. Discover and validate the GitHub
repository and authentication, then ask me how often and when the review should
run. Preview one review before creating the schedule. Do not post, label, close,
approve, merge, commit, or push anything.
```

Claude Code:

```text
/pr-issue-review Set up recurring, read-only reviews of new or updated issues
and pull requests for this repository. Discover and validate the GitHub
repository and authentication, then ask me how often and when the review should
run. Preview one review before creating the schedule. Do not post, label, close,
approve, merge, commit, or push anything.
```
