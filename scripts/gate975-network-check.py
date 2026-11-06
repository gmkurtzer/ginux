#!/usr/bin/python3
import errno
import socket


server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server.bind(("0.0.0.0", 18080))
if server.getsockname()[1] != 18080:
    raise SystemExit("translated domain port leaked through getsockname")
server.listen(1)

conflict = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
try:
    conflict.bind(("0.0.0.0", 18080))
except OSError as error:
    if error.errno != errno.EADDRINUSE:
        raise
else:
    raise SystemExit("translated bind conflict unexpectedly succeeded")
finally:
    conflict.close()

denied_bind = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
try:
    denied_bind.bind(("0.0.0.0", 18082))
except OSError as error:
    if error.errno != errno.EPERM:
        raise
else:
    raise SystemExit("denied ingress bind unexpectedly succeeded")
finally:
    denied_bind.close()

client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
client.connect(("127.0.0.1", 18081))
accepted, _ = server.accept()
client.sendall(b"ginux-gate975")
if accepted.recv(64) != b"ginux-gate975":
    raise SystemExit("translated socket payload mismatch")

denied_connect = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
try:
    denied_connect.connect(("127.0.0.1", 25))
except OSError as error:
    if error.errno != errno.EPERM:
        raise
else:
    raise SystemExit("denied egress connect unexpectedly succeeded")
finally:
    denied_connect.close()

accepted.close()
client.close()
server.close()
print("GINUX_NETWORK_BIND_ACCEPT_CONFLICT: PASS")
print("GINUX_GATE975_NETWORK_POLICY_TRANSLATION: PASS")
