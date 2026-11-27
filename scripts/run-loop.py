#!/usr/bin/env python3
"""One iteration of the GambiarraLauncher loop: build, run, watch, classify, report.

Runs the diagnostics build against the local game, follows its stderr live,
stops as soon as the run has told us something (the world's first frame, a
fatal error, or the time limit), and writes a short report instead of a
megabyte of log.

    python gambiarra_loop.py --label wave32 --build
    python gambiarra_loop.py --label dred --dred --timing --timeout 180
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

# Raiz do repositorio: este script vive em scripts/.
TREE = Path(__file__).resolve().parents[1]
EXE = TREE / "target" / "diag" / "gambiarra-cli.exe"
# O dump e SEU: aponte GAMBIARRA_GAME_DIR para a pasta dele. Nada do jogo entra aqui.
GAME = Path(os.environ.get("GAMBIARRA_GAME_DIR", "")) / "game_scarlett_master.exe"
RUNS = TREE / "target" / "run-loop"
CARGO = TREE

ANSI = re.compile(r"\x1b\[[0-9;]*m")

# (bucket, pattern). First match wins, so the specific rules come first.
RULES = (
    ("frame_saved", re.compile(r"frame saved|frame not saved", re.I)),
    ("gpu_removed", re.compile(r"device removed|foi suspensa|0x887A0005|0x887A0020|0x887A0006|breadcrumb|DRED:", re.I)),
    ("fatal", re.compile(r"unhandled exception|panicked at|host GPU failure|0xc0000005", re.I)),
    ("world_frame", re.compile(r"the world.s first frame", re.I)),
    ("frame_saved", re.compile(r"frame saved|frame not saved", re.I)),
    ("raytracing", re.compile(r"BVH|image_bvh|raytrac|acceleration structure", re.I)),
    ("shader_no_translate", re.compile(r"shader does not translate", re.I)),
    ("wave32_on_wave64", re.compile(r"wave lane count of 32", re.I)),
    ("mesh_missing", re.compile(r"no mesh shaders|shader model 6\.6", re.I)),
    ("pipeline_refused", re.compile(r"pipeline refused|no host pipeline", re.I)),
    ("dispatch_dropped", re.compile(r"without a host pipeline dropped|draws are dropped", re.I)),
    ("texture_refused", re.compile(r"no host texture|texture not placed|castable formats", re.I)),
    ("black_frame", re.compile(r"black frame|levels after the black", re.I)),
    ("not_implemented", re.compile(r"method not implemented|unimplemented", re.I)),
    ("startup_data", re.compile(r"startup archive|appdata0\.rpf", re.I)),
    ("slow", re.compile(r"slow host submission|slow list", re.I)),
    ("debug_layer", re.compile(r"D3D12: ", re.I)),
)

# Buckets whose lines are worth keeping verbatim, to read the cause from.
VERBATIM = {"fatal", "gpu_removed", "shader_no_translate", "raytracing", "mesh_missing", "world_frame"}

# What the run reached, in order.
MILESTONES = (
    ("host device", re.compile(r"host D3D12 device ready")),
    ("wave lanes", re.compile(r"host wave lanes")),
    ("title running", re.compile(r"title:", re.I)),
    ("first present", re.compile(r"presenting to a window|present compositor")),
    ("pipelines", re.compile(r"shader translated|pipeline library")),
    ("world frame", re.compile(r"the world.s first frame")),
)


def signature(line):
    """The line with its run-specific numbers taken out, to group by."""
    s = re.sub(r"^\S+Z\s+", "", line)
    s = re.sub(r"0x[0-9a-fA-F]+", "0xH", s)
    s = re.sub(r"\b[0-9a-f]{12,16}\b", "HASH", s)
    s = re.sub(r"\b\d{2,}\b", "N", s)
    return s[:200]


def build(log):
    env = dict(os.environ)
    env["CARGO_HOME"] = str(CARGO / "cargo")
    env["RUSTUP_HOME"] = str(CARGO / "rustup")
    env["PATH"] = str(CARGO / "cargo" / "bin") + os.pathsep + env["PATH"]
    cmd = ["cargo", "build", "--profile", "diag", "-p", "gambiarra", "--features", "diagnostics"]
    with log.open("w", encoding="utf-8", errors="replace") as f:
        r = subprocess.run(cmd, cwd=TREE, env=env, stdout=f, stderr=subprocess.STDOUT)
    if r.returncode != 0:
        text = log.read_text("utf-8", "replace")
        errs = [l for l in text.splitlines() if l.startswith("error") or l.strip().startswith("-->")]
        print("BUILD FAILED")
        print("\n".join(errs[:24]) or text[-2000:])
    return r.returncode == 0


def kill(proc):
    if proc.poll() is None:
        subprocess.run(["taskkill", "/PID", str(proc.pid), "/T", "/F"],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            pass


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", required=True)
    ap.add_argument("--timeout", type=float, default=300)
    ap.add_argument("--keys", default="40:Enter,55:Enter,70:Enter,90:Enter,110:Enter")
    ap.add_argument("--console", default="series-x")
    ap.add_argument("--build", action="store_true")
    ap.add_argument("--dred", action="store_true", help="GAMBIARRA_D3D12_DRED=1")
    ap.add_argument("--timing", action="store_true", help="GAMBIARRA_GPU_TIMING=1")
    ap.add_argument("--no-debug-layer", action="store_true")
    ap.add_argument("--skip-pipelines", default="", help="GAMBIARRA_SKIP_PIPELINES=<hashes>")
    ap.add_argument("--env", action="append", default=[], help="NAME=VALUE, repeatable")
    ap.add_argument("--after-world", type=float, default=12,
                    help="seconds to keep running once the world renders")
    args = ap.parse_args()

    run = RUNS / "{:%Y%m%d-%H%M%S}-{}".format(datetime.now(), args.label)
    run.mkdir(parents=True, exist_ok=True)
    frames = run / "frames"

    if args.build and not build(run / "build.log"):
        return 2
    if not EXE.exists():
        print("no runner at {}".format(EXE))
        return 2

    env = dict(os.environ)
    env["GAMBIARRA_CAPTURE"] = str(frames)
    env["GAMBIARRA_CAPTURE_WORLD"] = "0,2,8"
    if args.keys:
        env["GAMBIARRA_KEYS"] = args.keys
    if not args.no_debug_layer:
        env["GAMBIARRA_D3D12_DEBUG"] = "1"
    if args.dred:
        env["GAMBIARRA_D3D12_DRED"] = "1"
    if args.timing:
        env["GAMBIARRA_GPU_TIMING"] = "1"
    if args.skip_pipelines:
        env["GAMBIARRA_SKIP_PIPELINES"] = args.skip_pipelines
    for pair in args.env:
        name, _, value = pair.partition("=")
        env[name] = value

    err_path, out_path = run / "stderr.log", run / "stdout.log"
    started = time.time()
    with err_path.open("wb") as err, out_path.open("wb") as out:
        proc = subprocess.Popen(
            [str(EXE), "run", str(GAME), "--execute", "--console", args.console],
            cwd=str(TREE), env=env, stdout=out, stderr=err,
        )

        counts, kept, sigs, reached = {}, {}, {}, {}
        verdict, cause, world_at = "", "", None
        read = 0
        while True:
            now = time.time() - started
            with err_path.open("r", encoding="utf-8", errors="replace") as f:
                f.seek(read)
                new = f.read()
                read = f.tell()
            for raw in new.splitlines():
                line = ANSI.sub("", raw).rstrip()
                if not line:
                    continue
                for name, pattern in MILESTONES:
                    if name not in reached and pattern.search(line):
                        reached[name] = now
                for bucket, pattern in RULES:
                    if not pattern.search(line):
                        continue
                    counts[bucket] = counts.get(bucket, 0) + 1
                    s = signature(line)
                    sigs.setdefault(bucket, {})
                    sigs[bucket][s] = sigs[bucket].get(s, 0) + 1
                    if bucket in VERBATIM and len(kept.setdefault(bucket, [])) < 6:
                        kept[bucket].append(line[:300])
                    if bucket == "world_frame" and world_at is None:
                        world_at = now
                    elif bucket == "fatal" and not verdict:
                        verdict, cause = "FATAL", line[:300]
                    elif bucket == "gpu_removed" and "device removed" in line.lower() and not verdict:
                        verdict, cause = "GPU_REMOVED", line[:300]
                    break
            if verdict:
                time.sleep(2)  # let the crash handler finish writing
                break
            if world_at is not None and now - world_at > args.after_world:
                verdict = "GAMEPLAY"
                break
            if proc.poll() is not None:
                verdict = verdict or ("GAMEPLAY" if world_at is not None else "EXITED")
                break
            if now > args.timeout:
                verdict = "TIMEOUT"
                break
            time.sleep(0.4)
        kill(proc)

    took = time.time() - started
    shots = sorted(p.name for p in frames.glob("*.png")) if frames.exists() else []
    summary = {
        "label": args.label, "verdict": verdict, "cause": cause, "seconds": round(took, 1),
        "exit": proc.returncode, "reached": dict((k, round(v, 1)) for k, v in reached.items()),
        "counts": counts, "frames": shots, "run": str(run),
    }
    (run / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")

    lines = ["# {} - {} ({:.0f}s, exit {})".format(args.label, verdict, took, proc.returncode), ""]
    if cause:
        lines += ["## cause", "    " + cause, ""]
    lines.append("## reached: " + (", ".join("{} @{:.0f}s".format(k, v) for k, v in reached.items()) or "nothing"))
    lines += ["", "## counts"]
    for bucket, n in sorted(counts.items(), key=lambda kv: -kv[1]):
        lines.append("- {}: {}".format(bucket, n))
    for bucket in ("fatal", "gpu_removed", "shader_no_translate", "raytracing", "mesh_missing", "world_frame"):
        if bucket in kept:
            lines += ["", "## {} (first {})".format(bucket, len(kept[bucket]))]
            lines += ["    " + l for l in kept[bucket]]
    for bucket in ("pipeline_refused", "texture_refused", "not_implemented", "debug_layer"):
        top = sorted(sigs.get(bucket, {}).items(), key=lambda kv: -kv[1])[:5]
        if top:
            lines += ["", "## {} - top signatures".format(bucket)]
            lines += ["    {:5d}  {}".format(n, s) for s, n in top]
    lines += ["", "## frames: {} {}".format(len(shots), ", ".join(shots[:10]) if shots else "(none)"),
              "", "log: {}".format(err_path)]
    report = "\n".join(lines)
    (run / "report.md").write_text(report, encoding="utf-8")
    print(report)
    return 0 if verdict == "GAMEPLAY" else 1


if __name__ == "__main__":
    sys.exit(main())
