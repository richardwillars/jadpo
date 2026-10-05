# Unattended validation progress

**Authorized:** owner requested unattended execution after the semantic-area
agent proposal. Feature expansion and Wasm remain paused.
**Follow-up:** `jadpo-validation-waves`, this task, every 30 minutes while
packages are active. The three queued local waves, independent review and
bounded cold-start pilot are now complete; the follow-up is **paused** at
this verified checkpoint. Further feature/release work remains outside this
bounded test programme and must retain the decisions and external gates below.

## Operating rules

- Read accepted specifications before implementation; cite the contract for
  expected behaviour. Do not invent semantics or weaken policy/assertions.
- Use at most three workers and one integrating/reviewing parent. Reuse live
  agents rather than starting duplicate work. Assign disjoint files.
- Preserve existing uncommitted changes. Do not roll back someone else's work,
  stage the entire repository, or claim the existing mixed tree is a clean
  release baseline. No publishing/deployment or website changes.
- Agents write focused tests and findings; the parent reviews production fixes.
  Keep reproduced failures as regression cases. Run focused checks during work,
  then the common verification command after integration.
- `python3 tools/verify.py` is the supported-language gate. Golden compilation
  and all 44 behavioural obligations remain explicitly open; never convert a
  related unit test into a passing golden case.

## Work queue

| Wave | Area | State | Ownership / evidence |
|---|---|---|---|
| 1 | Lexer/parser/modules | complete for bounded wave | 29 new Rust tests; `syntax-findings.md` |
| 1 | Types/values/constraints | complete for bounded wave | 41 new core tests; `types-findings.md` |
| 1 | Failures/callables/disclosure | complete for bounded wave | 22 new core tests; `failures-findings.md` |
| 1 | Integration and review | passed | Nine runtime field-contract cases, nine verifier cases, five targeted mutations and full 30-step gate |
| 2 | Persistence/transactions | complete for bounded wave | Five Rust tests; ten runtime cases on each database; `persistence-findings.md` |
| 2 | Authentication/policy | complete for bounded wave | Seven Rust tests; seven runtime cases on each database; `auth-policy-findings.md` |
| 2 | Config/time/fixtures | complete for bounded wave | Eleven Rust tests; nine runtime cases; `config-time-findings.md` |
| 2 | Integration and review | passed | Four artifact cases, two secret-flow mutations and full 36-step gate |
| 3 | Targets/artifacts/tooling | complete for bounded wave | 11 artifact/runtime cases, eight tooling tests, five discovery tests, Unicode and source-location regressions |
| 3 | Independent challenge | passed | Two catalogue mutations and one migration mutation; independent config race review |
| 3 | Cross-feature closure | passed with named gaps | Fixture/migration isolation, 22 CONFIG/POLICY trigger tests, local/CLI operational tests and 12-obligation cold-start pilot |

Wave-two persistence priority: inherited nullable field references now have an
explicit resolved semantic representation. Audit SQL DDL, persistence manifests,
relationship handling and artifact field plans that still inspect only authored
`field_type.nullable` (`target.rs` and `artifacts.rs`). Cover nullable reference
chains with actual database round trips; do not count type tests as SQL proof.

## Baseline

The prior full supported run passed 198 Rust tests, 180 compile fixture pairs,
four editor tests, ten example builds, authored tests, 92 local runtime cases,
22 PostgreSQL persistence cases and 24 PostgreSQL-mode auth cases.
The first combined run exposed a previously skipped dev-runtime rollback test;
its fake Bun launcher and silent prerequisite skips were corrected.

`examples/golden-todo` still has 60 diagnostics and 44 unexecuted obligations.
Four auth contract conflicts and one query-count clarification are recorded in
`golden-obligations.json`. Service/JWT, full AUTH-P7/P8, source migration and
outside review are not completed by this unattended test effort.

## Completed wave-one checkpoint

Full supported verification passed at
`build/validation/20260929T235522-79715/report.json`: all 30 steps passed,
290 Rust tests, 180 compile fixtures, four editor tests, ten example builds,
authored tests, 101 local runtime cases, 22 PostgreSQL persistence cases and
24 PostgreSQL-mode authentication cases. Nine Python verifier-contract tests
also pass. Golden remains 60 diagnostics and all 44 obligations unexecuted;
the report explicitly says `supported_checks_passed_with_open_gates`.

Wave one added 92 Rust integration tests and nine runtime cases. It fixed
whitespace-sensitive subtraction, spurious dotted-constructor value lookup,
lost inherited nullability/redundant nullable references, unreachable failures
contaminating exact failure sets, and generated field constructor/validation
defects. Findings documents retain initial failures, fixes and proof limits.

The verifier now has nine independent inventory/obligation tests. They exposed
and fixed nested runtime suites being omitted, entity-only example directories
escaping classification, and unsupported golden obligations accepting a false
passing label. A temporary-copy mutation review reintroduced each defect;
all three mutations were caught by their intended test and the unmodified
baseline passed. Evidence: `build/validation/verifier-mutation-review.json`.

Target integration follow-up: an authored test confirmed that dotted field
constructors compile but fail at runtime with `Customer is not defined`.
Evidence: `build/validation/field-constructor-runtime-probe.json`. The failure
worker completed the bounded target correction and nine lasting runtime
regression tests, including local/chained field constraints, named refinements,
nullable fields and evaluation once. Those cases passed the full gate.
Semantic acceptance alone does not prove target behaviour.

That compiler challenge is complete: both unmodified tests passed, both mutants
compiled and failed their intended assertions, and both restored tests passed.
See `compiler-mutation-findings.md` and
`build/validation/compiler-mutation-review/2026-09-29-jd1hlvr9/`. This establishes
two specific regression detectors, not comprehensive mutation coverage.

Fixture 28 now spells its already-nullable argument `Customer.email` rather
than redundant `Customer.email?`. Its existing expected nullable-query rejection
is unchanged; the correction preserves the fixture's intended error boundary.

## Completed wave-two checkpoint

Full supported verification passed at
`build/validation/20260930T001821-88658/report.json`: all 36 steps passed,
313 Rust tests, 180 compile fixtures, four editor tests, ten example builds,
authored tests, 131 local runtime/artifact cases and 63 cases across four
PostgreSQL-mode runs. Nine verifier-contract tests also pass. This wave added
23 Rust tests, 30 local runtime/artifact cases and two new PostgreSQL modes
(17 cases). Golden remains 60 diagnostics and 44 unexecuted obligations.

Confirmed defects fixed: inherited nullable fields generated SQL
`NOT NULL` and false persistence metadata; OpenAPI and validator plans lost
inherited nullability; explicit construction could erase a secret local's
classification. Parent artifact suite initially passed one and failed three
cases; all four now pass, along with the 57 existing core library tests.
Persistence added real SQLite/PostgreSQL round trips. The new
`validation-persistence` PostgreSQL mode uses a fresh cluster and dedicated URL.

