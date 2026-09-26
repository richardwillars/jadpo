# Jadpo implementation roadmap

**Status:** active progress tracker
**Last updated:** 2026-09-26
**Current phase:** DX0.5/P10.6 implementation and DX2 external exit evidence, before P11 under an explicit P10R review deferral

This document is the implementation control plane. It records what must be
built, what evidence completes each phase, what is deliberately deferred, and
what should happen next.

Progress is evidence-based. A phase is not complete because code exists or
because it feels nearly finished; every exit gate must be satisfied and linked.

### 2026-09-25 roadmap correction

The original charter required complete golden applications to force the
important semantic decisions before substantial compiler construction. The
project instead built the seed compiler and began persistence before completing
the canonical todo application. That work remains useful exploratory evidence,
but it is not evidence that the product thesis is true and must not be allowed
to determine the comparison protocol after the fact.

P10R is therefore a mandatory interruption, not an additional implementation
feature. Until its exit gate is met:

- completed P10 capabilities may receive defect fixes, reproducibility work,
  and stabilisation evidence; new work must remain assigned to an explicit
  later exploratory phase rather than silently expanding completed P10;
- technical implementation may continue through P10.5, P11, and P12 only as
  explicitly labelled exploratory work; it cannot satisfy a release-equivalent
  assurance gate or product-validation claim;
- the complete todo design, policy/proof model, threat model, human-approval
  protocol, first-user hypothesis, and falsification thresholds must remain
  versioned so later external evidence cannot be folded silently into the
  original candidate; and
- completed compiler work is classified as exploratory scaffolding rather than
  validation of the language or product.

### 2026-09-25 project-owner deferral

The project owner explicitly chose to defer the outside contract review and five
first-user sessions and continue implementation. On 2026-09-25 the owner
clarified that the five sessions should occur only after the planned technical
implementation is complete because implementation has so far been inexpensive.
Technical work may therefore continue through P10.5, P11, and the P12 reference
implementations. This does not satisfy, waive, or retroactively pass the P10R
exit gate. All such work remains exploratory evidence: P11 release-equivalent
assurance claims, Milestone C completion, comparative validation, and product-
validation conclusions remain blocked until the deferred evidence is completed.

The candidate contracts, thresholds, fixtures, and protocols must retain dated
digests before P11/P12 implementation proceeds. If later external review
changes them, the original and revised versions and the implementation exposure
to each must be recorded. The five sessions occur after feature-complete
implementation and before final P12 comparative trials or a
continue/redesign/pivot/stop decision.

The correction preserves the intended technical scope. Its purpose is to make
the existing scope testable without allowing implementation momentum to decide
the result.

## 1. Milestones

### Milestone A — semantic compiler

The first meaningful compiler can run:

```text
jadpo check <project>
jadpo inspect <project>
```

It parses the core language, resolves declarations, builds a semantic graph,
checks nominal typing and failure propagation, and emits human-readable plus
machine-readable diagnostics. `inspect` emits a semantic manifest.

Milestone A comprises phases P0–P7.

### Milestone B — runnable compiler

The first end-to-end compiler can run:

```text
jadpo build <project>
```

It generates a TypeScript/Bun application and runtime metadata for the
Jadpo seed application. The result serves one real route with boundary
validation and safe failure handling.

Milestone B comprises phases P8–P9.

### Milestone C — basic backend prototype

The compiler supports the canonical todo application with Postgres, SQLite (local), SQLite (D1),
authentication required by default, ownership policy, CRUD, safe failures, and
generated artifacts.

Milestone C comprises P10, the corrective P10R gate, P10.5, the DX0.5 local
development loop, and P11. P10R must complete before Milestone C can be
claimed. By explicit owner direction, P10.5, DX0.5, and P11 implementation may
proceed before the deferred external evidence arrives, but it remains
exploratory and cannot produce release-equivalent assurance. DX0.5 must pass
before P11 application-authoring work begins so the golden todo is developed
through the same compiler feedback loop intended for users and agents.

### Milestone D — falsification

The difficult order/payment application and a strong TypeScript baseline test
whether the approach deserves further investment.

The candidate comparison protocol and minimum editor, language-server,
formatter, renderer, and agent-context baseline described by the developer
tooling workstream receive dated digests before implementation continues. P12
reference implementation may proceed under the review deferral; final trials
execute the externally reviewed frozen protocol without redefining it in
response to results.

Milestone D is phase P12.

## 2. Status vocabulary

| Status        | Meaning                                                      |
| ------------- | ------------------------------------------------------------ |
| `not started` | No implementation evidence exists beyond planning.           |
| `in progress` | At least one deliverable exists, but the exit gate is unmet. |
| `deferred`    | An explicit owner decision postponed required evidence; the exit gate remains unmet. |
| `blocked`     | Work cannot continue without a named decision or dependency. |
| `complete`    | Every exit-gate item is satisfied and linked below.          |

Percentages are intentionally avoided. They imply precision without proving
which semantic risks remain.

## 3. Progress summary

| Phase | Deliverable                               | Status      | Evidence or next action                                                                                                                                                                 |
| ----- | ----------------------------------------- | ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0    | Design foundation                         | complete    | Charter, semantic model, type system, failure model, acceptance cases                                                                                                                   |
| P1    | Jadpo seed and core grammar            | complete    | [Core grammar](grammar-v0.1.md), [seed application](../examples/jadpo-seed/app.jadpo), [expected semantics](../examples/jadpo-seed/expected.md), [issue log](language-issues.md) |
| P2    | Executable compiler fixtures              | complete    | [Eighty-five source/expectation pairs](../tests/compile/README.md) cover syntax, names, types, failures, disclosure, effects, typed CRUD, patches, relationships, modules, local mutation, enums, matching, operators, authored tests, and the executable P10.6 surface |
| P3    | Rust workspace and CLI                    | complete    | [Rust workspace](../jadpo/README.md), deterministic scaffold manifest, discovery tests, stable build-stage diagnostics                                                               |
| P4    | Lexer, parser, and syntax tree            | complete    | Span-preserving lexer and AST, recovering parser, exact seed outline, fixture coverage, real `jadpo check` syntax pass                                                               |
| P5    | Declaration index and semantic graph      | complete    | Deterministic IDs, resolved declaration references, field/refinement nodes, callable edges, semantic JSON manifest                                                                       |
| P6    | Nominal type and constraint checker       | complete    | Expression typing, nominal compatibility, validated constructors, nested structured-field selection, nullable/optional records, invariant collections, stable fixture diagnostics      |
| P7    | Failure and effect checker                | complete    | Closed failure propagation, typed rejection context, derived route status, disclosure contracts, effect checks; Milestone A                                                            |
| P8    | Derived artifacts and layout boundaries   | complete    | Nine byte-stable artifact files, explicit discovery/output boundary, generated diagnostic catalogue/reference, three layout candidates, and deterministic static scaffold                                                       |
| P9    | TypeScript/Bun target and runtime         | complete    | Dependency-free generated Bun target and six real HTTP acceptance cases; Milestone B                                                                                                   |
| P10   | Postgres and persistence constructs       | complete    | Typed CRUD, inferred transactions, foreign keys, bounded repeated relationship loads, atomic multi-field updates, named compound uniqueness, and precise unique-conflict mappings pass SQLite and live PostgreSQL exit suites            |
| P10R  | Assurance and validation reset            | deferred    | Candidate packages exist; outside review and five first-user sessions are deferred until feature-complete implementation, not passed                                                    |
| P10.5 | Pre-P11 language completion               | complete    | Exploratory under P10R deferral; patches, bounded relationships, migration review, index acceptance, bounded modules/imports, and immutable-value/scoped-local-rebinding semantics are implemented; advanced extensions remain deferred |
| DX0.5 | Checked local development loop             | in progress | JSON plus source-rendered diagnostics, coalesced atomic watch, compiler-owned HTTP health, structured runtime faults, and initial Bun restart with invalid-edit continuity implemented; startup rollback and structured shutdown remain |
| P10.6 | Problems, routes, and entity boundary      | in progress | Explicit failure kinds and flat context, exact callable failures with `attempt`, typed path bindings, `auth: none`, and local/named route behaviour are executable; operational boundary mapping, handler arms, optional-persistence grammar, and policy authority remain recorded decisions rather than invented syntax |
| DX1   | Compiler-backed language service            | complete    | Standard `jadpo lsp`, live unsaved diagnostics, symbols, cross-file definitions/references, hover, contextual completion, signature help, semantic tokens, rename, formatting, generated-artifact links, and a persistent VS Code client pass protocol tests |
| DX2   | Guided diagnostics and agent context        | implementation correction in progress | The cross-audience conformance suite and invalid-auth golden scenario are executable. Existing identifier-derived placeholder copy is now classified debt and keeps the strict completion gate red; rule-specific copy and real fixtures remain required before external trials. |
| P11   | Authentication, policy, and golden todo   | not started | May proceed as exploratory implementation after P10.6 and DX0.5; Milestone C and assurance claims still require the deferred P10R evidence                                               |
| P12   | Order/payment application and TS baseline | not started | Reference implementations may proceed; external sessions and protocol freeze precede final comparative trials and the continuation decision                                             |

The bounded module core now parses explicit logical module headers and selective
imports, enforces private-by-default visibility, and rejects dependency cycles.
Advanced namespaces stay deferred until a golden-application split demonstrates
their need.

## 4. Phase details

### P0 — Design foundation

**Objective:** establish enough semantic authority to avoid inventing the
language inside compiler code.

**Deliverables:**

- [language charter](charter.md);
- [semantic model](semantic-model.md);
- [type system](type-system.md);
- [failure model](failure-model.md);
- [type-system acceptance cases](../examples/type-system-cases.md);
- [failure-model acceptance cases](../examples/failure-model-cases.md);
- [decision register](decision-register.md).

**Exit gate:** type compatibility, validated construction, failure
classification, disclosure, and authority order are documented without relying
on the source conversation.

**Status:** complete.

### P1 — Jadpo seed and core grammar

**Objective:** turn the broad language proposal into one small compiler contract
that can be implemented without guessing.

**Deliverables:**

- a canonical [Jadpo seed application](../examples/jadpo-seed/app.jadpo);
- its [expected semantic facts](../examples/jadpo-seed/expected.md);
- a frozen-for-implementation [core grammar](grammar-v0.1.md);
- a living [language issue log](language-issues.md);
- a list of constructs explicitly excluded from the core.

**Exit gate:** every token in the seed application is covered by the grammar;
every seed declaration has an expected semantic identity; unresolved broader
features are logged rather than represented with pseudocode.

**Status:** complete.

### P2 — Executable compiler fixtures

**Objective:** convert prose acceptance examples into files that can drive
implementation test-first.

**Initial fixture set:**

1. valid `Email` literal construction;
2. invalid constant email;
3. wrong identifier domain;
4. field-to-parent widening;
5. sibling field rejection;
6. raw primitive callable parameter rejection;
7. undeclared failure propagation;
8. automatic `NotFound` HTTP mapping;
9. internal failure context excluded from public output;
10. arbitrary exception rejection.

**Fixture contract:** each case contains source plus an expectation describing:

- compile success or diagnostic code;
- primary source span;
- expected/received semantic types where relevant;
- selected refinement or propagation path;
- generated boundary metadata where relevant.

**Exit gate:** the initial ten fixtures are machine-readable and can be consumed
by the future test harness without manually interpreting Markdown.

**Status:** complete.

### P3 — Rust workspace and CLI

**Objective:** create the smallest durable compiler shell.

**Planned layout:**

```text
jadpo/
    crates/
        syntax/
        semantic/
        diagnostics/
        core/
        cli/
runtime/
    typescript/
stdlib/
tests/
```

**Commands:**

```text
jadpo check <project>
jadpo inspect <project>
jadpo build <project>   // placeholder until P9
```

**Exit gate:** the workspace builds; the CLI discovers source files; fixture
tests can invoke it; unsupported commands fail with stable diagnostics.

**Evidence:**

- dependency-free [Rust workspace](../jadpo/Cargo.toml);
- compiler CLI and source discovery;
- ten fixtures and the Jadpo seed discovered by workspace tests;
- deterministic `inspect` scaffold manifest;
- stable unsupported-command and deferred-build diagnostics;
- all workspace unit and documentation tests passing.

**Status:** complete.

### P4 — Lexer, parser, and syntax tree

**Objective:** parse the core grammar while preserving useful source spans.

**Required work:**

- lexer and trivia handling;
- error-recovering parser;
- syntax tree and typed AST accessors;
- multiple diagnostics in one run;
- parser snapshots and malformed-source fixtures;
- deterministic formatting of diagnostics.

**Exit gate:** the seed parses without diagnostics, every initial fixture
reaches the intended syntax node, and malformed source recovers far enough to
report more than the first error.

**Evidence:**

- dependency-free lexer with tokens for the complete core vocabulary;
- retained whitespace and line-comment trivia;
- UTF-8-safe byte spans that slice original source;
- route-path, string, integer, decimal, keyword, and punctuation tokens;
- recoverable invalid-escape, unterminated-string, and unexpected-character
  diagnostics;
- Jadpo seed and all ten initial fixtures lex without diagnostics;
- recovering recursive-descent parser and span-preserving typed AST;
- exact declaration-outline snapshot for the Jadpo seed;
- all ten initial fixtures reach their intended syntax, including the stable
  `SYN_UNSUPPORTED_THROW` node and diagnostic;
- malformed source reports multiple syntax errors while preserving both later
  declarations;
- `jadpo check` now runs discovery, lexing, and parsing and reports aggregate
  syntax failure without discarding specific diagnostics;
- fourteen Rust unit tests plus all documentation tests passing.

**Status:** complete.

### P5 — Declaration index and semantic graph

**Objective:** establish semantic identity before checking behaviour.

**Required nodes and edges:**

- named types and record declarations;
- field types and `refines` edges;
- functions, actions, failures, and routes;
- callable dependencies;
- failure-kind relationships;
- source spans and deterministic semantic IDs.

The initial project uses one application-wide namespace. Modules and imports
remain deferred.

**Exit gate:** `jadpo inspect` emits a deterministic manifest for the seed,
including every expected node and refinement edge in its expectation document.

**Evidence:**

- application-wide declaration index plus explicit prelude nodes;
- deterministic semantic IDs derived from sorted semantic names;
- addressable nodes for ordinary fields and scoped public/internal failure fields;
- resolution of type, field-type, callable signature, failure-kind, `fails`, and
  route references;
- stable diagnostics for duplicates, unknown names, wrong name kinds, unknown
  callees, and non-callable invocations;
- refinement edges for named types and every nominal field type;
- callable dependency edge from the seed route to its action;
- `jadpo inspect` emits a deterministic semantic JSON manifest containing
  nodes, source spans, refinements, and calls;
- the seed contract test proves all fourteen expected authored nodes and all
  eight expected refinement edges;
- every syntax-valid initial fixture indexes without a name-resolution error;
- eighteen Rust unit tests plus all documentation tests passing.

