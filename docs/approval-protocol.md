# Human policy approval protocol

**Status:** candidate P10R protocol  
**Security property:** separation of implementation and policy authority

## 1. Release rule

A policy weakening cannot produce release-equivalent success unless a protected
CI/review system supplies a valid attestation. An ordinary repository file,
commit signature, agent assertion, local environment variable, or generated
test cannot serve as that attestation.

Local builds may inspect and render an unapproved change. Their result must be
visibly `non-releasable` and use a distinct exit/status artifact. There is no
“temporary” success mode that CI can mistake for an approved release.

## 2. Canonical approval subject

The compiler emits a canonical `ApprovalSubject` containing:

- policy schema and compiler versions;
- before/after policy digests;
- before/after semantic-graph digests;
- a stable list of individual human decisions;
- affected actors, routes, entities, fields, relationships, lifecycles,
  services, secrets, jobs, configuration, and deployment surfaces;
- direct and transitive reads, writes, emissions, external effects,
  transactions, retries/idempotency, and failure/disclosure changes;
- newly possible counterexamples;
- evidence classifications and unresolved uncertainty; and
- the exact source/artifact provenance being considered.

Canonicalization sorts by stable semantic identity, includes absence explicitly,
normalises text and numeric encodings, and is versioned. The attestation binds
to the digest of the canonical bytes, not a UI rendering.

## 3. Decision granularity

Each weakening is a separate decision. A reviewer chooses the narrowest option
that expresses intent. Blanket “approve all changes” is not accepted when the
compiler can separate decisions.

An approval record contains:

- subject digest and decision identifier;
- authenticated reviewer identity and authority/role;
- decision (`approve`, `reject`, or `request clarification`);
- selected policy alternative and its exact delta;
- required rationale;
- issue/change identifier;
- issued-at and optional expiry;
- superseded approval identifier, when applicable; and
- review-system attestation identity.

The implementation agent identity may never satisfy the reviewer-authority
predicate for the same change.

## 4. Validation

CI rebuilds the subject from a clean checkout and verifies every decision. It
rejects an approval that is missing, expired, revoked, superseded, self-issued,
issued by an unauthorised reviewer, created for another compiler/policy schema,
bound to another graph/policy digest, incomplete for the current decision set,
or already consumed where the policy requires one-time use.

The initial implementation should use a protected code-review/CI attestation
provider with authenticated identities and branch/environment protection. The
project does not invent its own cryptography.

## 5. Review surface

The primary surface is an intent-first behavioural review, not a shortened code
diff. For each decision it presents:

1. the original request and why a human decision is necessary;
2. current versus proposed behaviour in domain language;
3. actor- and scenario-based “what becomes possible” examples;
4. direct and transitive semantic-graph paths;
5. reads, writes, effects, lifecycle, disclosure, configuration, and deployment
   impact;
6. proved, runtime-validated, tested, operational, assumed, unsupported, and
   uncertain claims as separate categories;
7. counterexamples and alternatives; and
8. the narrow choices available and exact policy delta of each.

Reviewers can drill into source, proof obligations, tests, runtime boundaries,
and generated target code without losing the decision context. A stable
non-graphical export contains the same facts for CI, accessibility, and archive.

The web UI consumes versioned jadpo artifacts. It may not infer policy or
semantic meaning independently.

## 6. Required adversarial cases

| ID | Attempt | Required result |
|---|---|---|
| AP-01 | agent adds an ordinary `approval.json` | reject: untrusted issuer |
| AP-02 | agent copies a valid approval from another change | reject: subject digest mismatch |
| AP-03 | policy unchanged but semantic graph differs | reject: graph digest mismatch |
| AP-04 | graph unchanged but policy differs | reject: policy digest mismatch |
| AP-05 | same reviewer is the implementation agent | reject: separation-of-authority failure |
| AP-06 | reviewer lacks policy authority | reject: authority mismatch |
| AP-07 | approval expired or was revoked | reject |
| AP-08 | one of two policy decisions is approved | reject release; show remaining decision |
| AP-09 | compiler/schema version changes interpretation | reject; require re-render/review |
| AP-10 | source is rebased without semantic change | accept only if canonical subject digest remains exact |
| AP-11 | UI summary omits a transitive external effect | fail review-surface conformance test |
| AP-12 | protected attestation provider unavailable | fail closed for release, permit local inspection only |

The same cases are frozen in machine-readable form at
[`tests/assurance/approval-protocol-v0.1.json`](../tests/assurance/approval-protocol-v0.1.json).
They are candidate expectations until the approval gate exists; documentation
alone is not implementation evidence.

## 7. Comprehension experiment

P10R compares a normal pull-request diff, a concise behavioural diff, and the
relationship/effect-aware decision view. Counterbalance order and measure
correct answers, critical risks noticed, false confidence, review time,
clarification requests, and approval quality. Include locally reasonable diffs
whose transitive behaviour is unsafe.

The protocol fails if identity mechanics are secure but reviewers cannot
reliably understand the decision. A target of at least 80% correct frozen
behaviour/policy/effect/lifecycle answers applies; critical misunderstandings
are reported individually rather than hidden in an average.
