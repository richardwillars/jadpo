# Jadpo fixture contract

Each fixture is a `.jadpo` file paired with an `.expect.json` file of the same
base name. The compiler test harness discovers these pairs recursively.

After building the CLI, run `ruby tests/compile/verify.rb` from the repository
root to verify pass/fail status, diagnostic aliases, and exact primary byte
ranges for every pair. Selected semantic assertions are additionally exercised
by the Rust compiler tests.

Expectation format version 1 contains:

```json
{
  "fixture_version": 1,
  "result": "pass",
  "semantic_assertions": {}
}
```

or:

```json
{
  "fixture_version": 1,
  "result": "fail",
  "diagnostics": [
    {
      "code": "TYPE_EXAMPLE",
      "primary_match": "unique source text"
    }
  ]
}
```

`primary_match` must occur exactly once in the source file. The harness converts
it to an expected byte range, allowing readable fixtures without brittle manual
line/column maintenance. Compiler diagnostics still emit ordinary line/column
and byte spans.

Pass fixtures may assert selected semantic facts rather than snapshotting the
entire internal representation. The current harness checks selected graph,
typing, failure, disclosure, and route facts as well as diagnostic codes and
exact primary spans.

Diagnostic codes in these fixtures are part of the compiler-facing contract.
Renaming one requires updating the roadmap, relevant specification, and all
affected fixture expectations together.

The current 180 pairs comprise the original ten type/failure cases plus
coverage for optional omission, nullable widening, implicit narrowing,
semantic-to-primitive unwrapping, incomplete records, invariant collections,
transitive failure propagation, function/action separation, missing failure
context, duplicate public codes, typed failure context, nested selection
through structured fields, and rejection of nested selection before nullable
record handling, typed entity creation, and rejection of persistent writes from
functions, typed optional entity lookup, rejection of reads from functions,
rejection of nullable query predicates before cross-adapter null semantics are
defined, typed required lookup with an explicit not-found binding, rejection of
a non-`NotFound` binding, rejection of an undeclared missing failure, typed
required update/delete, accepted fixed-shape multi-field updates, rejection of
duplicate update fields, enforcement of mutation missing/conflict failure
kinds, named compound uniqueness, precise conflict binding, rejection of
unknown constraint identities, accepted omission-aware input patches, rejection
of required patch fields and unknown entity fields, enforcement that an
empty-patch failure derives from `InvalidValue`, patch-derived writes and
overlap rejection, and valid optional owning-parent traversal with rejection of
a required traversal over a nullable reference, plus unique-backed optional
inverse-one loading.
The latest fixtures also cover the bounded owning-parent/optional-inverse
depth-two path, its exact nested output type, and an explicit logical owning
relationship name that remains distinct from its stored foreign-key field.
Fixtures 51–52 close the initial value-semantics boundary: a `var mut` local may
be rebound only with a value compatible with its established nominal type,
while parameters and ordinary `var` bindings produce stable
`TYPE_ASSIGN_IMMUTABLE` diagnostics.

Fixtures 53–58 add the language-learning completion slice: plain and
data-carrying closed enums, exhaustive matching over enums, booleans, open
scalars and nullable values, `some(value)` narrowing, fixed operator
precedence, and authored `test`/`assert` blocks. Their failure cases cover
invalid payload construction and patterns, incomplete matches, incompatible
operators, and non-Boolean assertions.

Fixtures 125–159 extend the contract with typed configuration and secret flow,
the closed authentication principal/strategy/resolution graph and boundary
reachability, canonical named arguments, Temporal/locales/generated lifecycle
fields, language-wide casing and standard namespace ownership, fixed-clock and
typed-config test fixtures, scoped direct/membership policy, central effect
inheritance, field/output/relationship proofs, application invoke policy, and
derived restricted-field disclosure rejection. Recursive multi-file fixtures
remain one pair each.

