#!/usr/bin/env python3
"""crust_port -- survey Ginux against Crust, and build what Crust can compile.

Ginux is a Rust workspace.  Crust (https://github.com/brentharts/crust) lowers
a subset of Rust to C and compiles it with its own toolchain, with no rustc or
cargo involved.  This script is the bridge: it walks the Ginux tree, asks Crust
what it understands, compiles what it can, and writes a report so that each
change to Crust (or to Ginux) can be measured against the last one.

    ./crust_port.py                      # survey + build everything
    ./crust_port.py survey --blockers    # what stops the rest, ranked
    ./crust_port.py build                # compile the files that translate
    ./crust_port.py survey kernel/src    # restrict to some paths
    ./crust_port.py diff old.json        # compare this run with a saved one

Crust is located with --crust, then $CRUST_DIR, then ../crust.  Outputs go to
build/crust/ (already ignored by git): objects under obj/, and report.json.
"""

import argparse
import collections
import concurrent.futures as cf
import importlib.util
import json
import multiprocessing
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_ROOTS = ["kernel", "boot", "control-plane", "services", "crates",
                 "tools"]
OUT = os.path.join(HERE, "build", "crust")
COMPILE_TIMEOUT = 300

crustos = None          # Crust's tools/crustos.py, loaded by load_crust()


def find_crust(explicit):
    """Return the Crust checkout directory, or exit with a useful message."""
    candidates = [explicit, os.environ.get("CRUST_DIR"),
                  os.path.join(HERE, "..", "crust")]
    for c in candidates:
        if c and os.path.isfile(os.path.join(c, "tools", "crustos.py")):
            return os.path.abspath(c)
    sys.exit("cannot find Crust.  Clone https://github.com/brentharts/crust "
             "next to this repo,\nset CRUST_DIR, or pass --crust PATH.")


def load_crust(crust_dir):
    """Import Crust's own survey/compile helpers rather than reimplementing."""
    global crustos
    path = os.path.join(crust_dir, "tools", "crustos.py")
    spec = importlib.util.spec_from_file_location("crustos", path)
    crustos = importlib.util.module_from_spec(spec)
    sys.modules["crustos"] = crustos
    spec.loader.exec_module(crustos)
    # Crust's helpers shell out with cwd=ROOT; make sure they agree with us.
    crustos.BUILD = OUT
    return crustos


def crust_revision(crust_dir):
    import subprocess
    proc = subprocess.run(["git", "-C", crust_dir, "rev-parse", "--short",
                           "HEAD"], capture_output=True, text=True)
    return proc.stdout.strip() or "unknown"


def collect(paths):
    roots = [os.path.join(HERE, p) if not os.path.isabs(p) else p
             for p in (paths or DEFAULT_ROOTS)]
    roots = [r for r in roots if os.path.exists(r)]
    return sorted(set(crustos.sources(roots)))


def group_of(path):
    """Top-level area a file belongs to: 'crates/oci', 'kernel', ..."""
    rel = os.path.relpath(path, HERE).split(os.sep)
    if rel[0] in ("crates", "services", "tools") and len(rel) > 2:
        return "/".join(rel[:2])
    return rel[0]


# --------------------------------------------------------------------------
# survey
# --------------------------------------------------------------------------

def _classify(path):
    r = crustos.classify(path)
    return {"path": os.path.relpath(path, HERE), "outcome": r.outcome,
            "items": r.items, "lines": r.lines, "covered": r.covered,
            "error": r.error}


def run_survey(files, jobs):
    if jobs > 1:
        ctx = multiprocessing.get_context("fork")
        with ctx.Pool(jobs) as pool:
            # Biggest files first so one huge file doesn't end the run alone.
            order = sorted(files, key=os.path.getsize, reverse=True)
            rows = pool.map(_classify, order, chunksize=1)
    else:
        rows = [_classify(p) for p in files]
    return sorted(rows, key=lambda r: r["path"])


def print_survey(rows, blockers, top, show_files):
    total = len(rows)
    counts = collections.Counter(r["outcome"] for r in rows)
    lines = sum(r["lines"] for r in rows)
    print("Ginux survey -- %d .rs files, %d lines\n" % (total, lines))
    for label, key in [("translated", crustos.TRANSLATED),
                       ("partial", crustos.PARTIAL),
                       ("failed", crustos.FAILED),
                       ("empty", crustos.EMPTY)]:
        n = counts[key]
        print("  %-11s %4d  %5.1f%%" % (label, n,
                                        100.0 * n / total if total else 0))

    by_group = collections.defaultdict(collections.Counter)
    for r in rows:
        by_group[group_of(os.path.join(HERE, r["path"]))][r["outcome"]] += 1
    print("\n  %-26s %5s %5s %5s %5s %5s" %
          ("area", "files", "ok", "part", "fail", "empty"))
    for g in sorted(by_group):
        c = by_group[g]
        print("  %-26s %5d %5d %5d %5d %5d" %
              (g, sum(c.values()), c[crustos.TRANSLATED],
               c[crustos.PARTIAL], c[crustos.FAILED], c[crustos.EMPTY]))

    failed = [r for r in rows if r["outcome"] == crustos.FAILED and r["error"]]
    if blockers:
        print("\nmost frequent messages (of %d failing files):" % len(failed))
        for msg, n in collections.Counter(
                crustos.normalize(r["error"]) for r in failed
        ).most_common(top):
            print("  %4d  %s" % (n, msg[:100]))

    if show_files:
        print()
        for r in rows:
            tag = r["outcome"]
            err = ("  " + crustos.normalize(r["error"])[:80]
                   if r["error"] else "")
            print("  %-10s %6d  %s%s" % (tag, r["lines"], r["path"], err))