Focused integration update: persistence now passes five Rust tests and ten
runtime cases on each database; auth/policy passes seven Rust tests and seven
runtime cases on each database. Added local patch binding/shadowing panic
regressions and effective-nullability identity/foreign-key rejection tests.
Both secret-flow mutations (constructor propagation and temporal-helper
rejection) compile and fail their intended new tests, while original/restored
baselines pass; see `secret-mutation-findings.md`.

Config/time also exposed static/runtime Instant endpoint mismatches and the
pinned Bun version ignoring `--no-env-file`. The compiler-owned `test`/`dev`
launchers, common verifier, disposable PostgreSQL runner and subprocess tests
now use an explicit empty environment file (`/dev/null`, `NUL` on Windows).
Specific launch docs were corrected. These changes preserve the approved
explicit-configuration contract; they do not introduce dotenv support.

## Completed wave-three checkpoint

Final supported verification passed at
`build/validation/20260930T004547-5939/report.json`: all **39 steps passed**,
**374 Rust tests**, 180 compile fixtures, four editor tests, ten example builds,
authored tests, **155 local runtime/artifact cases** and **63 PostgreSQL cases**
across four isolated modes. Nine verifier-contract tests also pass. An earlier
38-step checkpoint, before Unicode/location follow-up, remains at
`build/validation/20260930T004013-2564/report.json` (367 Rust, 147 local runtime).
No stale successful report was reused after those follow-up changes.

This wave added 61 Rust tests and 24 local runtime cases. Independent assertions
exposed and fixed OpenAPI field/inherited/payload constraints, reference identity
schemas, typed failure details and same-status failure alternatives; LSP local
capture; config edits overwritten during staging; Unicode text-length divergence;
and project-role diagnostics pointing to a valid declaration. Five discovery
tests and 22 policy/config trigger tests correct the catalogue's previously
omitted families and false reference boundaries. The inventory is 480 codes;
references do not prove exhaustive diagnostic reachability or assertion coverage.

Five reviewed SQLite/fixture runtime cases and eight migration/staging Rust
cases establish their bounded isolation, preservation and rollback contracts.
A migration literal-corruption mutant and two catalogue discovery mutants all
compiled and failed the intended unchanged assertions; baselines/restores pass.
Independent configuration review has before/after public-API race evidence.
Its final check/rename remains best-effort against arbitrary external writers,
not atomic compare-and-swap.

A fresh context-isolated agent used frozen public docs/compiler inputs and
submitted a card service after four compiler invocations, reporting about four
minutes. The parent independently ran the predeclared hidden grader: **12/12
obligations passed**. Exact inputs, attempts, hashes, source and output are
retained under `build/validation/fresh-agent-pilot/20260930-wave3/`. Follow-up docs
and diagnostic-location corrections did not alter that frozen pilot or score.
This is one internal probe, not P10R/P12 comparative or human evidence.

## Disposition and next boundaries

The queued locally executable validation packages and their independent review
are complete. The heartbeat is paused rather than waking repeatedly to report
an unchanged gate. Do not imply complete language coverage or release readiness.

Still open:

- Golden todo: 60 diagnostics and all 44 integrated behavioral obligations
  unexecuted. The four recorded auth conflicts and query-budget ambiguity were
  reconciled on 2026-09-30 against approved AUTH-001 and the owner’s explicit
  query-accounting choice; source migration and unsupported behaviors remain.
- Set/Map HTTP wire encoding: generated schemas advertise JSON shapes that
  handlers reject. Key encoding, duplicates and null behavior are not settled;
  reproductions are preserved in `build/validation/set-map-wire-contract/`.
- Formal P10R approval, first-user/comprehension study and counterbalanced P12
  comparison require the specified external participants/frozen protocol.
- Source migration/deprecation, service/JWT, general migration execution,
  production qualification and Wasm remain separately scoped roadmap work.
  Broader fuzz/mutation, platform and deployment evidence is not supplied by
  this fixed local corpus. The mixed pre-existing uncommitted tree is preserved;
  no global staging, history rewrite, deployment or release was performed.

Resume with a specific approved follow-on package, retaining every recorded
contract gap. Do not silently weaken the golden app or choose wire semantics
merely to make the supported gate green.

## 2026-09-30 checkpoint and reconciliation follow-up

The owner authorized reviewable Git checkpoints and golden-contract reconciliation.
Commits `8160b94`, `6b74abc`, `9796a45` and `7465a83` preserve design/candidate,
coordinated compiler, validation infrastructure and editor-source slices. The
compiler changes are interdependent and were not presented as independently
reconstructable per-feature history. The old scratch example edit and stale
packaged VSIX remain outside these commits.

All five recorded golden auth/accounting discrepancies are now reconciled, with
all 44 case IDs retained and all still unexecuted. The owner chose one principal
lookup and separate credential query accounting. Candidate profile mappings no
longer contradict lookup-free signed identity validation; profile data remains
on authoritative application entities. See the golden REVIEW for exact changes,
authority, old/new hashes and remaining implementation limits. The heartbeat
remains paused; this follow-up is the currently authorized checkpoint package.

The reconciled implementation checkpoint is `df2d7be`. A clean `git archive` of
that commit, containing no untracked/ignored workspace files, passed the full
39-step gate: 374 Rust tests, 180 fixture pairs, four editor tests, nine verifier
contract tests, 155 local runtime cases and 63 PostgreSQL cases. Golden remains
60 diagnostics and 44 unexecuted obligations. The machine-readable
[checkpoint record](checkpoints/2026-09-30.json) pins the verified commit and
report hash; raw local evidence is preserved under
`build/validation/checkpoint-reconciliation/clean-run/`. This is a local clean
source rebuild, not a hosted CI run, independent approval or deployment claim.

The refreshed golden capability ledger removes stale claims that enum, selector,
clock, fixture, path-binding and policy cores are absent. It also records GF-032:
CREATE-002, READ-003 and LIST-004 expect 422/specific codes whereas the current
target returns 400/invalid_request. Their original expectations are retained and
the map marks them needs_clarification, not passed. This is separate from the
five resolved authentication/accounting discrepancies.

## 2026-09-30 unattended continuation through WASM-EXP1

The owner requested continued unattended work until the Wasm experiment is done.
This supersedes the previous heartbeat's validation-only scope and paused state.
The active goal and existing 30-minute heartbeat now cover agreed authentication,
validation, then the bounded experiment. Do not stop at a completed work package.
The starting implementation checkpoint is `fa7e7d5`; the scratch example edit and
untracked VSIX remain excluded. No production target migration is authorized.

