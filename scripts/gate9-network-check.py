#!/usr/bin/python3
import errno
import socket


server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server.bind(("127.0.0.1", 0))
address = server.getsockname()
server.listen(1)

conflict = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
try:
    conflict.bind(address)
except OSError as error:
    if error.errno != errno.EADDRINUSE:
        raise
else:
    raise SystemExit("second bind unexpectedly succeeded")
finally:
    conflict.close()

client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
client.connect(address)
accepted, _ = server.accept()
client.sendall(b"ginux-network-check")
if accepted.recv(64) != b"ginux-network-check":
    raise SystemExit("accepted socket payload mismatch")

accepted.close()
client.close()
server.close()
print("GINUX_NETWORK_BIND_ACCEPT_CONFLICT: PASS")
