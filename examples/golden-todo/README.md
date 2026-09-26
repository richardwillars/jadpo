# Golden todo design contract

**Status:** candidate freeze for P10R review  
**Contract version:** `todo-v0.1`  
**Compiler support:** intentionally incomplete

This directory defines the todo backend that P10.5 and P11 must implement. It
is design evidence, not a demonstration of what the current compiler happens to
support. The source deliberately uses required authentication, policy,
omission-aware patches, lifecycle, job, service, configuration, and migration
constructs before their compiler implementations exist.

The package has two independent authorities:

1. [`policy.jadpo`](policy.jadpo) defines human-owned permission, field, effect,
   and lifecycle intent. Nothing else may weaken it.
2. [`acceptance.json`](acceptance.json) defines externally observable behaviour
   within that policy envelope.

The remaining artifacts are subordinate evidence:

3. [`app.jadpo`](app.jadpo) is the proposed canonical implementation source.
4. [`expected-audit.md`](expected-audit.md) defines the derived review surface.
5. [`adversarial-changes.md`](adversarial-changes.md) defines change-pressure
   cases and required compiler dispositions.
6. [`typescript-baseline.md`](typescript-baseline.md) freezes the comparison
   stack and equivalent obligations.
7. [`language-friction.md`](language-friction.md) records every construct not
   expressible by the compiler at the time of this candidate freeze.
8. [`REVIEW.md`](REVIEW.md) records author-side contradictions and the pending
   independent-review checklist.

The cross-package
[`evidence-map-v0.1.json`](../../tests/assurance/evidence-map-v0.1.json) links
every acceptance case to named proof/runtime-validation rules and threat-model
entries. It also keeps planned evidence visibly distinct from evidence that
already exists.

If source and acceptance behaviour disagree, the contradiction blocks freeze
until it is resolved. If acceptance behaviour exceeds policy, policy wins and
the case is invalid until a human-reviewed contract revision reconciles them.
An agent may not edit policy to make an implementation compile. Policy changes
require the approval protocol defined during P10R.

## Product behaviour

The application supports:

- session-cookie and OIDC bearer authentication normalised to one actor;
- authenticated-by-default routes and one explicit public health route;
- fail-closed handling of invalid, conflicting, or ambiguous credentials;
- owner-scoped todo CRUD with omission-aware patches;
- stable cursor pagination and stable child ordering without N+1 reads;
- explicit owner relationship integrity and deletion lifecycle;
- optional due dates, overdue reminder scheduling, and a declared mail service;
- a soft-delete retention window followed by an idempotent purge job;
- typed configuration, secret handling, readiness, and startup preflight;
- generated structural tests plus authored business tests; and
- derived route, data, effect, failure, configuration, and policy evidence.

Sharing and administrator access are not present in the initial contract. They
enter through the frozen change sequence so the experiment can measure policy
evolution rather than beginning at the final answer.

## Freeze rule

This is a **candidate** freeze. P10R review may correct ambiguity once. After it
is marked frozen, changes require a new contract version and a rationale; P11
must not weaken or rewrite the cases to fit compiler limitations. A case may be
classified `unsupported`, `cannot prove`, or `requires human decision`, but it
may not silently disappear.

Run `python3 tools/verify-p10r.py` from the repository root to validate the
package and print informational candidate digests. Those digests are not a
freeze record until the independent P10R review is complete.