Current assignments:

- `validation_types`: AUTH-P5 service identity/ownership, verifier-only opaque
  credentials, rotation/revocation and bounded exchange; runtime/generator and
  focused black-box evidence. Root integrates shared files.
- `validation_failures`: AUTH-P6 pinned JOSE dependency evidence, isolated JWT
  verification/discovery runtime and adversarial tests. Root integrates generated
  dependency closure, semantic settings and selector/authority handling.
- `cold_start_pilot`: preregister `docs/wasm-experiment-plan.md`, tool/host
  preflight and paper compatibility. No experiment implementation before auth
  prerequisites and post-change validation pass.
- Root: compiler integration, independent review, full verification, coherent
  Git checkpoints, gate accounting and later experiment integration.

Read-only Cloudflare connector access succeeded. This establishes account access,
not Worker deployment or runtime evidence. Disposable experiment deployment is
within the requested experiment; existing applications and paid plan changes are
outside this scope. The experiment protocol will freeze its source and toolchain
after prerequisites, before lowering or measurement. Formal external review and
unimplemented golden obligations remain separately recorded gates.

### Runtime extensions verified; ready for bounded Wasm probes

All assigned authentication work is integrated and reviewed. Final full gate:
`build/validation/20260930T024217-25646/report.json`, SHA-256
`7325a2ce5851b6e63361dfc2185b3b0c145fab7ba8550a3b3ed0d8acfd318ad9`.
45 steps passed: 383 Rust, 180 compile fixture pairs, four editor, 21 Python,
205 local runtime and 91 PostgreSQL cases. PostgreSQL explicitly skips the one
SQLite-only JWT query counter. No full golden case is marked executed.

The service/JWT findings and runtime guide retain the exact supported shapes,
install/provenance evidence and boundaries. The golden already declared
`NotPermitted` for disabled principals; allowing that existing 403 category
fixes a compiler restriction, not an unresolved owner decision. Existing
`Rejected` / 422 cases remain valid. All 44 golden expectations are untouched.

Next: commit this coordinated authentication boundary, freeze the Wasm protocol
and fixture, prepare the pinned tools, then execute the equal route probes.
Do not repeat the completed auth work or wait for another “continue”. The
scoped auth milestone plus comprehensive validation is satisfied; full P11
golden/tooling/external assurance remains separately open as documented.

## 2026-09-30 WASM-EXP1 execution started

Authentication extensions are committed as `4dc5604`; the subsequent 45-step
supported validation gate is recorded above. Continue through the experiment,
not another auth checkpoint. Frozen protocol, canonical fixture, acceptance
cases, ABI and toolchain inputs: `7a19a46`; common checked-model projection and
verified local host tooling: `96016e6`. Coin flip chose Rust-mediated then direct.
Rust probe started 01:57:27 UTC with a 45-minute cap. The shared Bun baseline and
portable case runner are being established alongside it. No route conclusion,
full-slice correctness, performance result or cloud execution is claimed yet.

Wrangler authentication is available. Its local-only packaging smoke passed;
that is not compiler evidence. Experiment scope remains six cumulative focused
hours, including setup; `experiments/wasm-exp1/effort.json` tracks allocations.
Do not modify the frozen fixture/acceptance criteria to rescue a route. The
actual cloud run, required semantics, measurements, final report and cleanup
remain required within the stopping rules. The golden application's open gates
remain separate and explicit.

## 2026-09-30 WASM-EXP1 completed

The owner-authorized through-Wasm run reached its experiment endpoint. Both
source-derived routes passed P01–P15 locally and on actual Cloudflare. The
provisional Rust full slice passed A01–A16 against Bun SQLite and a SQLite
Durable Object using the same core hash; A17 rejection and A18 source mutation/
reconstruction passed separately. A01 exact-output and nullable-parameter
review findings were fixed and challenged with retained regressions.

Forty formal timing runs produced 79,855,562 checked requests and zero measured
errors. I/O p95 was 33.34×/36.14× the Bun baseline at concurrency one/eight.
The recommendation is **defer adoption for further backend development**,
with the Rust/direct route comparison **inconclusive**. The prototype's
per-request instances/JSON boundary and known oversized-frame inner-location
gap remain explicit. No production backend switch follows.

The [results](../../docs/wasm-experiment-results.md) link all acceptance,
measurement, independent-review and source-hash evidence. Both disposable
Workers and the synthetic Durable Object namespace/data were deleted, with API
readback confirming removal. Source, scripts and evidence were committed in
separate route, host, full-slice, cloud and measurement checkpoints. Unrelated
user edits were preserved. The experiment follow-up is paused on completion.

Remaining roadmap work resumes at P11/golden on Bun: 60 recorded diagnostics
and 44 unexecuted integrated cases remain, alongside AUTH-P7/P8 qualification,
P10R/P12 and external first-user/review gates. Passing this bounded experiment
does not mark any of those exits complete.

## 2026-09-30 owner-requested Wasm optimisation extension

Completed a separately bounded attribution/reuse/host-cache follow-up. Eighteen
regression tests (10,289 assertions), retained15probe/16runtime cases for both
local variants and actualCloudflare, five negative gates, source mutation and
clean rebuild passed. Eighty timing runs had5,496,882 correct operations; final
throughput30–31k/s versus38–39k/s Bun and p95 0.041–0.043ms. All detailed
scopes and limitations are in [the report](../../docs/wasm-optimization-results.md).
The disposable optimisation Worker and authority were deleted and verified.
Production remains Bun; the previous heartbeat remains paused.

### 2026-09-30 — Owner-requested boundary and HTTP extension complete

The follow-up retained existing pooling/JSON contracts and measured constant and
host descriptor optimisations. Local P15/A16, five negatives, four rollback
counterexamples, source mutation/rebuild and 20 tests (10,502 assertions) passed.
The final separate-process Node-client/Bun-server comparison completed 140 HTTP
runs and 3,457,655 requests without errors; 50 isolated runs completed 1,281,101
operations. Earlier aborted harness runs remain separate evidence. Small reads
are near Bun; larger values still cost more. No production target change, new
cloud qualification or new unattended continuation is implied. See
[results](../../docs/wasm-http-experiment-results.md). The prior heartbeat stays paused.

### 2026-09-30 — Owner-requested Wasm performance continuation complete

