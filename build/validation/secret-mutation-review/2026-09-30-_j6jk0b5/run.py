from pathlib import Path
import difflib, hashlib, json, os, shutil, subprocess, time

ROOT = Path('/Users/richardwillars/Documents/Codex/LLM-coding')
WORK = Path('/private/tmp/jadpo-secret-mutations-_j6jk0b5')
EVIDENCE = Path('/Users/richardwillars/Documents/Codex/LLM-coding/build/validation/secret-mutation-review/2026-09-30-_j6jk0b5')
source = ROOT / "jadpo"
compiler = WORK / "jadpo"

def excluded(directory, names):
    skipped = [name for name in names if name.endswith((".sqlite", ".sqlite-shm", ".sqlite-wal", ".db")) or name == ".DS_Store"]
    if Path(directory) == source and "target" in names:
        skipped.append("target")
    return skipped

shutil.copytree(source, compiler, ignore=excluded)
assert (compiler / "crates/core/src/target/first_party.rs").is_file()
assert (compiler / "data/iana-zones-2026c.txt").is_file()
assert not (compiler / "target").exists()
def sha(data): return hashlib.sha256(data).hexdigest()
hashes = {str(path.relative_to(compiler)): sha(path.read_bytes()) for path in sorted(compiler.rglob("*")) if path.is_file()}
(EVIDENCE / "source-hashes.json").write_text(json.dumps(hashes, indent=2) + "\n")
path = compiler / "crates/semantic/src/typecheck.rs"
original = path.read_text()
test_path = compiler / "crates/core/tests/validation_config_time_contract.rs"
tests_sha = sha(test_path.read_bytes())
mutations = [
  ("constructor-secret", "explicit_construction_cannot_remove_secret_classification",
   "        result.secret = argument_type.as_ref().is_some_and(|value| value.secret);",
   "        result.secret = false;"),
  ("temporal-secret", "temporal_helpers_are_not_secret_sinks",
   "        // Temporal helpers are ordinary computation, not declared adapter\n        // sinks. Representation compatibility cannot declassify a secret.\n        if received.secret {\n            self.push_diagnostic(\"CONFIG_SECRET_FLOW\", source, range);\n            return;\n        }\n",
   "        // MUTANT: accept secret temporal arguments through ordinary compatibility.\n"),
]
results = []
environment = dict(os.environ)
environment["CARGO_TARGET_DIR"] = str(WORK / "cargo-target")

def run(label, test, phase):
    command = ["cargo", "test", "--offline", "--manifest-path", str(compiler / "Cargo.toml"), "-p", "jadpo-core", "--test", "validation_config_time_contract", test, "--", "--exact", "--nocapture"]
    start = time.monotonic()
    result = subprocess.run(command, cwd=WORK, env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    log = EVIDENCE / f"{label}-{phase}.log"
    log.write_text(result.stdout)
    built_test = "Running tests/validation_config_time_contract.rs" in result.stdout and "could not compile" not in result.stdout
    passed = result.returncode == 0 and "1 passed; 0 failed" in result.stdout
    killed = result.returncode == 101 and built_test and "missing CONFIG_SECRET_FLOW" in result.stdout and f"test {test} ... FAILED" in result.stdout and "0 passed; 1 failed" in result.stdout
    record = {"mutation":label,"test":test,"phase":phase,"command":command,"exit_code":result.returncode,"elapsed_seconds":round(time.monotonic()-start,3),"test_executed":built_test,"passed":passed,"killed_by_intended_assertion":killed,"source_sha256":sha(path.read_bytes()),"test_sha256":sha(test_path.read_bytes()),"log":log.name}
    results.append(record)
    (EVIDENCE / "results.json").write_text(json.dumps(results,indent=2)+"\n")
    print(json.dumps({"mutation":label,"phase":phase,"exit_code":result.returncode,"passed":passed,"killed":killed}), flush=True)
    assert record["test_sha256"] == tests_sha
    return passed if phase != "mutant" else killed

for label, test, before, after in mutations:
    assert path.read_text() == original
    assert original.count(before) == 1, (label, "mutation anchor ambiguous")
    assert run(label,test,"baseline"), "baseline did not pass"
    mutant=original.replace(before,after,1)
    (EVIDENCE / f"{label}.diff").write_text(''.join(difflib.unified_diff(original.splitlines(keepends=True),mutant.splitlines(keepends=True),fromfile="a/jadpo/crates/semantic/src/typecheck.rs",tofile="b/jadpo/crates/semantic/src/typecheck.rs")))
    try:
        path.write_text(mutant)
        detected=run(label,test,"mutant")
    finally:
        path.write_text(original)
    restored=run(label,test,"restored")
    assert detected, "mutant was not caught by the intended executable assertion"
    assert restored, "restored baseline did not pass"
summary={"status":"both_mutants_caught", "snapshot":str(compiler), "evidence":str(EVIDENCE), "mutations":len(mutations), "runs":len(results), "source_restored":sha(path.read_bytes())==hashes["crates/semantic/src/typecheck.rs"], "tests_unchanged":sha(test_path.read_bytes())==tests_sha, "production_files_modified":False, "no_application_database_used":True}
(EVIDENCE / "summary.json").write_text(json.dumps(summary,indent=2)+"\n")
print(json.dumps(summary),flush=True)