**Status:** complete.

### P6 — Nominal type and constraint checker

**Objective:** implement the distinctive type rules before broad language
features.

**Required checks:**

- nominal named types;
- field refinements;
- exact compatibility and widening;
- no implicit narrowing or sibling substitution;
- no primitive substitution;
- complete record construction;
- `T?`, `none`, and input omission;
- validated `Type(value)` construction;
- compile-time literal constraints;
- primitive prohibition in application callable signatures;
- initial invariant collection behaviour when collections enter the core.

**Exit gate:** the initial type fixtures pass with stable diagnostic codes and
spans; the compiler never delegates semantic compatibility to TypeScript.

**Evidence:**

- local expression environments and nominal field selection;
- exact compatibility plus widening only along declared refinement paths;
- nested selection through a structured field resolves via its declared named
  record and preserves that record's nested field identities;
- rejection of implicit narrowing, sibling substitution, and semantic-to-
  primitive unwrapping;
- primitive application-signature diagnostics covering the whole annotation;
- validated `Type(value)` construction with compile-time `format`, length,
  numeric, and core-pattern checks for literals;
- complete record construction, unknown-field checks, optional omission,
  nullable field identity, and `none` handling;
- invariant generic arguments for the initial collection model;
- typed public/internal failure context values;
- stable codes, exact primary spans, and inferred-type assertions across the
  type-focused fixtures;
- the seed type-checks without diagnostics.

**Status:** complete.

### P7 — Failure and effect checker

**Objective:** make expected failure flow closed and mechanically visible.

**Required checks:**

- standard failure kinds;
- domain failure declarations and stable codes;
- `reject` payload validation;
- `fails` sets and transitive propagation;
- route-reachable failure derivation;
- public versus internal disclosure schemas;
- rejection of arbitrary `throw`, strings, and numeric status mapping;
- semantic propagation paths;
- initial effect distinction between functions and actions.

`attempt` remains deferred until direct rejection and propagation are solid.

**Exit gate:** core failure fixtures pass; the seed's route failure response is
derived rather than handwritten; Milestone A is complete.

**Evidence:**

- complete initial standard-kind catalogue and deterministic HTTP defaults;
- domain failure contracts with stable code, safe message, and separate public
  and internal field inventories;
- duplicate public-code rejection;
- required, unknown, duplicate, and typed rejection-context validation;
- direct and transitive `fails` propagation checks;
- route-reachable failures and status codes derived from the invoked action;
- functions cannot reject or call actions;
- arbitrary `throw` remains a syntax-level error;
- the checked manifest includes inferred expression types, failure contracts,
  disclosure fields, and route failure mappings;
- all failure/effect fixtures and the seed pass their expected contracts.

**Status:** complete. Milestone A is complete.

### P8 — Derived artifacts

**Objective:** prove the semantic model is useful before producing executable
target code.

**Outputs:**

- `app.meta.json` semantic manifest;
- route and callable inventory;
- failure/disclosure audit;
- OpenAPI subset;
- validator plan;
- compatibility report for public failure codes;
- project-structure proposal and scaffold specimen;
- deterministic static-base scaffolding contract shared by interactive and
  non-interactive use.

**Exit gate:** generated artifacts accurately describe the seed and are stable
across repeated builds. Authored-source discovery and compiler-owned output
locations are explicit enough that P9 cannot accidentally establish them.
The same compiler version can reproduce the selected base scaffold without an
LLM making directory or naming decisions.

**Evidence:** [`jadpo artifacts`](../jadpo/README.md) emits the seven-file
[generated artifact contract](generated-artifacts.md); unit tests prove repeated
emission and relative/absolute invocation are byte-identical. Generated source
is excluded from discovery. The [structure workstream](project-structure.md)
records three candidate trees and selects the elastic single-application
prototype. `jadpo new` creates a valid five-file base, produces identical
bytes for identical names, and refuses to overwrite non-empty destinations.

**Status:** complete.

### P9 — TypeScript/Bun target and minimal runtime

**Objective:** complete one request from wire input to safe response.

**Runtime slice:**

- HTTP routing;
- request/correlation IDs;
- input decoding and generated validation;
- action invocation;
- output validation and exact serialisation;
- automatic failure mapping;
- generic fault containment;
- semantic source metadata.

The generated TypeScript is a disposable artifact, not normal developer-facing
source.

**Exit gate:** the seed route accepts valid input, rejects invalid input safely,
maps its domain failure automatically, never discloses internal context, and
passes an end-to-end HTTP test. Milestone B is complete.

**Evidence:** `jadpo build` emits the [generated Bun target](runtime-target-v0.1.md)
beneath `build/target/`. The [runtime acceptance
suite](../tests/runtime/jadpo-seed.test.ts) uses a real localhost listener to
prove success, malformed JSON, malformed semantic values, constrained values,
closed input shapes, derived 422 mapping, and non-disclosure. Bun bundles the
target with auto-install disabled. The compiler rejects bare package imports,
dependency manifests, lockfiles, and `node_modules`; only Bun built-ins and
compiler-owned relative modules are permitted. Authenticated routes fail
generation until their runtime exists.

**Status:** complete. Milestone B is complete.

### P10 — Postgres and persistence constructs

**Objective:** add the smallest honest persistence model.

**Order:**

1. entity schema metadata;
2. generated tables for a fresh database;
3. `create`;
4. required/optional/many query cardinality;
5. validated row decoding;
6. simple `update` and `delete`;
7. constraint-error normalisation;
8. entity identity, uniqueness, and indexes;
9. relationship declarations and generated foreign keys;
10. explicit relationship loading through joins or bounded batched queries;
11. inferred action transactions across multiple operations;
12. migration planning only after fresh-schema behaviour works.

#### P10 relationship and joined-loading slice

The golden pressure case is “load a user and all of that user's todo tasks.”
This is a product capability, not merely a database optimisation. The language
must describe the relationship and requested result shape so the compiler can
derive storage, queries, validation, documentation, and policy checks together.

The joined-load pressure case selected an explicit logical-name clause:
`owner_id: User.id references User.id as owner on_delete cascade`. This keeps
the storage field, nominal target, traversal name, and lifecycle action visible
in one declaration. Inferring `owner` from `_id` was rejected as convention
magic and a separate relationship declaration was rejected for duplicating the
target and ownership facts. Omitting `as` keeps the field name as the logical
name for existing concise cases. Composite-reference syntax remains a later
migration/design pressure test rather than a hidden assumption here.

**Required relationship semantics:**

- explicit one-to-one, many-to-one, and one-to-many declarations with named
  ownership direction; many-to-many starts with an explicit join entity rather
  than an invisible table;
- nominal reference typing, so a `Todo.owner_id` relationship to `User.id`
  cannot be populated with an unrelated UUID-shaped field;
- generated foreign keys and indexes for both SQLite and PostgreSQL, with
  explicit `restrict`, `cascade`, or nullable-reference behaviour on deletion;
- required versus optional parent references and the corresponding orphan
  rules;
- relationship cycles and maximum eager-load depth diagnosed rather than
  recursively expanded without bound.

**Required loading semantics:**

- an explicit load/include selection at the query site; ordinary entity access
  never performs hidden lazy queries or creates an N+1 path;
- typed nested results such as a complete `User` plus `List<Todo>`, with zero
  children represented by an empty list rather than a missing parent;
- deterministic child ordering and explicit pagination before a to-many load
  can be accepted;
- parent pagination applied to parents, not flattened join rows;
- compiler choice between a safe join and a bounded batched query when joining
  multiple to-many relationships would cause Cartesian multiplication;
- validation and nominal decoding of every parent and child row before the
  nested result becomes trusted application data;
- derived query-plan metadata showing joins, batch boundaries, predicates,
  ordering, cardinality, and selected fields.

**Security boundary:** relationship traversal must not bypass ownership or
policy. P10 establishes the typed query and storage plan; P11 must prove that
parent and child policy filters compose and that an inaccessible child cannot
leak through an otherwise accessible parent.

**Executable exit evidence:** fixtures and SQLite/PostgreSQL runtime tests cover
zero, one, and many todos for one user; multiple users without cross-user
mixing; missing/invalid parents; foreign-key rejection; deterministic ordering;
parent pagination; rollback-safe lifecycle behaviour; and equivalent nested
JSON on both adapters. A query-count assertion proves the generated path is
bounded and does not regress to N+1 execution.

**Exit gate:** typed CRUD works against Postgres and raw driver errors never
cross the adapter boundary.

**Current evidence:** the [persistence v0.1 slice](persistence-v0.1.md) adds
typed create/read/update/delete expressions, rejects persistence operations from
functions, emits matching Postgres/SQLite fresh schemas and parameterised
statements, validates database rows before trust, normalises raw driver errors,
and infers one transaction for every transitively mutative action. Nested
mutative calls reuse the scoped adapter, read-only actions avoid write
transactions, and required-mutation cardinality checks remain inside the same
boundary. Fixed-shape multi-field updates are nominally checked and lower to
one parameterised statement. Named compound uniqueness is checked and emitted
for both adapters; identity, single-field unique, and compound-unique failures
normalise to compiler-owned identities before selecting typed conflict
bindings. It also checks named inverse collections and
lowers repeated required-parent includes to one parent query plus independently
bounded children, while many-parent includes use independent parent-page joins
merged by a deterministic key. Exact query counts and SQL are exposed in
persistence metadata. The SQLite and PostgreSQL
runtime tests prove successful CRUD, zero/multiple-row handling, typed
not-found/conflict mapping, rollback, driver-fault behaviour, foreign-key
lifecycle, and zero/one/many nested child results without cross-parent mixing.
Relationship paths beyond the bounded depth-two slice and required inverse-one
semantics, precise foreign-key/check conflict mapping, and advanced
isolation/external-effect boundaries remain deferred beyond the bounded P10.5
core. A clean Postgres 16 run passes all 22 live adapter tests,
including inferred rollback and precise single/compound constraint mappings;
raw driver errors remain contained behind `PersistenceFault`.

**Status:** complete.

### P10.5 — Pre-P11 language completion

**Objective:** settle the language surfaces that the complete golden todo must
exercise before authentication and policy implementation begins: owning-parent
and nested relationship traversal, one-to-one ergonomics, relationship declaration
syntax, migration identity, omission-aware patches, authored imports/modules,
and value/reference/mutation semantics. The original evidence order required a
P10R freeze before this pass; under the explicit owner deferral, this work is
exploratory until the later external evidence and freeze are complete.

**Status:** complete as exploratory implementation under the explicit P10R
review deferral. Direct optional-field patch inputs preserve omission separately
from explicit `none`, reject empty patches, and lower to one finite supplied-flag
SQL shape on SQLite and PostgreSQL. Fixed derived writes may run unconditionally
or only when a named patch field was supplied, closing GF-013B without dynamic
SQL. Required-child queries can also traverse one owning reference with explicit
`required` or `optional` cardinality through a bounded two-query plan; nullable
references preserve `none`. Unique-backed optional inverse declarations also
load through a bounded parent-plus-child plan and preserve `none` when no child
exists. This status does not authorise P11 assurance claims.
The first nested slice composes a non-nullable owning reference with an optional
inverse and fixes both maximum depth and query count in generated metadata.
The subsequent shape review rejects required inverse-one claims until totality
is enforceable (or absence is an explicit domain path), defers nested to-many
until every collection hop is bounded, and keeps many-to-many as an explicit
join entity. The provisional migration-identity contract is documented in
[migration identity v0.1](migration-identity-v0.1.md).
The bounded module core and value boundary are also implemented: imports are
selective and visibility-checked, ordinary parameters and returned data are
immutable values, and only type-compatible lexical `var mut` locals can be rebound.
No field/caller mutation or authored reference notation is accepted. More
powerful namespace or `inout`-style features require new application evidence.

The parallel DX0.5 developer-tooling slice is an exit prerequisite for P11
application authoring. It does not define language semantics, but it must expose
the authoritative compiler loop used to build the golden todo: structured JSON
diagnostics, watched checking/building, atomic generated-output promotion, and
a live Bun process that advances only to a successful build. Its detailed gate
is defined in the [developer tooling workstream](developer-tooling.md).

### P10R — Assurance and validation reset

**Objective:** restore a fair evidence sequence before implementation choices
become irreversible, and turn the central assurance claim into a bounded model
that can fail.

This phase does not add runtime capability. It freezes the contracts against
which P11 and P12 will be judged.

#### P10R.1 — Complete golden todo design contract

Write the complete canonical todo application as design evidence, not as
pseudocode tailored to current compiler support. The frozen package must include:

- canonical source for all behaviour required by the validation plan;
- black-box acceptance tests and adversarial change requests;
- intended policy, lifecycle decisions, configuration, service contract, job,
  authored business tests, and expected derived audit;
- expected compiler decisions, including explicit `cannot prove`,
  human-decision, and unsupported-capability outcomes;
- a ledger of every construct the current language cannot yet express; and
- a strong TypeScript baseline specification naming its framework, schema,
  persistence, policy, static-analysis, test, and agent-context facilities.

The design may expose necessary language changes, but those changes enter the
issue log and acceptance fixtures before implementation. The existing compiler
must not be used to silently reduce the application to what it already supports.

#### P10R.2 — Policy and proof kernel

Create a small normative policy/proof specification before implementing policy.
It must define:

- the supported actor, role, tenant, ownership, field, route, lifecycle, public
  access, and external-effect predicates;
- the exact proof obligations created by every query, mutation, projection,
  route, relationship traversal, service call, and policy weakening;
- the facts that may be introduced by authentication, validated input, guarded
  queries, trusted configuration, and prior proof steps;
- composition rules across calls, nested queries, relationships, and actions;
- the difference between `proved`, `rejected`, `requires human decision`,
  `unsupported`, and `cannot prove`;
- the conservative behaviour for ambiguity: absence of a proof never becomes
  permission;
- soundness claims made by the project and claims explicitly not made; and
- minimal positive, negative, and indeterminate fixtures for every rule.

The phrase “compiler proof” may only be used for a property tied to a named rule
and fixture. Other properties must be described as validation, generation,
testing, convention, or residual risk as appropriate.

#### P10R.3 — Threat model and trusted computing base

Create one authoritative threat model covering accidental and adversarial agent
changes, ambiguous human requests, hostile boundary data, cross-tenant access,
secret disclosure, undeclared egress, dependency/provider failure, generated
target defects, CI bypass, and misuse of escape hatches.

For every advertised assurance property, record:

- the protected asset and relevant attacker or failure source;
- the compiler/runtime boundary that prevents or detects it;
- the components trusted for the claim, including compiler, generated runtime,
  database constraints, policy store, approval mechanism, CI, deployment
  configuration, runtime built-ins, and external contracts;
- whether enforcement is static, runtime, generated-test, operational, or
  human-review based;
- known bypasses, escape hatches, and residual risk; and
- executable or review evidence required before the claim may appear in product
  documentation.