The value-path experiment compared isolated ownership transfers and speed-oriented
Rust builds, then retained the combined candidate. Local checks passed: 21 tests /
10,586 assertions, two native runtime tests, P15/A16, five rejection gates, four
rollback counterexamples and deterministic mutation/rebuild. HTTP: 140 runs /
3,693,399 requests, zero errors. It won concurrent small-read throughput, p95 and CPU
in all five pairs; larger values remain slower and concurrent-write stalls remain
unresolved. Twelve separate diagnostic runs located long pauses inside server-side
application/storage calls on both targets, without establishing the exact cause.
No EC2/Cloudflare resources or production migration; heartbeat remains paused.
See [latest results](../../docs/wasm-value-path-results.md).

## 2026-10-01 — Explicit route success outcomes

Implemented the accepted `success: created` and `success: no_content` route
modes for the P11 golden CRUD path. Created routes require a typed matching
output and return HTTP 201 with validated JSON. No-content routes require a
`Unit` result with no declared output and return an empty HTTP 204. OpenAPI and
route inventory record the success response; route inventory advances to schema
version 2. The five negative/positive compile fixtures are represented by three
new pairs, bringing the corpus to 183.

The focused artifact/runtime suite passed 13 cases and 657 assertions. The full
45-step verification run at
`build/validation/20261001T063843-17177/report.json` has status
`supported_checks_passed_with_open_gates`: 392 Rust tests, 183 compile pairs,
four editor tests, 207 local runtime cases, and 91 PostgreSQL runtime cases. One
PostgreSQL-mode JWT query-counter check is skipped because that instrumentation
is SQLite-only. Golden source check now reports 57 diagnostics, down from its
60-diagnostic baseline; all 44 integrated behavioral cases remain unexecuted.
No acceptance expectation or human-owned policy was changed. GF-032's boundary
status/code question and the other declared golden, P10R, and external evidence
gates remain open.

## 2026-10-01 — Golden todo entity dossier migration started

Added the separate successor-source package at
`examples/golden-todo-migration` and retained the original candidate source and
policy as contract evidence. Its four persistent entity dossiers express User,
Service, ServiceCredential, and Todo with primary authority persistence,
unique fields, ownership references, inverse collections, and delete actions.
Shared enums and title/credential refinements live under `values/`.

`jadpo check examples/golden-todo-migration --diagnostic-format=json` passed
with zero diagnostics across five source files. This is a compiler-check result,
not runtime or acceptance evidence. The current entity-field grammar does not
support inline server defaults; migrated create actions must explicitly assign
the frozen active/open defaults. No policy, operation, route, authentication,
configuration, job, or acceptance behavior has moved yet. The original
candidate remains at 57 diagnostics with all 44 integrated cases unexecuted.

### Value and projection contracts

Extended the checked source package to six `.jadpo` files and 22 declarations:
added the supported patch/disable input records, Todo cursor and output
projections, public health output, reminder request/receipt values, and shared
bounded/enumerated value declarations. The command
`jadpo check examples/golden-todo-migration --diagnostic-format=json` passed
with zero diagnostics. `CreateTodo`'s omitted due-date default and `ListTodos`'
URL/query/cursor/default contract remain parked, preserving their original
pressure source and acceptance obligations.

### Configuration, user authentication, and liveness

Migrated typed configuration and secret bindings, the bounded application
revocation contract and closed principal, signed browser/API user credentials,
JWT user validation, active-user authority resolution, and the static public
`GET /health/live` route. The full source check now passes ten files and 32
declarations with zero diagnostics. The public route returns a constant typed
health response and reads no configuration or persistence.

This does not execute runtime or acceptance behavior. Service API-key
resolution/exchange, protected operations and routes, readiness, and the 44
golden cases remain open. Browser cookie runtime needs a configured origin that
the frozen candidate does not yet declare. The five-minute application bound
remains authored; the current validator grammar has no per-token `expires`
setting.

## 2026-10-01 — Full supported-check verification

The first verifier attempt stopped at the inventory contract because the new
`examples/golden-todo-migration` source tree was not yet classified. Added it to
the validation manifest as an exploratory `design_contract` package and
clarified in the validation README that it supplies compiler-check evidence,
not integrated runtime or acceptance evidence. The sandboxed rerun then
reached a loopback bind restriction in the watch-protocol suite; rerunning the
same verifier with localhost access completed all supported checks.

Report: `build/validation/20261001T100354-43738/report.json` records
`supported_checks_passed_with_open_gates`, with all 45 recorded steps passed.
This includes the Rust workspace, compiler and editor checks, authored tests,
runtime suites, and all six PostgreSQL modes. The report correctly remains
`release_equivalent: false`: the frozen golden candidate still has 57
diagnostics and its integrated behavioural evidence is `not_run`. The listed
golden, fresh-agent, independent P10R, external assurance, deployment, and
differential-conformance gates remain open.

## 2026-10-01 — User self-read scope migrated

The unchanged frozen policy gives User self-access the direct identity
predicate `principal.user.user_id == User.id`. Migrated that exact scope as an
immutable `UserRole.self` binding on the User dossier identity. The policy
analyzer now treats that dossier identity as the same-entity principal source;
it still rejects a role binding on a non-identity scalar. Empty field policies
keep provisioning and lifecycle fields from entering self-readable
projections. The migration source check passes ten files and 34 declarations
with zero diagnostics. The focused `identity_role_binding` suite passes all
three cases, and `validation_diagnostic_policy` passes all 18. This is compiler
and policy-model evidence, not runtime or golden acceptance evidence; User
disablement, lifecycle, protected routes, and the 44 golden cases remain open.

## 2026-10-01 — Identity-bound policy full verification

After adding identity-bound direct role support, the full
`python3 tools/verify.py` gate completed with all 45 recorded steps passed,
including the Rust workspace, compiler/editor checks, authored and runtime
suites, and six PostgreSQL modes. Report:
`build/validation/20261001T104241-50479/report.json`. The status remains
`supported_checks_passed_with_open_gates`, `release_equivalent` is false, and
the frozen golden candidate remains at 57 diagnostics with integrated behavior
unrun. The User self-role source/model tests do not count as route/runtime or
acceptance evidence. The verifier continues to list the remaining migration,
external assurance, deployment, compatibility, broader campaign, and
differential-conformance gates.

## 2026-10-01 — Nullable input omission default

Added the bounded `default none` input form needed by frozen GF-013A. The
`jadpo-syntax` parser unit suite passes 38 tests; the `jadpo-core` suite passes
70 unit tests and its integration suites, including new input-validator and
OpenAPI assertions. `cargo run --offline --locked --manifest-path
jadpo/Cargo.toml -p jadpo-cli -- check examples/golden-todo-migration
--diagnostic-format=json` passes ten files and 35 declarations with zero
diagnostics. This confirms source and target contract generation, not the
frozen CREATE-004 HTTP behavior. The 44 golden acceptance cases remain
unexecuted.

## 2026-10-01 — Latest supported-check verification

