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

The current eighty-four pairs comprise the original ten type/failure cases plus
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

Expectation files retain established compiler codes as migration aliases. CLI
and LSP diagnostics expose the canonical lower-dotted `ruleId` alongside those
aliases through diagnostic schema version 2.
