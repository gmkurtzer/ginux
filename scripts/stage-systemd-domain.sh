#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
GINUX_OCI_CONFIG_FILE=./config/rocky-systemd-oci.env
export GINUX_OCI_CONFIG_FILE
. ./scripts/oci-common.sh

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
domain_root=/var/lib/ginux/domains/gate9/root
gate9_dnf=${GINUX_GATE9_DNF:-on}
gate9_network=${GINUX_GATE9_NETWORK:-on}

case $gate9_dnf in
    on|off|failure) ;;
    *) printf '%s\n' 'GINUX_GATE9_DNF must be on, off, or failure' >&2; exit 64 ;;
esac
case $gate9_network in
    on|off) ;;
    *) printf '%s\n' 'GINUX_GATE9_NETWORK must be on or off' >&2; exit 64 ;;
esac

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/domain-gate9.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate9.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-oci-rootfs.tar"
rocky_scp scripts/gate9-shell-check.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate9-check"
rocky_scp scripts/gate9-network-check.py \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate9-network-check"

rocky_ssh "set -eu; sudo rm -rf /var/lib/ginux/domains/gate9; sudo mkdir -p '$domain_root'; sudo tar -xf /var/tmp/ginux-oci-rootfs.tar -C '$domain_root'; sudo mkdir -p '$domain_root/run' '$domain_root/proc/1' '$domain_root/proc/self/fd' '$domain_root/sys/fs/cgroup/systemd' '$domain_root/sys/fs/cgroup/init.scope' '$domain_root/sys/fs/cgroup/system.slice/console-getty.service' '$domain_root/sys/fs/cgroup/system.slice/systemd-user-sessions.service' '$domain_root/dev/shm' '$domain_root/dev/pts' '$domain_root/usr/local/sbin'; sudo touch '$domain_root/etc/resolv.conf'; if test -e /etc/resolv.conf; then sudo cp /etc/resolv.conf '$domain_root/etc/resolv.conf'; fi; printf '%s\\000' '/sbin/init --system --unit=multi-user.target' | sudo tee '$domain_root/proc/1/cmdline' >/dev/null; printf '0::/\\n' | sudo tee '$domain_root/proc/1/cgroup' '$domain_root/proc/self/cgroup' >/dev/null; printf 'cpu memory pids\\n' | sudo tee '$domain_root/sys/fs/cgroup/cgroup.controllers' >/dev/null; sudo touch '$domain_root/sys/fs/cgroup/cgroup.procs' '$domain_root/sys/fs/cgroup/cgroup.subtree_control' '$domain_root/sys/fs/cgroup/init.scope/cgroup.procs' '$domain_root/sys/fs/cgroup/system.slice/console-getty.service/cgroup.procs' '$domain_root/sys/fs/cgroup/system.slice/console-getty.service/cgroup.events' '$domain_root/sys/fs/cgroup/system.slice/systemd-user-sessions.service/cgroup.procs' '$domain_root/sys/fs/cgroup/system.slice/systemd-user-sessions.service/cgroup.events'; sudo mknod -m 600 '$domain_root/dev/console' c 5 0; sudo ln -s / '$domain_root/proc/1/root'; sudo install -m 755 /var/tmp/ginux-gate9-check '$domain_root/usr/local/sbin/ginux-gate9-check'; sudo install -m 755 /var/tmp/ginux-gate9-network-check '$domain_root/usr/local/sbin/ginux-gate9-network-check'; sudo rm -f '$domain_root/etc/systemd/system/getty.target' '$domain_root/etc/systemd/system/console-getty.service'; sudo ln -sf /usr/lib/systemd/system/console-getty.service '$domain_root/etc/systemd/system/getty.target.wants/console-getty.service'; printf '%s\\n' '#%PAM-1.0' 'auth required pam_unix.so' 'account required pam_unix.so' 'password required pam_unix.so' 'session required pam_unix.so' | sudo tee '$domain_root/etc/pam.d/login' >/dev/null; printf 'root:ginux\\n' | sudo chroot '$domain_root' /usr/sbin/chpasswd; sudo chown -R 1000:1000 '$domain_root'; sudo find '$domain_root' -type d -exec chmod u+w {} +; sudo chmod 600 '$domain_root/etc/shadow'; test -x '$domain_root/sbin/init'; chmod 0755 /var/tmp/ginux"
rocky_ssh "printf 'cpu  1 0 1 100 0 0 0 0 0 0\\nbtime 1\\nprocesses 1\\nprocs_running 1\\nprocs_blocked 0\\n' | sudo tee '$domain_root/proc/stat' >/dev/null; printf '1.00 0.00\\n' | sudo tee '$domain_root/proc/uptime' >/dev/null; printf 'MemTotal:        1048576 kB\\nMemFree:          524288 kB\\nMemAvailable:     524288 kB\\n' | sudo tee '$domain_root/proc/meminfo' >/dev/null; printf 'none / proc rw,nosuid,nodev,noexec,relatime 0 0\\n' | sudo tee '$domain_root/proc/mounts' >/dev/null; printf '0.00 0.00 0.00 1/1 1\\n' | sudo tee '$domain_root/proc/loadavg' >/dev/null; printf 'Linux version 6.12.0-ginux (ginux)\\n' | sudo tee '$domain_root/proc/version' >/dev/null; sudo chown 1000:1000 '$domain_root/proc/stat' '$domain_root/proc/uptime' '$domain_root/proc/meminfo' '$domain_root/proc/mounts' '$domain_root/proc/loadavg' '$domain_root/proc/version'"
rocky_ssh "sudo mkdir -p '$domain_root/run/host' '$domain_root/proc/sys/fs'; printf '9223372036854775807\\n' | sudo tee '$domain_root/proc/sys/fs/file-max' >/dev/null; printf '1048576\\n' | sudo tee '$domain_root/proc/sys/fs/nr_open' >/dev/null; for service_file in '$domain_root'/usr/lib/systemd/system/*.service; do test -e \"\$service_file\" || continue; service=\${service_file##*/}; service_cgroup='$domain_root'/sys/fs/cgroup/system.slice/\$service; sudo mkdir -p \"\$service_cgroup\"; sudo touch \"\$service_cgroup/cgroup.procs\" \"\$service_cgroup/cgroup.events\" \"\$service_cgroup/cgroup.kill\"; done; sudo mkdir -p '$domain_root/sys/fs/cgroup/system-modprobe.slice/modprobe@configfs.service'; sudo touch '$domain_root/sys/fs/cgroup/system-modprobe.slice/modprobe@configfs.service/cgroup.procs' '$domain_root/sys/fs/cgroup/system-modprobe.slice/modprobe@configfs.service/cgroup.events' '$domain_root/sys/fs/cgroup/system-modprobe.slice/modprobe@configfs.service/cgroup.kill'; test -p '$domain_root/run/initctl' || sudo mkfifo '$domain_root/run/initctl'; if test ! -e '$domain_root/etc/localtime' && test ! -L '$domain_root/etc/localtime'; then sudo ln -s /usr/share/zoneinfo/UTC '$domain_root/etc/localtime'; fi; sudo chown -R 1000:1000 '$domain_root/run/host' '$domain_root/proc/sys' '$domain_root/sys/fs/cgroup'; sudo chown 1000:1000 '$domain_root/run/initctl'"

if test "$gate9_dnf" = off; then
    rocky_ssh "sudo touch '$domain_root/run/ginux-skip-dnf'; sudo chown 1000:1000 '$domain_root/run/ginux-skip-dnf'"
elif test "$gate9_dnf" = failure; then
    rocky_ssh "sudo touch '$domain_root/run/ginux-fail-dnf'; sudo chown 1000:1000 '$domain_root/run/ginux-fail-dnf'"
fi
if test "$gate9_network" = off; then
    rocky_ssh "sudo touch '$domain_root/run/ginux-skip-network'; sudo chown 1000:1000 '$domain_root/run/ginux-skip-network'"
fi

printf '%s\n' 'GINUX_GATE9_STAGE: PASS'
