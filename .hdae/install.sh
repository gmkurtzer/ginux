#!/bin/sh

set -eu

PROGRAM=${0##*/}
STDIN_INSTALL=0
case "$PROGRAM" in
    sh|dash|bash|ksh|zsh)
        STDIN_INSTALL=1
        PROGRAM=hdae-install
        ;;
esac
REPOSITORY_URL=${HDAE_REPOSITORY_URL:-https://github.com/gmkurtzer/hdae.git}
TARGET=
INSTALL_CODEX=0
INSTALL_CLAUDE=0
MODE=copy
VERSION=
UPDATE=0
UNINSTALL=0
FORCE=0
TEMP_DIR=
BOOTSTRAP_START='<!-- hdae:context-recovery:start -->'
BOOTSTRAP_END='<!-- hdae:context-recovery:end -->'
BOOTSTRAP_MANAGED=0

usage() {
    cat <<EOF
Usage: $PROGRAM [OPTIONS]

Install HDAE Agent Skills into a project.

  --all               Install discovery links for Codex and Claude Code
  --codex             Install discovery links for Codex
  --claude            Install discovery links for Claude Code
  --copy              Copy skills into the target project (default)
  --link              Link agents directly to this HDAE checkout
  --target DIRECTORY  Select the target project (default: Git root or cwd)
  --update            Fetch HDAE and replace an installer-managed copy
  --version REF       Git tag, branch, or commit used with --update
  --uninstall         Remove an installer-managed HDAE installation
  --force             Replace locally modified installed skills
  --help              Show this help
EOF
}

die() {
    printf '%s: %s\n' "$PROGRAM" "$*" >&2
    exit 1
}

cleanup() {
    if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}

trap cleanup EXIT HUP INT TERM

absolute_dir() {
    (cd "$1" 2>/dev/null && pwd -P)
}

sha256_stream() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 | awk '{print $1}'
    elif command -v openssl >/dev/null 2>&1; then
        openssl dgst -sha256 | awk '{print $NF}'
    else
        die "a SHA-256 tool is required (sha256sum, shasum, or openssl)"
    fi
}

tree_digest() {
    directory=$1
    [ -d "$directory" ] || die "cannot checksum missing directory: $directory"
    (
        cd "$directory"
        find . \( -type f -o -type l \) -print | LC_ALL=C sort | while IFS= read -r file; do
            if [ -L "$file" ]; then
                printf 'link %s\n' "$file"
                readlink "$file"
            else
                # Preserve the historical sha256sum/shasum record format while
                # normalizing the hash utility output itself.
                printf '%s\n' "$file"
                printf '%s  %s\n' "$(sha256_stream <"$file")" "$file"
            fi
        done
    ) | sha256_stream
}

manifest_value() {
    key=$1
    file=$2
    [ -f "$file" ] || return 1
    sed -n "s/^${key}=//p" "$file" | sed -n '1p'
}

remove_managed_link() {
    link=$1
    expected=$2
    if [ -L "$link" ]; then
        actual=$(readlink "$link")
        if [ "$actual" = "$expected" ]; then
            rm "$link"
        fi
    fi
}

print_agents_bootstrap() {
    cat <<'EOF'
<!-- hdae:context-recovery:start -->
## HDAE context recovery

For every task that uses HDAE, and after any context compaction, reset, resume,
or handoff, read `.agents/skills/hdae-development/SKILL.md` completely before
continuing. Then read the skill-directed references and authoritative HDAE
project checkpoint. Do not rely on compacted chat as a substitute.
<!-- hdae:context-recovery:end -->
EOF
}

extract_agents_bootstrap() {
    file=$1
    sed -n "/^${BOOTSTRAP_START}$/,/^${BOOTSTRAP_END}$/p" "$file"
}

