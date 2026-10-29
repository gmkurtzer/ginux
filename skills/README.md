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

- `hdae-gated-development`: creates and executes architecture-led plans with
  explicit acceptance gates and review stopping points.
- `maintainer-triage`: monitors and triages issues and pull requests, then
  drafts evidence-based maintainer responses without external mutation by
  default.

The skills define repeatable procedures. Persistent monitoring still requires
a scheduler or GitHub event workflow to invoke `maintainer-triage`.
