#!/usr/bin/env python3
"""Verify and digest the candidate P10R contract without third-party packages."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TODO = ROOT / "examples" / "golden-todo"

CONTRACT_FILES = (
    ROOT / "tools" / "verify-p10r.py",
    TODO / "README.md",
    TODO / "acceptance.json",
    TODO / "policy.jadpo",
    TODO / "app.jadpo",
    TODO / "expected-audit.md",
    TODO / "adversarial-changes.md",
    TODO / "typescript-baseline.md",
    TODO / "language-friction.md",
    TODO / "REVIEW.md",
    ROOT / "docs" / "research-brief.md",
    ROOT / "docs" / "policy-proof-v0.1.md",
    ROOT / "docs" / "validation-rules-v0.1.md",
    ROOT / "docs" / "threat-model.md",
    ROOT / "docs" / "approval-protocol.md",
    ROOT / "docs" / "comparison-protocol.md",
    ROOT / "docs" / "first-user-review-guide.md",
    ROOT / "docs" / "comprehension-study.md",
    ROOT / "research" / "first-user-review-record.template.json",
    ROOT / "tests" / "assurance" / "policy-proof-v0.1.json",
    ROOT / "tests" / "assurance" / "approval-protocol-v0.1.json",
    ROOT / "tests" / "assurance" / "evidence-map-v0.1.json",
    ROOT / "tests" / "assurance" / "comprehension-v0.1.json",
)

REQUIRED_CASE_FAMILIES = {
    "AUTH",
    "PUBLIC",
    "CREATE",
    "READ",
    "LIST",
    "REL",
    "PATCH",
    "DELETE",
    "USER",
    "JOB",
    "CONFIG",
    "BOUNDARY",
}


def fail(message: str) -> None:
    print(f"P10R verification failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def verify_files() -> None:
    missing = [path.relative_to(ROOT) for path in CONTRACT_FILES if not path.is_file()]
    if missing:
        fail("missing contract files: " + ", ".join(map(str, missing)))

    for path in CONTRACT_FILES:
        if path.suffix == ".json":
            try:
                json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError) as error:
                fail(f"cannot parse {path.relative_to(ROOT)}: {error}")


def verify_acceptance() -> tuple[str, set[str]]:
    path = TODO / "acceptance.json"
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot parse {path.relative_to(ROOT)}: {error}")

    version = document.get("contractVersion")
    if version != "todo-v0.1":
        fail(f"unexpected contractVersion {version!r}")
    if document.get("status") != "candidate-freeze":
        fail("acceptance status must remain candidate-freeze until external review")

    cases = document.get("cases")
    if not isinstance(cases, list) or not cases:
        fail("acceptance cases must be a non-empty list")

    identifiers: set[str] = set()
    families: set[str] = set()
    for index, case in enumerate(cases):
        if not isinstance(case, dict):
            fail(f"case {index} is not an object")
        identifier = case.get("id")
        if not isinstance(identifier, str) or not re.fullmatch(r"[A-Z]+-[0-9]{3}", identifier):
            fail(f"case {index} has invalid id {identifier!r}")
        if identifier in identifiers:
            fail(f"duplicate case id {identifier}")
        identifiers.add(identifier)
        families.add(identifier.split("-", 1)[0])
        for key in ("kind", "title", "expect"):
            if key not in case:
                fail(f"case {identifier} is missing {key}")

    missing_families = sorted(REQUIRED_CASE_FAMILIES - families)
    if missing_families:
        fail("missing acceptance families: " + ", ".join(missing_families))

    return version, identifiers


def heading_ids(path: Path) -> set[str]:
    return set(
        re.findall(
            r"^### ([A-Z][A-Z0-9-]+)$",
            path.read_text(encoding="utf-8"),
            flags=re.MULTILINE,
        )
    )


def verify_assurance_fixtures(acceptance_ids: set[str]) -> tuple[int, int, int, int]:
    proof_rules = heading_ids(ROOT / "docs" / "policy-proof-v0.1.md")
    validation_rules = heading_ids(ROOT / "docs" / "validation-rules-v0.1.md")
    all_rules = proof_rules | validation_rules
    if not proof_rules or not validation_rules:
        fail("could not discover proof and validation rule headings")

    proof_path = ROOT / "tests" / "assurance" / "policy-proof-v0.1.json"
    proof_cases = json.loads(proof_path.read_text(encoding="utf-8"))["cases"]
    proof_ids: set[str] = set()
    polarities: dict[str, set[str]] = {rule: set() for rule in proof_rules}
    for case in proof_cases:
        identifier = case.get("id")
        rule = case.get("rule")
        polarity = case.get("polarity")
        if identifier in proof_ids:
            fail(f"duplicate proof fixture id {identifier}")
        proof_ids.add(identifier)
        if rule not in proof_rules:
            fail(f"proof fixture {identifier} names unknown rule {rule}")
        if polarity not in {"positive", "negative", "indeterminate"}:
            fail(f"proof fixture {identifier} has invalid polarity {polarity}")
        polarities[rule].add(polarity)
    for rule, seen in polarities.items():
        if seen != {"positive", "negative", "indeterminate"}:
            fail(f"proof rule {rule} lacks positive/negative/indeterminate fixtures")

    approval_path = ROOT / "tests" / "assurance" / "approval-protocol-v0.1.json"
    approval_cases = json.loads(approval_path.read_text(encoding="utf-8"))["cases"]
    approval_ids = {case.get("id") for case in approval_cases}
    approval_spec = (ROOT / "docs" / "approval-protocol.md").read_text(encoding="utf-8")
    expected_approval_ids = set(re.findall(r"^\| (AP-[0-9]{2}) \|", approval_spec, re.MULTILINE))
    if approval_ids != expected_approval_ids:
        fail("approval fixtures do not exactly match the approval protocol table")

    evidence_path = ROOT / "tests" / "assurance" / "evidence-map-v0.1.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    acceptance_map = evidence.get("acceptance", {})
    mapped_acceptance_ids = set(acceptance_map)
    if mapped_acceptance_ids != acceptance_ids:
        missing = sorted(acceptance_ids - mapped_acceptance_ids)
        extra = sorted(mapped_acceptance_ids - acceptance_ids)
        fail(f"evidence acceptance mismatch; missing={missing}, extra={extra}")

    threat_spec = (ROOT / "docs" / "threat-model.md").read_text(encoding="utf-8")
    expected_threats = set(re.findall(r"^\| (TM-[0-9]{2}) \|", threat_spec, re.MULTILINE))
    if not expected_threats:
        fail("could not discover threat identifiers from threat model")
    for identifier, mapping in acceptance_map.items():
        rules = set(mapping.get("rules", []))
        threats = set(mapping.get("threats", []))
        if not rules or not rules <= all_rules:
            fail(f"acceptance {identifier} has empty or unknown rules: {sorted(rules - all_rules)}")
        if not threats or not threats <= expected_threats:
            fail(f"acceptance {identifier} has empty or unknown threats: {sorted(threats - expected_threats)}")

    threat_claims = evidence.get("threatClaims", {})
    if set(threat_claims) != expected_threats:
        fail("evidence map must cover every threat-model entry exactly")
    known_evidence = acceptance_ids | proof_ids | approval_ids
    for threat, claim in threat_claims.items():
        rules = set(claim.get("rules", []))
        if not rules or not rules <= all_rules:
            fail(f"threat {threat} has empty or unknown rules: {sorted(rules - all_rules)}")
        references = claim.get("evidence", [])
        planned = claim.get("planned", [])
        if not references and not planned:
            fail(f"threat {threat} has no evidence or planned evidence")
        for reference in references:
            if reference not in known_evidence and not reference.startswith("EVIDENCE-"):
                fail(f"threat {threat} has unknown evidence reference {reference}")
        for reference in planned:
            if not reference.startswith("PLAN-"):
                fail(f"threat {threat} planned reference lacks PLAN- prefix: {reference}")

    comprehension_path = ROOT / "tests" / "assurance" / "comprehension-v0.1.json"
    comprehension = json.loads(comprehension_path.read_text(encoding="utf-8"))
    if set(comprehension.get("surfaces", [])) != {
        "pull_request_diff",
        "behavioural_diff",
        "relationship_effect_view",
    }:
        fail("comprehension instrument must contain the three comparison surfaces")
    questions = comprehension.get("questions", [])
    question_ids: set[str] = set()
    critical_categories: set[str] = set()
    total_points = 0
    for question in questions:
        identifier = question.get("id")
        if not isinstance(identifier, str) or not re.fullmatch(r"COMP-[0-9]{2}", identifier):
            fail(f"invalid comprehension question id {identifier!r}")
        if identifier in question_ids:
            fail(f"duplicate comprehension question id {identifier}")
        question_ids.add(identifier)
        facts = question.get("expectedFacts", [])
        fact_ids = [fact.get("id") for fact in facts]
        if not facts or len(fact_ids) != len(set(fact_ids)):
            fail(f"comprehension question {identifier} has missing or duplicate expected facts")
        for fact in facts:
            points = fact.get("points")
            if not isinstance(points, int) or points <= 0 or not fact.get("fact"):
                fail(f"comprehension question {identifier} has invalid scoring fact")
            total_points += points
        if question.get("critical") is True:
            critical_categories.add(question.get("category"))
    required_critical_categories = {
        "surface",
        "authentication",
        "authorization",
        "creation",
        "lifecycle",
        "external_effect",
        "failure",
        "approval",
    }
    if not required_critical_categories <= critical_categories:
        fail("comprehension instrument lacks required critical categories")

    return len(proof_cases), len(approval_cases), len(questions), total_points


def verify_contract_identity(version: str) -> None:
    identity_files = (TODO / "README.md", TODO / "app.jadpo", TODO / "policy.jadpo")
    for path in identity_files:
        if version not in path.read_text(encoding="utf-8"):
            fail(f"{path.relative_to(ROOT)} does not name {version}")


def verify_markdown_links() -> int:
    markdown_files = [ROOT / "README.md"]
    markdown_files.extend(sorted((ROOT / "docs").glob("*.md")))
    markdown_files.extend(sorted(TODO.glob("*.md")))

    checked = 0
    for source in markdown_files:
        text = source.read_text(encoding="utf-8")
        for raw_target in re.findall(r"\[[^\]]*\]\(([^)]+)\)", text):
            target = raw_target.split("#", 1)[0]
            if not target or "://" in target or target.startswith("mailto:"):
                continue
            checked += 1
            if not (source.parent / target).resolve().exists():
                fail(f"broken link in {source.relative_to(ROOT)}: {raw_target}")
    return checked


def print_digests() -> None:
    for path in CONTRACT_FILES:
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        print(f"{digest}  {path.relative_to(ROOT).as_posix()}")


def main() -> None:
    verify_files()
    version, acceptance_ids = verify_acceptance()
    verify_contract_identity(version)
    proof_case_count, approval_case_count, question_count, comprehension_points = (
        verify_assurance_fixtures(acceptance_ids)
    )
    link_count = verify_markdown_links()
    print(
        "P10R candidate verified: "
        f"{version}, {len(acceptance_ids)} acceptance cases, "
        f"{proof_case_count} proof fixtures, {approval_case_count} approval fixtures, "
        f"{question_count} comprehension questions/{comprehension_points} points, "
        f"{link_count} local links"
    )
    print("Candidate digests (informational; not a freeze manifest):")
    print_digests()


if __name__ == "__main__":
    main()