The threat model must distinguish a deterministic decision from a correct or
sound decision. It must not describe the compiler/runtime itself as outside the
trusted computing base.

#### P10R.4 — Human approval protocol

Specify an approval mechanism that an implementation agent cannot satisfy by
editing ordinary repository files. At minimum, a releasable policy weakening
must produce an approval record bound to:

- a canonical digest of the exact policy and affected semantic graph;
- the behavioural/policy diff presented to the reviewer;
- authenticated reviewer identity and authority;
- the decision, rationale, and expiry or supersession behaviour; and
- the compiler and policy-schema version used to interpret it.

CI must reject missing, stale, self-issued, replayed, or scope-mismatched
approval. Local development may inspect an unapproved change, but it may not
produce a release-equivalent success. Acceptance cases must include an agent
editing source and attempting to manufacture or reuse approval.

The first implementation may use a protected CI/review-system attestation rather
than cryptography invented by this project. The security property is separation
of authority, not a bespoke signature format.

This work must not assume that conventional pull-request review is an effective
human assurance mechanism. The project begins from the observation that code
review is increasingly skipped or superficial because generated changes are
large, reviewers lack the originating context, behaviour is distributed across
files and framework layers, plausible-looking code is difficult to distrust,
and defects may hide in indirect effects, omitted cases, generated behaviour,
configuration, or interactions between otherwise reasonable changes.

P10R must investigate and record why reviewers fail to understand or challenge
agent-generated changes, including at least:

- missing product intent, constraints, and rejected alternatives;
- diff volume and plausible-looking implementation detail overwhelming the
  meaningful decision;
- behaviour and invariants distributed across routes, actions, policy,
  persistence, jobs, services, configuration, and generated artifacts;
- indirect reads, writes, external calls, retries, lifecycle changes, and other
  side effects that are not obvious from the edited lines;
- reviewers being unable to distinguish compiler-proved facts, tested claims,
  agent assertions, assumptions, and unresolved uncertainty;
- inadequate time, unclear ownership, approval fatigue, social pressure to
  unblock a change, and habituation to mostly-correct generated output; and
- interfaces that ask a human to approve an entire change when only a small
  number of decisions genuinely require human judgement.

The approval surface should therefore be a compiler-derived behavioural and
decision review, not a shortened code diff. For each requested human decision,
it must present, in domain language:

- the original request and the decision requiring approval;
- the current behaviour, proposed behaviour, and reason for the change;
- affected actors, routes, entities, fields, policies, lifecycles, services,
  secrets, jobs, and deployment/configuration surfaces;
- direct and transitive reads, writes, emissions, external effects, transaction
  boundaries, retry/idempotency consequences, and failure/disclosure changes;
- the relevant relationship path through the semantic graph rather than a list
  of disconnected files;
- which claims are statically proved, runtime validated, test-supported,
  operationally enforced, assumed, unsupported, or still uncertain;
- counterexamples and adversarial cases considered, including what newly becomes
  possible; and
- the narrowest available choices, their consequences, and the exact policy
  delta each choice authorises.

Source and generated-code views remain available for investigation, but approval
must not depend on a reviewer reconstructing the behaviour from them. The review
UI must support drilling from the behavioural summary to the responsible source,
proof obligation, test, or runtime boundary without losing the decision context.

The intended product includes a custom web-based UI, so the review experience is
not constrained by pull-request conventions or a static linear diff. Treat that
as a core product opportunity rather than a presentation detail. The UI should
be able to test richer review models such as:

- an intent-first change narrative that progressively reveals implementation
  evidence;
- interactive before/after views of routes, policies, data access, lifecycle,
  failures, configuration, and external effects;
- explorable semantic-graph paths showing why a declaration or decision affects
  apparently distant behaviour;
- filters for newly public surfaces, weakened protections, destructive changes,
  secret access, external egress, transaction changes, and escape hatches;
- actor- and scenario-based simulation answering questions such as “what can
  this user do now that they could not do before?”;
- direct comparison of accepted, rejected, and alternative policy choices with
  their downstream consequences;
- focused approval of individual human-owned decisions instead of one blanket
  approval for an entire implementation; and
- stable links from every claim to its proof rule, boundary validation, test,
  runtime evidence, source declaration, and residual uncertainty.

The web UI must consume versioned compiler/semantic artifacts rather than infer
meaning independently or become a second source of language semantics. Its
interactive presentation may evolve quickly, but every displayed assurance
claim must remain reproducible in machine-readable evidence and in a usable
non-graphical export for CI, accessibility, and archival review.

The research must compare at least a normal pull-request diff, a concise
behavioural diff, and the proposed relationship/effect-aware approval view.
Measure comprehension accuracy, important risks noticed, false confidence,
review time, requests for clarification, and approval/rejection quality. Include
changes whose individual lines look reasonable but whose transitive behaviour is
unsafe. If reviewers cannot reliably understand the decision from the proposed
surface, the approval protocol has failed even when its identity and attestation
mechanics are secure.

#### P10R.5 — First-user and adoption hypothesis

Document one initial research user without narrowing the language's intended
technical scope. The current candidate is:

> An experienced product engineering team using coding agents for substantial
> greenfield or newly isolated SaaS-backend work, where a human remains
> accountable for security and policy but cannot economically review all
> generated implementation code.

Define the accountable human, daily user, adoption trigger, existing
alternative, acceptable switching cost, required interoperability, trust
objections, and the smallest credible adoption path. The first path should be a
new or isolated service with generated HTTP/database boundaries, not a mandatory
rewrite of an existing system.

Interview or structured-review evidence from at least five people matching the
candidate profile must test whether the stated pain, review surface, audit, and
approval flow are valuable. These interviews are product evidence, not proof of
compiler safety. If another user profile proves stronger, record the change and
its reasons before P12 rather than silently broadening the audience.

#### P10R.6 — Preregistered comparison protocol and thresholds

Give the candidate protocol a dated digest before P11 implementation begins.
External review and the final freeze occur after feature-complete implementation
and before comparative trials. Any review-driven revision must preserve and
report the original digest and the implementation exposure to it. The protocol
must include a
severity rubric, adjudication rules, raw record format, randomised or
counterbalanced run order, clean checkpoints, and at least three independent
runs per stack. Use the same model/settings for the primary comparison; any
cross-model results are reported separately. An evaluator who did not implement
the relevant change classifies defects and comprehension answers from frozen
rubrics while blinded to the stack where the artifact permits it.

The initial continuation thresholds are:

1. **Critical safety:** no preregistered critical authentication, tenant,
   authorization, secret-disclosure, or destructive-lifecycle case may be
   silently accepted by the language. It must be rejected or require the named
   human decision before release-equivalent success.
2. **Safety advantage:** across preregistered high/critical adversarial cases,
   the language must prevent or escalate at least 80%, and must outperform the
   TypeScript baseline by at least 25 percentage points. Detection only by a
   generated test does not count as semantic prevention.
3. **Valid-work friction:** no more than 20% of ordinary non-adversarial changes
   may require a language/compiler change, unsupported-capability disposition,
   or escape hatch. Repeated compiler attempts count as friction even when the
   final program succeeds.
4. **Escape hatches:** no more than 10% of ordinary changes may require an escape
   hatch, and none may bypass a critical assurance property without the frozen
   approval protocol.
5. **Agent efficiency:** median time-to-correct-change and uncached token use may
   each be at most 25% worse than TypeScript. A safety win does not erase larger
   productivity costs; it requires an explicit continuation decision.
6. **Diagnostic quality:** at least 80% of first diagnostics must identify the
   correct violated obligation or missing decision, and the median valid repair
   must take no more than two compile/edit cycles.
7. **Comprehension:** independent reviewers and fresh agents must answer at least
   80% of frozen behaviour, policy, effect, and lifecycle questions correctly;
   the language/audit result must not trail TypeScript by more than 10 percentage
   points in any critical category.

These thresholds may be challenged once during P10R. After protocol freeze they
cannot be changed in response to results. Results between thresholds are
reported as mixed or inconclusive, not rounded into a win. Failure of thresholds
2–7 triggers an explicit continue/redesign/framework-pivot/stop decision;
failure of threshold 1 blocks the current assurance claim regardless of other
scores.

#### P10R.7 — Documentation authority and reading surface

Restructure documentation without deleting design history:

- a concise research brief owns the problem, first user, hypotheses,
  counter-hypothesis, and continuation rules;
- normative specifications contain only current semantics and use explicit
  `accepted`, `provisional`, or `open` markers at the relevant rule;
- the decision register owns decision status and links to one normative home
  rather than repeating full specifications;
- the roadmap owns sequence and evidence, never semantic authority;
- implementation evidence is separated from product-validation evidence; and
- conversation extraction and obsolete syntax move to an archive reading path,
  outside the normal onboarding sequence.

Add a short documentation consistency check to each phase exit: current status,
authority, named evidence, open questions, and links must agree across the
research brief, decision register, normative specification, and roadmap.

**P10R exit gate:** all seven packages are reviewed and frozen; the canonical
todo source and acceptance suite exist independently of compiler support; every
assurance claim maps to a named proof/validation rule and threat-model entry;
the approval protocol has adversarial acceptance cases; the first-user review is
recorded; comparison thresholds and adjudication are preregistered; and the
documentation index exposes one unambiguous current reading path.

**Status:** in progress.

**Candidate evidence (not yet independently reviewed or frozen):**

- [golden todo contract](../examples/golden-todo/README.md), including proposed
  source, protected policy, machine-readable black-box cases, a twenty-change
  adversarial sequence, expected audit, TypeScript baseline, and language-
  friction ledger;
- [policy and proof kernel](policy-proof-v0.1.md) with named rules, conservative
  outcomes, trusted fact sources, obligations, non-claims, and a minimal fixture
  matrix, now represented by 39 machine-readable positive/negative/indeterminate
  fixtures;
- [runtime validation rules](validation-rules-v0.1.md) and a machine-readable
  evidence map covering all 39 todo cases and all twenty threat entries;
- [threat model](threat-model.md) naming the trusted computing base, enforcement
  layers, evidence, bypasses, and residual risk;
- [approval protocol](approval-protocol.md) binding independent attestations to
  canonical policy/semantic digests and defining adversarial replay and
  comprehension cases, with twelve machine-readable attack fixtures;
- [research brief](research-brief.md) freezing the candidate first user and
  TypeScript counter-hypothesis, plus a
  [structured first-user review instrument](first-user-review-guide.md) and
  machine-readable record template; and
- [comparison protocol](comparison-protocol.md) defining clean checkpoints,
  counterbalancing, raw records, severity, adjudication, comprehension, and the
  continuation thresholds, plus a twelve-question, 43-point
  [comprehension instrument](comprehension-study.md) with a machine-readable
  scoring key.

The remaining gate is review evidence, not more unreviewed surface area: resolve
ambiguities found by independent contract review, freeze digests and fixtures,
conduct the five structured first-user reviews, and record the freeze decision.

### P10.6 — Recoverable problems, routes, and entity boundary revision

**Status:** unblocked syntax, failure-flow, route, artifact, and target work is
implemented; the explicitly unresolved operational mapping, handler-arm,
optional-persistence, and protected policy-authority choices remain open.

**Why this phase exists:** first-user review of the implemented P7 and P10
surfaces found two related abstraction leaks before P11. The failure model
closes expected domain rejection but leaves recoverable operational conditions
outside the callable contract. The entity model also makes persistence part of
the meaning of `entity`, forcing domain identity, storage, and policy into one
assumption. P10.6 revises those boundaries before authentication and policy
make them harder to change.

#### Failure declaration and disclosure decisions

- Retain the term `failure`; it distinguishes a typed negative outcome from an
  uncatchable compiler/runtime defect.
- Replace the visually understated `failure Name: Kind` relationship with an
  explicit `kind Kind` member in the failure body.
- Define each application failure once and reuse it across callables. Group
  declarations by domain rather than redefining failures per action.
- Retain an explicit stable public `code` and an optional static safe
  `message`. A source declaration may be renamed without silently changing the
  public code.
- Retain separate declaration-time `public` and `internal` context schemas.
  Public fields are part of the boundary contract; internal fields never enter
  a client response merely because a failure is produced.
- Flatten context values at production sites. `reject`, persistence mappings,
  and future adapters supply one object; the canonical declaration permanently
  determines which fields are public or internal.
- Reject overlapping public/internal field names so flattened construction is
  unambiguous.
- At every production site require all non-optional context fields, reject
  unknown and duplicate fields, resolve every value in lexical scope, and
  check its exact nominal type. Adding a required context field is therefore a
  checked change across every production site.

The intended declaration shape is:

```text
failure CustomerNotFound {
    kind NotFound
    code "customer_not_found"
    message "Customer not found."

    public {
        customer_id: Customer.id
    }

    internal {
        lookup_id: Customer.id
    }
}
```

An intended production site is flat:

```text
missing: CustomerNotFound {
    customer_id: input.id
    lookup_id: input.id
}
```

#### Exhaustive recoverable-problem decisions

- Broaden `fails` from domain failures to the complete set of recoverable
  problems that a callable may propagate. This includes application-defined
  failures and normalized operational problems.
- For every callable expression, compute a success type and a closed set of
  recoverable problems. Every member of that set must be handled locally,
  mapped to another declared failure, or propagated in the enclosing
  callable's `fails` set.
- Apply this rule to functions, actions, jobs, handlers, and generated boundary
  operations. Purity remains separate: a function may handle a failure from a
  fallible pure call but still may not perform persistence or other effects.
- Add one explicit `attempt` mechanism. Without a handler block it unwraps the
  success value and visibly propagates the recoverable problem set. With a
  handler block it handles or translates named problems; any residual problem
  must still appear in the enclosing `fails` set.
- Require `attempt` at every fallible expression, even when the only decision
  is propagation. Require the authored `fails` clause to equal the exact
  reachable unhandled set: missing and stale extra entries both fail
  compilation, and compiler inference is diagnostic help rather than an
  implicit source edit.
- Make handling exhaustive after accounting for declared propagation. No
  failure or operational problem may disappear through a wildcard, implicit
  catch, or unchecked target exception.
- Reuse the language's existing exhaustive-match analysis where possible, but
  do not require authored `Result<T, E>` plumbing through ordinary source.
- Finalize how an `attempt` arm supplies a replacement success value. A
  `recover` spelling is a candidate, not yet accepted grammar.

The intended propagation shape is:

```text
action load_customer(id: Customer.id) -> Customer
    fails CustomerNotFound, Unavailable
{
    return attempt query required Customer {
        where: id == id
        missing: CustomerNotFound {
            customer_id: id
        }
    }
}
```

#### Operational problems and defects

- Expose a deliberately small, source-agnostic built-in operational vocabulary.
  The initial candidates are `Unavailable`, `TimedOut`, `RateLimited`, and
  `OutcomeUnknown`.
- Do not expose adapter, vendor, database, or provider names in those problem
  types. An action handles `Unavailable`, not `PrimaryStore.Unavailable` or a
  raw driver exception. The attempted operation provides source context to the
  reader; compiler/runtime metadata retains exact provenance for diagnostics,
  audit, and telemetry.
