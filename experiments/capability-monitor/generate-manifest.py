#!/usr/bin/env python3
"""Derive the monitor's small route binding from checked projection artifacts.

This is intentionally a fixture-local compiler integration seam.  The monitor
does not infer policy from source text: it consumes this deterministic output
alongside the checked contract and application module.
"""
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PROJECTED = ROOT / "experiments/capability-host/build/projected/program.json"
CONTRACT = ROOT / "experiments/capability-host/build/contract.json"
OUTPUT = ROOT / "experiments/capability-monitor/manifest.json"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def declarations(program):
    return {item["name"]: item for item in program["declarations"]}


def plan_ids(contract):
    return {
        item["descriptor"]["operation"]: item["descriptor"]["semanticOperationId"]
        for item in contract["plans"]
    }


def failure_name(node):
    if isinstance(node, str):
        return node
    if isinstance(node, dict) and node.get("op") == "reject":
        return node.get("failure")
    return None


def effects(node, decls, plans, current):
    """Walk checked lowering in evaluation order and collect storage effects."""
    if isinstance(node, list):
        for item in node:
            yield from effects(item, decls, plans, current)
        return
    if not isinstance(node, dict):
        return
    if node.get("op") == "call":
        target = node.get("target")
        if target not in decls:
            raise ValueError(f"missing callable declaration {target!r}")
        yield from effects(decls[target].get("body", []), decls, plans, target)
        return
    host_effect = node.get("hostEffect")
    if host_effect in ("storage.read", "storage.update"):
        plan = plans.get(current)
        if plan is None:
            raise ValueError(f"missing checked storage plan for {current!r}")
        conflicts = node.get("conflicts") or []
        conflict = None
        if conflicts:
            conflict = failure_name(conflicts[0].get("failure"))
        yield {
            "plan": plan,
            "write": host_effect == "storage.update",
            "missing": failure_name((node.get("missing") or {}).get("failure")),
            "conflict": conflict,
        }
        return
    for value in node.values():
        yield from effects(value, decls, plans, current)


def terminal_failure(body):
    for node in reversed(body):
        if isinstance(node, dict) and node.get("op") == "reject":
            return node.get("failure")
    return None


def main():
    program = json.loads(PROJECTED.read_text())
    contract = json.loads(CONTRACT.read_text())
    decls = declarations(program)
    plans = plan_ids(contract)
    records = {
        name: [field["name"] for field in decl.get("fields", [])]
        for name, decl in decls.items()
        if decl.get("kind") == "record"
    }
    routes = []
    contract_routes = {(r["path"], r["method"]): r for r in contract["routes"]}
    for boundary in program["boundaryDeclarations"]:
        if boundary.get("kind") != "route":
            continue
        target = boundary["run"]["target"]
        operation = boundary["run"]["targetId"]
        checked = contract_routes[(boundary["path"], boundary["method"].upper())]
        route_effects = list(effects(decls[target]["body"], decls, plans, target))
        if not route_effects:
            raise ValueError(f"route {boundary['path']} has no storage effects")
        output = boundary["output"]["name"]
        projection = records.get(output)
        if not projection:
            raise ValueError(f"route {boundary['path']} output {output!r} is not a record")
        route = {
            "path": boundary["path"],
            "method": boundary["method"].upper(),
            "operation": operation,
            "atomic": checked["atomic"],
            "effects": route_effects,
            "projection": projection,
        }
        failure = terminal_failure(decls[target].get("body", []))
        if failure:
            route["failure"] = failure
        routes.append(route)
    manifest = {
        "schemaVersion": 1,
        "checkedProjectionSha256": digest(PROJECTED),
        "contractSha256": digest(CONTRACT),
        "routes": routes,
    }
    OUTPUT.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"wrote {OUTPUT.relative_to(ROOT)} ({len(routes)} routes)")


if __name__ == "__main__":
    main()
