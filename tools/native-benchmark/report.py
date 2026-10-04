#!/usr/bin/env python3
"""Render recorded benchmark data, optionally comparing semantic-identical runs."""
import argparse
import json
from pathlib import Path
import statistics


def load(path):
    data = json.loads(path.read_text())
    if data.get("schema") != "nextnc-native/benchmark-results/1" or data.get("complete") is not True:
        raise ValueError("Only a completed benchmark matrix can be reported")
    names = [case["case"] for case in data["cases"]]
    if len(set(names)) != len(names) or set(names) != {case["name"] for case in data["inputs"]["cases"]}:
        raise ValueError("Missing or duplicated benchmark case")
    compiler = None
    for case in data["cases"]:
        if set(case["phases"]) != {"prepare", "compile", "load"}:
            raise ValueError("Missing or unexpected benchmark phase")
        identity = None
        expected = next(item for item in data["inputs"]["cases"] if item["name"] == case["case"])
        for phase_name, phase in case["phases"].items():
            if len(phase["runs"]) != data["processesPerCasePhase"]:
                raise ValueError("Missing benchmark process")
            for run in phase["runs"]:
                worker = run["worker"]
                if len(worker["samples"]) != data["samplesPerProcess"]:
                    raise ValueError("Missing benchmark sample")
                if (worker["case"] != case["case"] or worker["phase"] != phase_name
                        or worker["executable"] is not False
                        or worker["result"]["semantics"] != expected["expected"]
                        or worker["result"]["audit"]["execution_authorized"] is not False):
                    raise ValueError("Worker phase/geometry/permission evidence disagrees")
                binding = worker["inputIdentity"]
                code = (worker["compilerSHA256"], worker["schemaSHA256"], worker["policySHA256"])
                if compiler is not None and code != compiler:
                    raise ValueError("Compiler/schema/policy changed within the matrix")
                if code != (binding["compiler_sha256"], binding["schema_sha256"], binding["policy_sha256"]):
                    raise ValueError("Compiler identity disagrees with artifact binding")
                compiler = code
                if binding["source_sha256"] != expected["sourceSHA256"] or binding["setup_sha256"] != expected["setupSHA256"]:
                    raise ValueError("Worker input hashes disagree")
                current = (binding, worker["result"])
                if identity is not None and current != identity:
                    raise ValueError("Identity/ordered command evidence differs across calls")
                identity = current
                if phase_name != "prepare" and (worker["artifactSHA256"] != case["artifact"]["sha256"]
                                                or worker["artifactBytes"] != case["artifact"]["bytes"]):
                    raise ValueError("Bundle evidence differs across calls")
                if any(sample["index"] != i or sample["temperature"] != ("first_call" if i == 0 else "warm")
                       or sample["coreWallNs"] <= 0 for i, sample in enumerate(worker["samples"])):
                    raise ValueError("Invalid timing sample")
            first = [r["worker"]["samples"][0]["coreWallNs"] for r in phase["runs"]]
            warm = [s["coreWallNs"] for r in phase["runs"] for s in r["worker"]["samples"][1:]]
            computed = {"firstCallMedianNs": statistics.median(first), "warmMedianNs": statistics.median(warm),
                        "warmMinNs": min(warm), "warmMaxNs": max(warm),
                        "maxPeakResidentBytes": max(r["memory"]["peakResidentBytes"] for r in phase["runs"])}
            if any(phase[k] != v for k, v in computed.items()) or computed["maxPeakResidentBytes"] <= 0:
                raise ValueError("Benchmark summaries disagree with raw evidence")
    return data


