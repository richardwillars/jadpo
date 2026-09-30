# Candidate contract review log

**Contract:** `todo-v0.1`  
**Review type:** author-side consistency review  
**Independent review:** pending

This log records contradictions found before the independent P10R review. It is
not external validation and cannot satisfy the P10R review or first-user gates.

## 2026-09-25 consistency pass

| Finding | Resolution |
|---|---|
| Acceptance was ranked above human-owned policy, allowing a conflicting test to appear authoritative. | Split authority: policy owns permissions/effects/lifecycle; acceptance owns observable behaviour inside the policy envelope; any contradiction blocks freeze. |
| The `overdue_reminders` job selected future todos within a lookahead window. | Removed the unused lookahead configuration and changed eligibility to `due_at < at`. |
| `TodoAlreadyCompleted` was declared but unreachable. | Removed the failure rather than inventing lifecycle behaviour. |
| Create-time omission of nullable `due_at` was unspecified. | Added an explicit `none` input default and black-box case `CREATE-004`; logged missing input-default semantics. |
| The validation plan required invalid-owner, reassignment, and parent-deletion cases, but only invalid-owner existed. | Added `REL-003` for prohibited reassignment and `REL-004` for parent hard-delete restriction. |
| Changing a due date after a reminder could leave the todo permanently ineligible. | The patch now clears `reminder_sent_at` whenever `due_at` is supplied; added `PATCH-005` and logged the derived-update requirement. |
| Authentication declarations mapped claims but did not state active-user resolution. | Added explicit resolution through `User.authentication_subject` with active status. |
| Patch-derived reminder clearing contradicted the one-line `job_only` field policy. | Expanded the field policy: the job may write delivery time, a due-date patch may only clear it, and clients can never write it directly. |
| Provider email claims could become application identity even though the database owns user email. | Removed provider-email mapping; actor user ID and email now come from the active-user resolution. |
| Readiness was modelled as an operator route without an operator identity or policy. | Moved readiness to the deployment plane; it is no longer an application route or a third implicit authentication scheme. |
| Several service/job types were referenced but undeclared, and a pure helper was unused. | Declared service message/receipt values and mapped failures; simplified job actions to `None`; removed the unused helper. |
| The acceptance contract required `201`/`204`, but source relied on an unspecified success-status convention. | Added typed `created` and `no_content` route success declarations and logged the missing route capability. |
| List query parameters allowed both omission and `none`, although URL query decoding needs only omission. | Made `due_before` and `after` non-nullable optional fields. |

## 2026-09-27 authentication/configuration alignment

| Finding | Resolution |
|---|---|
| The candidate treated authentication as user-only OIDC/session mapping and omitted API/service clients. | Expanded the frozen behavior matrix to browser, opaque/JWT user API, and service API-key/exchange paths that all produce a closed user/service principal. |
| The old actor shape mixed provider subject with authoritative profile data. | Kept provider claims only as intermediate resolution input; authoritative user/service records own application identity and profile data. |
| Revocation semantics implicitly required a lookup on every request. | Selected bounded mode for ordinary traffic, explicit maximum delay, and fresh-authority checks for sensitive routes; correctness never depends on a cache. |
| Configuration used positional `secret required restart/reloadable` modifiers and a second lifecycle choice. | Migrated the golden configuration to structured in-source binding bodies; all v0.1 values are startup-bound and secrets have no defaults. |

## 2026-09-30 authentication reconciliation

Authority: approved AUTH-001 section 2, plus the owner's explicit clarification
that a fresh route's budget is **one principal lookup, with credential/session
checks counted separately**. This is an author-side candidate correction, not
an independent P10R approval or a claim that any golden case executed.

The pre-reconciliation candidate is preserved in commit `8160b94` (acceptance
SHA-256 `a2e06d9622c2e212bbfa0f9a3e9e2adc496888cfa555dade96013bc7b67f5606`). The revised acceptance SHA-256 is
`acfc317db191764ebef1714f22af2f37e6ae1b93c2c609c195527197ef252ab0`. All 44 case IDs remain; non-authentication cases other than the
explicit user-disable follow-ups retain their exact content. `policy.jadpo`
and the approved AUTH section-2 contract are unchanged by this reconciliation.

| Case | Correction and preserved obligation |
|---|---|
| AUTH-005 | Two presented credentials yield 401 `ambiguous_credentials` before validation, even if one is invalid. Zero database queries remains required. |
| AUTH-006 | Pin issuance, disablement, request and expiry times. The existing fresh `DELETE /users/alice` must return 403 `user_disabled` for an otherwise valid unexpired cookie, with valid browser/CSRF context and no business writes. |
| AUTH-008 | Call the credential a short-lived signed bearer, matching both the source validator and bounded behavior. Cache outage succeeds without either principal- or credential-authority queries. This does not pretend that an opaque credential can skip authoritative validation. |
| AUTH-010 | Use declared `DELETE /users/alice` and its 204 result; the previous POST route did not exist. Assert exactly one principal-authority query and record credential checks separately, as the owner chose. No exact total-authentication-query budget is inferred. |
| USER-001 | Keep disabled status, 204 and retained todos. Add explicit unexpired fresh rejection, failed refresh with no credential issuance, and ordinary rejection at original credential expiry within the five-minute bound. An ordinary request need not succeed during the grace interval; earlier denial is allowed. |

`Principal.user.email` was also inconsistent with lookup-free signed validation:
AUTH-001 excludes profile data from signed credentials. No business expression
in this candidate used that principal field. Remove it and its two resolution
mappings; application-owned email remains on `User` and its projections and is
read through application data operations. Never copy provider email into a
credential or profile field to satisfy a zero-query assertion.

Query counters now have explicit non-overlapping categories. A combined
credential/principal SQL query is counted once as principal authority; additional
credential/session queries are counted separately. Application queries remain
separate from those authentication counters. This records physical queries,
not a single logical resolver call that could conceal multiple database reads.

These corrections do not implement the golden app. Service/JWT adapters, the
candidate exchange endpoint, canonical entity/policy migration, route success
and input/query bindings, lifecycle behavior and job/service contracts remain
tracked implementation or design work. Refresh follow-up names the compiler-owned
operation and named `UserDisabled` failure instead of inventing an HTTP endpoint
or treating a host operation as an HTTP response. A future executable harness
must execute every timed follow-up; structural candidate validation is not
runtime evidence.

## Verification

Run:

```text
python3 tools/verify-p10r.py
```

The verifier checks the required package, parses and structurally validates the
acceptance contract, rejects duplicate/malformed case IDs, verifies required
case families and local documentation links, and prints deterministic SHA-256
digests. Printed digests are informational until independent review has resolved
the candidate and a separate freeze decision records them.

## Independent-review checklist

- Verify each acceptance case is permitted by policy and represented by source.
- Identify unspecified observable behaviour rather than filling it in from
  framework convention.
- Challenge exactly-one authentication and invalid-beside-valid handling.
- Challenge concealment, timing, and enumeration behavior.
- Review patch omission, status transitions, reminder replay, due-date changes,
  and job concurrency/failure windows.
- Review user disablement, retained children, hard purge, and existing-data
  migration behavior.
- Trace every audit claim to a proof rule, threat entry, test, or explicit
  residual risk.
- Verify TypeScript receives an expert baseline and equal agent context.
- Record disagreements and disconfirming evidence; do not edit the candidate
  silently during review.
