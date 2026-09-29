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
  classification, owner, context, alternatives, replacements, and impact; and
- focused Rust tests provide bounded scenario references. Their assertions and
  actual execution must still be reviewed; merely naming a code is not proof
  that the compiler emitted it. Operational failures use disposable filesystem
  and process scenarios where possible, alongside audience projection tests.

Catalogue discovery parses Rust and collects diagnostic string literals from
production modules, including `core/src/target/`. It excludes test-only functions,
test modules, integration tests, comments and the catalogue's own prose. The
CONFIG, POLICY and TEST families are included. The current inventory has 480
codes; this is an inventory of recognized literal codes, not arbitrary dynamic
code-generation analysis.

Evidence discovery parses compile expectation JSON and bounded Rust `#[test]`
function bodies. It excludes ignored tests, comments, nested helper declarations
and text after the function. The diagnostics crate's own inventory/rendering
tests cannot certify their own emitter evidence. The index records references,
not assertion execution, branch coverage or proof that a diagnostic is reachable.
The full verifier executes suites; focused triggering tests and mutation reviews
provide the stronger evidence for their specifically listed cases.

`ROUTE_AUTH_VALUE_INVALID` is the first complete golden scenario. Its fixture
proves one root error over `nonke`, continued parsing of subsequent route items,
two non-preferred human-owned security choices, and parity across JSON,
terminal, Problems, hover, details, and Quick Fix.

The permanent catalogue gate rejects placeholder copy and any inventoried code
without a compile-fixture or bounded Rust-test reference. Run it directly with:

```text
cd jadpo
cargo test -p jadpo-diagnostics every_public_diagnostic_is_authored_and_has_conformance_evidence
```

The gate is active. A passing reference index must not be reported as exhaustive
trigger coverage. New CONFIG/POLICY/TEST cases execute real compiler/API/CLI
paths; their findings record initial omissions and proof limits.

The `SYN_*` catalogue family is fully authored. Its public copy now describes
the concrete grammar rule and a usable next step; the general parser expectation
also carries the bounded `expected` and `found` facts supplied by the parser.
Trigger-scenario coverage requires executing and reviewing the referenced tests.

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
to-many, owning-parent, inverse-one, and bounded two-hop. Trigger-fixture
evidence remains separate from the static reference index.

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

The current 480-code inventory has rule-specific summary, reason and next-step
copy. The zero-placeholder and evidence-reference tests are permanent. Adding a
new family must also add discovery coverage; an omitted family cannot be made
complete by a green catalogue loop.

The `ROUTE_*` family now has real compile-trigger coverage for every code in
addition to its all-audience projection coverage.

The `FAIL_*` family now also has real compile-trigger coverage for every code.

The `SEM_*` family now also has real compile-trigger coverage for every code.
