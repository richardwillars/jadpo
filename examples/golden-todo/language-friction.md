# Golden todo language-friction ledger

**Snapshot:** current compiler after P10  
**Status:** input to P10.5/P11, not permission to weaken the application

The current compiler cannot check `app.jadpo` or `policy.jadpo` in full. That
is expected. Each gap below must receive a minimal fixture and a language-issue
disposition before implementation. The original failed design pressure remains
evidence even after support is added.

| ID | Required construct | Current disposition | Issue |
|---|---|---|---|
| GF-001 | application declaration and secure authentication default | missing language capability | `AUTH-001` |
| GF-002 | two authentication strategies normalised to one actor | missing language capability | `AUTH-001` |
| GF-003 | exactly-one strategy selection and conflicting credential rejection | missing policy/proof rule | `AUTH-001`, `POLICY-001` |
| GF-004 | authored actor declaration and authenticated user lookup | missing language capability | `AUTH-001` |
| GF-005 | enums, qualified defaults, and boundary decoding | provisional syntax, no compiler support | `TYPE-006` |
| GF-006 | typed configuration, secrets, provenance, preflight, reload policy | missing language capability | `CONFIG-001` |
| GF-007 | compound non-unique indexes | missing persistence capability | `DATA-003` |
| GF-008 | output projection from entities | partial semantic design only | `QUERY-001` |
| GF-009 | path and query parameter declarations | missing route capability | `ROUTE-001` |
| GF-009A | typed success semantics for created and no-content routes | missing route capability | `ROUTE-001` |
| GF-010 | cursor pagination with compound stable ordering | missing query capability | `QUERY-001` |
| GF-011 | optional predicate composition | missing query capability | `QUERY-001` |
| GF-012 | omission-aware patch application | implemented for direct optional-field patch inputs; golden integration pending | `DATA-002` |
| GF-013 | explicit supplied-field inspection | missing input operation | `DATA-002` |
| GF-013A | nullable input default used to make create-time `due_at` omissible | missing input-default semantics | `DATA-002` |
| GF-013B | patch-dependent derived write clears `reminder_sent_at` when `due_at` is supplied | implemented as fixed derived `set:` with `when input.due_at supplied`; golden integration still depends on other gaps | `DATA-002`, `ASYNC-001` |
| GF-014 | optional and required relationship traversal with filters | direct traversal plus a bounded owning-parent/optional-inverse depth-two path are implemented; relationship filters and other nested shapes remain scheduled | `DATA-001` |
| GF-015 | one-to-one/required parent include spelling | explicit `references User.id as owner` naming plus unique-backed `inverse ...: optional ... via ...` are implemented; required inverse-one semantics and composite-reference pressure remain scheduled | `DATA-001`, `DATA-004` |
| GF-016 | policy declarations and proof obligations | P10R design required | `POLICY-001` |
| GF-017 | concealed authorization/not-found equivalence | provisional failure rule | `FAIL-002`, `POLICY-001` |
| GF-018 | clock capability and deterministic test injection | missing standard capability | new `TIME-001` |
| GF-019 | service contract, egress, secret injection, failure mapping | deferred | `SERVICE-001` |
| GF-020 | job schedule, bounds, concurrency, retries, and idempotency | deferred | `ASYNC-001` |
| GF-021 | bounded iteration syntax | provisional, no resource rule | `TYPE-002`, `ASYNC-001` |
| GF-022 | service call followed by durable success marker | unresolved effect/delivery semantics | `ASYNC-001` |
| GF-023 | bounded batch hard delete | missing persistence capability | `DATA-003`, `ASYNC-001` |
| GF-024 | soft-delete migration after existing data | migration identity/lifecycle missing | `DATA-003` |
| GF-025 | generated structural tests and authored clock/service fixtures | test grammar open | new `TEST-001` |
| GF-026 | modules/imports for a real multi-file source tree | bounded explicit-module/selective-import core implemented; golden split and advanced namespace features pending | `MOD-001` |
| GF-027 | approval-bound human-owned policy artifact | P10R protocol required | `POLICY-001` |
| GF-028 | deployment-plane readiness distinct from application routes/auth | missing operational capability | `CONFIG-001` |
| GF-029 | public liveness response with no dependency checks | target can expose public route, not health semantics | `CONFIG-001` |
| GF-030 | rename-stable schema/migration identity | scheduled | `TOOL-001`, `DATA-003` |

## Required order

P10.5 should address only the gaps already assigned to it: relationships,
patches, pagination/query completion where needed by those features, migration
identity, modules, and value/reference semantics. Authentication, policy,
configuration, services, jobs, and approvals remain P11 or later and must be
implemented against frozen proof/threat/approval contracts.

The bounded value/reference item is now closed without adding a golden-todo
capability: ordinary parameters and returned data are immutable values, and
`var mut` permits only type-compatible lexical rebinding. The golden design presents
no need for caller-visible mutation, so `inout`, references, field assignment,
and observable aliasing remain absent rather than speculative.

No gap is resolved by embedding unchecked TypeScript in authored source. If a
constrained escape hatch becomes necessary, its authority, data/effect surface,
and audit visibility require a separate human decision and adversarial case.