Fixtures 59–70 establish the executable P10.6 subset: explicit `kind` members,
flat failure context, exact `fails` sets, mandatory `attempt` at fallible call
sites, typed and exactly matched route placeholders, `auth: none`, and exactly
one named or inline route behaviour. They also reject stale failure declarations
overlapping public/internal context names, and unacknowledged persistence
expressions. They also cover function-level failure propagation, fallible pure
calls, inline-action exact failure sets, multi-placeholder typed paths, and the
authenticated route default.
Path fields also reject nullable, optional, constraint, reference, and
persistence modifiers so transport decoding cannot silently acquire entity
storage semantics.
Duplicate members in a callable's exact `fails` set are rejected rather than
silently deduplicated.
An inline action's declared failure surface is also derived into route
inventory and OpenAPI in the same way as a named action.
Duplicate route items are diagnosed rather than silently replacing an earlier
transport or behaviour declaration.
Block-valued route items use the same colon separator as scalar items;
`path:` and `action:` are canonical and the compiler offers an automatic edit
for the rejected colonless spellings.
Fixture 71 covers an invalid route authentication value as one human-owned root
diagnostic. It also asserts the agent-facing summary, reason, owner, kind,
bounded context, two security choices, edit replacements, and public-contract
impact rather than accepting a generic token error or dependent route-item
cascades.
Fixture 72 covers a route item written at file scope. It requires a concrete
top-level-declaration explanation and guided alternatives, rather than the
identifier-derived “expected declaration” fallback.
Fixture 73 proves that `auth`, `input`, `output`, and `run` receive the same
single-step colon insertion as block-valued `path` and `action` items, while
the parser continues through all later route members.
Fixture 74 proves that a remaining general parser expectation publishes the
actual required syntax and encountered token. The public message may not fall
back to “unexpected token.”
Fixtures 75–76 cover the lexer-level invalid-escape and unexpected-character
diagnostics that complete catalogue discovery recovered from behind escaped
Rust string literals. Both assert exact source ranges and authored repair copy.
Fixtures 77–80 complete real trigger coverage for the route family: a missing
behaviour, duplicate typed path binding, duplicate URL placeholder, and invalid
placeholder spelling. Each remains a single root diagnostic with exact public
copy rather than a dependent parser cascade.
Fixture 81 completes the failure-family trigger set with duplicate and unknown
flat context values at a rejection site. It also fixes the duplicate-field copy
to describe a value supplied twice rather than a declaration repeated twice.
Fixtures 82–85 complete semantic-name trigger coverage: duplicate declaration,
unknown reference, wrong declaration kind, unknown callable, and a resolved but
non-callable failure name.

Fixture 108 records the executable unified-data-model prototype: object shapes and
their nested object/list fields use `type`, standard `Email` is available from
the prelude, persistence is declared separately with `persist`, and callable
failures appear before the return arrow. Its negative companion requires an
unknown persisted field to produce one contextual `SEM_UNKNOWN_NAME` error.
DATA-007 now accepts the first-class entity/query/transaction model, so this
fixture remains comparison and compatibility evidence rather than final
language authority.
Fixture 109 proves that the prelude-owned `Url` and `IpAddress` constructors
reject invalid constants at compile time.
Fixture 110 begins P10.7 with the complete permitted pure-call matrix:
function-to-function, action-to-function, and action-to-action calls, including
transitive chains and a named action route boundary.
Fixture 111 proves that target suspension is inferred from persistence and
propagated through action calls while a pure helper remains synchronous and
free of target-level `Promise` or persistence parameters.
Fixture 112 rejects authored `async` and `await` with explicit effect diagnostics
while recovering around the ordinary action declarations and calls.
Fixture 113 makes exhaustive outcome matching executable for local recovery,
explicit failure mapping, and exact propagation without exposing a source-level
`Result` value. Fixture 114 rejects missing or duplicate success arms, missing,
duplicate, unknown, and wildcard failure arms, infallible or non-call subjects,
and a success arm that does not produce the matched successful value.
Fixture 115 gives an unrecognised outcome arm its own authored pattern
diagnostic and exact conformance evidence.
Fixtures 116–123 cover the bounded DATA-007/TX-001/CONSISTENCY-001 negative
surface: mutation ownership and persistence capability, read-only queries,
mandatory freshness, explicit multi-owner consistency, same-domain atomicity,
query-to-action rejection, raw-query placement, and guarded value-receiver
mutation. Fixtures 120–121 on the passing side exercise a persistent entity
dossier with derived representation metadata, a named authoritative query,
qualified/dot operations, and an explicitly atomic same-domain composition.
The positive and negative nested role fixtures then prove that `entities/`
accepts one authoritative entity dossier and rejects unrelated top-level
behaviour with `DATA_PROJECT_ROLE_INVALID`.
Fixture 123 proves that a top-level named query can compose authoritative reads
from two persistent entities into a complete identity-free value while keeping
the exact failure and freshness contracts visible.

Fixture 126 on the passing side records the fresh-authority route requirement.
Fixtures 126–127 on the failing side and fixture 127 on the passing side begin
AUTH-P1: one application owns an immediate or positive bounded revocation
default, one closed principal supplies exactly one user and one service variant,
and the graph rejects duplicate project declarations, missing or duplicate
variants, and zero, missing, or inapplicable revocation delays.
Fixture 128 adds the AUTH-P1b credential topology: each named strategy owns one
reserved cookie or bearer slot and one or more named validators with explicit
signed, opaque, API-key, or JWT modes and closed user/service principal links.
Its failing companion rejects duplicate strategies, slots, and validators,
empty validator sets, unknown modes or principal variants, and bearer
credentials configured in URL query or path locations.
Fixture 129 begins AUTH-P2 by checking authoritative principal resolution:
credential claims cannot populate application-owned identity fields, lookup
authority must be unique, active-state predicates must be Boolean, mapped
fields must be nominally compatible, and every declared resolution constructs
a complete principal variant.

Expectation files retain established compiler codes as migration aliases. CLI
and LSP diagnostics expose the canonical lower-dotted `ruleId` alongside those
aliases through diagnostic schema version 2.

The first-party authentication secret-sink fixture rejects literal signing keys;
the generated first-party example and focused core test cover configured adapters
and the protected-route generation gate.
