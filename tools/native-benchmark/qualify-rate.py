"""Exercise the release Rust CLI on saved synthetic benchmark inputs; no controller."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def save(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    inputs, output = args.inputs.resolve(), args.output.resolve()
    require(not output.exists(), "Output must be a new directory")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    require(not subprocess.check_output([
        "git", "status", "--porcelain", "--untracked-files=no", "--",
        "src-rust", "crates", "Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain.toml"
    ], cwd=root, text=True).strip(), "Compiler inputs must be committed")
    subprocess.run(["cargo", "build", "--release", "--locked"], cwd=root, check=True)
    binary = root / "target/release" / ("nextnc-native.exe" if os.name == "nt" else "nextnc-native")
    reference = root / "tests-rust/fixtures/admission/reference.json"
    identity = {"compilerRevision": revision, "executableSHA256": sha(binary),
                "referenceSHA256": sha(reference), "runnerSHA256": sha(Path(__file__)),
                "inputManifestSHA256": sha(inputs / "manifest.json")}
    manifest = json.loads((inputs / "manifest.json").read_text(encoding="utf-8"))
    cases = [c for c in manifest["cases"] if c["name"].endswith("34000") or "inch" in c["name"]]
    require(len(cases) == 6, "Expected four 34000-motion cases and two inch cases")
    output.mkdir(parents=True)
    save(output / "started.json", {**identity, "complete": False})
    results = []
    for case in cases:
        name = case["name"]
        directory = output / name
        directory.mkdir()
        source, setup = inputs / case["source"], inputs / case["setup"]
        require(sha(source) == case["sourceSHA256"] and sha(setup) == case["setupSHA256"],
                f"Input hash mismatch: {name}")
        environment = os.environ.copy()
        environment["PATH"] = ""
        environment["NEXTNC_DIAGNOSTICS"] = str(directory / "diagnostics")

        def invoke(label, arguments):
            started = time.perf_counter()
            run = subprocess.run([str(binary), *map(str, arguments)], cwd=root,
                                 env=environment, capture_output=True, timeout=240)
            elapsed = time.perf_counter() - started
            (directory / (label + ".stdout.json")).write_bytes(run.stdout)
            (directory / (label + ".stderr.txt")).write_bytes(run.stderr)
            require(run.returncode == 0, f"{name}/{label}: exit {run.returncode}")
            report = json.loads(run.stdout)
            require(report["executable"] is False and report["liveBinding"] == "not_checked",
                    f"Unexpected execution claim: {name}/{label}")
            require(report["diagnostics"]["status"] == "saved", "Diagnostics were not archived")
            return report, elapsed

        published, _ = invoke("publish", ["publish", source, setup, "--store", directory / "store"])
        artifact = Path(published["artifact"])
        artifact_hash = sha(artifact)
        require(artifact_hash == published["artifactSHA256"], "Artifact hash mismatch")
        analyzed, elapsed = invoke("analyze", ["analyze-rate", artifact, reference])
        repeated, _ = invoke("repeat", ["analyze-rate", artifact, reference])
        demand = analyzed["commandDemand"]
        require(demand == repeated["commandDemand"], "Analysis must be deterministic")
        require(sha(artifact) == artifact_hash == analyzed["artifactSHA256"], "Analysis mutated artifact")
        require(demand["reference"]["input_sha256"] == identity["referenceSHA256"], "Wrong reference")
        require(demand["reference"]["native_executor_qualified"] is False, "Reference misrepresented")
        require(demand["execution_authorized"] is False, "Execution incorrectly authorized")
        for key in ("native_capacity_commands_per_second", "predicted_native_mailbox_calls",
                    "predicted_downstream_pieces"):
            require(demand[key] is None, f"Unqualified native estimate: {key}")
        counts, expected = demand["counts"], case["expected"]
        require(counts["motion_commands"] == expected["motions"], "Lost source motions")
        require(counts["line_commands"] == expected["lines"], "Wrong line count")
        require(counts["planar_circular_commands"] + counts["helical_commands"] == expected["circular"],
                "Lost analytic circular geometry")
        require(counts["helical_commands"] == expected["helices"], "Lost helices")
        require(counts["multi_turn_commands"] == expected["multiTurns"], "Lost multiple turns")
        require(counts["prepared_commands"] == counts["motion_commands"] + counts["state_commands"]
                + counts["reviewed_waypoints"], "Incomplete state/waypoint accounting")
        require(sum(counts["state_by_kind"].values()) == counts["state_commands"], "Missing state kinds")
        require(counts["prepared_commands"] == published["audit"]["commands"], "Audit count disagreement")
        warnings = [w["code"] for w in demand["warnings"]]
        if expected["machine"] == "lathe":
            require(counts["per_revolution_motions"] == expected["motions"], "Lost G95 dimension")
            require(demand["densest_nominal_window"] is None, "Invented G95 timing")
            require("SYNCHRONIZED_DURATION_UNKNOWN" in warnings, "Missing G95 diagnostic")
        elif "polyline" in name:
            require("NOMINAL_DENSITY_ABOVE_REFERENCE" in warnings, "Missing dense-line screening")
        require(sha(source) == case["sourceSHA256"] and sha(setup) == case["setupSHA256"],
                "Inputs changed during analysis")
        results.append({"case": name, "sourceSHA256": sha(source), "setupSHA256": sha(setup),
                        "artifactSHA256": artifact_hash, "artifactBytes": artifact.stat().st_size,
                        "bundleIdentity": analyzed["identity"], "counts": counts,
                        "knownFeedAndDwellSeconds": demand["known_feed_and_dwell_seconds"],
                        "densestNominalWindow": demand["densest_nominal_window"],
                        "observedReferenceRange": demand["observed_reference_all_call_rate_range"],
                        "warnings": warnings, "analysisProcessSeconds": elapsed,
                        "deterministicReport": True, "unchangedArtifact": True,
                        "rawAnalysisSHA256": sha(directory / "analyze.stdout.json")})
        print(f"PASS {name}: {counts['prepared_commands']} commands", flush=True)
    require(sha(binary) == identity["executableSHA256"] and sha(reference) == identity["referenceSHA256"],
            "Executable/reference changed during qualification")
    save(output / "results.json", {"schema": "nextnc-native/rate-qualification/1", **identity,
         "complete": True, "executionAuthorized": False, "cases": results,
         "scope": "Offline release CLI validation; process times include reload and diagnostics, not motion performance"})


if __name__ == "__main__":
    main()
