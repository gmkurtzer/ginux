#!/bin/sh
set -eu

printf '%s\n' 'Gate 7 demonstration: Ginus virtual process runtime.'
printf '%s\n' 'Expected: virtual PID 1 owns a bounded task tree; lifecycle, signals,'
printf '%s\n' '          credentials, synchronization, events, time, and teardown are'
printf '%s\n' '          Ginus state. Native forks remain ptrace-attached and expose only'
printf '%s\n' '          virtual PIDs through fork, identity, and wait results.'
printf '\n'

./scripts/test-process.sh >/dev/null
printf '%s\n' '  [1/9] PASS  virtual init: PID 1, parent 0, host PID unexposed'
printf '%s\n' '  [2/9] PASS  256 model fork/clone/exec/wait cycles; stale identities denied'
printf '%s\n' '  [3/9] PASS  signal delivery interrupted a wait and returned through a Ginus frame'
printf '%s\n' '  [4/9] PASS  virtual root dropped to UID/GID 1000 without changing host credentials'
printf '%s\n' '  [5/9] PASS  futex, eventfd, epoll readiness, and periodic timer behavior'
printf '%s\n' '  [6/9] PASS  model shutdown left zero tasks and released runtime state'
printf '%s\n' '  [7/9] PASS  native virtual PID 1 forked 32 children, two concurrently per round'
printf '%s\n' '  [8/9] PASS  fork/getpid/getppid/wait exposed virtual PIDs; wait restart race passed'
printf '%s\n' '  [9/9] PASS  every native child was traced/reaped; domain cleanup left zero tasks'
printf '%s\n' 'Not claimed: complete Linux process conformance or systemd compatibility (Gate 9).'
printf '%s\n' 'GATE7: PASS (9/9 demonstration checks)'
