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