validate_agents_bootstrap() {
    file=$TARGET/AGENTS.md
    [ ! -L "$file" ] || die "refusing to modify symlinked AGENTS.md: $file"
    [ ! -e "$file" ] || [ -f "$file" ] || die "AGENTS.md is not a regular file: $file"
    [ -f "$file" ] || return 0

    start_count=$(grep -Fxc "$BOOTSTRAP_START" "$file" || true)
    end_count=$(grep -Fxc "$BOOTSTRAP_END" "$file" || true)
    if [ "$start_count" -eq 0 ] && [ "$end_count" -eq 0 ]; then
        return
    fi
    if [ "$start_count" -ne 1 ] || [ "$end_count" -ne 1 ]; then
        die "AGENTS.md contains incomplete or duplicate HDAE recovery markers"
    fi

    recorded_managed=$(manifest_value agents_bootstrap "$MANIFEST" || true)
    recorded_digest=$(manifest_value agents_bootstrap_digest "$MANIFEST" || true)
    current_digest=$(extract_agents_bootstrap "$file" | sha256_stream)
    desired_digest=$(print_agents_bootstrap | sha256_stream)
    if [ "$recorded_managed" = 1 ]; then
        if [ "$FORCE" -ne 1 ] && { [ -z "$recorded_digest" ] || [ "$recorded_digest" != "$current_digest" ]; }; then
            die "installer-managed HDAE block in AGENTS.md was modified; inspect it or rerun with --force"
        fi
        BOOTSTRAP_MANAGED=1
    elif [ "$current_digest" != "$desired_digest" ]; then
        die "AGENTS.md already contains unmanaged HDAE recovery markers"
    fi
}

remove_agents_bootstrap() {
    file=$TARGET/AGENTS.md
    [ -f "$file" ] || return 0
    temp=$file.hdae.$$
    sed "/^${BOOTSTRAP_START}$/,/^${BOOTSTRAP_END}$/d" "$file" >"$temp"
    cat "$temp" >"$file"
    rm -f "$temp"
    [ -s "$file" ] || rm -f "$file"
}

install_agents_bootstrap() {
    file=$TARGET/AGENTS.md
    if [ "$BOOTSTRAP_MANAGED" -eq 1 ]; then
        remove_agents_bootstrap
    elif [ -f "$file" ] && grep -Fqx "$BOOTSTRAP_START" "$file"; then
        return
    fi

    if [ -s "$file" ]; then
        printf '\n' >>"$file"
    fi
    print_agents_bootstrap >>"$file"
    BOOTSTRAP_MANAGED=1
}

for argument in "$@"; do
    case "$argument" in
        --all) INSTALL_CODEX=1; INSTALL_CLAUDE=1 ;;
        --codex) INSTALL_CODEX=1 ;;
        --claude) INSTALL_CLAUDE=1 ;;
        --copy) MODE=copy ;;
        --link) MODE='link' ;;
        --update) UPDATE=1 ;;
        --uninstall) UNINSTALL=1 ;;
        --force) FORCE=1 ;;
        --help|-h) usage; exit 0 ;;
    esac
done

while [ "$#" -gt 0 ]; do
    case "$1" in
        --target)
            [ "$#" -ge 2 ] || die "--target requires a directory"
            TARGET=$2
            shift 2
            ;;
        --version)
            [ "$#" -ge 2 ] || die "--version requires a Git reference"
            VERSION=$2
            shift 2
            ;;
        --all|--codex|--claude|--copy|--link|--update|--uninstall|--force)
            shift
            ;;
        --help|-h)
            shift
            ;;
        *)
            die "unknown option: $1"
            ;;
    esac
done

if [ -n "$VERSION" ]; then
    UPDATE=1
fi

if [ "$MODE" = link ] && [ "$UPDATE" -eq 1 ]; then
    die "--link cannot be combined with --update or --version; update the checkout separately"
fi

if [ -z "$TARGET" ]; then
    TARGET=$(git rev-parse --show-toplevel 2>/dev/null || pwd -P)
fi

[ -d "$TARGET" ] || die "target directory does not exist: $TARGET"
TARGET=$(absolute_dir "$TARGET")
HDAE_DIR=$TARGET/.hdae
MANIFEST=$HDAE_DIR/manifest

