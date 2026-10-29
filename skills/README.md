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
- `hdae-review-scheduler`: applies HDAE gates to the design, dry run, creation,
  and first-run verification of recurring read-only issue and pull-request
  reviews.

The skills define repeatable procedures. `hdae-review-scheduler` configures a
supported scheduler or GitHub event workflow to invoke the triage procedure;
it does not grant that automation maintainer mutation authority.