def semantics(case):
    return case["phases"]["prepare"]["runs"][0]["worker"]["result"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--baseline", type=Path)
    args = parser.parse_args()
    data = load(args.results)
    baseline = load(args.baseline) if args.baseline else None
    if baseline:
        for field in ("inputManifestSHA256", "environment", "processesPerCasePhase", "samplesPerProcess"):
            if baseline[field] != data[field]:
                raise ValueError(f"Comparison requires identical {field}")
        old = {case["case"]: case for case in baseline["cases"]}
        for case in data["cases"]:
            if case["case"] not in old or semantics(case) != semantics(old[case["case"]]):
                raise ValueError("Comparison changed complete ordered command/span identity or semantics")
            a = case["phases"]["prepare"]["runs"][0]["worker"]
            b = old[case["case"]]["phases"]["prepare"]["runs"][0]["worker"]
            if any(a[key] != b[key] for key in ("schemaSHA256", "policySHA256")):
                raise ValueError("Comparison changed source/bundle schema or command policy")
    out = ["# Native compiler benchmark results", "",
           f"Compiler source: `{data['sourceCommit']}`. Worker SHA-256: `{data['workerSHA256']}`.",
           f"Environment: {data['environment']['platform']}; {data['environment']['processor']}.", "",
           f"Each case/phase uses {data['processesPerCasePhase']} fresh processes and {data['samplesPerProcess']} calls per process. "
           "First call means a fresh process, with OS page caches uncontrolled. Warm values are medians of the remaining calls.", "",
           "## Measurements", "",
           "| Case | Prepare first/warm (ms) | Bundle compile first/warm (ms) | Validated reload first/warm (ms) | Peak RSS prepare/compile/load (MiB) |",
           "| --- | ---: | ---: | ---: | ---: |"]
    for case in data["cases"]:
        phases = [case["phases"][phase] for phase in ("prepare", "compile", "load")]
        timing = [f"{p['firstCallMedianNs']/1e6:.2f} / {p['warmMedianNs']/1e6:.2f}" for p in phases]
        memory = " / ".join(f"{p['maxPeakResidentBytes']/2**20:.1f}" for p in phases)
        out.append(f"| {case['case']} | {' | '.join(timing)} | {memory} |")
    if baseline:
        out += ["", "## Matched baseline comparison", "", f"Baseline source: `{baseline['sourceCommit']}`. "
                "Inputs, environment, schema/policy and complete ordered commands/spans agree. "
                "Values below are candidate/baseline ratios; below 1 is a measured reduction. "
                "These repeated workstation measurements do not establish causality for small timing differences or real-time deadlines.", "",
                "| Case | Prepare warm ratio | Compile warm ratio | Reload warm ratio | Peak RSS ratio prepare/compile/load |",
                "| --- | ---: | ---: | ---: | ---: |"]
        for case in data["cases"]:
            ratios = [case["phases"][p]["warmMedianNs"] / old[case["case"]]["phases"][p]["warmMedianNs"] for p in ("prepare", "compile", "load")]
            memory = [case["phases"][p]["maxPeakResidentBytes"] / old[case["case"]]["phases"][p]["maxPeakResidentBytes"] for p in ("prepare", "compile", "load")]
            out.append(f"| {case['case']} | {' | '.join(f'{r:.3f}' for r in ratios)} | {' / '.join(f'{r:.3f}' for r in memory)} |")
    out += ["", "## Interpretation", "",
            "Prepare includes complete source/setup parsing and independent command audits. Bundle compile repeats prepare and includes independent reload; "
            "columns overlap and must not be added. Reload revalidates embedded inputs, identities and every serialized command. "
            "It restores no live binding or execution permission.", "",
            "Core timing excludes input reads, buffer copies, result checking/destruction and diagnostic output. "
            "OS peak RSS includes those allocations and warm-process allocator retention. The raw results retain every sample, "
            "per-process memory counter, artifact identity and exact command/span digest. "
            "No controller, step generator, machining-time or Raspberry Pi measurement is represented.", ""]
    with args.output.open("x", encoding="utf-8", newline="\n") as stream:
        stream.write("\n".join(out))


if __name__ == "__main__":
    main()