- Keep `OutcomeUnknown` distinct from `Unavailable`. In particular, a failed
  write may have taken effect even when acknowledgement was lost; fallback or
  retry is unsafe without idempotency or reconciliation evidence.
- Allow application logic to catch normalized operational problems and select
  a fallback, secondary capability, domain-specific translation, or early
  response. Any recoverable problem raised by that fallback is checked in the
  same way.
- Allow a generic operational problem to propagate directly, using a safe
  built-in boundary code, or be translated into a named application failure
  with its own `kind`, stable `code`, message, and disclosure contract.
- Keep true defects and impossible states outside the catchable problem set.
  Generated-code invariant failures, corrupt runtime state, and compiler/runtime
  defects terminate the operation and are contained and reported at the
  runtime boundary.

#### Boundary mapping decisions

- Continue to derive HTTP status from semantic `kind`; application and route
  code may not choose numeric statuses ad hoc.
- Continue to derive a named application's stable response error identifier
  from its declared `code`.
- Give built-in operational problems safe generic boundary codes and messages.
  Finalize the HTTP status and retry semantics for `OutcomeUnknown` before
  implementation.
- Derive every route's domain and operational error surface from the callable
  graph. Generated OpenAPI, audit, tests, and runtime adapters must agree with
  that graph.

#### Entity, persistence, and policy decisions

- Redefine `entity` as identity-bearing domain data rather than data that is
  necessarily stored in a database. Keep `value` for structured data without
  entity identity.
- Permit ordinary construction and use of an entity without a persistence
  capability.
- Make persistence an optional explicit part of an entity contract. Only an
  entity with persistence may participate in generated insert/query/update/
  delete operations, indexes, uniqueness, references, and migrations.
- Separate ordinary entity construction from persistence. Revisit the current
  database `create` spelling; `insert` is a candidate because it does not imply
  that constructing an entity value requires storage.
- Place entity-specific access, field, and lifecycle policy under the entity's
  semantic definition so reviewers do not have to reconcile a distant policy
  catalogue with the data it governs.
- Retain an application/capability-level policy home for authentication
  strategy, public routes, cross-entity rules, external effects, secrets, jobs,
  and deployment behaviour.
- Preserve the human-approval boundary for policy changes even if policy is
  semantically colocated with an entity. The implementation must decide whether
  physical colocation, protected source regions, or a checked augmentation form
  best preserves that authority.

An illustrative direction, not final grammar, is:

```text
entity Customer {
    id: Uuid
    email: Email

    persistence {
        identity id
        unique email
    }

    policy {
        // Entity-specific access, field, and lifecycle rules.
    }
}
```

An entity without `persistence` remains constructible domain data but has no
generated storage operations.

#### Route, action, and locality decisions

- Preserve `route`, `action`, and `function` as distinct semantic declarations:
  a route is an HTTP boundary, an action is reusable effectful domain
  behaviour, and a function is reusable pure computation.
- Do not require every route to delegate to a one-to-one named action. The
  canonical starting point for small endpoint-specific behaviour is a local
  inline action inside the route.
- Extract a named action when behaviour is reused by another route, action,
  job, handler, or test boundary; when it represents a stable domain command;
  or when it needs its own policy, transaction, idempotency, or observability
  boundary. Do not extract merely to reproduce controller/service layering.
- A route contains exactly one behaviour form: either an inline `action` body
  or `run:` invoking a named action. Supplying both or neither is invalid.
- `run:` remains because it explicitly maps route-local typed transport values
  into a genuinely reusable named action. It is not required for the common
  inline case and cannot contain arbitrary route logic.
- An inline action has the same effect, transaction, policy, return, and
  recoverable-problem rules as a named action. Its closed problem set is
  declared on the action header as `action: fails A, B { ... }`.
- Inline action structure is brace-delimited. Indentation and line breaks are
  never semantic; no YAML-style continuation or tab-sensitive syntax is
  permitted.
- Retain required authentication as the route default. Replace the current
  `auth: public explicitly` spelling with the exact opt-out `auth: none`.
  Omission never disables authentication, and the override must remain
  conspicuous in source, audit, tests, and generated documentation.
- Keep authentication and authorisation separate. `auth: none` changes the
  inbound identity requirement; it does not bypass entity or application
  policy. A no-auth route whose behaviour requires an authenticated actor is a
  compile/proof error.
- Configure authentication strategies outside ordinary route business logic.
  Strategy narrowing may be added later, but route syntax must not expose
  provider SDKs, raw tokens, cookies, or provider-specific identity data.
- Keep body, path, query, and ordinary-header decoding in the route boundary.
  These become validated typed bindings visible to the inline action or passed
  explicitly through `run:`. Named actions and functions remain transport-
  independent.
- Declare path placeholders in the route template as `{name}` and type them in
  a brace-delimited `path: { name: Type }` group. Expose validated values through
  the `path.name` namespace so they cannot collide with query, header, or body
  fields.
- Require an exact one-to-one correspondence between template placeholders and
  path bindings: every placeholder is declared once, every declaration appears
  in the template, and duplicate placeholders are invalid.
- Reserve authentication credentials and trusted identity headers for the
  generated authentication adapter; ordinary header binding cannot expose
  them to application logic.
- Preserve the field-style `name: value` route body and braces, including
  block-valued `path:` and `action:` items. Final query/header/
  body binding spelling remains a focused grammar task; the accepted semantics
  do not authorise decorators, significant indentation, parameter annotations,
  or a wholesale route syntax redesign.

The accepted local-first shape is:

```text
route GET /customers/{customer_id}/orders/{order_id} {
    path: {
        customer_id: Customer.id
        order_id: Order.id
    }

    action: fails CustomerNotFound, OrderNotFound, Unavailable {
        // Endpoint-local effectful behaviour.
    }

    output: OrderView
}
```

An explicit authentication exception is:

```text
route GET /health {
    auth: none
    output: Health

    action: {
        return Health { ok: true }
    }
}
```

The extraction form remains:

```text
route POST /orders {
    input: CreateOrder
    output: OrderView
    run: place_order(input)
}
```

#### Required implementation evidence

- focused parser, semantic, failure-flow, and target fixtures for explicit
  `kind`, flat context construction, and exact context checking;
- direct, transitive, handled, mapped, and propagated operational-problem
  fixtures across functions and actions;
- exhaustive `attempt` fixtures, including fallback operations that introduce
  new problems;
- runtime cases for unavailable reads, definitely-not-executed writes, unknown
  write outcomes, safe idempotent retry, and unsafe fallback rejection;
- route/OpenAPI/audit agreement for named failures and generic operational
  problems;
- route fixtures proving authenticated default, exact `auth: none` opt-out,
  inline-action problem checking, mutually exclusive inline/`run:` behaviour,
  exact multi-placeholder path binding and namespacing, transport binding,
  named-action extraction, and whitespace-insensitive brace structure;
- constructible non-persistent entity fixtures and diagnostics rejecting
  persistence operations against them;
- persistent entity parity with the existing SQLite/PostgreSQL evidence; and
- entity-local policy fixtures plus approval/proof evidence showing that
  colocation cannot bypass human-owned policy authority.

**Exit gate:** every recoverable problem is mechanically handled, mapped, or
propagated; no raw adapter exception enters authored source; defects remain
contained; non-persistent entities work without storage; persistent entities
retain the existing database guarantees; and entity-local policy composes with
the application policy and approval model.

### P11 — Authentication, policy, and golden todo

**Objective:** implement the frozen P10R assurance contracts and demonstrate the
safe-default backend thesis on the canonical todo application without changing
the test to match the compiler.

**Required behaviour:**

- authentication required unless public is explicit;
- explicit public access and weaker requirements remain visible security
  exceptions rather than protections the LLM must remember to enable;
- multiple authentication strategies behind one generated adapter boundary,
  with deterministic selection, validated claim mapping, no implicit privilege
  merging, and fail-closed errors;
- a provider-independent typed actor containing identity, tenant, allowlisted
  user information, permissions/capabilities, and authentication strength;
- current actor and ownership scope visible to actions and policy without
  exposing provider SDKs, tokens, or session mechanics;
- a typed configuration contract, secret-safe environment binding, and
  fail-closed startup/readiness behaviour;
- todo CRUD and partial updates;
- `T?` versus omitted patch fields;
- concealed lookup where policy requires it;
- generated route/failure/security audit;
- reminder job and one reviewed external service after synchronous CRUD works.

P11 must preserve a frozen-toolchain result. If the todo exposes a language or
compiler defect, record the original outcome, change the issue/specification and
focused fixtures, then rerun from a named checkpoint. Report the frozen and
improved results separately.

**Exit gate:** the canonical todo application runs; every policy decision has a
named proof result; the threat-model evidence is satisfied; an agent cannot
self-approve a weakening; and the adversarial ownership, tenancy, secret, public
route, and lifecycle changes meet the preregistered P10R thresholds. Milestone C
is complete only after an independent review confirms the frozen evidence.

### P12 — Order/payment application and TypeScript baseline

**Objective:** try to falsify the language after the prototype works.

**Required pressure:** money/currency, inventory, provider contracts, payment
declines, idempotency, webhooks, retries, transactions, refunds, migrations,
role collisions, non-trivial pricing logic, and whether data-carrying enum
variants materially prevent invalid payment/provider states. The enum trial must
cover exhaustive matching, variant payload typing, boundary tagging and
decoding, compatibility, persistent representation, database constraints, and
migration behaviour; attractive surface syntax alone is not sufficient reason
to implement the feature.

Build two behaviourally equivalent applications: one authored in this language
and one authored with a strong, opinionated TypeScript stack. Give them the same
predeclared acceptance tests, requirements, adversarial prompts, and ordered
change sequence. The TypeScript implementation must be a credible competitor,
not a deliberately weak control.

P12 uses the protocol, thresholds, severity rubric, baseline definition, and
reporting template frozen in P10R. Verify their digests before the first run;
do not reinterpret or extend them after seeing comparative results. Preserve
three costs separately: building or repairing the language and compiler,
authoring the application against the frozen toolchain, and adapting the
application after a language/toolchain change. None may disappear into
unmeasured setup.

Run at least three clean trials per stack with counterbalanced order. The
order/payment requirements, prompts, acceptance tests, starting repositories,
and allowed agent context are identical. A solution learned in an earlier run
is either made available symmetrically or recorded as contamination. Report
per-run results and distributions; do not present only pooled totals.

**Required experiment record:**

- pin compiler, runtime, dependency, model, tool, prompt, and starting-repository
  versions so each result is reproducible;
- record wall-clock and active agent time, input/output tokens, number of turns,
  context supplied, files and declarations touched, compiler/test iterations,
  human interventions, and final authored/generated source size for every
  change;
- record defects, security or policy mistakes, regressions, incomplete work,
  documentation drift, and which mechanism first detected each one;
- record every language compiler error or warning encountered during ordinary
  implementation, including repeated diagnostics, false positives, false
  negatives, unclear messages, and cases where the agent became stuck;
- record every desired expression that the language could not represent
  directly, every workaround, and every escape hatch, including the additional
  code, tokens, time, risk, and loss of clarity it caused;
- record equivalent TypeScript friction rather than attributing every problem
  on that side simply to “TypeScript”; and
- retain raw run logs and per-change results, not only a retrospective summary.

Every compiler-friction or workaround incident must receive an explicit review.
Classify it as a compiler defect, diagnostic/documentation defect, missing
standard capability, missing language feature, intentional safety boundary,
agent misunderstanding, tooling/ecosystem gap, or experiment/setup problem.
Then choose and justify one disposition: fix the implementation, improve the
diagnostic or documentation, change the language, add a constrained standard
capability, accept the friction as a worthwhile guardrail, provide a reviewed
escape hatch, or reject the attempted use case. Language changes must enter the
issue log and gain focused fixtures before the application is retried; they
must not be silently patched in merely to make the new language win. Preserve
the failed run, then rerun the affected change from a named clean checkpoint and
report both the original and improved results. Distinguish the initially frozen
comparison from any later tuned-toolchain comparison.

The final critical review applies every P10R continuation threshold and must
separately answer:

- which implementation was easier to write and change, and why;
- which reached correct behaviour faster;
- which consumed fewer tokens and required less context;
- which produced fewer and less severe defects;
- which diagnostics shortened or lengthened the agent loop;
- which required fewer workarounds, escape hatches, and human decisions;
- which repository was easier for a human and a fresh agent to understand;
- which advantages came from the language rather than its framework,
  scaffolding, generated code, or novelty; and
- whether the evidence supports continuing the language, changing its design,
  pivoting the work into a TypeScript framework, or stopping.

Both quantitative measurements and a candid qualitative account are required.
An inconvenient result, including TypeScript winning overall, is a successful
experiment if it is well evidenced.

**Exit gate:** all clean trials are complete; both applications are judged
against the same acceptance suite; the raw per-change experiment ledger,
independent adjudication, comprehension results, and incident reviews are
complete; all proposed language changes are linked to issues and fixtures; each
preregistered threshold has a pass/fail/mixed result; and the required
continue/redesign/framework-pivot/stop decision is recorded. A failed critical
safety threshold cannot be overridden by productivity or source-size results.

## 5. Project structure workstream

An opinionated project structure is now an explicit roadmap concern. The goal
is to reduce navigation cost and unnecessary naming/layout decisions for humans
and agents while remaining viable for small, large, multi-domain,
integration-heavy, and multi-application repositories.

The work is tracked in the [project structure workstream](project-structure.md)
and `LAYOUT-001` in the [language issue log](language-issues.md). P8 must produce
concrete candidate trees and select a prototype before P9 fixes runtime and
generated-output paths. Enforcement remains open until the todo and
order/payment applications provide evidence.

The intended progression is convention, scaffold, warning, and potentially
compiler enforcement with a narrow reviewed escape—not immediate rejection of
every non-canonical repository.

Project creation follows the same principle. The initial direction is one
static, deterministic base produced by `jadpo new`. A human-facing wizard,
if added, is only an interface over explicit flags; an LLM uses the same
non-interactive command. Optional capability packs may add canonical material
later, but they cannot invent or rearrange the project architecture.

## 6. Developer tooling and LLM presentation workstream

Editor and presentation support are tracked in the
[developer tooling workstream](developer-tooling.md), with open decisions in
`TOOL-002`, `TOOL-003`, and `TOOL-004` in the
[language issue log](language-issues.md).

The compiler is the one semantic authority. A standard LSP should reuse its
parser, semantic graph, inferred types, documentation identifiers, and
structured diagnostics rather than implementing editor-only language rules.
The delivery order is:

1. a stable machine language/file identifier, TextMate grammar, file
   association, comments/brackets/folding, and fenced-snippet fixtures;
2. a checked local-development loop with versioned JSON diagnostics,
   source-file watching, atomic rebuilds, last-known-good serving, and
   automatic Bun restart/reload after successful builds;