# --------------------------------------------------------------------------
# build
# --------------------------------------------------------------------------

def _compile(args):
    i, row = args
    src = os.path.join(HERE, row["path"])
    obj = os.path.join(OUT, "obj", "%03d_%s.o" % (
        i, row["path"].replace(os.sep, "_")[:-3]))
    os.makedirs(os.path.dirname(obj), exist_ok=True)
    try:
        ok, proc = crustos.compile_one(src, obj)
    except Exception as e:                                     # noqa: BLE001
        return row["path"], False, None, "%s: %s" % (type(e).__name__, e)
    return row["path"], ok, obj if ok else None, \
        None if ok else crustos.diagnostic(proc)


def run_build(rows, jobs, verbose):
    todo = [(i, r) for i, r in enumerate(rows)
            if r["outcome"] in (crustos.TRANSLATED, crustos.PARTIAL)]
    print("\nbuilding %d files that Crust translates ..." % len(todo))
    results = {}
    with cf.ThreadPoolExecutor(max_workers=jobs) as ex:
        for path, ok, obj, err in ex.map(_compile, todo):
            results[path] = {"ok": ok, "object": obj, "error": err}
            if verbose:
                print("  %-5s %s%s" % ("ok" if ok else "skip", path,
                                       "" if ok else "  " + (err or "")))
    built = sum(1 for v in results.values() if v["ok"])
    print("  %d of %d produce an object file" % (built, len(todo)))
    causes = collections.Counter(v["error"] for v in results.values()
                                 if not v["ok"])
    if causes:
        print("\n  what stops the rest:")
        for msg, n in causes.most_common(10):
            print("    %3d  %s" % (n, msg))
    return results


# --------------------------------------------------------------------------
# report / diff
# --------------------------------------------------------------------------

def write_report(rows, build, crust_dir, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    for r in rows:
        r["built"] = (build or {}).get(r["path"], {}).get("ok")
    doc = {"time": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
           "crust": crust_revision(crust_dir), "files": rows}
    with open(path, "w") as f:
        json.dump(doc, f, indent=1)
    print("\nreport: %s" % os.path.relpath(path, HERE))


def cmd_diff(old_path, new_path):
    old = {r["path"]: r for r in json.load(open(old_path))["files"]}
    new = {r["path"]: r for r in json.load(open(new_path))["files"]}
    rank = {crustos.EMPTY: 0, crustos.FAILED: 1, crustos.PARTIAL: 2,
            crustos.TRANSLATED: 3}
    better = worse = 0
    for p in sorted(new):
        a, b = old.get(p), new[p]
        if not a:
            print("  new      %s (%s)" % (p, b["outcome"]))
        elif a["outcome"] != b["outcome"] or a.get("built") != b.get("built"):
            d = rank[b["outcome"]] - rank[a["outcome"]]
            better += d > 0
            worse += d < 0
            print("  %-8s %s: %s -> %s  built %s -> %s" % (
                "better" if d > 0 else "worse" if d < 0 else "changed", p,
                a["outcome"], b["outcome"], a.get("built"), b.get("built")))
    print("\n%d improved, %d regressed" % (better, worse))
    return 1 if worse else 0


# --------------------------------------------------------------------------

def main(argv):
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", nargs="?", default="all",
                    choices=("all", "survey", "build", "diff"))
    ap.add_argument("paths", nargs="*",
                    help="files or directories (diff: OLD.json [NEW.json])")
    ap.add_argument("--crust", help="path to the Crust checkout")
    ap.add_argument("--blockers", action="store_true",
                    help="rank the messages that stop translation")
    ap.add_argument("--files", action="store_true", help="list every file")
    ap.add_argument("--top", type=int, default=15)
    ap.add_argument("-j", "--jobs", type=int,
                    default=max(1, (os.cpu_count() or 2) - 1))
    ap.add_argument("-v", "--verbose", action="store_true")
    ap.add_argument("--report", default=os.path.join(OUT, "report.json"))
    args = ap.parse_args(argv)

    crust_dir = find_crust(args.crust)
    load_crust(crust_dir)

    if args.mode == "diff":
        if not args.paths:
            ap.error("diff needs OLD.json [NEW.json]")
        new = args.paths[1] if len(args.paths) > 1 else args.report
        return cmd_diff(args.paths[0], new)

    files = collect(args.paths)
    if not files:
        print("no .rs files found")
        return 1
    print("crust %s at %s" % (crust_revision(crust_dir), crust_dir))
    t0 = time.time()
    rows = run_survey(files, args.jobs)
    print_survey(rows, args.blockers or args.mode == "all", args.top,
                 args.files)
    build = None
    if args.mode in ("all", "build"):
        build = run_build(rows, args.jobs, args.verbose)
    write_report(rows, build, crust_dir, args.report)
    print("done in %.1fs" % (time.time() - t0))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