After adding the nullable input default, `python3 tools/verify.py` completed all
45 recorded steps. Report:
`build/validation/20261001T113106-57571/report.json`. The migration source check
also passes ten files and 35 declarations. The verifier remains
`supported_checks_passed_with_open_gates`, not release-equivalent; the frozen
golden candidate still has 57 diagnostics and its integrated behavioral
evidence remains unrun. The subsequent `create_todo` source experiment exposed
unresolved current-syntax mismatches and was removed, leaving the checked
migration package at ten files and 35 declarations.

## 2026-10-01 — P10.7 live PostgreSQL nested-savepoint evidence

Extended the entity-dossier runtime scenario to select either SQLite or the
verifier-owned disposable PostgreSQL database. In both modes, a nested action
updates a customer and appends a revisioned authority change record, then fails;
the caller handles the failure, observes the original row, performs another
update, and commits. The persisted state contains only the create and final
update, with contiguous revisions, no rolled-back payload, and one stable
operation timestamp. SQLite passed one case/seven assertions; PostgreSQL 16.3
passed the same case/seven assertions.

Registered `entity-dossier` as the seventh PostgreSQL mode in the shared full
verifier. The full run passed all 46 steps, including the runtime regression,
and produced `build/validation/20261001T120404-64902/report.json`. The report
remains `supported_checks_passed_with_open_gates`, not release-equivalent; the
frozen golden candidate still has 57 diagnostics and all 44 behavioral cases
remain unexecuted. P10.7 still needs broader concurrency/retry evidence and
cross-store execution; this case verifies the accepted nested-savepoint
contract only.

## 2026-10-01 — Compile-corpus parser mutation campaign

Expanded the deterministic malformed-input property test to load every
compile-pass and compile-fail `.jadpo` fixture and sample tokens across each
file. Each selected token is deleted, replaced with a structural delimiter,
and replaced with a multibyte unexpected character; every mutation is checked
for deterministic parsing, valid UTF-8 token/diagnostic spans, and a correct
EOF token. The suite asserts that at least 3,000 corpus-derived mutations run
under the existing 10-second child-process timeout.

The expanded campaign found an invalid diagnostic range in malformed nested
calls. When `parse_arguments` failed, the caller used an all-zero default
tuple, creating a call-expression range whose end preceded its start. Incomplete
invocation/object/construction expressions now return a bounded `Missing` node
range instead. The deterministic mutation case remains in the campaign.

`cargo test --offline --locked --manifest-path jadpo/Cargo.toml -p
jadpo-syntax --test validation_fuzz_bounded` passed in 2.60 seconds after the
fix. This adds bounded parser robustness evidence across accepted syntax
families, not a sustained coverage-guided fuzz campaign, deep-nesting/resource-
limit proof, or implementation-code mutation score.

The full `python3 tools/verify.py` run after the parser fix passed all 46 steps,
including the expanded Rust campaign and all seven disposable PostgreSQL modes.
Report: `build/validation/20261001T122253-68755/report.json`. The status remains
`supported_checks_passed_with_open_gates`, not release-equivalent; golden
compilation still has 57 diagnostics and all 44 behavioral cases are
unexecuted.

## 2026-10-01 — P10.7 concurrent transaction-context evidence

Extended `entity-dossier` with 24 concurrent same-entity updates, each using a
distinct operation timestamp. The test checks that every committed revision is
contiguous, every change record retains its own timestamp and payload, and the
final row matches the highest revision. Both SQLite and disposable PostgreSQL
16.3 passed two cases and 35 assertions.

The latest full `python3 tools/verify.py` run passed all 46 steps and all seven
PostgreSQL modes: [`20261001T125217-74578/report.json`](../../build/validation/20261001T125217-74578/report.json).
The report remains `supported_checks_passed_with_open_gates`, not
release-equivalent; the golden candidate still has 57 diagnostics and all 44
behavioral cases are unexecuted. This is same-process concurrency evidence. It
does not establish cross-process writer isolation, deadlock recovery or retry
semantics, nor derived-store delivery. A separate-process SQLite probe returned
`SQLITE_BUSY` at transaction start under concurrent writers. That exploratory
fixture was removed because TX-001 leaves the default retry/failure contract
open; no policy was selected from this observation.

An attempted P11 `Todo.by_id` migration was removed after the current query
grammar rejected the required `id` plus `deleted_at` predicate combination.
The frozen source and policy were unchanged; the existing named-query gate
remains open.

## 2026-10-01 — P10.7 PostgreSQL cross-process writers

Added a PostgreSQL-only case with two independent Bun processes concurrently
updating the same entity through separate runtime instances. Across the 24
updates, PostgreSQL 16.3 retained every operation timestamp and payload, wrote
contiguous revisions, and left the row value matching the highest revision.
The PostgreSQL entity-dossier suite passed three cases and 65 assertions; the
SQLite suite continues to pass its two same-process cases and 35 assertions.
The latest full [`python3 tools/verify.py` report](../../build/validation/20261001T125836-76197/report.json)
passes all 46 steps and all seven PostgreSQL modes, with
`supported_checks_passed_with_open_gates` status.

A multi-process SQLite probe returned `SQLITE_BUSY` at `transaction.begin`.
Because TX-001 leaves the default contention/retry behavior open, the probe is
recorded without introducing an implementation or acceptance expectation. The
SQLite cross-process contract remains pending an owner choice.

## 2026-10-01 — P11 Todo by-id source action and route

The Todo dossier now declares an authoritative `by_id` query. Its required
identity lookup carries the automatic `TodoRole.owner` read obligation, and an
exhaustive nullable match maps soft-deleted rows to the same concealed
`TodoNotFound` failure before constructing the frozen `TodoView`. Entity action
`Todo.get` propagates that failure, and authenticated `GET /todos/{todo_id}`
binds the ID path parameter and exposes the derived 404 response. This keeps the
compiler's automatic policy source independent from the lifecycle check rather
than trying to encode both in the currently single-predicate query syntax.

`jadpo check examples/golden-todo-migration --diagnostic-format=json` passes
ten source files, 39 declarations, and zero diagnostics. `jadpo inspect` records
the call path from the route through `Todo.get` to `Todo.by_id`, and derives the
route's `TodoNotFound`/404 failure. A prior build of the query-only slice
recorded the proved owner-read policy obligation. With the protected route
present, `jadpo build examples/golden-todo-migration` now fails closed with
`toolchain.target_auth_not_implemented`: the selected authentication adapters,
configuration, or principal mapping are not supported together by the target
generator. No generated runtime or golden behavior is claimed; route execution
and the 44 frozen cases remain open.

