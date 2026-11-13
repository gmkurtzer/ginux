#!/usr/bin/env python3
"""Run real launch workloads against an existing binary and fixture (Linux/root)."""
import json
import os
from pathlib import Path
import re
import shutil
import resource
import signal
import subprocess
import sys
import tempfile


def main():
    if len(sys.argv) != 3 or os.geteuid() != 0:
        sys.exit("usage (as root): test-launch.py <ginux binary> <launch fixture>")
    binary, fixture = (str(Path(arg).resolve()) for arg in sys.argv[1:])
    template = (Path(__file__).resolve().parent.parent / "config/instance-gate5.toml").read_text()
    with tempfile.TemporaryDirectory(prefix="ginux-launch-") as directory:
        work = Path(directory)
        # CI checkouts may live below a private home directory. The
        # unprivileged bootstrap probe must be able to execute the binary.
        shutil.copyfile(binary, work / "ginux")
        (work / "ginux").chmod(0o755)
        binary = str(work / "ginux")
        root = work / "root"
        (root / "bin").mkdir(parents=True)
        (root / "work").mkdir()
        shutil.copyfile(fixture, root / "bin/fixture")
        (root / "bin/fixture").chmod(0o755)
        (root / "work/input").write_bytes(b"configured working directory\n")
        (root / "work/large").write_bytes(b"r" * (1024 * 1024 + 1))
        (root / "input").write_bytes(b"root working directory\n")
        (root / "work/a..b").mkdir()
        counter = 0

        def invoke(args, expected, outcome, *, stdout=b"", changes=None, env=None, pass_fds=(),
                   stdin=None):
            nonlocal counter
            counter += 1
            config = template.replace('init = "/bin/bash"', 'init = "/bin/fixture"')
            config = config.replace("args = []", "args = " + json.dumps(args))
            config = config.replace('["container=ginux"]', json.dumps(env or []))
            for old, new in (changes or {}).items():
                config = config.replace(old, new)
            config_path = work / f"{counter}.toml"
            config_path.write_text(config)
            result_path = work / f"{counter}.json"
            proc = subprocess.run([binary, "launch", str(config_path), str(root),
                                   "--result-file", str(result_path)], capture_output=True,
                                  timeout=20, pass_fds=pass_fds, stdin=stdin)
            record = json.loads(result_path.read_text())
            wanted = {"version": 1, "outcome": outcome, "code": None,
                      "signal": None, "generation": None}
            if outcome == "exited":
                wanted["code"] = expected
            elif outcome == "signaled":
                wanted["signal"] = expected - 128
            if proc.returncode != expected or proc.stdout != stdout or record != wanted:
                raise RuntimeError(f"{args}: exit={proc.returncode}, stdout={proc.stdout!r}, "
                                   f"record={record!r}, stderr={proc.stderr.decode(errors='replace')}")
            if any(not line.startswith(b"ginux: ") for line in proc.stderr.splitlines()):
                raise RuntimeError(f"unprefixed diagnostics: {proc.stderr!r}")
            print(f"PASS launch {args}: {outcome} ({expected})")
            return proc

        for code in (0, 3, 120, 127, 128, 255):
            invoke(["exit", str(code)], code, "exited")
        invoke(["args", "one two", ""], 0, "exited", env=["GREETING=hello world", "EMPTY="],
               stdout=b"one two\n\nGREETING=hello world\nEMPTY=\n")
        # Largest documented argv/environment, mostly quotes and backslashes,
        # so the normalized configuration exceeds 4 KiB.
        big_args = [("q\"b\\" * 32)[:128] for _ in range(6)]
        big_env = [f"BIG{i}=" + ("\"\\" * 128)[:251] for i in range(8)]
        invoke(["args", *big_args], 0, "exited", env=big_env,
               stdout="".join(f"{line}\n" for line in big_args + big_env).encode())
        invoke(["cwd"], 0, "exited", stdout=b"configured working directory\n")
        invoke(["cwd"], 0, "exited", stdout=b"root working directory\n",
               changes={"args = [": 'workdir = "/"\nargs = ['})
        invoke(["exit"], 0, "exited", changes={"args = [": 'workdir = "/work/a..b"\nargs = ['})
        # The default /work already exists in this root; a missing workdir must be created.
        invoke(["exit"], 0, "exited", changes={"args = [": 'workdir = "/created-workdir"\nargs = ['})
        if not (root / "created-workdir").is_dir():
            raise RuntimeError("missing workload directory was not created as a directory")
        invoke(["trap"], 133, "signaled")
        invoke(["handler"], 0, "exited", stdout=b"handled\n")
        invoke(["segv"], 139, "signaled")
        # fcntl cannot make the host kernel signal another process.
        invoke(["guard"], 0, "exited")
        invoke(["fork"], 0, "exited")
        invoke(["int80"], 1, "supervisor-error")
        invoke(["descriptor"], 0, "exited")
        invoke(["list"], 0, "exited", stdout=b"input\n")
        # A directory as a standard stream would anchor *at calls outside the
        # root, so launch refuses it; a regular file stays a valid stdin.
        outside = work / "outside"
        outside.mkdir()

        def refused(stream, flags):
            stream_config = work / f"stream-{stream}-{flags}.toml"
            stream_config.write_text(template.replace('init = "/bin/bash"', 'init = "/bin/fixture"')
                                    .replace("args = []", 'args = ["exit"]'))
            result_path = work / f"stream-{stream}-{flags}.json"
            directory = os.open(outside, flags)
            try:
                streams = {"stdin": None, "stdout": subprocess.PIPE, "stderr": subprocess.PIPE}
                streams[stream] = directory
                proc = subprocess.run([binary, "launch", str(stream_config), str(root),
                                       "--result-file", str(result_path)], timeout=20, **streams)
            finally:
                os.close(directory)
            record = json.loads(result_path.read_text())
            reason = b"standard " + {"stdin": b"input", "stdout": b"output", "stderr": b"error"}[stream]
            # With stderr a directory the refusal has nowhere to print.
            if (proc.returncode != 2 or record["outcome"] != "supervisor-error"
                    or (stream != "stdout" and proc.stdout)
                    or (stream != "stderr" and reason + b" is a directory" not in proc.stderr)):
                raise RuntimeError(f"directory {stream} was not refused as expected: "
                                   f"exit={proc.returncode} record={record!r} stderr={proc.stderr!r}")
            print(f"PASS launch directory {stream} refused")

        for stream in ("stdin", "stdout", "stderr"):
            refused(stream, os.O_RDONLY | os.O_DIRECTORY)
        refused("stdin", os.O_PATH | os.O_DIRECTORY)
        with open(root / "input", "rb") as regular:
            invoke(["exit"], 0, "exited", stdin=regular)
        # Instance contract denials reach the workload with EPERM, and audit
        # records name the rule; cwd exits 91 when its open fails.
        contract_start = template.index("[contract]")
        contract_end = template.index("[storage]")
        original = template[contract_start:contract_end]

        def rules(deny=(), syscall_deny=(), allow=("*",), syscall_allow=("*",)):
            replacement = ('[contract]\nversion = 1\ncatalog = 2\ndefault = "deny"\n'
                           + f'allow = {json.dumps(allow)}\ndeny = {json.dumps(deny)}\n'
                           + '[contract.syscalls]\ncontrol = "paranoid"\ndefault = "deny"\n'
                           + f'allow = {json.dumps([v.removeprefix("syscall.") for v in syscall_allow])}\ndeny = {json.dumps([v.removeprefix("syscall.") for v in syscall_deny])}\n\n')
            return {original: replacement}

        def denied(args, code, operation, syscall, *, changes):
            proc = invoke(args, code, "exited", changes=changes)
            if not re.search(rb"^ginux: launch contract instance=\S+ digest=[0-9a-f]{64}$",
                             proc.stderr, re.M):
                raise RuntimeError("launch did not report its resolved contract")
            record = (rb"^ginux: audit sequence=\d+ phase=entry syscall=" + str(syscall).encode()
                      + rb" operation=" + operation.encode()
                      + rb" policy-source=exact-deny policy-rule=0 action=deny result=-1$")
            if not re.search(record, proc.stderr, re.M):
                raise RuntimeError(f"missing winning denial: {proc.stderr!r}")

        denied(["cwd"], 91, "syscall.open", 2, changes=rules(syscall_deny=["syscall.open"]))
        denied(["cwd"], 0, "syscall.write", 1, changes=rules(deny=["syscall.write"]))
        denied(["cwd"], 91, "filesystem.path.read", 2, changes=rules(deny=["filesystem.path.read"]))
        denied(["args", "hidden"], 0, "device.console.write", 1, changes=rules(deny=["device.console.write"]))
        denied(["handler"], 86, "signal.virtual.deliver", 13, changes=rules(deny=["signal.virtual.deliver"]))
        for selector in ("device.console.read", "proc.system.read", "sys.device.read",
                         "time.clock.read", "syscall.clock_gettime"):
            proc = invoke(["exit"], 2, "supervisor-error", changes=rules(deny=[selector]))
            if b"reason=unenforced-deny operation=" + selector.encode() not in proc.stderr:
                raise RuntimeError(f"unenforced denial accepted: {selector}: {proc.stderr!r}")
        invoke(["exit"], 0, "exited", changes=rules(deny=[
            "device.mmio.direct", "device.dma.direct", "network.packet.raw", "network.*"]))
        proc = invoke(["cwd"], 91, "exited", changes=rules(allow=["filesystem.path.read", "syscall.exit"], syscall_allow=["syscall.exit_group"]))
        if not re.search(rb"operation=syscall.open policy-source=default-deny policy-rule=0 action=deny result=-38", proc.stderr):
            raise RuntimeError("missing default syscall denial")
        denied(["args", "hidden"], 0, "syscall.write", 1,
               changes=rules(allow=["syscall.exit_group"], syscall_deny=["syscall.write"]))
        # Native typed controls cannot silently become unenforced promises.
        for section in ('[contract.filesystem]\nwrite = ["/work"]',
                        '[contract.process]\nmax_processes = 2',
                        '[contract.kernel]\nproc_read = ["/"]',
                        '[contract.devices]\nread = ["/dev/null"]'):
            proc = invoke(["exit"], 2, "supervisor-error", changes={"[storage]": section + "\n[storage]"})
            if b"launch does not enforce native typed controls" not in proc.stderr:
                raise RuntimeError(f"typed control did not reach launch validation: {proc.stderr!r}")
        proc = invoke(["args", "visible"], 0, "exited", stdout=b"visible\n",
                      changes={'enabled = true': 'enabled = false'})
        if b"ginux: audit " in proc.stderr:
            raise RuntimeError("disabled audit emitted a decision record")
        # The thread identity matches the virtual pid: set_tid_address returns
        # it instead of the host tid, and PI futexes, which would store the
        # host tid in the futex word, are denied.
        invoke(["virtual-tid"], 0, "exited",
               stdout=b"set_tid_address=4242\ngetpid=4242\ngettid=-38\nfutex_lock_pi=-1\n"
               b"pi_word=0\nfutex_lock_pi_flags=-1\npi_flags_word=0\n")
        invoke(["ysinfo"], 0, "exited",
               stdout=b"sysinfo=0\nsysinfo-zero=1\nmem-unit=1\nsysinfo-null=-14\n")
        invoke(["oversized"], 0, "exited",
               stdout=b"read=1048576\npread=1048576\n" + b"x" * 4096 + b"write=4096\n")
        invoke(["random"], 0, "exited", stdout=b"getrandom=256\n")
        with (work / "secret").open("w+b") as secret:
            secret.write(b"host-only data")
            secret.flush()
            secret.seek(0)
            invoke(["descriptor", str(secret.fileno())], 0, "exited", pass_fds=(secret.fileno(),))
        proc = invoke(["budget"], 0, "exited")
        config_begin = proc.stderr.index(b"ginux: effective config begin\n")
        config_end = proc.stderr.index(b"ginux: effective config end\n")
        config_bytes = config_end - config_begin
        trace_bytes = sum(len(line) + 1 for line in proc.stderr.splitlines()
                          if line.startswith(b"ginux: audit sequence="))
        if (proc.stderr.count(b"audit truncated") != 1 or trace_bytes > 65536
                or len(proc.stderr) - config_bytes > 68000):
            raise RuntimeError(f"audit output was not bounded: trace={trace_bytes}, total={len(proc.stderr)}")
        # The budget drops whole records: the notice is a line of its own and
        # every syscall record before it is complete.
        lines = proc.stderr.splitlines()
        record = re.compile(rb"ginux: audit sequence=\d+ phase=(exit result=-?\d+|"
                            rb"entry syscall=\d+ operation=\S+ policy-source=\S+ "
                            rb"(policy-rule=\d+ )?action=\S.* result=\S+)")
        if (b"ginux: audit truncated budget=65536" not in lines
                or any(line.startswith(b"ginux: audit sequence=") and not record.fullmatch(line)
                       for line in lines)):
            raise RuntimeError(f"audit budget split a record: {proc.stderr[-400:]!r}")
        invoke(["exit"], 2, "supervisor-error", changes={
            'profile = "balanced"': 'profile = "security"',
            'backend = "ptrace-native"': 'backend = "interpreter"'})
        invoke(["exit"], 2, "supervisor-error", changes={
            'init = "/bin/fixture"': 'init = "/bin/missing"'})
        invoke(["exit"], 2, "supervisor-error", changes={
            'args = [': 'workdir = "/missing/child"\nargs = ['})
        invoke(["exit"], 2, "supervisor-error", changes={
            'schema_version = 1': 'schema_version = 2'})
        invoke(["args", "x" * 128], 0, "exited", stdout=b"x" * 128 + b"\n")
        invoke(["args", "x" * 129], 2, "supervisor-error")
        invoke(["args"] * 8, 2, "supervisor-error")
        invoke(["args"], 0, "exited", env=["X=" + "x" * 254], stdout=b"X=" + b"x" * 254 + b"\n")
        invoke(["args"], 2, "supervisor-error", env=["X=" + "x" * 255])
        invoke(["args"], 2, "supervisor-error", env=["X=x"] * 9)
        invoke(["exit"], 2, "supervisor-error", changes={
            'args = [': 'workdir = "/work/../bin"\nargs = ['})
        invoke(["exit"], 2, "supervisor-error", changes={
            '[storage]\nassignments = []': '[storage]\nassignments = ["volume:data"]'})
        # A result collision must fail before even parsing the configuration.
        occupied = work / "occupied.json"
        occupied.write_bytes(b"keep me\n")
        for result in (occupied, work / "absent/result.json"):
            proc = subprocess.run([binary, "launch", str(work / "missing.toml"), str(root),
                                   "--result-file", str(result)], capture_output=True, timeout=10)
            if proc.returncode != 2 or proc.stdout or b"not fresh or not creatable" not in proc.stderr:
                raise RuntimeError(f"result reservation failed: {proc!r}")
        if occupied.read_bytes() != b"keep me\n":
            raise RuntimeError("existing result was overwritten")
        print("PASS result reservation rejects collisions and missing parents")
        config_path = work / "completion.toml"
        config_path.write_text(template.replace('init = "/bin/bash"', 'init = "/bin/fixture"')
                               .replace("args = []", 'args = ["exit", "0"]'))
        command = [binary, "launch", str(config_path), str(root)]
        proc = subprocess.run(command, capture_output=True, timeout=10)
        if proc.returncode != 0 or proc.stdout:
            raise RuntimeError(f"launch without result file failed: {proc!r}")
        result_path = work / "closed-stdout.json"
        proc = subprocess.run(command + ["--result-file", str(result_path)], capture_output=True,
                              timeout=10, preexec_fn=lambda: os.close(1))
        if proc.returncode != 0 or json.loads(result_path.read_text())["code"] != 0:
            raise RuntimeError(f"closed stdout corrupted completion: {proc!r}")
        print("PASS optional result file and closed stdout")

        def limit_result_size():
            signal.signal(signal.SIGXFSZ, signal.SIG_IGN)
            resource.setrlimit(resource.RLIMIT_FSIZE, (16, 16))

        proc = subprocess.run(command + ["--result-file", str(work / "short-result.json")],
                              capture_output=True, timeout=10, preexec_fn=limit_result_size)
        if proc.returncode != 1 or proc.stdout or b"result-file write failed" not in proc.stderr:
            raise RuntimeError(f"result write failure returned success: {proc!r}")
        print("PASS real result-file write failure returns supervisor failure")

        config_path.write_text(config_path.read_text().replace('["exit", "0"]', '["wait"]'))
        result_path = work / "stopped.json"
        proc = subprocess.Popen(command + ["--result-file", str(result_path)], stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE)
        try:
            if proc.stdout.readline() != b"ready\n":
                raise RuntimeError("signal workload did not start")
            children = Path(f"/proc/{proc.pid}/task/{proc.pid}/children").read_text().split()
            if len(children) != 1:
                raise RuntimeError(f"unexpected supervised children: {children}")
            os.kill(int(children[0]), signal.SIGSTOP)
            output, diagnostics = proc.communicate(timeout=10)
            if proc.returncode != 1 or output or b"group stops are unsupported" not in diagnostics:
                raise RuntimeError(f"group stop was not handled: {diagnostics!r}")
            if json.loads(result_path.read_text())["outcome"] != "supervisor-error":
                raise RuntimeError("group stop completion was incorrect")
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.communicate()
        print("PASS group stop fails explicitly without hanging")
        for args in (["launch"], ["launch", "config"], ["launch", "config", "root", "extra"],
                     ["launch", "config", "root", "--result-file", ""]):
            proc = subprocess.run([binary, *args], capture_output=True, timeout=10)
            if proc.returncode != 2 or proc.stdout or b"launch usage" not in proc.stderr:
                raise RuntimeError(f"invalid launch argv ran a workload: {proc!r}")
        print("PASS incomplete launch commands report usage without running a demo")
        for args in (["config"], ["native", "config"], ["devices", "config"], ["confg", "x.toml"],
                     ["--help"], ["help"]):
            proc = subprocess.run([binary, *args], capture_output=True, timeout=10)
            if proc.returncode != 2 or proc.stdout or b"ginux: usage:" not in proc.stderr:
                raise RuntimeError(f"unknown or incomplete argv ran the demonstration: {proc!r}")
        print("PASS unknown and incomplete commands report usage without running a demo")
        # Exercise the bootstrap failure in a real unprivileged process.
        work.chmod(0o755)
        config_path = work / "unprivileged.toml"
        config_path.write_text(template)
        result_dir = work / "unprivileged"
        result_dir.mkdir(mode=0o777)
        result_dir.chmod(0o777)
        proc = subprocess.run([binary, "launch", str(config_path), str(root), "--result-file",
                               str(result_dir / "result.json")], capture_output=True, timeout=10,
                              user=65534, group=65534, extra_groups=[])
        record = json.loads((result_dir / "result.json").read_text())
        if proc.returncode != 2 or proc.stdout or record["outcome"] != "supervisor-error":
            raise RuntimeError(f"unprivileged bootstrap did not fail explicitly: {proc!r}")
        print("PASS unprivileged bootstrap reports supervisor-error")
    print("test-launch: PASS")


if __name__ == "__main__":
    main()
