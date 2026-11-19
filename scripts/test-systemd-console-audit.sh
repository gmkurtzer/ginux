#!/bin/sh
set -eu

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/ginux-console-audit.XXXXXX")
cleanup() {
    rm -rf "$work_dir"
}
trap cleanup 0 HUP INT TERM

write_header() {
    printf '%s\n' 'GINUX_INSTANCE_START: id=console-audit-test' >"$1"
}

accepted_log="$work_dir/accepted.log"
write_header "$accepted_log"
printf '%s\n' \
    'emulate surface=unsupported-unix-address length=29 result=-1' \
    >>"$accepted_log"
./scripts/check-systemd-console.sh "$accepted_log" >/dev/null

for diagnostic in \
    'emulate surface=unsupported-unix-address length=invalid result=-1' \
    'emulate surface=unsupported-unix-address length=29 result=0' \
    'systemd: unsupported runtime feature'
do
    rejected_log="$work_dir/rejected.log"
    write_header "$rejected_log"
    printf '%s\n' "$diagnostic" >>"$rejected_log"
    if ./scripts/check-systemd-console.sh "$rejected_log" >/dev/null 2>&1; then
        printf 'console audit unexpectedly accepted: %s\n' "$diagnostic" >&2
        exit 1
    fi
done

printf '%s\n' 'systemd console audit regression: PASS'
