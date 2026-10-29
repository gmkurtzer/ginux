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
Use $hdae-development to help me develop this project.
```

Claude Code:

```text
/hdae-development Help me develop this project.
```

The user may begin with only an idea. The skill makes the agent responsible for
eliciting and confirming the structured project state, persisting it before
implementation, proposing complete gates, and stopping for human direction.

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
