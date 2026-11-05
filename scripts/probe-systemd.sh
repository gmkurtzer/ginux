#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
GINUX_OCI_CONFIG_FILE=./config/rocky-systemd-oci.env
export GINUX_OCI_CONFIG_FILE
. ./scripts/oci-common.sh

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
domain_root=/var/lib/ginux/domains/gate9/root
log=build/gate9-systemd-baseline.log

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/domain-gate9.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate9.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-oci-rootfs.tar"
rocky_scp scripts/gate9-shell-check.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate9-check"

rocky_ssh "set -eu; sudo rm -rf /var/lib/ginux/domains/gate9; sudo mkdir -p '$domain_root'; sudo tar -xf /var/tmp/ginux-oci-rootfs.tar -C '$domain_root'; sudo mkdir -p '$domain_root/run' '$domain_root/proc/1' '$domain_root/proc/self/fd' '$domain_root/sys/fs/cgroup/systemd' '$domain_root/sys/fs/cgroup/init.scope' '$domain_root/sys/fs/cgroup/system.slice/console-getty.service' '$domain_root/sys/fs/cgroup/system.slice/systemd-user-sessions.service' '$domain_root/dev/shm' '$domain_root/dev/pts' '$domain_root/usr/local/sbin'; printf '%s\\000' '/sbin/init --system --unit=multi-user.target' | sudo tee '$domain_root/proc/1/cmdline' >/dev/null; printf '0::/\\n' | sudo tee '$domain_root/proc/1/cgroup' '$domain_root/proc/self/cgroup' >/dev/null; printf 'cpu memory pids\\n' | sudo tee '$domain_root/sys/fs/cgroup/cgroup.controllers' >/dev/null; sudo touch '$domain_root/sys/fs/cgroup/cgroup.procs' '$domain_root/sys/fs/cgroup/cgroup.subtree_control' '$domain_root/sys/fs/cgroup/init.scope/cgroup.procs' '$domain_root/sys/fs/cgroup/system.slice/console-getty.service/cgroup.procs' '$domain_root/sys/fs/cgroup/system.slice/console-getty.service/cgroup.events' '$domain_root/sys/fs/cgroup/system.slice/systemd-user-sessions.service/cgroup.procs' '$domain_root/sys/fs/cgroup/system.slice/systemd-user-sessions.service/cgroup.events'; sudo mknod -m 600 '$domain_root/dev/console' c 5 0; sudo ln -s / '$domain_root/proc/1/root'; sudo install -m 755 /var/tmp/ginux-gate9-check '$domain_root/usr/local/sbin/ginux-gate9-check'; sudo rm -f '$domain_root/etc/systemd/system/getty.target' '$domain_root/etc/systemd/system/console-getty.service'; sudo ln -sf /usr/lib/systemd/system/console-getty.service '$domain_root/etc/systemd/system/getty.target.wants/console-getty.service'; printf '%s\\n' '#%PAM-1.0' 'auth required pam_unix.so' 'account required pam_unix.so' 'password required pam_unix.so' 'session required pam_unix.so' | sudo tee '$domain_root/etc/pam.d/login' >/dev/null; printf 'root:ginux\\n' | sudo chroot '$domain_root' /usr/sbin/chpasswd; sudo chown -R 1000:1000 '$domain_root'; sudo chmod 600 '$domain_root/etc/shadow'; test -x '$domain_root/sbin/init'"

set +e
./scripts/expect-systemd-login.exp \
    "$ROCKY_KEY" "$ROCKY_SSH_PORT" "$ROCKY_SSH_USER" "$domain_root" "$log" \
    >/dev/null
status=$?
set -e

printf 'systemd baseline exit: %s\n' "$status"
tail -n 40 "$log"
rocky_ssh 'sudo rm -rf /var/lib/ginux/domains/gate9; rm -f /var/tmp/ginux /var/tmp/gate9.toml /var/tmp/ginux-oci-rootfs.tar /var/tmp/ginux-gate9-check'

test "$status" -eq 0
grep -Fq 'Reached target multi-user.target - Multi-User System.' "$log"
grep -Fq 'GINUX_AUTHENTICATED' "$log"
grep -Fq 'GINUX_LOCKDOWN_HOST_PATHS: BLOCKED' "$log"
grep -Fq 'GINUX_LOCKDOWN_DEVICES: BLOCKED' "$log"
grep -Fq 'GINUX_LOCKDOWN_MOUNT: BLOCKED' "$log"
grep -Fq 'GINUX_LOCKDOWN_NAMESPACE: BLOCKED' "$log"
grep -Fq 'GINUX_SHELL_CHECK_DONE' "$log"
printf '%s\n' 'systemd login baseline: PASS (authenticated managed shell and returned to login)'
