# Gate 7 process-runtime demonstration

Run the Gate 7 demonstration with:

```sh
make demo-gate-7
```

The target boots the cached Rocky Linux 10 host under QEMU TCG and runs the
Ginux-us process runtime fixtures. The model fixture shows virtual PID 1, repeats
256 bounded fork/clone/exec/wait cycles, rejects stale identities, delivers and
returns from a virtual signal handler, drops virtual credentials, exercises
futex, eventfd, epoll-style readiness and periodic timers, then verifies
complete teardown.

The native fixture runs 32 fork/wait lifecycles with two concurrent children
per round. Ginux-us attaches every child, assigns monotonic virtual PIDs, translates
`fork`, `getpid`, `getppid`, and `wait4` results, and restores translated wait
arguments across an interrupted/restarted wait race. It checks all 32 child
exits and finishes with zero managed tasks.

The demonstration deliberately does not claim complete Linux process
conformance or systemd compatibility. Systemd is Gate 9.
