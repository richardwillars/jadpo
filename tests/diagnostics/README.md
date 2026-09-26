# Diagnostic conformance suite

The DX2 presentation contract is tested as one semantic diagnostic projected
to agents, terminals, and IDEs. The suite deliberately does not approve a
snapshot merely because it is stable.

Run the active conformance suite with:

```text
cd jadpo
cargo test --workspace
cargo build -p jadpo-cli
cd ..
ruby tests/compile/verify.rb
cd editors/vscode
npm test
```

Coverage is split by responsibility:

- `jadpo-diagnostics` enumerates every compiler-owned public code and validates
  the complete version-2 agent schema, bounds, repair objects, impact, aliases,
  JSON validity, and secret/ANSI exclusion;
- `jadpo-cli::terminal` renders every catalogue entry through plain and rich
  modes and proves that neither projection loses the shared explanation,
  choices, impact, rule, or help identity;
- `jadpo-cli::lsp` renders every entry through the IDE hover projection and
  exercises Problems, UTF-16 ranges, Quick Fix ordering, revision checks, and
  edit previews through scenario tests;
- `editors/vscode/test` verifies that Problems uses only the human summary and
  that the details view preserves and safely escapes the complete shared repair
  protocol; and
- compile fixtures prove exact root-diagnostic sequences and byte ranges. A
  fixture may additionally assert the exact summary, reason fragment, repair
  classification, owner, context, alternatives, replacements, and impact.

Catalogue discovery scans diagnostic identifier tokens rather than naively
splitting Rust source on quotation marks. Escaped JSON literals therefore
cannot hide later compiler diagnostics from the conformance suite; the complete
enumerated catalogue currently contains 310 codes.

`ROUTE_AUTH_VALUE_INVALID` is the first complete golden scenario. Its fixture
proves one root error over `nonke`, continued parsing of subsequent route items,
two non-preferred human-owned security choices, and parity across JSON,
terminal, Problems, hover, details, and Quick Fix.

The catalogue currently contains explicitly classified copy debt inherited
from the original identifier-derived fallback. The normal suite rejects any
entry falsely labelled as authored and prevents format regressions for every
entry. The stricter completion gate is intentionally visible as the ignored
Rust test
`strict_public_catalogue_has_no_placeholders_and_every_code_has_a_real_fixture`.
Run it explicitly with:

```text
cd jadpo
cargo test -p jadpo-diagnostics strict_public_catalogue_has_no_placeholders_and_every_code_has_a_real_fixture -- --ignored
```

It must be made non-ignored, and must pass, before DX2 can be called complete.
This red gate is the work queue for replacing every remaining generic message
with rule-specific copy and a real triggering fixture.

The `SYN_*` catalogue family is fully authored. Its public copy now describes
the concrete grammar rule and a usable next step; the general parser expectation
also carries the bounded `expected` and `found` facts supplied by the parser.
Real trigger-fixture coverage is still tracked independently by the strict gate.

The `SEM_*` name-resolution family is also fully authored and contextual.
Diagnostics name the source spelling, say what that field, parameter, return,
route item, call, or `fails` entry needs, suggest the nearest compatible name,
and use concrete advice such as “define `CustomerID` as a type.” A failure
`kind` mistake describes the accepted predefined categories instead of asking
the reader to understand compiler declaration classes.

Human terminal and IDE projections turn the stable `helpId` into a full
`https://jadpo.dev/docs/diagnostics/...` guidance link. Agent JSON retains the
stable relative identifier required by the versioned schema.

The `EFFECT_*` and `FAIL_*` families are fully authored and mostly backed by
existing compiler fixtures. Their copy explains the function/action purity
boundary, exact failure-set propagation, failure-kind mappings, and flat
public/internal context rules.

The `ROUTE_*` family is fully authored. It covers authentication ownership,
exactly one behavior form, consistent item separators, duplicate items, and the
one-to-one typed relationship between URL placeholders and `path:` fields.

The `DATA_*` family is fully authored. It explains compound storage
constraints, canonical identity, owning/inverse relationship shape, nominal
reference compatibility, delete behavior, and cycles without leaking adapter
implementation details.

The core `TYPE_*` subfamily is fully authored. It covers calls, constructors,
assignment, operators, record and enum construction, field selection, primitive
signature boundaries, and nominal compatibility. Match and persistence/query
type rules remain separately tracked so partial progress cannot masquerade as a
complete type-checker catalogue.

Core type and failure emitters now supply typed facts rather than only selecting
a rule. Type mismatches name the received and required types; constructors name
their target, input type, and argument counts; record errors name the record and
field; unknown names offer a compatible nearest-name suggestion; constrained
literals name the failed constraint; and primitive signatures say whether the
problem is a parameter or return value. Failure diagnostics name the function
or action, escaping failure, context field and disclosure, predefined category,
or duplicate public code involved. Compile expectations assert this exact
context and wording. The fixture verifier rejects unknown expectation keys so a
misspelled or obsolete assertion can no longer pass silently.

The `TYPE_MATCH_*` subfamily is fully authored. Its copy distinguishes
exhaustiveness, unreachable and duplicate arms, subject-pattern compatibility,
nullable narrowing, closed-enum variants, and payload bindings. Persistence and
query type rules remain separately tracked.

The complete `TYPE_*` family is now fully authored. Persistence copy preserves
the distinct contracts for entity operations, deterministic query bounds,
constraint mappings, omission-aware patches, and each supported include plan:
to-many, owning-parent, inverse-one, and bounded two-hop. Real trigger-fixture
coverage remains independently enforced by the strict completion gate.

The `MOD_*`, `FMT_*`, and `LSP_*` families are fully authored. Module copy
explains explicit visibility and acyclic imports; formatter copy distinguishes
check drift from write failure; and language-server transport failures provide
editor-specific recovery without exposing message bodies.

The `INDEX_*` and `JADPO_*` families are fully authored. Index advice now
distinguishes stale evidence, checked-edit failures, registry identity, and
rollback recovery. Project, scaffold, artifact, and target-generation failures
describe the preserved state and a safe next action; protected-route generation
remains an explicit human-owned authentication-boundary decision.

The `CLI_*` family is fully authored. Command-shape errors now give the exact
usable form, while watch/dev and incident-enrichment failures explain process,
revision, manifest, and output-stream recovery without copying event payloads
or secrets into diagnostics.

Migration identity, decision, and plan diagnostics are fully authored. They
preserve immutable artifact and stable-identity guarantees, distinguish stale
change sets from malformed files, and keep unresolved, missing-evidence,
rejected-change, and strategy selection outcomes visibly human-owned. SQL
review-generation diagnostics remain separately tracked.

The complete `MIG_*` family is now fully authored. SQL-review copy distinguishes
unsupported change, strategy, expression, predicate, literal, and type cases
from identity corruption, and gives SQLite rebuild failures their precise
entity, field, constraint, index, reference, rename, or shape recovery path.

All 310 enumerated public diagnostic codes now have rule-specific summary,
reason, and recommended-next-step copy. The active zero-placeholder test is
permanent. Completion still depends on replacing the strict gate's synthetic
render coverage with a real trigger fixture for every code.

The `ROUTE_*` family now has real compile-trigger coverage for every code in
addition to its all-audience projection coverage.

The `FAIL_*` family now also has real compile-trigger coverage for every code.

The `SEM_*` family now also has real compile-trigger coverage for every code.
