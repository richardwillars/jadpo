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