The refreshed full `python3 tools/verify.py` run passed all 46 steps and all
seven PostgreSQL modes. Report:
[`20261001T132458-81588/report.json`](../../build/validation/20261001T132458-81588/report.json).
Status remains `supported_checks_passed_with_open_gates`, not release-equivalent;
the golden candidate still has 57 diagnostics and all 44 integrated behavior
cases remain unexecuted. The default sandbox initially denied the localhost
socket used by the watch rollback test; the same verifier passed with local
socket access enabled.

## 2026-10-01 — P11 generated Todo identity and create contract

Added the bounded `generated: identity` field option for required UUID identity
fields on persistent entities. Compiler-created values use `crypto.randomUUID()`;
the generated route boundary still rejects an `id` supplied by request input.
The SQLite runtime suite proves distinct persisted UUIDs and input rejection.
Type construction now accepts the declared field type and its refinements,
including a closed enum variant for `Todo.status`. Policy analysis accepts an
explicit constructor for a role-bound owner field only when the constructor's
value comes from the authenticated principal; focused tests reject a request
value through the same constructor.

The migrated Todo action preserves future-only due dates, maps create
conflicts, uses the authenticated owner and initial `open` state, and returns
the generated identity and lifecycle timestamps. Source checking passes ten
files and 42 declarations. The generated-identity syntax is included in the
TOOL-005 rule matrix and exact formatter unit surface; the matrix now covers 122
grammar productions, and corpus conformance includes 57 passing and 128 failing
compile fixtures.

The full verification report
[`20261001T135910-88879/report.json`](../../build/validation/20261001T135910-88879/report.json)
passes all 47 steps and seven PostgreSQL modes, including 185 compile-fixture
pairs and the new runtime suite. It remains
`supported_checks_passed_with_open_gates`, not release-equivalent. Building the
protected migrated route still stops with human-owned
`toolchain.target_auth_not_implemented`; no full golden behavior is claimed.
The separate SQLite cross-process contention choice and TOOL-005 line-break
normalization conflict remain unresolved owner decisions.

The migrated package now also has a protected `POST /todos` route which passes
the typed user principal variant into `Todo.create_todo`, plus a service API-key
validator and active `ServiceCredential.verifier` authority resolution; source
checking covers eleven files and 44 declarations. First-party generated handlers exercise
`current_principal.user` and `.service`, with runtime coverage for a matching
user principal and a 403 when a service principal reaches the user-only route.
The new compile fixtures bring the corpus to 58 pass and 129 fail cases (187
pairs); the grammar-corpus formatter conformance test includes the new positive
syntax case. The selected auth adapter/configuration/principal target gate remains
human-owned and still blocks full migrated-package target generation. TOOL-005
now also calls for a table-driven exact-output unit case per grammar production
and syntactic alternative, including extra spaces, tabbing/indentation, and
excess newlines, with removed-line-break exactness pending its owner decision.

## 2026-10-01 — Protected principal projection and full verification

Protected routes now accept `current_principal.user` and
`current_principal.service` as narrowed values for typed queries and actions.
Generated handlers check the authenticated principal's tag and return the
existing authorization fault on a mismatch. The first-party auth runtime suite
passes 25 tests and 250 assertions, including a matching user route and a
service principal rejected from a user-only route. A compile-fail fixture also
confirms that a public route cannot invoke principal-dependent behavior even
when the callable grants public invocation.

The golden migration now checks eleven source files and 43 declarations,
including protected `POST /todos` deriving the owner from the user principal.
`jadpo build examples/golden-todo-migration` remains fail-closed at the
human-owned `toolchain.target_auth_not_implemented` target gate, so this is not
integrated or golden behavioral evidence.

The full validation report
[`20261001T145157-96062/report.json`](../../build/validation/20261001T145157-96062/report.json)
passes all 47 steps and seven PostgreSQL modes. It verifies 187 compile-fixture
pairs; the first-party principal projection test passes in the full run. Status
remains `supported_checks_passed_with_open_gates`, not release-equivalent: the
golden candidate still has 57 diagnostics and all 44 integrated behavior cases
remain unexecuted. The report's remaining release gates are unchanged. The
TOOL-005 matrix covers all 122 grammar productions, and the new route syntax is
in the automatically discovered formatter corpus. The scheduled exact-output
unit-test work now explicitly covers every production and syntactic alternative
under extra spaces, tabs/indentation errors, and excess newlines; nonblank
line-break normalization remains gated on its owner decision.

After that full run, the migrated authentication source gained the frozen
service API-key validator and active `ServiceCredential.verifier` resolution,
including a safe service subject mapping and the `ServiceDisabled` rejection.
`jadpo check examples/golden-todo-migration --diagnostic-format=json` passes
with zero diagnostics across eleven files and 44 declarations. This source
check does not change the full report's runtime scope: generated target
validation remains blocked by the human-owned adapter/configuration/principal
gate, and service credential issuance/exchange is still open.

## 2026-10-01 — Service-principal route coverage and owner mapping

The service-auth fixture now exercises `current_principal.service` on a
service-only route. A valid service credential reaches the route, while a user
credential receives 403. The focused suite passes 15 tests and 213 expectations.
The full supported-check run at
[`20261001T151424-112/report.json`](../../build/validation/20261001T151424-112/report.json)
passes all 47 steps and seven PostgreSQL modes, including the service-auth
suite and all 187 compile-fixture pairs. It remains
`supported_checks_passed_with_open_gates`; the golden candidate has 57
diagnostics and its 44 integrated behavior cases have not run.

The migrated service API-key validator now explicitly binds its credential
owner to `ServiceCredential.service_id`, with `ServiceCredential.verifier`
remaining the authority lookup key. The migrated package checks eleven files
and 44 declarations with zero diagnostics. Its target build still fails closed
at `toolchain.target_auth_not_implemented` on the protected Todo route. The
owner-mapping edit was made after the full run and received a focused source
check; it adds no runtime evidence. Service issuance/exchange is implemented in
the generated runtime, while authored golden issuance/exchange operations and
their integrated acceptance remain open.

### 2026-10-01 — Todo patch operation migrated

Migrated the frozen `PATCH /todos/{todo_id}` behavior into the Todo dossier and
added its typed protected route. The action rejects empty patches and past due
dates, resets `reminder_sent_at` when `due_at` is supplied, performs the
authoritative `Todo.by_id` lookup, and then uses the entity-scoped required
update with automatic `TodoRole.owner` policy. Source checking passes eleven
files and 47 declarations with zero diagnostics. `jadpo build` still fails
closed at `toolchain.target_auth_not_implemented` on the protected GET route,
before producing the migrated target; no PATCH runtime or integrated golden
case is claimed. The source shape does not settle atomic interaction with the
still-gated soft-delete lifecycle.

### 2026-10-01 — Formatter exact-output corpus and full validation