if [ "$UNINSTALL" -eq 1 ]; then
    [ -f "$MANIFEST" ] || die "no installer-managed HDAE copy found in $TARGET"
    installed_digest=$(manifest_value skills_digest "$MANIFEST" || true)
    current_digest=$(tree_digest "$HDAE_DIR/skills")
    if [ "$FORCE" -ne 1 ] && [ "$installed_digest" != "$current_digest" ]; then
        die "installed skills were modified; inspect them or rerun with --force"
    fi
    bootstrap_managed=$(manifest_value agents_bootstrap "$MANIFEST" || true)
    if [ "$bootstrap_managed" = 1 ]; then
        validate_agents_bootstrap
    fi
    remove_managed_link "$TARGET/.agents/skills/hdae-development" "../../.hdae/skills/hdae-development"
    remove_managed_link "$TARGET/.agents/skills/pr-issue-review" "../../.hdae/skills/pr-issue-review"
    remove_managed_link "$TARGET/.claude/skills/hdae-development" "../../.hdae/skills/hdae-development"
    remove_managed_link "$TARGET/.claude/skills/pr-issue-review" "../../.hdae/skills/pr-issue-review"
    if [ "$bootstrap_managed" = 1 ]; then
        remove_agents_bootstrap
    fi
    rm -rf "$HDAE_DIR/skills"
    rm -f "$HDAE_DIR/install.sh" "$HDAE_DIR/VERSION" "$MANIFEST"
    rmdir "$HDAE_DIR" 2>/dev/null || true
    printf 'Removed installer-managed HDAE skills from %s\n' "$TARGET"
    exit 0
fi

if [ "$INSTALL_CODEX" -eq 0 ] && [ "$INSTALL_CLAUDE" -eq 0 ]; then
    if [ -t 0 ]; then
        printf 'Install HDAE for:\n  1) Codex\n  2) Claude Code\n  3) Both\nSelection [3]: '
        IFS= read -r selection
        case ${selection:-3} in
            1) INSTALL_CODEX=1 ;;
            2) INSTALL_CLAUDE=1 ;;
            3) INSTALL_CODEX=1; INSTALL_CLAUDE=1 ;;
            *) die "invalid selection" ;;
        esac
    else
        INSTALL_CODEX=1
        INSTALL_CLAUDE=1
    fi
fi

if [ "$STDIN_INSTALL" -eq 1 ]; then
    SCRIPT_DIR=
    SOURCE_ROOT=
    SOURCE_LABEL=standard-input
else
    SCRIPT_DIR=$(absolute_dir "$(dirname "$0")")
    SOURCE_ROOT=$SCRIPT_DIR
    SOURCE_LABEL=$SCRIPT_DIR
fi

if [ "$UPDATE" -eq 1 ] || [ -z "$SOURCE_ROOT" ] || [ ! -d "$SOURCE_ROOT/skills/hdae-development" ]; then
    command -v git >/dev/null 2>&1 || die "git is required to fetch HDAE"
    TEMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/hdae.XXXXXX")
    ref=${VERSION:-main}
    if ! git clone --quiet --depth 1 --branch "$ref" "$REPOSITORY_URL" "$TEMP_DIR/source" 2>/dev/null; then
        git clone --quiet "$REPOSITORY_URL" "$TEMP_DIR/source"
        (cd "$TEMP_DIR/source" && git checkout --quiet "$ref") || die "cannot resolve HDAE version: $ref"
    fi
    SOURCE_ROOT=$TEMP_DIR/source
    SOURCE_LABEL=$REPOSITORY_URL@$ref
fi

for skill in hdae-development pr-issue-review; do
    [ -f "$SOURCE_ROOT/skills/$skill/SKILL.md" ] || die "source is missing skill: $skill"
done

