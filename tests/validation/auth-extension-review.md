# Independent authentication extension review

**Date:** 2026-09-30. **Scope:** the in-progress authoritative-result selector
change in `target.rs`, service-owner/JWT setting validation in `typecheck.rs`,
and the relevant first-party adapter boundary. This is a focused local review,
not an AUTH-P7/P8 exit, external security assessment or deployment approval.
Implementation continued concurrently; the statements below identify the
specific behavior checked rather than approving later unreviewed changes.

## Findings

### Resolved: declare JWT bearer-only in the semantic checker

The first inspected diff allowed a JWT validator on a cookie strategy. The
authoritative-result selector checked configured validator mode and principal
kind, while the external first-party result returned before browser CSRF checks.
The existing cookie rejection test did not cover that configuration: its cookie
strategy simply had no JWT validator. The supported-target gate at that point
still rejected JWT, so this was an integration hazard rather than evidence of a
deployed CSRF bypass.

Reported to the coordinating task before completion of JWT integration. It added
`TYPE_AUTH_JWT_TRANSPORT` and authored diagnostic copy. The new independent
regression changes the accepted bearer strategy into a distinct cookie slot
while preserving its real JWT validator; it now receives that diagnostic. The
unchanged JWT-bearer control remains valid. This implements the accepted
[AUTH-P6 bearer constraint](../../docs/authentication-plan.md#auth-p6--opt-in-jwt-bearer-adapter)
without weakening protected routes.

**No further blocking defect found in this bounded selector/settings review
after that repair.** This conclusion does not establish completion of JWT
target integration, package/install evidence, all service lifecycle cases or
broader principal mappings.

## Checks against the accepted contract

| Concern | Observation and limit |
| --- | --- |
| Exactly one credential; no fallback | Inventory and malformed/ambiguous checks still precede adapter validation. Existing duplicate-cookie, duplicate-bearer and competing-slot tests pass without adapter work. An authoritative result is accepted only for a JWT validator of the returned kind in the selected strategy. |
| Verified identity substitution | `normalizeAuthenticationResolution` checks principal kind and subject against the external result's verified kind/subject. Wrong subject and user/service substitution fail. JWT subjects acquire local IDs through authority; inventing a local UUID before that operation is unnecessary. First-party credentials separately pin stored local IDs in `authority(...)`. |
| Principal/profile disclosure | The new path uses the same closed `normalizeAuthPrincipal` schema as existing paths. Extra result keys, raw claims, undeclared profile/permission fields and malformed UUIDs fail as authority invariants. This establishes schema containment; real JWT claim projection and public-route serialization still need integration cases. |
| Duplicate authority work | A request-local authoritative result returns directly after normalization, including on fresh routes. Fake adapter tests report one validation and zero additional resolves. This proves no second selector read; the real JWT adapter must still prove that its first authority read actually occurred once. |
| Request isolation | No authority result cache was introduced by the selector. Forty delayed interleaved fresh/nonfresh calls retained their own subjects with zero extra resolve calls. This is bounded concurrency evidence, not a stress or race proof for the complete adapter. |
| Failure classes | Inactive, missing, duplicate and unavailable authority outcomes retain their respective declared/invariant classifications. Unexpected adapter throws are contained as availability faults. Recovery must not turn a provider/profile object into a principal. |
| JWT settings | Only issuer, audience and JWKS URI are admitted in JWT mode; nonsecret textual/URL configuration is required. Algorithm/package/raw-secret settings and duplicate settings are rejected. Endpoint settings on first-party modes are rejected. HTTPS/runtime endpoint validation remains a separate startup/adapter obligation. |
| Service ownership | Owner setting is restricted to a service API-key validator, its service authority entity, and a required nonsecret persistent reference. Invalid/nonreference/nullable/unrelated fields are covered by existing tests; mode/kind misuse and duplicate owner settings are covered by the added tests. Target generation additionally checks the supported identity-reference shape. |

The source of these expectations is the accepted
[authentication contract](../../docs/authentication-plan.md), especially the
closed principal, exactly-one-credential, service-owner, authority-freshness and
JWT bearer/claim-containment requirements. A valid authority result is trusted
compiler-owned adapter output, not an application- or client-authored envelope.

## Evidence actually run

Only targeted tests were run; no full verification gate or cloud operation was
performed by this review.

| Command / probe | Result |
| --- | --- |
| `cargo test --manifest-path jadpo/Cargo.toml -p jadpo-core --test auth_extension_review` | 4 passed |
| `cargo test --manifest-path jadpo/Cargo.toml -p jadpo-core --test auth_adapter_settings` | 3 passed |
| `bun --no-install --env-file=/dev/null test tests/runtime/authentication-selector.test.ts` | 13 passed, 81 assertions |
| Ad hoc Bun stdin probe against the generated selector | 4 malformed/profile principal cases rejected; 40 interleaved authoritative requests retained their own subjects; no additional resolve calls |

The runtime commands used the generated authentication-selector fixture already
present from the coordinating work. Its authoritative-result branch was
inspected before execution. This review did not independently run a complete
checked regeneration or full HTTP/JWT boundary suite. The ad hoc probes were
not added as persistent authored tests and must not be counted as new suite
cases.

New persistent test file:
[`auth_extension_review.rs`](../../jadpo/crates/core/tests/auth_extension_review.rs).
It contains the bearer positive control, actual cookie-JWT regression,
service-owner mode/kind rejection and duplicate owner rejection. No compiler,
runtime, shared test, auth-document or roadmap file was modified by this review.

## Integration checks still owned by the coordinating task

- Prove supported target generation is bearer-only and initially user-JWT-only;
  reject multiple JWT validators sharing a slot when selection/settings would
  be ambiguous. These are bounded target-support limits, not a new language
  claim that all service JWTs can never be supported.
- Count the real external path's authority operation on both normal and fresh
  requests, including repeated requests, disabled/missing/duplicate rows and
  database unavailability. Zero extra selector resolves must not be mistaken
  for zero total authority checks.
- Verify a signed JWT carrying profile/role/permission claims cannot substitute
  authoritative local fields or enter public output/logs, and that wrong-slot
  JWTs and invalid-beside-valid credentials fail before business work.
- Retain existing signed/opaque/API-key revocation and principal-ID replacement
  tests across the integration. Do not let external dispatch create a fallback
  path after first-party credential rejection.

Cloudflare readiness is unrelated to this auth result: the coordinating task
reported a successful Workers-subdomain read but a permission failure reading
subscriptions. Account plan remains unverified and no deployment is claimed.