Added checked-in canonical output snapshots for all 58 compile-pass fixtures.
The formatter unit suite reparses each source and snapshot, preserves the
token/comment/string inventory, checks idempotence, and compares exact output
after extra legal spaces, tabs, incorrect indentation, CRLF, and excess blank
lines. The surface-family matrix covers every HTTP method, supported
authentication topology, comments, strings, persistence forms, callables,
expressions, and authored tests. Removed nonblank-line-break exactness remains
gated on FMT-005. The focused formatter cases and the full `jadpo-core` package
pass.

The full verifier report
[`20261001T155715-6434/report.json`](../../build/validation/20261001T155715-6434/report.json)
passes all 47 steps and seven PostgreSQL modes, including the Rust workspace,
187 compile-fixture pairs, and service authentication. Its status is
`supported_checks_passed_with_open_gates`: the frozen golden application still
has 57 diagnostics and its 44 integrated behavior cases were not run. The
separate migrated source checks eleven files and 47 declarations with zero
diagnostics; target generation still stops at the human-owned
`toolchain.target_auth_not_implemented` protected-route gate.

## 2026-10-01 — TOOL-005 grammar alternative exact-output coverage

Added exact-output links from the formatter rule matrix to representative
checked-in snapshots. Added a focused source/snapshot and unit test for accepted
alternatives absent from the compile-pass corpus: `reference_delete_action: restrict`,
`inverse_cardinality: many` in entity and compatibility persistence
forms, `representation_declaration: projection` without a strategy, and
`freshness: read_your_writes`, `bounded_staleness`, and `eventual`. The focused
test checks parseability, exact formatting, idempotence, tabs/spaces, incorrect
indentation, CRLF, and excess blank lines. The complete `jadpo-core` suite
passes (73 library unit tests plus every integration test binary); the focused
formatter rustfmt check and `git diff --check` also pass. FMT-005's removed-line-break output contract
remains an owner decision.

## 2026-10-01 — Supported checks after formatter coverage

The full `python3 tools/verify.py` run at
[`20261001T164609-19362/report.json`](../../build/validation/20261001T164609-19362/report.json)
passed all 47 supported steps and seven PostgreSQL modes, including the Rust
workspace, 187 compile-fixture pairs, formatter snapshots and perturbations,
editor, authored/runtime suites, and service authentication. An initial sandbox
run stopped at the watch rollback test because localhost binding was denied;
the retry with local socket permission passed. Status remains
`supported_checks_passed_with_open_gates`: the frozen golden candidate still
has 57 diagnostics and its 44 integrated behavior cases remain unexecuted.

## 2026-10-01 — Formatter production traceability enforced

Added a unit test that reads the accepted EBNF and checks that every named
production appears in the formatter matrix on a row with a valid exact-output
snapshot link. The expanded `jadpo-core` suite passes 74 library tests plus all
integration test binaries. The full verifier report at
[`20261001T165920-24403/report.json`](../../build/validation/20261001T165920-24403/report.json)
passes all 47 supported steps and seven PostgreSQL modes. Its status remains
`supported_checks_passed_with_open_gates`; the frozen golden candidate still
has 57 diagnostics and its 44 integrated behavior cases remain unexecuted.

## 2026-10-01 — TOOL-005 alternative audit expanded

A follow-up scan of accepted grammar alternatives found cases not present in
the original positive-fixture corpus: descending order with pagination, `else`,
negative numeric literals, escaped strings, `pattern`/`format` constraints,
nullable `default none`, scalar config defaults, and optional authentication
validator settings. The focused exact-output source/snapshot now covers these
alongside `restrict`, inverse `many` in both spellings, strategy-free
`projection`, and all freshness alternatives. The formatter matrix, roadmap,
and issue tracker now list the same coverage. The focused exact-output test
passes, and the complete `jadpo-core` suite passes 74 library tests plus every
integration-test binary. All 47 steps in the full verifier passed, including
seven PostgreSQL modes; report:
[`20261001T170939-31977/report.json`](../../build/validation/20261001T170939-31977/report.json).
The status remains `supported_checks_passed_with_open_gates`: the frozen golden
candidate has 57 diagnostics and 44 integrated behavior cases remain unrun.

## 2026-10-01 — TOOL-002 generated-artifact navigation

Added the `Jadpo: Open Generated Artifact` picker to the VS Code extension. It
shows compiler artifacts that exist in the current project's `build/` folder
and covers every stable artifact emitted by `derive_artifacts`, including the
conditional authentication audit. The paths come from a curated, tested
relative-path list; the command opens generated files in a side editor without
adding source document links. The extension README, developer-tooling plan, and
TOOL-002 issue record now describe this surface. The VS Code package suite
passes all six tests, including registry safety, manifest activation, and
command contribution checks. The package manifest parses as JSON, updated
documentation links resolve, and `git diff --check` passes. The full repository
verifier was not rerun because the changed runtime surface is covered by its
editor test command, which this package suite includes.

## 2026-10-01 — TOOL-003 renderer adapters and fixtures

Added a shared renderer package for the stable `jadpo` fence identifier. Shiki
reuses the VS Code TextMate grammar; Prism and Highlight.js registration
adapters, plus a Monaco Monarch tokenizer and editor configuration, derive
their language vocabulary from that grammar. Added fenced examples for
declarations, constraints, persistence, failures, routes, comments, strings,
and malformed source. Registration-shape tests pass for all four adapters,
the fixture uses six canonical Jadpo fences, and the complete VS Code package
suite now runs those tests. Real renderer engines are not installed in this
workspace, so renderer-host previews remain open; TOOL-003 is not marked
complete.

The refreshed full verifier at
[`20261001T174311-42768/report.json`](../../build/validation/20261001T174311-42768/report.json)
passes all 47 supported steps and seven PostgreSQL modes. The editor step runs
all 11 VS Code/rendering tests, including the four adapter registration-shape
checks. The result remains `supported_checks_passed_with_open_gates`; the frozen
golden source and release evidence gates are unchanged.

## 2026-10-01 — TOOL-005 compatibility enum coverage

An EBNF terminal audit found the compatibility `type Name = Enum { ... }`
alternative was absent from both compile-pass fixtures and the focused
formatter case. Added it to the focused source/snapshot pair, so its exact
output is checked after extra spaces/tabs, incorrect indentation, CRLF, and
excess blank lines. Updated the formatter matrix, TOOL-005 issue, and DX1
coverage summary to name this spelling. The full `jadpo-core` suite passes,
including 74 library tests and every integration-test binary; local Markdown
link validation and `git diff --check` also pass. The full verifier then passed
all 47 steps and seven PostgreSQL modes; see
[`20261001T180732-46495/report.json`](../../build/validation/20261001T180732-46495/report.json).
The first verifier attempt lacked localhost socket permission for its rollback
test; the required permission was applied on retry. The source and expected
output keep the legacy enum spelling in the whitespace perturbation loop.

