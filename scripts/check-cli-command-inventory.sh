#!/bin/sh
set -eu

cargo build -q -p ginux-cli
cli=target/debug/ginux-cli
document=docs/cli-command-model.md

inventory() {
    sed -n "s/^<!-- cli-inventory:$1 \(.*\) -->$/\1/p" "$document" |
        tr ' ' '\n' |
        LC_ALL=C sort
}

inventory public >build/cli-public.expected
"$cli" --help |
    sed -n '/^Commands:/,/^$/p' |
    awk 'NF >= 2 && $1 != "Commands:" { print $1 }' |
    LC_ALL=C sort >build/cli-public.actual
cmp build/cli-public.expected build/cli-public.actual

inventory helper >build/cli-helper.expected
sed -n '/^case \$command in$/,/^esac$/p' scripts/ginux-host-instance-helper.sh |
    sed -n 's/^[[:space:]]*\([a-z][a-z-]*\)).*/\1/p' |
    LC_ALL=C sort >build/cli-helper.actual
cmp build/cli-helper.expected build/cli-helper.actual

inventory runtime >build/cli-runtime.expected
grep 'c_string_equals(command, b"' crates/ginux-init/src/launch.rs |
    sed 's/.*c_string_equals(command, b"\([^"]*\)").*/\1/' |
    LC_ALL=C sort -u >build/cli-runtime.actual
cmp build/cli-runtime.expected build/cli-runtime.actual

printf '%s\n' 'CLI_INVENTORY: PASS (public, helper, and runtime command surfaces classified)'
