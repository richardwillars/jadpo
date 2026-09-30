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