## 2026-10-01 — TOOL-002 agent query surface audit

Audited the remaining TOOL-002 note against the actual `jadpo-agent` skill,
CLI help, and DX2 delivery status. The skill is checked in and routes agents to
versioned JSON diagnostics, compiler semantic inspection, generated artifacts,
standard LSP queries, and secret-safe local incident enrichment. `jadpo --help`
exposes the corresponding `check`, `inspect`, `artifacts`, `incident`, and
`lsp` commands. Updated the issue and tooling plan to record the implementation
and keep fresh-agent comprehension/repair-cycle outcomes in DX2, where they
belong. This is compiler-backed query routing, not a claim that the deferred
human trials passed.

## 2026-10-01 — TOOL-003 Prism and Shiki engine smoke check

Used cached packages in an isolated temporary directory, without changing
project dependencies. Prism 1.30.0 highlighted all six canonical Jadpo fences
and emitted comment, string, keyword, type, number, operator, and punctuation
tokens. Shiki 4.4.3 rendered the same six fences from the TextMate grammar and
produced ten Jadpo scopes, including keywords, primitive types, numbers,
comments, and strings. Highlight.js and Monaco still have registration-shape
checks only, and visual comparison in actual renderer hosts plus plain-text
fallback behavior remain open; TOOL-003 stays in progress.

## 2026-10-01 — NAME-002 lexer conformance slice

Expanded the syntax-boundary tests from a partial keyword sample to all 91
exact reserved spellings currently recognized by `keyword_kind`. The test also
checks keyword-containing identifier extensions, exact keyword casing, ASCII
identifier boundary forms, number-versus-identifier token boundaries, and a
non-ASCII leading character with its UTF-8 diagnostic span. Verification:
`cargo test --locked --quiet --manifest-path jadpo/Cargo.toml -p jadpo-syntax`
passes 38 unit tests, the bounded malformed-input test, and all 19 syntax
boundary tests. Case-sensitive semantic references and contextual-name
placement remain open; no naming contract or lexer behavior was changed.

## 2026-10-01 — NAME-002 parser and lookup audit

The accepted grammar distinguishes the lexical identifier shape from semantic
name casing. Semantic casing currently requires type-like names to begin with
an uppercase ASCII letter and continue with ASCII letters or digits; runtime
names begin lowercase and allow lowercase letters, digits, and underscores,
excluding repeated or trailing underscores. A CLI probe confirmed exact-case
value lookup: `Username` does not resolve a declared `UserName` and reports the
unknown-value diagnostic at the reference span. Added
`type_name_resolution_is_case_sensitive` and
`semantic_name_shapes_separate_types_from_runtime_names`. They distinguish
`UserName` from `Username` and cover type/runtime casing, digit continuations,
and accepted/rejected underscore forms. All 36 `jadpo-semantic` unit tests pass.
A separate parser probe
found that `return` can occupy a binding-name slot but is then parsed as a
statement keyword in expression position, making that binding unreadable. The
parser currently accepts a broader set of contextual-name spellings than
grammar §3 documents. Updated NAME-002 and RM-201 to record these findings and
regression evidence; no language behavior or naming contract changed.
The hard-reserved versus contextual keyword policy is waiting for the owner's
choice.

## 2026-10-01 — RM-501 cross-area validation gap inventory

Reviewed the current language issues, validation entry point, golden obligation
map, and existing syntax/artifact findings against the roadmap's next validation
campaigns. Added [roadmap-gap-inventory.md](roadmap-gap-inventory.md) with
specific missing evidence for syntax, types, failures, transactions, auth/policy,
config/time, tooling, integrated golden behavior, and sustained fuzz/mutation.
The inventory distinguishes repository-local tests from contracts that need an
owner choice or an external database/provider/review environment. It preserves
the known Set/Map wire mismatch, SQLite `SQLITE_BUSY` observation, protected
golden target-generation gate, FMT-005 conflict, renderer-host gaps, and the
unexecuted golden cases without inventing expected behavior. RM-501 is complete;
RM-502–505 retain their own test/evidence work and dependencies at this
checkpoint.

## 2026-10-01 — RM-502 seeded parser/type/failure campaign

Added `jadpo/crates/semantic/tests/validation_fuzz_semantic.rs`, which loads
the sorted compile-pass/fail corpus and analyzes 2,048 reproducible seeded
mutations through parser, semantic graph, type checking, and failure analysis.
Every stage is rerun to check determinism; token and diagnostic spans must land
on valid UTF-8 boundaries. Failures print seed/case/corpus metadata and a
source reduced by a three-second/512-attempt delta debugger. A separate test
proves the reducer preserves a failure while shrinking. The campaign runs in
an isolated child with a 90-second deadline. The [campaign guide](fuzz-campaign.md)
requires minimized, fixed examples to be retained in the ordinary compile
fixture corpus. Verification command:
`cargo test --locked --quiet --manifest-path jadpo/Cargo.toml -p jadpo-semantic`
passes 36 unit tests and both integration tests; the fuzz campaign completed in
13.91 seconds. No generated failure was found. RM-502 is complete;
coverage-guided/deep-resource fuzzing and mutation evidence remain separate
work.

## 2026-10-01 — NAME-002 roadmap coverage expanded

Expanded RM-201 and the NAME-002 gap description to require a grammar-derived
matrix for every identifier-bearing production, including variable and type
declarations/references, keyword use in both name classes, exact case
sensitivity, lexical ASCII/allowed-character and leading-digit boundaries,
semantic type/runtime casing, underscores, Unicode, and stable CLI/LSP spans.
Contextual keywords that are accepted as names must also resolve at their use
sites. The accepted naming contract and compiler behavior remain unchanged;
keyword-position expectations still depend on the owner's reserved-word
policy decision.

## 2026-10-01 — RM-503 targeted mutation review, first slice

Recorded five baseline/mutant/restored experiments in
[rm503-mutation-findings.md](rm503-mutation-findings.md): exact-case semantic
type lookup, repeated and trailing underscore rejection, complete-token keyword
recognition, and generated Unicode scalar length. All five mutants compiled
where applicable and were killed by their intended tests; the Unicode runtime
baseline/restored suite passed eight tests and 87 assertions, while the mutant
failed seven tests on supplementary-scalar boundaries. The experiments ran in
an isolated temporary copy and the current source/test SHA-256 fingerprints
match. No primary workspace production source or assertion changed. This is a
partial targeted sample, not a broad mutation score; additional obligation
areas remain under RM-503. `git diff --check` passes.
