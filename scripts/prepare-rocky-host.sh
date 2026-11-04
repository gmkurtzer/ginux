#!/bin/sh
set -eu

. ./config/rocky-host-image.env
. ./scripts/rocky-host-common.sh

./scripts/verify-rocky-host.sh
mkdir -p build/cache/rocky build/rocky-seed

if [ ! -f "$ROCKY_KEY" ]; then
    ssh-keygen -q -t ed25519 -N '' -C ginux-gate1 -f "$ROCKY_KEY"
fi
public_key=$(cat "$ROCKY_KEY.pub")

cat > build/rocky-seed/meta-data <<EOF
instance-id: ginux-gate1
local-hostname: ginux-rocky
EOF

cat > build/rocky-seed/user-data <<EOF
#cloud-config
users:
  - default
  - name: ginux
    gecos: Ginux-us Gate 1
    groups: [wheel]
    shell: /bin/bash
    sudo: ["ALL=(ALL) NOPASSWD:ALL"]
    ssh_authorized_keys:
      - $public_key
ssh_pwauth: false
disable_root: true
runcmd:
  - systemctl enable --now sshd
EOF

rm -f build/rocky-seed.iso build/rocky-seed.iso.cdr
hdiutil makehybrid \
    -quiet \
    -iso \
    -joliet \
    -default-volume-name cidata \
    -o build/rocky-seed.iso \
    build/rocky-seed

if [ -f build/rocky-seed.iso.cdr ] && [ ! -f build/rocky-seed.iso ]; then
    mv build/rocky-seed.iso.cdr build/rocky-seed.iso
fi
test -f build/rocky-seed.iso
