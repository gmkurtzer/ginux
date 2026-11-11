#!/usr/bin/python3
import errno
import os
import socket
import time


name = os.environ["GINUX_GATE17_NAME"]
peer = os.environ["GINUX_GATE17_PEER"]

if os.path.exists(f"/ginux-gate17/{peer}"):
    raise SystemExit("peer overlay marker is visible")
if not os.path.exists(f"/ginux-gate17/{name}"):
    raise SystemExit("own overlay marker is missing")

with open("/proc/sys/kernel/hostname", encoding="ascii") as source:
    hostname = source.read().strip()
with open("/proc/sys/kernel/random/boot_id", encoding="ascii") as source:
    boot_id = source.read().strip()

virtual_pids = sorted(
    entry for entry in os.listdir("/proc") if entry.isdigit()
)
if "1" not in virtual_pids:
    raise SystemExit("virtual PID 1 is missing")

translated = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
translated.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
translated.bind(("127.0.0.1", 18180))
translated.listen(1)

shared = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
shared_result = "winner"
try:
    shared.bind(("127.0.0.1", 18170))
    shared.listen(1)
except OSError as error:
    if error.errno != errno.EADDRINUSE:
        raise
    shared_result = "conflict"

os.makedirs("/ginux-gate17", exist_ok=True)
with open(f"/ginux-gate17/{name}-runtime", "w", encoding="ascii") as marker:
    marker.write(f"{name}\n")

print(
    "GINUX_GATE17_INSTANCE: "
    f"name={name} hostname={hostname} boot-id={boot_id} "
    f"pids={','.join(virtual_pids)} overlay=isolated translated-port=bound "
    f"shared-port={shared_result}",
    flush=True,
)

# The first instance keeps the ordinary shared-host bind alive long enough for
# the concurrently booting peer to observe the expected EADDRINUSE result.
if shared_result == "winner":
    time.sleep(30)

shared.close()
translated.close()
print(f"GINUX_GATE17_CHECK: PASS name={name}", flush=True)