3. compiler-parity diagnostics, symbols, definition/reference navigation,
   hover documentation, completion, and semantic tokens over LSP;
4. formatter, rename, conservative code actions, and links to generated
   contracts/audits;
5. a central diagnostic catalogue, human-first summaries, guided and verified
   repairs, decision ownership, bounded semantic context, clean IDE Quick Fix
   alternatives, and root-cause grouping;
6. audience-specific browser, telemetry, and agent incident schemas with
   secret-safe OpenTelemetry/structured-log adapters and local semantic
   enrichment through stable operation/source-revision IDs;
7. Shiki, Prism, Highlight.js, and Monaco adapters for documentation and chat
   clients that allow custom grammars; and
8. a compact skill/plugin or MCP interface that lets agents retrieve versioned
   grammar, diagnostics, symbols, and documentation from the same language
   service.

Fenced Markdown code can carry the stable language identifier, but highlighting
is ultimately controlled by the renderer. Unsupported hosts must receive exact
readable plain text; the project must not fake support by labelling source as a
different language. Inline backtick spans are not expected to receive rich
syntax colouring.

DX0 highlighting can proceed during P10–P11 without displacing semantic work.
DX0.5 is scheduled during P10.5 and must pass before P11 application authoring.
The minimum LSP and agent-context baseline must be recorded and frozen before
P12 so tooling quality is measured honestly and does not change midway through
the TypeScript comparison.

### DX2 guided-diagnostic and safe-enrichment contract

Implement diagnostics as compiler-owned semantic repair protocols rather than
scattered strings. One versioned catalogue defines each lower-case dotted rule
identifier, human summary and reason, typed context, recommended next step,
bounded alternatives, decision owner, impact, help identifier, fixtures, and
legacy aliases. Compiler call sites provide typed facts only.

The version-2 agent packet uses the clear keys `schemaVersion`, `diagnosticId`,
`sourceRevision`, `summary`, `reason`, `recommendedNextStep`, `alternatives`,
`ruleId`, `severity`, `location`, `context`, `impact`, and `helpId`. Repair kinds
are `automatic_fix`, `guided_choice`, and `human_decision`. Automatic fixes are
revision-bound compiler edits with a preview of behavioural and public-contract
impact; ambiguous semantic choices remain alternatives, and protected choices
become explicit human questions.

The LSP renders the same object without exposing raw JSON: the smallest precise
squiggle, plain summary in Problems, summary/reason/recommendation on hover,
recommended verified edit plus clean alternatives in Quick Fix, multi-file diff
preview, and an expandable impact/help view. Stale fixes fail closed. Parser and
type cascades group beneath a root cause, while terminal stage summaries move to
report status rather than appearing as extra problems.

Keep `CompilerDiagnostic`, `PublicFailureResponse`, `OperationalLogEvent`, and
`AgentIncidentPacket` as distinct audience types. Browser failures contain only
declared public data. Operational events contain safe correlation and semantic
IDs and map to structured JSON, OpenTelemetry, and constrained provider
adapters. Diagnostic/logging APIs reject arbitrary rendered values; secrets are
non-renderable and internal context is not automatically loggable. A trusted
local tool enriches a runtime event with the matching compiler graph and source
revision to give an LLM rich source, context, impact, occurrence, and repair
information without exporting customer data or secrets.

**Required evidence:**

- a generated catalogue manifest and documentation page covering every public
  diagnostic, with no remaining ad hoc user-facing message construction;
- fixtures for summary/reason rendering, each repair kind, decision ownership,
  related locations, bounded impact, root-cause grouping, and legacy aliases;
- CLI/JSON/LSP parity tests plus IDE protocol cases for Problems, hover, ordered
  Quick Fix alternatives, stale-edit rejection, and multi-file preview;
- counterfactual repair tests proving the recommended edit resolves the named
  diagnostic and accurately reports any new diagnostic or public-contract
  change;
- browser, log, trace, exporter-buffer, and provider-adapter tests planted with
  credential, header, body, connection-string, customer-data, and raw-exception
  canaries; and
- fresh-agent and first-user trials showing that common mechanical errors are
  corrected in one cycle and the overall median valid repair remains within the
  two-cycle roadmap threshold.

**Exit gate:** every public diagnostic is central, readable, actionable,
fixture-backed, and rendered consistently across CLI, JSON, documentation, and
IDE; safe preferred repairs are verifiably correct; semantic alternatives and
human decisions are explicit; no audience boundary can serialize another
audience's payload; and the secret-canary and repair-cycle evidence passes before
the P12 tooling freeze.

## 7. Typed configuration and deployment-readiness workstream

Configuration is part of the application contract, not an untyped collection
of process strings. Design it against the P11 golden todo application and
freeze its minimum tool/runtime behaviour before the P12 comparison. Exact
source syntax remains open under `CONFIG-001`; the required semantics do not.

**Authored contract and discoverability:**

- declare each value once with its semantic type, requiredness, optional
  default, constraints, documentation, and whether it is secret;
- give tooling and deployment systems a generated machine-readable manifest
  containing names, types, constraints, descriptions, requiredness,
  environment applicability, expected source, and safe validation checks—but
  never secret values;
- generate human documentation, local-development templates, IDE hover and
  completion data, and compact agent context from that same manifest;
- keep development, test, staging, and production differences in explicit
  value bindings or overlays rather than application-logic branches, and never
  silently substitute a development default in production; and
- track value provenance so diagnostics can identify the missing or invalid
  binding without printing its contents.

**Validation boundaries:**

- ordinary source `check` validates declarations, references, types,
  constraints, conflicting defaults, and environment coverage without needing
  access to production secrets;
- an environment-bound preflight validates the actual deployment bindings and
  exits deterministically with structured diagnostics when a required value is
  absent or malformed;
- runtime startup repeats validation before binding a public listener or
  reporting readiness, so bypassing preflight cannot create a partially
  configured service;
- parsing and local constraints are distinct from bounded live checks: for
  example, a database URL can be structurally valid yet fail a timed connection
  or minimum-capability probe; and
- all command output, logs, generated artifacts, health responses, and failure
  reports redact secret values and avoid echoing credentials embedded in URLs.

**Health and deployment contract:**

- liveness reports whether the process/event loop is functioning and must not
  fail merely because an external dependency is temporarily unavailable;
- readiness remains false until effective configuration is valid and declared
  dependency checks have passed, and can become false when a required
  dependency is no longer usable;
- live checks are explicit, bounded by timeouts, side-effect-free where
  possible, and classified as required or advisory rather than inferred from a
  value's type;
- health endpoints expose stable status/check identifiers and safe reasons,
  not configuration or secret values; and
- preflight exit status plus readiness provide the hosting platform with a
  deterministic promotion gate. The deployment controller owns rollback to
  the prior healthy revision; the application supplies the evidence and never
  claims it performed a rollback itself.

Rotation and reload require an explicit semantic decision: each value must be
restart-bound or safely reloadable, with atomic validation before replacement
and a defined response when a rotated value cannot pass its live check. Do not
add ambient reads of `process.env` to authored or generated application logic;
the compiler-owned boundary is the only place raw bindings become typed trusted
configuration.

**Executable exit evidence:** fixtures and runtime/deployment-harness tests
cover missing required values, malformed values, invalid environment overlays,
secret redaction, startup before listener binding, readiness success/failure
and timeout, liveness independence, live dependency recovery, and a failed new
revision that is not promoted (or is rolled back) while the prior revision
remains healthy.

## 8. Public-facing website and documentation benchmark workstream

**Status:** conditional and not started. Activate only after P12 records a
`continue`, `redesign`, or framework-pivot decision and the project owner
authorises a public-facing website. This workstream is not evidence that the
product thesis has already succeeded.

**Objective:** before designing or implementing the public website, reassess
high-quality developer documentation projects individually, identify the
characteristics that make each effective, and deliberately select the patterns
that fit this project's audiences and trust model. Do not copy a site's visual
style wholesale or infer quality from popularity alone.

### Benchmark set

The primary case studies are:

