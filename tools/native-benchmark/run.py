#!/usr/bin/env python3
"""Run the Rust-only worker in isolated processes; retain timings and OS peak RSS.

Python is development orchestration, not part of job preparation or execution.
No installed package, controller connection, Node process or GUI is used here.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import queue
import statistics
import subprocess
import threading
import time


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def peak_memory(pid):
    if os.name == "nt":
        # Query our own live child. No machine/controller or unrelated process access.
        class Counters(ctypes.Structure):
            _fields_ = [("cb", ctypes.c_uint32), ("PageFaultCount", ctypes.c_uint32)] + [
                (name, ctypes.c_size_t) for name in (
                    "PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage",
                    "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage", "QuotaNonPagedPoolUsage",
                    "PagefileUsage", "PeakPagefileUsage")]
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        psapi = ctypes.WinDLL("psapi", use_last_error=True)
        kernel.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
        kernel.OpenProcess.restype = ctypes.c_void_p
        kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        psapi.GetProcessMemoryInfo.argtypes = [ctypes.c_void_p, ctypes.POINTER(Counters), ctypes.c_uint32]
        handle = kernel.OpenProcess(0x0400 | 0x0010, False, pid)
        if not handle:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            counters = Counters()
            counters.cb = ctypes.sizeof(counters)
            if not psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb):
                raise ctypes.WinError(ctypes.get_last_error())
            return {"peakResidentBytes": counters.PeakWorkingSetSize,
                    "peakCommitBytes": counters.PeakPagefileUsage,
                    "method": "Windows GetProcessMemoryInfo PeakWorkingSetSize/PeakPagefileUsage"}
        finally:
            kernel.CloseHandle(handle)
    fields = dict(line.split(":", 1) for line in Path(f"/proc/{pid}/status").read_text().splitlines())
    return {"peakResidentBytes": int(fields["VmHWM"].split()[0]) * 1024,
            "peakVirtualBytes": int(fields["VmPeak"].split()[0]) * 1024,
            "method": "Linux /proc/PID/status VmHWM and VmPeak; peak virtual is not committed memory"}


def measure(command, timeout):
    start = time.perf_counter_ns()
    # Worker uses absolute paths and the Rust runtime only. PATH is deliberately absent.
    env = {key: os.environ[key] for key in ("SystemRoot", "WINDIR") if key in os.environ}
    child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE, text=True, encoding="utf-8", env=env)
    ready = queue.Queue()
    threading.Thread(target=lambda: ready.put(child.stdout.readline()), daemon=True).start()
    try:
        line = ready.get(timeout=timeout)
        elapsed = time.perf_counter_ns() - start
        if not line:
            _, error = child.communicate(timeout=5)
            raise RuntimeError(f"Worker exited {child.returncode}: {error}")
        result = json.loads(line)
        memory = peak_memory(child.pid)
        _, error = child.communicate("done\n", timeout=5)
        if child.returncode != 0:
            raise RuntimeError(f"Worker exited {child.returncode}: {error}")
        return {"processStartToResultWallNs": elapsed, "memory": memory, "worker": result}
    finally:
        if child.poll() is None:
            child.kill()
            child.communicate(timeout=5)
        for stream in (child.stdin, child.stdout, child.stderr):
            stream.close()


def write_new(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--worker", type=Path, required=True)
    parser.add_argument("--processes", type=int, default=3)
    parser.add_argument("--samples", type=int, default=6)
    parser.add_argument("--timeout", type=int, default=300)
    args = parser.parse_args()
    if not (1 <= args.processes <= 20 and 2 <= args.samples <= 101 and 1 <= args.timeout <= 3600):
        parser.error("require 1..20 processes, 2..101 samples and 1..3600 seconds timeout")
    root = Path(__file__).resolve().parents[2]
    git = lambda *argv: subprocess.check_output(["git", "-C", str(root), *argv], text=True).strip()
    if git("status", "--porcelain"):
        raise RuntimeError("Benchmark source checkout must be clean; commit the exact implementation first")
    build = subprocess.run(["cargo", "build", "--release", "--locked", "--example", "native_benchmark",
                            "--message-format=json-render-diagnostics"], cwd=root,
                           capture_output=True, text=True, timeout=args.timeout, check=True)
    built = [item["executable"] for line in build.stdout.splitlines()
             if (item := json.loads(line)).get("reason") == "compiler-artifact"
             and item.get("target", {}).get("name") == "native_benchmark" and item.get("executable")]
    if len(built) != 1:
        raise RuntimeError("Cargo did not attest exactly one benchmark executable")
    worker = args.worker.resolve(strict=True)
    if not worker.is_file() or worker != Path(built[0]).resolve(strict=True):
        raise RuntimeError("Worker must be the release executable Cargo just verified for this checkout")
    inputs = args.inputs.resolve(strict=True)
    manifest = json.loads((inputs / "manifest.json").read_text())
    if manifest["schema"] != "nextnc-native/benchmark-inputs/1":
        raise RuntimeError("Unknown fixture manifest")
    args.output.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    evidence = {"schema": "nextnc-native/benchmark-results/1", "sourceCommit": git("rev-parse", "HEAD"),
                "workerSHA256": sha(worker), "runnerSHA256": sha(Path(__file__)),
                "buildCommand": "cargo build --release --locked --example native_benchmark",
                "inputManifestSHA256": sha(inputs / "manifest.json"), "inputs": manifest,
                "environment": {"platform": platform.platform(), "machine": platform.machine(),
                                "processor": platform.processor(), "logicalCpus": os.cpu_count(),
                                "python": platform.python_version(),
                                "rustc": subprocess.check_output(["rustc", "-vV"], cwd=root, text=True).strip()},
                "processesPerCasePhase": args.processes, "samplesPerProcess": args.samples,
                "scope": "Offline compiler only; fresh processes do not imply cold OS page caches or machine execution",
                "timing": {"coreWallNs": "One complete phase; excludes input reads, result checking, destruction and diagnostic output. Bundle load excludes buffer copy.",
                           "compile": "Includes prepare, optional snapshot checks, encode, independent decode/audit; overlaps prepare/load; never add phase columns.",
                           "load": "Full embedded-input and serialized-command validation; never a trust-only deserialization or live binding.",
                           "processStartToResultWallNs": "Startup, file reads, all repeated phase calls, semantic/determinism checks and summary output; not one-job CLI latency.",
                           "memory": "OS process peak includes input buffers, repeated calls, allocator retention and result checks. One phase per fresh process; memory metrics differ by OS."},
                "cases": [], "complete": False}
    write_new(output / "started.json", evidence)
    for case in manifest["cases"]:
        name = case["name"]
        if Path(name).name != name:
            raise RuntimeError("Fixture name must be a basename")
        case_file = inputs / f"{name}.case.json"
        if json.loads(case_file.read_text()) != case:
            raise RuntimeError("Case descriptor differs from manifest")
        bundle_file = output / f"{name}.nncb"
        create = subprocess.run([str(worker), str(case_file), "create", "1", str(bundle_file)],
                                capture_output=True, text=True, timeout=args.timeout, check=True)
        artifact = json.loads(create.stdout)
        group = {"case": name, "artifact": artifact, "phases": {}}
        ordered = None
        for phase in ("prepare", "compile", "load"):
            runs = []
            for repeat in range(args.processes):
                result = measure([str(worker), str(case_file), phase, str(args.samples), str(bundle_file)], args.timeout)
                actual = result["worker"]
                digest = actual["result"]["orderedCommandSpanSHA256"]
                if ordered is not None and digest != ordered:
                    raise RuntimeError("Ordered commands/spans differ between processes or phases")
                ordered = digest
                if phase != "prepare" and actual["artifactSHA256"] != artifact["sha256"]:
                    raise RuntimeError("Bundle identity changed between create/compile/load")
                write_new(output / f"{name}-{phase}-{repeat}.json", result)
                runs.append(result)
            first = [r["worker"]["samples"][0]["coreWallNs"] for r in runs]
            warm = [s["coreWallNs"] for r in runs for s in r["worker"]["samples"][1:]]
            group["phases"][phase] = {"firstCallMedianNs": statistics.median(first), "warmMedianNs": statistics.median(warm),
                                      "warmMinNs": min(warm), "warmMaxNs": max(warm),
                                      "maxPeakResidentBytes": max(r["memory"]["peakResidentBytes"] for r in runs),
                                      "runs": runs}
            print(f"{name} {phase}: first {statistics.median(first)/1e6:.1f} ms; warm {statistics.median(warm)/1e6:.1f} ms; peak {group['phases'][phase]['maxPeakResidentBytes']/2**20:.1f} MiB", flush=True)
        evidence["cases"].append(group)
    evidence["complete"] = True
    write_new(output / "results.json", evidence)


if __name__ == "__main__":
    main()