if [ "$MODE" = link ]; then
    case "$SOURCE_ROOT" in
        "$TARGET"/*) ;;
        *)
            printf '%s: warning: links to %s will not be portable with the project\n' "$PROGRAM" "$SOURCE_ROOT" >&2
            ;;
    esac
    SKILLS_ROOT=$SOURCE_ROOT/skills
    INSTALLED_VERSION=${VERSION:-$(sed -n '1p' "$SOURCE_ROOT/VERSION" 2>/dev/null || printf 'development')}
else
    SKILLS_ROOT=$HDAE_DIR/skills
    INSTALLED_VERSION=${VERSION:-$(sed -n '1p' "$SOURCE_ROOT/VERSION" 2>/dev/null || printf 'development')}
fi

desired_link() {
    skill=$1
    if [ "$MODE" = copy ]; then
        printf '../../.hdae/skills/%s\n' "$skill"
    else
        printf '%s/%s\n' "$SKILLS_ROOT" "$skill"
    fi
}

validate_link() {
    agent_dir=$1
    skill=$2
    destination=$agent_dir/skills/$skill
    desired=$(desired_link "$skill")

    if [ -e "$destination" ] || [ -L "$destination" ]; then
        if [ -L "$destination" ] && [ "$(readlink "$destination")" = "$desired" ]; then
            return
        fi
        die "discovery path already exists: $destination"
    fi
}

if [ "$INSTALL_CODEX" -eq 1 ]; then
    validate_link "$TARGET/.agents" hdae-development
    validate_link "$TARGET/.agents" pr-issue-review
    if [ "$MODE" = copy ]; then
        validate_agents_bootstrap
    fi
fi

if [ "$INSTALL_CLAUDE" -eq 1 ]; then
    validate_link "$TARGET/.claude" hdae-development
    validate_link "$TARGET/.claude" pr-issue-review
fi

if [ "$MODE" = copy ]; then
    if [ -d "$HDAE_DIR/skills" ]; then
        recorded_digest=$(manifest_value skills_digest "$MANIFEST" || true)
        current_digest=$(tree_digest "$HDAE_DIR/skills")
        if [ "$FORCE" -ne 1 ] && { [ -z "$recorded_digest" ] || [ "$recorded_digest" != "$current_digest" ]; }; then
            die "installed skills were modified or are unmanaged; inspect them or rerun with --force"
        fi
    fi

    mkdir -p "$HDAE_DIR"
    stage=$HDAE_DIR/skills.new.$$
    rm -rf "$stage"
    mkdir -p "$stage"
    cp -R "$SOURCE_ROOT/skills/hdae-development" "$stage/"
    cp -R "$SOURCE_ROOT/skills/pr-issue-review" "$stage/"
    rm -rf "$HDAE_DIR/skills"
    mv "$stage" "$HDAE_DIR/skills"
    if [ "$SOURCE_ROOT/install.sh" != "$HDAE_DIR/install.sh" ]; then
        cp "$SOURCE_ROOT/install.sh" "$HDAE_DIR/install.sh"
        chmod +x "$HDAE_DIR/install.sh"
    fi
    if [ -f "$SOURCE_ROOT/VERSION" ] && [ "$SOURCE_ROOT/VERSION" != "$HDAE_DIR/VERSION" ]; then
        cp "$SOURCE_ROOT/VERSION" "$HDAE_DIR/VERSION"
    fi
fi

install_link() {
    agent_dir=$1
    skill=$2
    mkdir -p "$agent_dir/skills"
    destination=$agent_dir/skills/$skill

    desired=$(desired_link "$skill")

    if [ -e "$destination" ] || [ -L "$destination" ]; then
        if [ -L "$destination" ] && [ "$(readlink "$destination")" = "$desired" ]; then
            return
        fi
        die "discovery path already exists: $destination"
    fi
    ln -s "$desired" "$destination"
}

if [ "$INSTALL_CODEX" -eq 1 ]; then
    install_link "$TARGET/.agents" hdae-development
    install_link "$TARGET/.agents" pr-issue-review
    if [ "$MODE" = copy ]; then
        install_agents_bootstrap
    fi
fi

if [ "$INSTALL_CLAUDE" -eq 1 ]; then
    install_link "$TARGET/.claude" hdae-development
    install_link "$TARGET/.claude" pr-issue-review
fi

if [ "$MODE" = copy ]; then
    digest=$(tree_digest "$HDAE_DIR/skills")
    cat >"$MANIFEST" <<EOF
version=$INSTALLED_VERSION
source=$SOURCE_LABEL
mode=copy
skills_digest=$digest
agents_bootstrap=$BOOTSTRAP_MANAGED
agents_bootstrap_digest=$(print_agents_bootstrap | sha256_stream)
EOF
fi

printf 'HDAE %s installed in %s\n' "$INSTALLED_VERSION" "$TARGET"
# The dollar sign below is Codex skill-invocation syntax, not a shell variable.
# shellcheck disable=SC2016
[ "$INSTALL_CODEX" -eq 0 ] || printf '%s\n' 'Codex: Use $hdae-development to help me develop this project.'
[ "$INSTALL_CLAUDE" -eq 0 ] || printf '%s\n' 'Claude: /hdae-development Help me develop this project.'
