# Jadpo documentation

**Jadpo — one language for the whole web.**

This directory is the working source of truth for Jadpo. The canonical public
documentation address is [jadpo.dev/docs](https://jadpo.dev/docs/). This tree
extracts the substance of the original design conversation so future work does
not need that conversation for context.

## Start with the task

Read the relevant path, not the entire catalogue. For current work use the
[roadmap](implementation-roadmap.md), [saved golden-delivery plans](work-plans/golden-delivery-planning.md), [whole-roadmap assessment](work-plans/roadmap-assessment.md)
and [supported checks](../tests/validation/README.md). For commands use the
[cheat sheet](workflow-cheat-sheet.md); for current totals use the [daily report](progress/latest.md).

| Upcoming work | Contract / decision | Current evidence and next action |
|---|---|---|
| RM-303/304 service adapter and fakes | [Frozen SERVICE-001](service-plan.md) and [D3 scope](decision-sprint.md#d3--service-001-and-the-service-side-of-async-001) | Reviewed local HTTP adapter and checked authored fakes are complete; see [history](implementation-history.md), with durable-job integration still open |
| RM-305–307 durable jobs | [Frozen ASYNC-001](async-plan.md) and [D4 scope](decision-sprint.md#d4--jobs-and-events-in-async-001) | RM-305 is complete; reviewed private outbox/claim foundation exists on both adapters. [Runtime scope and remaining bindings](runtime-target-v0.1.md#private-durable-delivery-storage-foundation-rm-306); 18 catalog traces remain contract-only |
| RM-402 transaction retries | [TX-001](transaction-retry-plan.md) | Deadline/active-call evidence and independent transaction review remain; see the [active queue](implementation-roadmap.md#delivery-order--golden-todo-first) |
| RM-603 local attestation | [Approval protocol](approval-protocol.md) | [AP cases](../tests/assurance/approval-protocol-v0.1.json); [local validation plan](work-plans/golden-delivery-planning.md#rm-601603--review-artifacts-and-local-attestation-validation); hosted authority is RM-604 |

For a previously discussed idea, use the roadmap's
[topic/legacy index and language-issue coverage](implementation-roadmap.md#discussion-and-legacy-work-index).
It includes Revset, Jev, deferred language questions and completed foundations.
The [conversation reconciliation](conversation-coverage.md#4-roadmap-reconciliation--2026-10-01)
records source coverage and limits; archived placement never means an unfinished
requirement was completed or cancelled.

## Recommended reading order

This is a reference catalogue, not a required reading sequence for every task.

1. [Research brief](research-brief.md) — the problem, first user, competing
   hypothesis, evidence state, and continuation rule.
2. [Language charter](charter.md) — the shortest statement of what is currently
   believed.
3. [Vision and design philosophy](vision.md) — the problem, thesis, division of
   responsibility, scope, and non-goals.
4. [Semantic model](semantic-model.md) — the concepts the compiler understands
   and the guarantees attached to them.
   - [Entity, query, and transaction model](entity-query-model.md) — the
     accepted identity/persistence boundary, entity operations, named reads,
     mutation ownership, project roles, multi-entity transactions, and
     authority/projection consistency across stores.
   - [Typed events and enforced subscribers](event-model.md) — RM-309 successor
     candidate: typed variants, mandatory transition facts, caller effect
     authority, durable fan-out and acceptance cases; not an implemented or
     frozen replacement of DATA/ASYNC contracts.
   - [AUTH-001 implementation plan](authentication-plan.md) — the decision
     contract, security invariants, staged compiler/runtime work, adversarial
     evidence, dependencies, and stop conditions for authentication.
   - [POLICY-001 implementation plan](policy-plan.md) — the approved scoped-role,
     membership, entity/field permission, automatic database scoping,
     validation, output, audit, and approval contract.
   - [CONFIG-001 implementation plan](configuration-plan.md) — the implemented
     in-source binding, local secret handoff, and automatic startup-validation
     core, plus the pending adapter readiness/deployment evidence.
5. [Type system](type-system.md) — nominal semantic and field types,
   compatibility, validated construction, and trust boundaries.
6. [Failure model](failure-model.md) — declared domain failures, operational
   faults, client disclosure, HTTP mapping, telemetry, and stack policy.
7. [Policy and proof kernel](policy-proof-v0.1.md) — pre-POLICY-001 candidate
   proof vocabulary retained for migration into the approved policy plan.
8. [Runtime validation rules](validation-rules-v0.1.md) — named generated,
   runtime, and operational boundaries that must not be mislabeled as proofs.
9. [Threat model](threat-model.md) — assets, boundaries, trusted components,
   enforcement evidence, bypasses, and residual risk.
10. [Approval protocol](approval-protocol.md) — candidate separation-of-authority
   and behavioural-review contract for policy weakening.
11. [Syntax draft](syntax.md) — the proposed human- and LLM-readable source
    notation.
    - [Naming and qualification](naming-and-qualification.md) — the accepted
      language-wide casing, ownership, import, callable, standard-library
      namespace, and canonical-spelling contract.
12. [Assurance model](assurance-model.md) — policy ownership, security defaults,
    compiler proofs, audits, tests, and CI gates.
13. [Golden todo design](../examples/golden-todo/README.md) — candidate canonical
    source, policy, black-box cases, adversarial sequence, and expected audit.
14. [Comparison protocol](comparison-protocol.md) — candidate preregistered
    trial, adjudication, measurement, and continuation thresholds.
15. [First-user review guide](first-user-review-guide.md) — screening, neutral
    session script, artifact tasks, bias controls, and synthesis rule.
16. [Comprehension study](comprehension-study.md) — counterbalanced review
    surfaces, administration, scoring, and frozen question rules.
17. [Compiler and runtime architecture](compiler-runtime.md) — toolchain,
    generated artifacts, target strategy, diagnostics, source mapping, and
    operations.
18. [Implementation roadmap](implementation-roadmap.md) — remaining epics and
    tasks, effort estimates, dependency IDs, completion conditions and external
    gates. [Implementation history](implementation-history.md) holds completed
    capabilities, evidence and the preserved earlier roadmap.
    [Task timing](task-timing/README.md) records the estimate calibration and
    start/finish/pause workflow for future work.
    [Adaptive roadmap workflow](roadmap-workflow.md) supplies local priorities,
    evidence gates and record locations for the personal skills.
    [Skill command cheat sheet](workflow-cheat-sheet.md) provides copyable prompts
    for ideas, planning, implementation, reviews and Goal controls.
    [Daily progress](progress/latest.md) summarises delivered work, remaining
    tasks, planning status and epic estimates; dated reports preserve changes.
19. [Project structure workstream](project-structure.md) — the case for a
    canonical layout, enforcement ladder, flexibility requirements, and
    pressure-test plan.
20. [Developer tooling workstream](developer-tooling.md) — compiler-backed LSP,
    editor highlighting, documentation, renderer adapters, and agent context.
21. [Generated artifact contract](generated-artifacts.md) — the stable P8 output
    tree, schema intent, discovery boundary, and reproducibility rules.
22. [Generated Bun target](runtime-target-v0.1.md) — the executable P9 slice,
    validation and containment boundaries, and acceptance evidence.
23. [Persistence slice](persistence-v0.1.md) — the first P10 entity metadata,
    parameterised typed CRUD operations, SQLite/PostgreSQL runtime,
    and explicit limits.
24. [Migration identity v0.1](migration-identity-v0.1.md) — the provisional
    checked-in identity registry, rename, lifecycle, and schema-diff contract.
25. [Pre-implementation decision sprint](decision-sprint.md) — the ordered
    owner-decision packages, external gates, deliberate non-decisions, and
    unattended build queue for currently parked work.
    - [TIME-001 and TEST-001 decision plan](time-testing-plan.md) — the approved
      instant/civil-time library, timezone and human-formatting rules, monotonic
      deadline, deterministic fixture, capability-fake, and test-evidence
      implementation contract.
26. [Agent development workflow](agent-workflow.md) — how humans, agents, and the
    compiler collaborate.
27. [Validation plan](validation-plan.md) — the deliberately hostile experiment
    intended to prove or kill the idea.
28. [Product and adoption considerations](product-strategy.md) — ecosystem,
    commercial, timing, and adoption risks.
29. [Decision register](decision-register.md) — accepted, provisional, rejected,
    and open design choices.
30. [Conversation coverage](conversation-coverage.md) — a traceability check
    showing where every substantive theme from the source discussion lives.

The [candidate golden todo](../examples/golden-todo/README.md) is the current
P10R design contract. The [historical todo sketch](../examples/chat-todo-sketch.md) preserves the
first concrete application, generated tests, policy, audit, and background-job
examples. The [other historical design sketches](../examples/chat-design-sketches.md)
preserve the initial booking, service, lifecycle, policy, and compiler-diagnostic
examples. They are evidence and design input, not current canonical syntax.

The [type-system acceptance cases](../examples/type-system-cases.md) are newer
normative examples: they define programs that must compile or fail once a
parser and type checker exist.

The [failure-model acceptance cases](../examples/failure-model-cases.md) do the
same for rejection, propagation, HTTP mapping, disclosure, redaction, telemetry,
provider/database normalisation, and runtime containment.

## Authority

The documents use five statuses:

- **Accepted** — the current working decision. New work should follow it unless
  an explicit decision changes it.
- **Provisional** — the preferred current direction, awaiting pressure-testing
  in complete examples.
- **Candidate freeze** — a complete P10R review package that is stable enough
  for contradiction-finding and independent review but is not yet accepted or
  implementation-authorising.
- **Open** — deliberately undecided.
- **Historical** — useful evidence from the conversation, not current syntax or
  semantics.

If documents appear to disagree:

1. the [decision register](decision-register.md) determines status;
2. the semantic model determines meaning;
3. the type-system specification determines compatibility and construction;
4. the failure-model specification determines failure classification,
   disclosure, and boundary mapping;
5. the syntax document determines current spelling;
6. the implementation roadmap tracks progress but cannot override semantic
   authority;
7. historical examples do not override current decisions.

Changes should update all affected documents rather than allowing design intent,
syntax, compiler behaviour, and examples to drift apart.

## Document ownership and maintenance

| Information | Owning location |
|---|---|
| Semantic decision/status and essential rationale | Decision register, linking the relevant specification |
| Behaviour, source spelling and acceptance contract | Relevant specification and canonical fixtures; label candidates explicitly |
| Open work, estimates and prerequisites | Implementation roadmap; no chronological progress log |
| Next steps, assumptions and pending questions | Existing task/batch plan under work-plans; update its checkpoint in place |
| Completed task and concise acceptance evidence | History entry above the preserved snapshot, linking raw results |
| Raw runs, experiments and timings | Existing validation/experiment/task-timing locations |
| Daily counts and summary | Generated progress snapshots; never a second manually maintained tracker |

- Update the owning source first; link to it elsewhere rather than copying its rules.
  A focused specification can own a contract even if its filename ends in `-plan`.
- Create a document only for a distinct durable contract, evidence set or handoff
  that does not fit an existing owner. Keep transient exploration and repeated
  status narration in chat; retain rejected choices only when they explain a decision.
- Save the minimum future context: outcome, constraints, authoritative sources,
  decision rationale, checks and unresolved work. Do not paste whole transcripts,
  command output or duplicated skill procedures into task plans.
- On completion, retain the plan as linked evidence while other tasks depend on it;
  move it to an archive only after updating inbound links. Preserve old anchors or
  a redirect when moving content. Frozen contracts and historical/raw evidence
  are not deleted or rewritten merely to reduce word count.
- Check new links/anchors and the relevant task reading path when changing structure.
  Refresh stale summaries by linking current evidence, not copying another snapshot.
  Retention of large generated/raw data needs an explicit policy before deletion.

## Current phase

The next product milestone is the complete golden todo on Bun. Use the
[active roadmap](implementation-roadmap.md) for its current gates and the
[validation ledger](../tests/validation/unattended-progress.md) for recorded checks.
[History](implementation-history.md) preserves completed capability evidence and
legacy requirements; it is not another active backlog. Alternative-runtime and
conditional follow-ups do not change the default target or current milestone.

P10R independent review, first-user/comprehension trials and protected approval
remain separate assurance gates. Technical implementation may proceed as
exploratory work; it does not validate the product thesis. Preserve frozen
candidate behaviour and human-owned policy while implementing accepted semantics.
The [core grammar](grammar-v0.1.md), [semantic specifications](semantic-model.md)
and [language issues](language-issues.md) remain the implementation references.