- [Rust documentation](https://doc.rust-lang.org/) for its progression from
  books and learning material to generated crate/API reference, local/offline
  documentation, examples, source links, and compiler-integrated help;
- [Stripe documentation](https://docs.stripe.com/) for API onboarding,
  language-switchable examples, sandbox/test-mode integration, personalised
  context, lifecycle guidance, and treatment of errors and failure states;
- [Django documentation](https://docs.djangoproject.com/) for its separation
  of tutorials, task-oriented how-to guides, conceptual explanations, and
  precise reference material, plus explicit version annotations;
- [ArchWiki](https://wiki.archlinux.org/) for community-maintained operational
  knowledge, troubleshooting alongside configuration, alternative approaches,
  and documentation useful beyond the project's immediate users;
- [PostgreSQL documentation](https://www.postgresql.org/docs/current/) for
  complete, authoritative, versioned manuals organised for learners, users,
  administrators, client developers, extension authors, and contributors;
- [Godot documentation](https://docs.godotengine.org/en/stable/) for combining
  beginner projects, application guides, generated API/class reference,
  editor-accessible help, translations, engine internals, and community notes;
  and
- [Twilio documentation](https://www.twilio.com/docs) for outcome-oriented
  quickstarts, consistent cross-product concepts, multi-language runnable
  examples, webhook guidance, and short time to a first successful result.

Specialist and complementary references should also receive bounded reviews:

- [MDN Web Docs](https://developer.mozilla.org/) for standards-based web
  reference, examples, cross-browser compatibility, and mixed learning and
  reference paths;
- [GitHub Docs](https://docs.github.com/) and project repositories/issues for
  large-scale product navigation, versioning, source-adjacent knowledge, and
  the boundary between official documentation and operational discussion;
- [DevDocs](https://devdocs.io/) for fast federated search, consistent
  presentation, keyboard navigation, and offline reference access;
- [Can I Use](https://caniuse.com/) for focused compatibility evidence and a
  compact answer to a single recurring developer question;
- [OWASP](https://owasp.org/) for consensus-based security guidance, explicit
  scope limits, risk communication, and the distinction between awareness
  material and verifiable standards;
- [freeCodeCamp](https://www.freecodecamp.org/learn/) for structured,
  practice-led beginner learning rather than authoritative reference; and
- the current [DevPortal Awards](https://devportalawards.org/) winners and
  juror reports, initially North Developer, Shell Developer Portal, and
  Redpanda Docs, as a watchlist of contemporary onboarding and portal patterns
  rather than assumed permanent exemplars.

Laravel, Vue, Tailwind CSS, Kubernetes, and FastAPI may be added as comparison
cases if later evidence supports their recurring reputation. Their inclusion
must not be justified only by an isolated testimonial or listicle.

### Current evidence baseline

Preserve the following findings as the reason for the initial shortlist, then
recheck them when the workstream begins:

- the [2025 Stack Overflow Developer Survey](https://survey.stackoverflow.co/2025/developers)
  received about 49,000 responses and reported technical documentation as the
  most-used learning resource, at nearly 68%; its recruitment was weighted
  toward engaged Stack Overflow users, so it establishes broad importance but
  not an impartial documentation ranking;
- a peer-reviewed [Rust adoption study](https://www.cs.umd.edu/~mwh/papers/rust-adoption.pdf)
  combined 16 professional interviews with 178 completed community surveys and
  reported 91% positive sentiment for official documentation; the authors note
  likely self-selection toward people who already view Rust positively;
- the open [APIbenchmarks index](https://www.apibenchmarks.com/) currently
  scores Stripe at 97.5/A+ for payment APIs and Twilio at 94.1/A+ for messaging
  APIs using a reproducible method, but documentation/developer experience is
  only 30% of the composite score and the benchmark itself must be reviewed;
- Django's own
  [writing guide](https://docs.djangoproject.com/en/stable/internals/contributing/writing-documentation/)
  explicitly separates tutorials, how-to guides, topic explanations, and
  reference; the same structure is now formalised by
  [Diataxis](https://diataxis.fr/) and used beyond Django, which is evidence of
  design influence rather than direct satisfaction measurement;
- ArchWiki is maintained by an official team and thousands of contributors,
  publishes current [activity statistics](https://wiki.archlinux.org/title/ArchWiki:Statistics),
  and is repeatedly used outside Arch; this is a strong operational and
  reputation signal, not a controlled comparative study;
- PostgreSQL states that its
  [official manual](https://www.postgresql.org/docs/current/preface.html) is
  written by developers and volunteers in parallel with the software and
  divides its material by user stage and role;
- Godot documents its maintenance and translation process across approximately
  1.1 million source words, versioned manuals, generated class reference, and
  user-note feedback in its
  [documentation-quality report](https://godotengine.org/article/ensuring-quality-godot-documentation/);
- MDN reports more than 15 million monthly users and collaboration with browser
  vendors, standards organisations, and Open Web Docs on its
  [about page](https://developer.mozilla.org/en-US/about); these are scale and
  governance signals reported by MDN itself; and
- the expert-juried [2025 DevPortal Awards results](https://pronovix.com/articles/best-developer-portals)
  selected North Developer as Best Overall, Best Onboarding, and Best Solution
  Portal. This is professional-jury evidence rather than mass-user evidence.

### Evidence standard

Each case study must distinguish:

- direct observation of the current website and representative user journeys;
- developer survey or task-completion evidence, including sample, date, and
  self-selection limits;
- reproducible benchmark results and what proportion of the score actually
  measures documentation;
- independent jury or professional-review evidence;
- scale, contribution, maintenance, and versioning signals; and
- community reputation or anecdote, clearly labelled as weaker evidence.

Retain dated links and captures for material decisions because these websites
change. Prefer primary research and published methodologies. Popularity,
traffic, GitHub stars, visual polish, and a project's overall developer
satisfaction are not substitutes for documentation usability.

### Common assessment rubric

Assess every site against the same questions so the review does not become a
collection of unrelated impressions:

1. Who are the intended audiences, what prior knowledge is assumed, and which
   user jobs receive an explicit path?
2. How are tutorials, how-to guides, explanation, reference, examples,
   troubleshooting, release notes, and migration guidance separated and
   connected?
3. How quickly can a new user reach a verified first success, and what setup,
   authentication, configuration, or conceptual friction occurs first?
4. How effective are navigation, search, page hierarchy, cross-linking,
   progressive disclosure, and orientation within a large documentation set?
5. Are examples complete, runnable, copy-safe, version-correct, multi-language
   where appropriate, and paired with expected output and failure cases?
6. How are versions, deprecations, compatibility, insecure releases,
   migrations, and behavioural changes made visible?
7. How closely are authored documentation, generated API reference, source,
   schemas, diagnostics, tests, and actual product behaviour kept in sync?
8. How well do error messages, diagnostics, troubleshooting paths, and support
   escalation lead a user from failure to recovery?
9. What feedback, contribution, editorial review, localisation, accessibility,
   mobile, performance, print, and offline mechanisms exist?
10. What machine-readable contracts, stable anchors, plain-text/Markdown
    representations, structured examples, and bounded LLM/agent retrieval paths
    exist without making AI output the authority?
11. Which trust signals expose provenance, security boundaries, known limits,
    ownership, freshness, and the distinction between normative and community
    content?
12. What maintenance team, workflow, tooling, telemetry, and ongoing cost are
    required to preserve the characteristic being considered?

Each review must include at least one realistic task completed from a clean
starting point. Record time to first useful result, wrong turns, search terms,
pages visited, unresolved questions, and whether the documentation or another
resource supplied the decisive answer.

### Required outputs

- one dated teardown per benchmark, using the common rubric and preserving
  evidence strength and limitations;
- a cross-case characteristic matrix that separates broadly reusable patterns
  from patterns dependent on the benchmark's domain, team size, or budget;
- a decision record for every characteristic adopted, adapted, deferred, or
  rejected, including maintenance cost and the user need it serves;
- a proposed public-site audience map, information architecture, content model,
  search/versioning strategy, contribution model, accessibility baseline, and
  machine-readable/agent access contract;
- low-fidelity prototypes for the home/get-started path, tutorial, how-to,
  concept page, reference page, diagnostic page, security/assurance evidence,
  migration guide, and release/version page; and
- task-based tests with representative first users before committing to the
  production visual system or website implementation.

The website must present generated language contracts, policy/security
evidence, examples, and human-authored explanation as distinct content types
with visible provenance. The compiler and versioned source artifacts remain
semantic authorities; the public site must not create a second hand-maintained
definition of language behaviour.

**Exit gate:** every primary case study has a completed evidence-backed
teardown; complementary reviews cover every capability selected for the site;
the characteristic matrix and decisions are reviewed; representative users can
complete the agreed discovery, first-success, reference, troubleshooting,
version-selection, and trust-verification tasks; and the approved information
architecture and content contracts are traceable to user needs and maintained
sources. Only then should production website design and implementation begin.

## 9. Scope guard

Until the current phase or an explicit workstream schedules them, do not
prioritise:

- package management or a registry;
- a full LSP or editor marketplace release before the P10 semantics are stable;
- native generated binaries;
- optimisation;
- macros or general metaprogramming;
- a broad standard library;
- multiple deployment targets;
- formatter polish beyond deterministic test formatting;
- modules beyond the initial global application namespace;
- services, jobs, events, and database syntax not needed by the current phase.

Work outside this guard requires an explicit roadmap change because it delays
the semantic proof.

The thin DX0 highlighting baseline and DX0.5 checked local-development loop are
exempt from this guard because they have an explicit workstream and cannot
define semantics. DX0.5 must call the existing compiler pipeline rather than
introducing watch-only language behaviour.

## 10. Progress update procedure

Every implementation change should update this document when it changes phase
evidence or status:

1. link the new artifact or test evidence;
2. check it against the phase exit gate;
3. update the summary status;
4. identify one concrete next action;
5. append a dated entry to the log below;
6. record any newly discovered language ambiguity in the issue log.

A phase may move backwards if a golden application invalidates its assumptions.

## 11. Progress log

### 2026-09-25 — P0 complete

- Consolidated the source conversation into the documentation set.
- Accepted nominal semantic and field types.
- Accepted validated `Type(value)` construction.
- Accepted the standard-kind failure architecture and safe disclosure model.
- Added normative type-system and failure-model acceptance cases.

### 2026-09-25 — P1 complete; P2 started

- Added the core grammar used by the first compiler slice.
- Added the canonical Jadpo seed application and expected semantic facts.
- Added the living language issue log.
- Deferred broader syntax explicitly rather than encoding it as pseudocode.
- Began P2 with the initial ten-fixture conversion target.

### 2026-09-25 — P2 and P3 complete; P4 started

- Converted the first ten type/failure cases into paired `.jadpo` and
  `.expect.json` fixtures.
- Validated JSON, source/expectation pairing, and unique expected source spans.
- Added the Rust workspace with syntax, semantic, diagnostic, compiler, and CLI
  crates without external dependencies.
- Added recursive `.jadpo` discovery and deterministic scaffold inspection.
- Added stable diagnostics for front-end and build phases not yet implemented.
- Verified the workspace with four passing unit tests and CLI smoke checks.

### 2026-09-25 — P4 lexer complete

- Added core token kinds, retained trivia, and UTF-8-safe byte ranges.
- Added lexical recovery and stable diagnostics for invalid escapes,
  unterminated strings, and unexpected characters.
- Verified clean lexing for the Jadpo seed and all ten compiler fixtures.
- Increased the passing Rust test count from four to nine.
- Tightened the core expression grammar to one qualified-name/postfix path
  before parser implementation.

### 2026-09-25 — P4 complete; P5 started

- Added the span-preserving typed AST and recovering recursive-descent parser.
- Added an exact declaration-outline snapshot for the Jadpo seed.
- Parsed every initial compiler fixture through its intended syntax node.
- Preserved arbitrary `throw` as an unsupported node with the stable
  `SYN_UNSUPPORTED_THROW` diagnostic.
- Demonstrated recovery across multiple errors in one malformed source file.
- Replaced the CLI's front-end placeholder with a real syntax-checking path.
- Increased the passing Rust test count from nine to fourteen.
- Opened P5 with parsed-project boundaries ready for declaration lowering.

### 2026-09-25 — P5 complete; P6 started

- Added a deterministic application-wide declaration index and explicit
  prelude catalogue.
- Made record and failure fields first-class semantic nodes using their
  `Type.field` or scoped failure identity.
- Resolved declaration-level type, failure, route, and callable references.
- Emitted nominal refinement and callable-dependency edges.
- Added spanned diagnostics for duplicate, unknown, wrong-kind, and
  non-callable names.
- Upgraded `jadpo inspect` from a file list to the semantic graph manifest.
- Upgraded `jadpo check` to gate syntax and declaration semantics.
- Proved the seed's fourteen authored nodes and eight refinement edges and
  indexed every syntax-valid initial fixture.
- Increased the passing Rust test count from fourteen to eighteen.

### 2026-09-25 — P6 complete; P7 started

- Added expression environments, typed field selection, and nominal
  compatibility over refinement paths.
- Enforced sibling separation, no implicit narrowing, and no semantic-to-
  primitive unwrapping.
- Added validated constructors and compile-time literal constraint checks.
- Added complete record construction, `optional`, nullable values, `none`, and
  invariant collection arguments.
- Enforced semantic application signatures instead of raw primitives.
- Expanded the executable fixture suite from ten to sixteen cases.

### 2026-09-25 — P7 and Milestone A complete; P8 started

- Added the standard failure-kind catalogue and HTTP mappings.
- Added failure contracts with explicit public/internal disclosure schemas.
- Enforced direct and transitive failure propagation and typed rejection
  payloads.
- Added required-context and duplicate-code diagnostics.
- Enforced the initial function/action effect boundary.
- Derived route failure responses from action contracts.
- Expanded `jadpo inspect` with inferred expression types, failure
  contracts, disclosure fields, and route mappings.
- Expanded the fixture suite to twenty-one paired cases.
- Verified twenty-four Rust unit tests plus all documentation tests.
- Completed the first semantic compiler milestone and opened P8.

### 2026-09-25 — Project structure workstream added

- Added project-layout standardisation as an explicit P8–P12 workstream.
- Recorded the goal of reducing navigation and low-value structural decisions
  for both developers and LLMs.
- Kept enforcement open pending concrete candidate trees and pressure tests.
- Required the P8 proposal to distinguish authored, generated, test, local,
  secret, and deployment material.
- Added staged enforcement criteria and explicit small, large, multi-domain,
  integration-heavy, and multi-application cases.

### 2026-09-25 — Deterministic scaffolding added to the workstream

- Selected a static canonical base as the initial scaffolding hypothesis.
- Required human prompts and LLM creation to map to the same finite command
  options and produce identical output.
- Deferred dynamic application profiles until repeated project shapes justify
  them.
- Defined future capabilities as additive packs that cannot rearrange the base
  architecture.
- Required scaffold version and creation recipe metadata for reproducibility
  and safe upgrades.

### 2026-09-25 — Nested structured-field implementation gap closed

- Made selection through a structured field traverse its declared named record
  rather than looking for a synthetic path-dependent record shape.
- Preserved the nested field's declared nominal identity: selecting
  `customer.billing_address.postal_code` now infers `Address.postal_code`.
- Added a dedicated diagnostic for selecting through a nullable structured
  field before handling `none`.
- Added fixture pair 22 with inferred-type and refinement assertions, plus
  fixture pair 23 for nullable-selection rejection and its exact primary span.
- Kept the verified suite at twenty-four Rust unit tests plus all documentation
  tests.

### 2026-09-25 — Comparative falsification protocol strengthened

- Required behaviourally equivalent language and TypeScript applications to
  follow the same acceptance suite and ordered change sequence.
- Added per-change capture of tokens, context, time, attempts, diagnostics,
  defects, interventions, source changes, workarounds, and escape hatches.
- Required every compiler-friction incident to be classified and given an
  explicit disposition, with language changes entering the issue and fixture
  process before retry.
- Made an honest qualitative review and the possibility of a TypeScript win,
  framework pivot, redesign, or stop decision part of the P12 exit gate.

### 2026-09-25 — P8 complete; P9 next

- Added `jadpo artifacts` with seven versioned JSON outputs beneath the
  compiler-owned `build/` boundary.
- Derived route/callable inventories, failure disclosure audit, validator plan,
  public-code compatibility snapshot, and the seed's OpenAPI 3.1 subset.
- Normalised metadata source paths so relative and absolute invocations are
  byte-identical, and excluded `build/` from source discovery.
- Added `jadpo new` with a deterministic compiling health-route scaffold,
  reproducible recipe metadata, and overwrite protection.
- Documented three concrete layout candidates and selected the elastic
  single-application tree for P9–P11 pressure testing.
- Increased the verified suite from twenty-four to thirty-one Rust unit tests,
  plus all documentation tests.

### 2026-09-25 — P9 and Milestone B complete; P10 next

- Upgraded `jadpo build` from a placeholder to checked TypeScript/Bun target
  generation while retaining the P8 artifact set.
- Generated dependency-free semantic validators, closed record boundaries,
  action execution, exact output validation/serialisation, request IDs, route
  dispatch, domain-failure mapping, and generic fault containment.
- Kept input validation local to the request boundary so output contract defects
  become contained 500 faults rather than misleading client 400 responses.
- Rejected authenticated target generation explicitly until the P11 runtime
  exists; no route is silently weakened to public access.
- Added six real HTTP acceptance cases for success, malformed email, constrained
  invite codes, malformed JSON, unknown fields, automatic 422 mapping, and
  internal-context non-disclosure.
- Verified thirty-three Rust unit tests, all documentation tests, Clippy with
  warnings denied, Bun target bundling, and six Bun HTTP tests.
- Completed the first runnable compiler milestone.

### 2026-09-25 — P10 typed create slice running

- Added `create Entity { ... }` to the lexer, parser, syntax tree, nominal type
  checker, effect checker, semantic traversal, and Bun target generator.
- Required entity-field identities at persistence construction and inferred the
  created entity as the expression result.
- Added `EFFECT_FUNCTION_PERSISTENCE`; persistent writes are action-only rather
  than silently permitted in pure functions.
- Generated deterministic entity metadata, matching Postgres and SQLite fresh
  schemas, `$n`/`?n` parameterised inserts, and a Bun SQLite adapter.
- Validated every `INSERT ... RETURNING` row as its entity before returning it
  to application code.
- Added the persistence seed, fixture pairs 24–25, and two real HTTP/SQLite
  acceptance cases proving invalid requests write nothing and valid rows round
  trip through storage.
- Added a dual generated adapter that selects PostgreSQL from `DATABASE_URL`
  and otherwise uses SQLite, without changing the source-language operation.
- Ran the PostgreSQL path against a fresh Postgres 16 cluster and added two
  equivalent HTTP/database acceptance cases.
- Verified thirty-four Rust unit tests and ten Bun HTTP tests for the create
  slice before beginning typed reads.
- Added `query optional Entity { where: field == value }` with nominal predicate
  typing, `Entity?` inference, action-only effect enforcement, and an explicit
  rejection for nullable predicate fields until null matching is specified.
- Generated parameterised `SELECT ... LIMIT 2` operations for SQLite and
  PostgreSQL, validated a single selected row, returned `none` for no row, and
  contained duplicate-row cardinality violations as operational faults.
- Added fixture pairs 26–28 and six cross-adapter HTTP/database cases covering
  no row, one row, and duplicate rows. The verified runtime suite is now sixteen
  Bun tests.

### 2026-09-25 — Developer tooling workstream added

- Made IDE support part of the product hypothesis rather than post-launch
  polish, with a compiler-backed LSP as the single semantic implementation.
- Staged lexical highlighting, navigation/hover/completion, formatting and code
  actions, renderer adapters, and agent-facing retrieval separately.
- Required a stable machine language identifier across file associations,
  Markdown fences, LSP, and grammar packages without forcing an early product
  name.
- Separated model output from client rendering: agents can emit correctly
  tagged fenced code, while highlighting depends on the host installing or
  recognizing the grammar.
- Added a P12 tooling freeze gate so the language-versus-TypeScript comparison
  records IDE, formatter, renderer, and agent-context support on both sides.

### 2026-09-25 — Checked local development loop scheduled

- Added DX0.5 during P10.5 as a required gate before P11 application authoring,
  so the golden todo is built through the intended human/agent feedback loop.
- Scheduled versioned JSON diagnostics, `jadpo watch`, and `jadpo dev`
  over the same authoritative compiler pipeline used by one-shot commands.
- Required atomic generated-output promotion, last-known-good serving during
  invalid edits, visible stale-revision state, readiness-gated Bun activation,
  automatic restart/reload after successful builds, and clean shutdown.
- Required protocol tests for valid/invalid/recovered edits, coalesced events,
  source creation/deletion, target and startup failures, continuity, ignored
  compiler-owned output, and child-process cleanup.
- Bound the later VS Code client and DX1 LSP to the same diagnostics and
  lifecycle state rather than terminal-text scraping or editor-only semantics.

### 2026-09-25 — Generated runtime dependency closure enforced

- Promoted “dependency-free Bun target” from a description to an accepted
  compiler contract: generated imports are limited to `bun`, `bun:*`, and
  compiler-owned relative TypeScript modules.
- Added `JADPO_TARGET_EXTERNAL_MODULE` and
  `JADPO_TARGET_DEPENDENCY_MANIFEST` generation guards so a future target change
  cannot silently introduce a registry dependency, package manifest, lockfile,
  or `node_modules` tree.
- Kept the deterministic scaffold free of package metadata and added a focused
  regression test for both rejected dependency forms.
- Verified thirty-five Rust unit tests, strict Clippy, and generated Bun
  bundling/HTTP execution with auto-install disabled. No `bun install` step is
  part of the project lifecycle.

### 2026-09-25 — Route item separators made consistent

- Standardised route metadata on the field-style `name: value` form: `auth:`,
  `input:`, `output:`, and `run:`.
- Updated canonical applications, compile fixtures, scaffold output, grammar,
  syntax examples, and failure-model documentation.
- Made the parser reject the former space-only spelling with a focused
  diagnostic regression test.
- Verified thirty-six Rust unit tests, formatting, and strict Clippy.

### 2026-09-25 — Required lookup and persistence fault boundary

- Added `query required Entity { ... missing: FailureName }` with a non-nullable
  result type and checked optional public/internal failure context.
- Required the missing failure to be declared by the enclosing action and to
  derive from `NotFound`; absence now becomes a typed rejection, never an
  untyped exception.
- Added three compile fixture pairs covering the valid form, a wrong failure
  kind, and undeclared propagation.
- Normalised SQLite and PostgreSQL driver exceptions into a compiler-owned
  `PersistenceFault` that retains the raw exception only as its cause.
- Added required-query and real driver-failure runtime coverage to both storage
  adapters while retaining dependency-free Bun output.
- Verified thirty-seven Rust tests, strict Clippy, dependency-free no-install
  bundles, and twenty-two generated Bun HTTP/database tests across SQLite and
  PostgreSQL.

### 2026-09-25 — Transactional typed update and delete

- Added `update required` and `delete required` expressions returning the
  validated affected entity rather than a row count.
- Required explicit `missing:` (`NotFound`) and `conflict:` (`Conflict`)
  bindings with ordinary declaration, propagation, context, and type checks.
- Limited the first update slice to one nominally typed field and added five
  fixture pairs covering valid mutations, update shape, and failure kinds.
- Generated parameterised SQLite/PostgreSQL update and delete statements plus
  deterministic mutation metadata.
- Preflighted up to two rows and executed mutations inside adapter-native
  transactions so multiple matches roll back without partial writes.
- Classified native SQLite and PostgreSQL constraint codes at the adapter
  boundary and converted update violations into the declared domain conflict.
- Verified thirty-eight Rust tests, strict Clippy, dependency-free no-install
  bundles, and thirty-six Bun HTTP/database tests across SQLite and PostgreSQL.

### 2026-09-25 — Relationship and joined-loading workstream added

- Promoted entity relationships from a generic deferred concern to an explicit
  P10/P11 deliverable, using `User` with ordered `Todo` children as the golden
  pressure case.
- Required nominal references, generated foreign keys/indexes, explicit delete
  lifecycle, typed nested result shapes, and bounded eager loading without
  hidden lazy queries.
- Required the compiler to choose and expose a join or bounded batched plan,
  preserve parent pagination, validate every row, and avoid Cartesian
  multiplication for multiple to-many loads.
- Added cross-adapter evidence for zero/one/many children, relationship
  integrity, deterministic ordering, lifecycle behaviour, nested JSON parity,
  and a query-count guard against N+1 regressions.
- Made policy composition across parent and child relationships a P11 security
  gate rather than assuming a database join is authorization-safe.

### 2026-09-25 — Entity identity, uniqueness, and indexes implemented

- Accepted postfix `identity`, `unique`, and `index` modifiers on entity fields
  while keeping storage properties out of nominal value compatibility.
- Added semantic rejection for persistence modifiers outside entities,
  nullable identity fields, and multiple identity fields on one entity.
- Generated stable named primary-key and unique constraints plus idempotent
  lookup indexes for both SQLite and PostgreSQL, including runtime schema
  initialisation and persistence-manifest metadata.
- Added an `Account` pressure entity and SQLite evidence that identity and
  unique duplicates fail while the declared non-unique index exists.
- Added an optional postfix `conflict:` binding to `create`, checked it against
  the action failure set and `Conflict` kind, and proved an HTTP 409 mapping
  without exposing the underlying driver constraint.
- Preserved the dependency-free target contract: the generated TypeScript uses
  only Bun's built-in database clients and requires no installation step.

### 2026-09-25 — Owning entity relationships and foreign keys implemented

- Accepted `references Target.field on_delete restrict|cascade|set_null` on an
  owning entity field, with an explicit lifecycle required in source.
- Required the owner to carry the exact nominal target-field type and the
  target to be a non-nullable identity or unique key; nullable-only `set_null`
  is checked statically.
- Generated stable named foreign keys and automatic owner-field indexes for
  SQLite/PostgreSQL, enabled SQLite foreign-key enforcement, and ordered fresh
  table creation so referenced entities exist before dependants.
- Diagnosed cross-entity dependency cycles that the fresh-schema generator
  cannot safely order rather than emitting broken SQL.
- Added the `User`–`Todo` pressure case and executable SQLite evidence for
  orphan rejection, owner indexing, and cascade deletion. Equivalent live
  PostgreSQL assertions are checked in when `DATABASE_URL` is available.

### 2026-09-25 — Ordered many-result queries implemented

- Accepted `query many` with a bound equality predicate and mandatory explicit
  `order_by: ... asc|desc` syntax.
- Required an identity/unique ordering key so collection order is deterministic
  without relying on adapter row order or an unstated tie-breaker.
- Generated parameterised SQLite/PostgreSQL many-result statements, exposed
  them in persistence metadata, and validated every returned entity.
- Added `List<T>` TypeScript generation and collection validation without any
  runtime dependency.
- Proved the `User`–`Todo` route returns todos in stable identity order on
  SQLite; the PostgreSQL suite contains the equivalent live assertion.

### 2026-09-25 — Relationship declaration syntax revisit queued

- Recorded the current owning-reference spelling as provisional rather than a
  stable language commitment.
- Scheduled a syntax comparison after inverse relationships and the joined-load
  pressure case reveal the full requirements.
- Kept ownership, cardinality, nominal target identity, lifecycle, inverse
  naming, composite-key viability, and IDE/LLM readability as explicit review
  criteria.

### 2026-09-25 — Explicit inverse loading implemented

- Accepted `inverse todos: many Todo via Todo.owner_id` as the first named
  parent-side relationship declaration, checked against the stored owning
  reference rather than treating it as a persisted field.
- Accepted one explicit required-parent include with a named output shape and
  mandatory identity/unique child ordering.
- Selected a bounded two-query batch for the first pressure case and exposed
  its strategy, cardinalities, query count, ordering, and SQL in persistence
  metadata.
- Validated the parent and every child before constructing nested output;
  zero children become an empty list and a missing parent maps to the declared
  typed failure.
- Added SQLite runtime coverage for zero/one/many children, cross-user
  isolation, missing parents, and the two-query plan. The PostgreSQL suite
  carries the equivalent nested result checks for a configured live database.

### 2026-09-25 — Explicit relationship pagination implemented

- Required every included to-many relationship to declare a positive literal
  `limit` and non-negative literal `offset` after its deterministic ordering.
- Rejected dynamic, zero, and negative bounds in the first slice so query cost
  remains statically visible and reviewable.
- Lowered both bounds to parameters in SQLite and PostgreSQL rather than
  interpolating them into generated SQL.
- Added runtime evidence that adjacent pages return the expected ordered child
  rows and metadata evidence for `LIMIT $2 OFFSET $3` query planning.

### 2026-09-25 — Parent-first paginated relationship joins implemented

- Accepted one inverse include on `query many` only when parent and child
  ordering and pagination are both explicit and statically bounded.
- Generated a parent-page CTE before child expansion, then ranked children per
  parent and applied each child window before a single left join.
- Regrouped flat adapter rows into `List<OutputShape>`, preserving parents with
  zero children and validating every nested value at the application boundary.
- Exposed `strategy: parent_page_join`, `query_count: 1`, the
  parent-before-join guarantee, adapter SQL, and all parameter positions in the
  persistence manifest.
- Added SQLite evidence that a parent with more children than its child limit
  does not consume another parent's page slot, plus stable adjacent parent-page
  behavior. Equivalent PostgreSQL coverage is checked in for a configured
  database.

### 2026-09-25 — Bounded multiple relationship includes implemented

- Changed query syntax and the AST from one optional include to repeated
  includes that must name one exact output shape.
- Rejected duplicate relationships, mixed result shapes, missing relationship
  fields, and independently invalid ordering or pagination.
- Lowered required-parent multiple includes to one parent query plus one
  bounded child query per relationship.
- Lowered paginated many-parent multiple includes to independent parent-page
  joins merged by the unique parent ordering key, avoiding Cartesian row
  multiplication and N+1 execution.
- Added `User`–`Todo`–`Note` SQLite/PostgreSQL pressure cases and plan metadata
  with exact query counts and `cartesian_product_avoided: true`.

### 2026-09-25 — Atomic fixed-shape multi-field updates implemented

- Allowed `update required` to name one or more distinct fields in `set:` and
  retained nominal type checking for every replacement.
- Rejected duplicate fields explicitly while keeping omission-aware dynamic
  patch selection outside this fixed-shape slice.
- Lowered every authored field set to one parameterised
  `UPDATE ... SET ... RETURNING` statement and recorded the exact SQLite and
  PostgreSQL SQL in persistence metadata.
- Added an `Account` pressure route proving two fields update together and that
  a uniqueness conflict leaves both previously stored values unchanged.
- Verified 51 Rust tests, strict Clippy, deterministic regeneration, and 29
  generated Bun HTTP/database tests against SQLite. The equivalent PostgreSQL
  assertions are checked in pending a supplied live `DATABASE_URL`.

### 2026-09-25 — Named compound constraints and precise conflicts implemented

- Added `constraint name: unique(field_a, field_b)` to entities, rejecting
  unknown, repeated, nullable, undersized, or duplicate field sets.
- Emitted stable named compound uniqueness for SQLite and PostgreSQL and
  recorded authored names plus ordered fields in persistence metadata.
- Allowed mutations to map several compiler-owned constraint identities before
  one optional fallback; duplicate and unknown mappings fail compilation.
- Normalised native identity, single-field unique, and compound-unique failures
  to the same safe logical names across both adapters without exposing raw
  database identifiers to application logic.
- Proved SQLite distinguishes `Account.handle` from
  `Account.tenant_owner`, returns their separate typed conflicts, and rolls
  back every other field in the failed update.
- Verified 53 Rust tests, strict Clippy, deterministic regeneration, and 29
  generated Bun HTTP/database tests. Equivalent PostgreSQL assertions are
  checked in pending a supplied live `DATABASE_URL`.

### 2026-09-25 — P10 persistence core complete

- Ran all 22 generated PostgreSQL acceptance cases against an isolated fresh
  Postgres 16 cluster, covering typed CRUD, relationships, bounded repeated
  includes, exact cardinality, inferred action rollback, multi-field atomicity,
  constraint-specific conflicts, fallback conflicts, and raw driver failures.
- Reproduced a Bun 1.2.20 PostgreSQL transaction defect where prepared/pipelined
  constraint failures rolled back on the server but left `SQL.begin()`
  unresolved. Generated PostgreSQL clients now use `prepare: false`; parameter
  binding remains intact while transactions complete and release reliably.
- Made the live test setup repeatable by removing its test-only partial unique
  index before each suite run.
- Verified 53 Rust tests, strict Clippy, 29 generated Bun/SQLite HTTP/database
  tests, and 22 live PostgreSQL HTTP/database tests.

### 2026-09-25 — Query clauses made structurally consistent

- Changed query, update, and delete syntax to consistently label clause values:
  `where:`, `order_by:`, `limit:`, `offset:`, `include:`, and `into:` now match
  the existing `set:`, `missing:`, and `conflict:` convention.
- Made the colon mandatory rather than retaining two accepted spellings, with a
  targeted missing-colon parser diagnostic for every clause label.
- Updated the grammar, design examples, persistence seed, compile fixtures, and
  generated persistence target.
- Verified 55 Rust tests, strict Clippy, and 29 generated Bun/SQLite
  HTTP/database tests.

### 2026-09-25 — Typed configuration and deployment validation queued

- Added a P11/P12 workstream for one typed configuration contract shared by
  source checking, generated documentation, IDE/LLM context, CI, runtime, and
  deployment systems.
- Separated static compiler checks from environment-bound preflight so normal
  source analysis does not require production secrets while an actual revision
  cannot start with absent or malformed values.
- Required secret-safe provenance and diagnostics, explicit environment
  overlays, no production fallback to development defaults, and an explicit
  restart-versus-reload decision for rotation.
- Split liveness from readiness and required bounded dependency capability
  checks without exposing configured values.
- Defined rollback as a deployment-controller action driven by deterministic
  preflight/readiness evidence, with an executable no-promote/rollback test.

### 2026-09-25 — Assurance and validation reset inserted

- Recorded that compiler and persistence construction overtook the charter's
  requirement to let complete golden programs force the semantic model first.
- Reclassified completed compiler work as exploratory implementation evidence,
  not validation of the language or product thesis.
- Capability-froze P10 and inserted mandatory P10R gates for the complete todo
  design, policy/proof kernel, threat model, independent human approval,
  first-user hypothesis, preregistered comparison thresholds, and documentation
  authority.
- Required repeated counterbalanced TypeScript/language trials, independent
  adjudication, explicit critical-safety and valid-work-friction thresholds,
  and a recorded continue/redesign/framework-pivot/stop decision.
- Defined human approval as a research problem in behavioural comprehension,
  not an assumption that conventional pull-request code review remains viable;
  P10R must compare code diffs with relationship- and effect-aware decision
  views and measure false confidence as well as review accuracy.
- Recorded the custom web UI as the primary opportunity for intent-first,
  interactive semantic-graph, scenario, effect, and per-decision review, while
  keeping all claims reproducible from compiler-owned artifacts.
- Preserved the planned technical scope; the correction changes evidence order
  and decision quality rather than reducing the intended backend domain.
- Added the candidate `todo-v0.1` source/policy contract, 39 black-box cases,
  adversarial sequence, expected audit, TypeScript baseline, and explicit
  language-friction ledger without claiming current compiler support.
- Added candidate proof, threat, independent-approval, research-user, and
  comparison protocols plus deterministic package/link/digest verification.
- Ran an author-side contradiction pass that corrected policy authority,
  overdue-reminder semantics, due-date rescheduling, relationship lifecycle
  coverage, application-owned identity data, deployment-plane readiness, and
  explicit success status behaviour. Independent review remains pending.
- Added omission-aware `patch:` syntax, semantic and failure checks, generated
  supplied-flag SQL plans, manifest evidence, four compiler fixtures, and an
  end-to-end SQLite test covering empty, omitted, explicit-`none`, and supplied
  fields. Also preserved nullability through qualified field references.
- Added atomic patch-derived `set:` writes with the narrow
  `value when patch.field supplied` rule, rejected patch/derived overlap, added
  positive and negative compiler fixtures, and proved at runtime that omission
  preserves the derived column while the triggering field clears it.
- Added explicit owning-parent includes for required-child queries with
  `required`/`optional` cardinality, exact nested output checking, a bounded
  two-query generated plan, persistence-manifest evidence, compiler fixtures,
  and SQLite coverage for required plus null/populated optional references.
- Added unique-backed `inverse ...: optional ... via ...` declarations,
  rejected non-unique owning fields, generated bounded optional-inverse plans,
  and proved absent, present, and duplicate-child behavior on SQLite.
- Added a depth-two `include: owner.profile optional` path with exact nested
  output checking, a fixed three-query plan, maximum-depth metadata, a compiler
  fixture, and SQLite evidence for absent and present leaves.
- Reviewed owning-reference surface syntax and implemented the explicit
  `references User.id as owner on_delete ...` form. Generated schemas retain
  `owner_id`, while semantic metadata, direct/nested includes, result fields,
  and bounded plan names use `owner`; fixture 50 and SQLite runtime coverage
  exercise the separation without suffix inference.
- Closed the remaining relationship-shape review without inventing false
  guarantees: required inverse totality remains rejected, nested collections
  require future per-hop bounds, many-to-many uses an explicit join entity, and
  recursive paths stay depth-bounded. Drafted the checked-in, rename-stable
  schema identity registry contract as the next implementation boundary.
- Implemented the schema identity registry boundary: deterministic
  non-overwriting initialization, canonical parsing, validation during normal
  compiler commands, addition-only registration, and explicit entity/field
  renames that retain immutable IDs, physical names, and prior paths. Manual
  drift and additions mixed with removals fail closed.
- Added immutable schema identity snapshots and deterministic identity-only
  change sets for additions, removals, logical/physical renames, and
  incompatible kind/owner changes. Output is explicitly marked
  `migration_plan: false`; no SQL or lifecycle approval is inferred.
- Extended immutable snapshots with nominal field types, nullability,
  persistence flags, intrinsic constraints, relationship target/name/delete
  lifecycle, and constraint/index members. Diffs now distinguish required from
  nullable additions and conservatively route every field-shape change to an
  existing-data decision.
- Added the fail-closed decision-requirement boundary. Existing-data and
  lifecycle blockers now expose typed unresolved requirements, admissible
  strategies, and mandatory evidence slots; reference delete-action changes
  are classified separately. No requirement is treated as a decision or
  approval, and migration planning remains false.
- Added non-overwriting authored decision templates bound to the complete
  canonical change set. Validation rejects stale bindings; missing, duplicate,
  or unexpected decisions; inadmissible strategies; and missing, unexpected,
  duplicate, or empty evidence. Passing validation proves completeness only and
  continues to emit `migration_plan: false`.
- Added immutable ordered PostgreSQL/SQLite migration review plans gated on
  successful decision validation. Plans distinguish adapter operations,
  classify rollback as automatic, conditional, or unavailable, and index every
  irreversible step. Evidence remains uncompiled, so plans are explicitly
  non-executable and contain no SQL.
- Added deterministic static query-backed index advice. Normal checking warns
  when recommendations exist; the evidence view records the callable and
  predicate/ordering role. Explicit acceptance updates authored source,
  rechecks it, registers the new index identity, and restores source if the
  operation fails. Existing identity, unique, explicit, and reference-backed
  indexes are suppressed; workload statistics and compound/partial advice
  remain future adapter work.
- Added the first reviewed SQL subset without creating an execution path.
  Primitive `literal(...)` backfills are checked against field types;
  identifiers/text are escaped; nullable additions emit forward/rollback SQL
  on both adapters; and required additions with backfill emit the PostgreSQL
  add/backfill/not-null sequence. Unsupported rebuild, transform, removal, and
  lifecycle paths fail closed.
- Added a bounded SQLite required-field rebuild for one primitive addition on
  an unrenamed table. The review contains transactional shadow-table creation,
  typed copy/backfill, table swap, rollback reconstruction, and foreign-key
  integrity checks. Existing scalar references retain their target and delete
  action; their generated lookup indexes, explicit indexes, and named compound
  uniqueness that excludes the new field are recreated. Multi-change rebuilds,
  constraints on the new field, and unsupported table shapes still fail
  closed. The forward and rollback sequences also pass a real SQLite smoke
  execution with preserved data and schema metadata.
- Extended the bounded SQLite review generator to coalesce compatible field
  additions by table. One shadow-table copy can now add multiple required
  primitive fields with independently checked literal backfills together with
  nullable fields populated as `NULL`; rollback removes the complete addition
  set in one reconstruction. Multiple affected tables remain deterministic
  units whose rollback order reverses the forward order. Added-field
  constraints, transforms, removals, and lifecycle changes still fail closed.
- Classified required-to-nullable field changes as safe nullability widenings
  rather than generic existing-data transformations. PostgreSQL review SQL now
  drops `NOT NULL` with a restoring rollback; SQLite rebuilds the current
  nullable table and reconstructs the prior required field on rollback. Both
  paths remain non-executable review artifacts, and narrowing or any concurrent
  incompatible SQLite change still fails closed.
- Added evidence-checked nullable-to-required narrowing. The authored decision
  must select `validate_existing` with the compiler-owned `not_null` predicate;
  arbitrary non-empty predicate claims and staged SQL requests fail closed.
  PostgreSQL emits a visible null-count review followed by enforcing `SET NOT
  NULL`; SQLite copies into a required shadow-table column, so remaining nulls
  prevent the rebuild. Both adapters emit a nullable rollback.
- Added the bounded authored module/import core. Explicit logical module
  headers, selective imports, and private-by-default/public declarations now
  produce manifest metadata and enforce cross-file visibility. Mixed
  ambient/modular projects, missing/private/duplicate/conflicting imports,
  duplicate modules, self-imports, and cycles fail with stable diagnostics. A
  two-file module seed passes real `check` and `inspect`; aliases, re-exports,
  relative imports, multi-file modules, external packages, and namespace-local
  duplicate names remain explicitly unsupported.
- Closed the P10.5 value/reference boundary with executable assignment syntax.
  `var mut` locals may be rebound only with values compatible with their
  established nominal type;
  parameters and ordinary `var` bindings reject assignment with
  `TYPE_ASSIGN_IMMUTABLE`. Assignment is local-slot rebinding, never field or
  caller-visible mutation. Fixtures 51–52 and generated TypeScript coverage
  preserve this contract, while `ref`/alias/address/`inout` syntax remains
  absent pending real application evidence.
- Began DX0.5 with the versioned machine diagnostic protocol. `jadpo check
  <project> --diagnostic-format=json` emits one deterministic version-1 report
  on success or failure, preserving diagnostic ordering, stable codes, severity,
  source byte ranges, notes, summary counts, and process exit status. Human
  output remains the default and both modes call the same checking pipeline.
- Added `jadpo watch` over the authoritative checked build. Stable content
  fingerprints cover authored sources and the schema identity registry while
  excluding generated/staging output; a 75 ms quiet window coalesces rapid
  saves. Human and versioned JSON events expose checking, success, failure,
  monotonic revisions, and stale retained output. Artifact generation now
  completes in a sibling staging tree and promotes only a complete revision,
  with the previous `build/` restored on promotion failure. Unit coverage proves
  generated-output exclusion and last-complete-build preservation; the real
  long-running JSON watcher also passes its initial-build smoke test.
- Added the first `jadpo dev` runtime loop. It validates `PORT`, starts Bun
  with dependency installation disabled, bounds readiness to a three-second
  localhost probe, preserves a ready server through invalid source revisions,
  restarts only after successful builds, and reports unexpected exit plus
  start/readiness/failure events. JSON runtime logs stay off the protocol stream.
  Bun 1.2.20 passed a real readiness event and HTTP 200 smoke request; Ctrl-C
  left no listener. Startup-failure restoration and portable structured
  shutdown remain explicit DX0.5 work.
- Closed the initial diagnostic/readiness presentation gap. Every generated
  target now supplies `GET /health` unless an authored public health route
  replaces it, and `jadpo dev` requires HTTP 200 from that endpoint rather
  than treating an open socket as ready. Human compiler diagnostics render the
  stable machine byte range as line/column, a bounded source excerpt, caret, and
  repair notes. Generated request/startup faults emit versioned compact JSON;
  raw target stacks are opt-in through `JADPO_DEBUG_TARGET_STACKS=1`.
- Completed the pre-P11 language-learning slice with plain closed enums
  and typed `match`. Enum variants are qualified nominal values, boundary
  validators reject unknown wire spellings, generated TypeScript uses string
  unions and `switch`, and OpenAPI plus validator plans publish the finite
  alternatives. Matches are exhaustive for enums and `Bool`; nullable `none`
  is explicit; open Text/numeric/value spaces require `_`; duplicate, unknown,
  unreachable, and incomplete patterns have stable diagnostics. Match subjects
  include child field selections and callable results. Data-carrying variants
  now use nominal payload fields, a validated closed `tag` representation,
  discriminated OpenAPI unions, checked construction, arm-local bindings, and
  exhaustive narrowing. Nullable matches support `some(value)` binding. The
  expression parser has fixed unary, arithmetic, ordering, equality, `and`, and
  `or` precedence with nominal operand checks. Top-level `test` plus Boolean
  `assert` compile to a structured `jadpo test` runner without target stack
  traces. `jadpo fmt` and read-only `--check` share deterministic formatting
  with the VS Code client. The standard `jadpo lsp` service now supplies live
  unsaved diagnostics, document/workspace symbols, cross-file definitions and
  references, inferred hover types, contextual completion, signature help,
  semantic tokens, compiler-indexed rename, and formatting without an editor-
  specific parser. Fixtures 53–58 cover the successful and principal language
  failure paths; protocol tests cover Unicode positions and cross-file imports.

### 2026-09-26 — Guided diagnostics and safe observability accepted

- Accepted one versioned compiler-owned diagnostic catalogue in place of
  scattered message strings and editor-specific advice.
- Selected readable lower-case dotted rule identifiers, human-first summaries,
  concise reasons, one recommended next step, bounded alternatives, explicit
  decision ownership, precise locations, bounded context/impact, help IDs, and
  revision-bound repair previews as the version-2 contract.
- Required IDEs to render the same semantic object through precise squiggles,
  plain Problems summaries, explanatory hover, a recommended verified Quick Fix
  followed by clean alternatives, multi-file diff preview, and expandable
  impact/help rather than raw JSON.
- Separated compiler diagnostics, public failure responses, operational events,
  and agent incident packets into non-interchangeable audience schemas.
- Required safe typed diagnostic/log values, non-renderable secrets, no
  assumption that internal context is loggable, and planted-secret tests across
  browser, log, trace, buffer, and third-party adapter outputs.
- Chose redacted structured/OpenTelemetry-compatible production events carrying
  stable semantic operation and source-revision IDs, with trusted local
  compiler-graph enrichment producing the rich runtime packet used by an LLM.
- Added DX2 and its catalogue, IDE, repair, disclosure, compatibility, and
  repair-cycle exit evidence before the P12 tooling freeze.

### 2026-09-26 — Problem, route, and entity boundary revision accepted

- Accepted an explicit `kind` member for reusable application failure
  declarations, while retaining stable public codes, safe messages, and
  declaration-owned public/internal context schemas.
- Selected flat failure construction with exact completeness, name, scope, and
  nominal-type checks at every production site.
- Expanded the planned callable contract from domain-only rejection to an
  exhaustive set of recoverable application and operational problems: each
  problem must be handled, mapped, or propagated in `fails`.
- Selected `attempt` as the single explicit propagation/handling mechanism;
  exact replacement-value syntax remains open.
- Selected source-agnostic operational categories rather than storage/provider-
  qualified names, with provenance retained in compiler/runtime metadata.
  `Unavailable`, `TimedOut`, `RateLimited`, and `OutcomeUnknown` are the initial
  candidates, and unknown write outcomes remain distinct from definite
  unavailability.
- Kept defects and impossible states outside catchable problem flow.
- Kept `route`, `action`, and `function` as distinct concepts while selecting a
  local-first route form: one-off behaviour is an inline brace-delimited action;
  reusable or independently meaningful behaviour is a named action invoked by
  `run:`. A route has exactly one of those forms.
- Made inline `fails` explicit in the action header, retained field-style route
  members, and rejected indentation-sensitive nesting and decorator syntax.
- Retained authentication-required-by-default and selected `auth: none` as the
  sole initial opt-out. Authentication remains separate from policy and from
  provider strategy configuration.
- Selected `{name}` path placeholders with a typed `path: { name: Type }` group,
  `path.name` access, and exact one-to-one compiler checks across multiple path
  parameters. Query, header, and body field spelling remains open.
- Accepted `entity` as identity-bearing domain data with optional explicit
  persistence, ordinary construction without a database, and entity-local
  access/field/lifecycle policy. Cross-cutting policy and human approval remain
  application-level concerns.
- Added P10.6 before P11 to implement and pressure-test the revised contracts
  without rewriting the already-recorded P7/P10 evidence.

### 2026-09-25 — Public website documentation benchmark queued

- Added a conditional public-facing website and documentation benchmark
  workstream for activation after the P12 continuation decision.
- Fixed the initial case-study set across Rust, Stripe, Django, ArchWiki,
  PostgreSQL, Godot, and Twilio, with complementary specialist and current
  developer-portal references.
- Defined a shared evidence standard, twelve-part assessment rubric, required
  task observations, cross-case decision artifacts, prototype coverage, and an
  exit gate that must pass before production website design begins.

## 12. Immediate next action

P10 is complete. The candidate P10R package now contains the canonical todo
source, black-box suite, adversarial sequence, expected audit, policy/proof
kernel, threat model, approval protocol, first-user hypothesis, comparison
protocol, TypeScript baseline, and language-friction ledger. Independently
review the package, resolve contradictions once, freeze artifact digests and
fixtures, conduct and record five first-user reviews, and make the P10R freeze
decision before any P11 assurance claim. By explicit project-owner direction,
the five sessions now follow feature-complete implementation; technical work
may continue through P11 and the P12 reference implementations but remains
exploratory until that evidence exists. Omission-aware patches and the
GF-013B patch-dependent reminder reset now have finite generated SQL shapes.
Direct optional/required owning-parent traversal now has a bounded generated
plan and runtime evidence, as does optional inverse-one loading and its bounded
depth-two composition. The owning-reference surface review selected an explicit
logical `as` name while preserving the stored key, and the remaining shape
review now has an explicit bounded disposition. Rename-stable identities,
shape-aware changes, exact-bound decisions, adapter review plans, the additive
SQL review subset, compatible reference/compound-preserving multi-addition
SQLite rebuilds, reversible nullability widening, and explicit query-backed
index acceptance, plus the bounded module/import and immutable-value/local-
rebinding cores are implemented. P10.5's bounded implementation is complete.
Module aliases, re-exports, relative imports, same-name namespaces,
caller-visible mutation, and authored reference notation remain unsupported
until golden-application evidence shows they are necessary; other migration
transforms continue to fail closed. The language-learning slice and initial
DX1 language service are complete. The next technical work returns to DX0.5
startup-failure restoration, portable structured shutdown, and the remaining
edit-recovery protocol cases, followed by the P10.6 problem, route, and entity
boundary revision before P11 application authoring.
