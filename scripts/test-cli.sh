#!/bin/sh
set -eu

cargo build -q -p ginux-cli
cli=target/debug/ginux-cli
test -x "$cli"

test_directory=$(mktemp -d "${TMPDIR:-/tmp}/ginux-cli-test.XXXXXX")
cleanup() {
    rm -rf "$test_directory"
}
trap cleanup 0 HUP INT TERM

"$cli" --help >"$test_directory/help"
"$cli" --version | grep -Eq '^ginux [0-9]+\.[0-9]+\.[0-9]+'
for command in create import set edit export list status start attach stop lock unlock validate inspect explain; do
    grep -Eq "^  $command[[:space:]]" "$test_directory/help"
    "$cli" "$command" --help >"$test_directory/help-$command"
    test -s "$test_directory/help-$command"
done
! grep -Eq '^  (diff|native|native-hook|launch)[[:space:]]' "$test_directory/help"

"$cli" validate config/instance-rocky-systemd.toml >"$test_directory/validate"
"$cli" inspect config/instance-rocky-systemd.toml >"$test_directory/inspect"
"$cli" explain config/instance-rocky-systemd.toml filesystem.path.read \
    >"$test_directory/explain"
grep -Fq 'Contract valid:' "$test_directory/validate"
grep -Fq 'Decisions:' "$test_directory/inspect"
grep -Fq 'Operation: filesystem.path.read' "$test_directory/explain"

mkdir "$test_directory/bin"
printf '%s\n' '#!/bin/sh' ": >'$test_directory/sudo-called'" 'exit 99' \
    >"$test_directory/bin/sudo"
chmod 755 "$test_directory/bin/sudo"
for invocation in \
    'list extra' \
    'status' \
    'set rocky' \
    'start rocky --unknown' \
    'diff left right' \
    'native config root'; do
    set +e
    PATH="$test_directory/bin:$PATH" "$cli" $invocation \
        >"$test_directory/rejected.out" 2>"$test_directory/rejected.err"
    status=$?
    set -e
    test "$status" -eq 2
    test ! -e "$test_directory/sudo-called"
done

set +e
PATH="$test_directory/bin:$PATH" "$cli" set rocky \
    --forward none \
    --forward protocol=tcp,external_port=8080,instance_port=80 \
    >"$test_directory/conflict.out" 2>"$test_directory/conflict.err"
status=$?
set -e
test "$status" -eq 2
test ! -e "$test_directory/sudo-called"
grep -Fq -- '--forward none conflicts' "$test_directory/conflict.err"

printf '%s\n' '#!/bin/sh' 'printf "%s\n" "$@" >"$GINUX_TEST_HELPER_ARGS"' 'exit 0' \
    >"$test_directory/bin/sudo"
chmod 755 "$test_directory/bin/sudo"
GINUX_TEST_HELPER_ARGS="$test_directory/helper-args" \
GINUX_INSTANCE_HELPER=/private/ginux-instance-helper \
PATH="$test_directory/bin:$PATH" \
    "$cli" create rocky \
        --set resources.vcpus=2 \
        --network default \
        --forward protocol=tcp,external_port=8080,instance_port=80
cat >"$test_directory/expected-helper-args" <<'EOF'
/private/ginux-instance-helper
--protocol
1
create
rocky
--set
resources.vcpus=2
--network
default
--forward
protocol=tcp,external_port=8080,instance_port=80
EOF
cmp "$test_directory/expected-helper-args" "$test_directory/helper-args"

set +e
./scripts/ginux-host-instance-helper.sh list >/dev/null 2>&1
unversioned_status=$?
./scripts/ginux-host-instance-helper.sh --protocol 99 list >/dev/null 2>&1
unsupported_status=$?
set -e
test "$unversioned_status" -eq 64
test "$unsupported_status" -eq 64

printf '%s\n' 'CLI_TEST: PASS (generated help, approved inventory, parser safety, versioned helper routing)'
