# Jadpo documentation

**Jadpo — one language for the whole web.**

This directory is the working source of truth for Jadpo. The canonical public
documentation address is [jadpo.dev/docs](https://jadpo.dev/docs/). This tree
extracts the substance of the original design conversation so future work does
not need that conversation for context.

## Recommended reading order

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
18. [Implementation roadmap](implementation-roadmap.md) — phase status, exit
    gates, evidence, scope guard, and immediate next work.
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

## Current phase

The project remains a hypothesis test. Milestone B and the P10 persistence core
are complete, including transactional typed CRUD across SQLite and all 22 live
Postgres 16 acceptance cases. P10.5, DX0.5, DX1, and the bounded
P10.6/P10.7 entity/query/transaction work are implemented as exploratory work
under the explicit P10R review deferral. The call, outcome, IDE,
persistence-free runtime, local savepoint, and authority-change-record slices
are implemented. The [CONFIG-001 plan](configuration-plan.md) now has an
implemented compiler/runtime core and first-party authentication secret sinks;
broader dependency readiness remains coupled to later service/platform work.
P11 has implemented AUTH-P0–P3, the Temporal/testing core, and the policy
enforcement core. The [first-party authentication checkpoint](../examples/first-party-authentication/README.md)
adds signed/opaque browser and API credentials, real protected routes, authority
checks, CSRF, and SQLite/PostgreSQL/HTTP evidence including restart and revocation.
The scoped browser/API milestone is complete; service/JWT authentication, richer
principal mappings, full golden integration, broader PostgreSQL coverage and
external approval evidence remain open. Comprehensive validation follows this
milestone, with Wasm probes after that phase.
Physical cross-store delivery and WORKFLOW-001 runtime execution also remain
decision-bound. The existing
`type` plus `persist` path remains supported compatibility evidence beside the
first-class entity dossier.
P10R independent review and five
first-user sessions remain required before Milestone C or any release-equivalent
assurance claim; by owner direction, those sessions follow feature-complete
implementation.
Current status and the next concrete action live in the [implementation
roadmap](implementation-roadmap.md).

The [core grammar](grammar-v0.1.md),
[Jadpo seed application](../examples/jadpo-seed/app.jadpo), and
[language issue log](language-issues.md) constrain implementation. The candidate
complete todo and assurance package still require independent review and freeze;
implementation progress does not satisfy or waive that evidence. The
order/payment application and executable TypeScript baseline remain the later
falsification test.

Naming, a production compiler, native generated output, package management, and
broad deployment tooling remain premature. IDE and LLM presentation support is
now an explicit staged workstream rather than an untracked afterthought.
