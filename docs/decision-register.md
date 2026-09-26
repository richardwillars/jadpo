# Decision register

**Status:** current design authority  
**Last consolidated:** 2026-09-26

This register separates decisions from attractive ideas. Changing an accepted
item should update the charter, affected specifications, and examples.

## 1. Accepted decisions

### Product and philosophy

- The target is backend development, initially ordinary SaaS/web applications.
- The environment is designed for agent-primary authorship and human
  readability/editability.
- The LLM is not trusted to provide assurance for its own implementation.
- Human intent and policy, LLM implementation, and compiler verification have
  distinct authority.
- The compiler is deterministic and deliberately boring.
- The optimisation target is minimum ambiguity per token, not minimum
  characters.
- Common constructs should have one canonical form.
- Do not name the language during the hypothesis phase.

### Bindings and data

- `var` is an immutable binding.
- `var mut` is a mutable binding.
- Rebinding an immutable value is invalid.
- Assignment rebinds only a bare, fixed-type `var mut` local with a compatible value; parameters and
  fields are not assignment targets.
- Parameters and returned data have immutable value semantics. Copying, moving,
  sharing, and copy-on-write are permitted implementation strategies only when
  source code cannot observe the difference. There is no accepted `ref`, alias,
  address, or `inout` syntax.
- There is no language `undefined`.
- There is no ambient language `null`.
- `T?` expresses a value that may be absent; the absent value is `none`.
- Omission is separate from nullability and belongs to input/object shape.
- Concrete typed objects always contain all declared fields.
- Database fields are non-nullable by default.
- Database `NULL` maps to language `none` for `T?`.
- Partial selections produce projection types, not partial entities.
- There is no general truthiness.
- A field type on a named record-like declaration is referenced as
  `Type.field`, such as `Customer.id` or `Customer.email`; it retains semantic
  identity rather than collapsing to its representation primitive.
- A field type is a nominal refinement of its declared type. It may widen to
  that type, but base-to-field narrowing and sibling-field substitution are not
  implicit.
- Selecting through a field whose declared type is a named record resolves
  members against that declared record. For example,
  `customer.billing_address.postal_code` has type `Address.postal_code` when
  `Customer.billing_address` refines `Address`; the compiler does not invent a
  synthetic `Customer.billing_address.postal_code` type.
- Field references inherit intrinsic value validation and nullability, but not
  shape omission, defaults, indexes, uniqueness, existence, authorisation, or
  permission to read a field.

### Types and boundaries

- Types are executable contracts rather than annotations.
- Every authored data shape is declared with `type`. Scalar refinements use
  `type Name = Parent { ... }`, structured records use
  `type Name = Object { ... }`, and closed alternatives use
  `type Name = Enum { ... }`. The former top-level `value`, `input`, `output`,
  `entity`, and `enum` declaration forms are superseded rather than retained as
  equivalent canonical spellings.
- `input:` and `output:` are boundary roles that reference any declared type;
  they are not type categories. Input use derives recursive closed decoding and
  validation, while output use derives recursive closed validation and
  serialization. Reusing a type at both boundaries never bypasses either
  boundary check.
- `Object` supports recursively nested closed object fields. `List<T>` supports
  recursively validated arrays, including `List<Object { ... }>`; compiler-
  synthesized nested identities remain anchored to the containing field rather
  than introducing structural compatibility between unrelated objects.
- Persistence is a separate opt-in declaration, `persist Type { ... }`. A
  normal object type is never stored merely because it has an `id` field.
  `persist` owns identity, uniqueness, indexes, references, inverse
  relationships, and migration consequences. Omitting it leaves a fully usable
  non-persistent application type.
- The initial prelude contains representation/time types `Bool`, `Int`,
  `Decimal`, `Text`, `Bytes`, `Uuid`, `Date`, `Time`, `DateTime`, `Duration`, and
  `Unit`; containers `Object`, `List<T>`, `Set<T>`, and `Map<K, V>`; and the
  compiler-owned validated semantic types `Email`, `Url`, and `IpAddress`.
  Policy-dependent concepts such as username, slug, phone number, postcode,
  money, and country code remain authored domain types rather than vague
  built-ins.
- Semantic and field types are nominal. Matching primitive representations or
  structures do not create implicit compatibility.
- `Type(value)` constructs a semantic or field value by validation; it is never
  an unchecked cast. Constant arguments are validated during compilation and
  dynamic arguments at runtime.
- Failed dynamic construction is compiler-visible and must be declared, mapped,
  or handled rather than returning `none` or throwing an ambient exception.
- Runtime validation occurs at HTTP input/output, database read/write,
  environment/config, queues/events, external responses, and deserialised cache
  boundaries.
- A successfully typed application value has actually been validated.
- Domain constraints should drive all relevant static, runtime, database, API,
  documentation, and test representations.
- A constrained semantic type describes an open set of validated values; its
  individual string or numeric values are not enum members. Finite domain
  alternatives and lifecycle states use an enum or another explicit domain
  model rather than magic sentinel values inside an open semantic type.
- Authored source references enum variants only with qualified dot syntax, such
  as `InviteStatus.blocked`. Call-style construction from a literal or dynamic
  representation—such as `InviteStatus("blocked")` or
  `InviteStatus(raw_text)`—is invalid. External values become enum variants only
  through declared boundary decoding or an explicit typed parsing operation.

### Backend semantics

- Database operations are compiler-understood constructs.
- Postgres is the initial opinionated database.
- Persistent creation uses `create Entity { ... }`, is permitted in actions but
  not functions, requires nominal entity-field values, and returns a validated
  entity rather than a driver result.
- Persistence SQL is compiler-generated and parameterised. Returned rows cross
  a generated validation boundary before becoming trusted entity values.
- The first read form is `query optional Entity { where: field == value }`. It
  returns `Entity?`, accepts one equality predicate on a non-nullable nominal
  entity field, treats multiple rows as an operational invariant fault, and is
  action-only in the initial effect model.
- Required-one read uses the same predicate plus `missing: FailureName`; its
  result is non-nullable and the bound failure must be declared and derive from
  `NotFound`.
- Required update/delete bind zero rows to a declared `NotFound` failure and
  database constraints to a declared `Conflict` failure. They return the
  validated affected entity rather than a row count.
- Required mutations preflight cardinality and execute transactionally; a
  multiple-row match is an operational fault and commits no mutation.
- Every action whose transitive effect graph contains a persistence mutation is
  atomic by default. The compiler supplies one transaction-scoped persistence
  context to all reads, writes, and nested callable invocations in that action.
  Nested mutative calls reuse the active transaction; read-only actions do not
  start a write transaction. Authors do not spell the routine boundary.
- Raw SQL is not ordinary application code.
- A stored owning reference may declare a separate logical traversal name with
  `references Target.field as relationship on_delete action`. The stored field
  remains explicit, omission of `as` uses that field name, and the compiler
  never infers a relationship name by stripping `_id` or another suffix.
- External calls occur through reviewed, typed service contracts.
- External/provider failures are normalised at the service boundary.
- Application failures are typed, declared once, and reusable. Every callable
  has a closed set of recoverable application failures and source-agnostic
  operational problems; each must be handled, mapped, or propagated through
  `fails`.
- Recoverable operational problems use a deliberately small vocabulary such as
  `Unavailable`, `TimedOut`, `RateLimited`, and `OutcomeUnknown`. Adapter,
  vendor, and storage names remain internal provenance rather than source-level
  problem types.
- Defects and impossible runtime states are not catchable failures. They are
  contained and reported at the runtime boundary.
- `attempt` is the single explicit mechanism for unwrapping, handling, mapping,
  or visibly propagating recoverable problems. Exhaustiveness is checked after
  declared propagation.
- Every fallible expression must be acknowledged with `attempt`. After local
  handling and mapping, the enclosing callable's authored `fails` clause must
  exactly equal its reachable unhandled problem set; a missing or stale extra
  entry is a compile error. The compiler may explain the inferred difference,
  but it never silently adds to or repairs the source contract.
- Every application domain failure derives from exactly one standard failure
  kind; the kind supplies default transport and runtime behaviour.
- An application failure declares its standard kind with an explicit `kind`
  member, a stable public `code`, an optional safe static `message`, and
  declaration-owned `public` and `internal` context schemas. Production sites
  supply one flat object whose fields are checked for completeness, uniqueness,
  lexical availability, and exact nominal type.
- Application code cannot throw arbitrary exceptions, reject strings, select
  numeric HTTP error statuses, or construct ad hoc error responses.
- Public failure output contains no application data by default. Any public
  detail has an explicit schema; internal context never implicitly crosses the
  client boundary.
- Expected domain failures use semantic traces without native stacks by default.
  Operational faults and defects retain cause chains and low-level stacks
  internally; public clients never receive them.
- `NotVisible` is distinct from `NotPermitted` so policy can conceal resource
  existence with a 404-shaped response.
- Disabling authentication on a route is explicit and conspicuous as
  `auth: none`.
- Required authentication is inherited by every route and is not repeated as
  `auth required` in canonical source.
- Provider-specific authentication terminates at a generated boundary. Routes,
  actions, and policy consume a stable authenticated actor, tenant, allowlisted
  user data, permissions/capabilities, and authentication strength rather than
  provider SDKs, tokens, or strategy names. Multiple configured strategies must
  resolve deterministically and must not merge privileges implicitly.
- A route is the HTTP boundary, an action is effectful domain behaviour, and a
  function is computation. The three remain distinct even when behaviour is
  colocated.
- A small, one-off endpoint keeps an inline `action` inside its route. A named
  action is extracted only for reuse, a stable domain command, or a boundary
  that deserves independent testing, policy, or transaction reasoning.
- A route contains exactly one behaviour form: either an inline `action` or a
  `run:` invocation of a named action. `run:` exists only for the extracted
  form and makes transport-to-domain argument mapping explicit.
- An inline action has the same semantics as a named action and declares its
  recoverable surface in its brace-delimited header, for example
  `action: fails TodoNotFound, Unavailable { ... }`.
- Route items use the field-style colon separator consistently, including
  block-valued `path:` and `action:` items as well as `auth:`, `input:`,
  `output:`, and `run:`. The former space-only spelling is invalid. Blocks use
  braces; indentation is never semantic.
- Path, query, header, and body bindings belong to the route transport
  boundary and are statically typed before the action runs. Authentication
  headers remain owned by the generated authentication boundary rather than
  ordinary action input.
- Route templates spell path placeholders as `{name}`. A brace-delimited
  `path: { name: Type }` group types them, and behaviour reads the validated
  values through `path.name`. Template placeholders and declarations must
  correspond exactly one-to-one; missing, extra, or duplicate names are
  invalid.
- Callable signatures place their closed recoverable surface before their
  success type: `action name(parameters) fails A, B -> Result`. This ordering
  applies consistently to actions and fallible functions; failure-free
  callables remain `name(parameters) -> Result`.
- `:` associates a named member with a type, value, or setting. Constraint and
  persistence settings therefore use `min_length: 6`, `identity: id`, and
  corresponding colon forms. `=` is reserved for defining a named type or
  binding/assigning a value. Parentheses are limited to callable interfaces,
  calls and scalar constructors, explicit expression grouping, and constructor-
  shaped patterns; persistence metadata does not masquerade as a function call.
- Policy is human-owned; audit is compiler-derived.
- CI blocks policy/implementation disagreement.
- Generated target code is not normal developer-facing source.
- `build/` is compiler-owned disposable output and is excluded from authored
  `.jadpo` discovery.
- P8 derived JSON artifacts are deterministic, versioned, project-relative,
  and contain no timestamp, random identifier, or machine-specific path.
- The first executable prototype is dependency-free TypeScript generated for
  Bun beneath `build/target/app.ts`; `build/` is reproducible and not committed
  by the canonical scaffold.
- Generated Bun targets may import only `bun`, `bun:*`, and compiler-owned
  relative TypeScript modules. They emit no package manifest, lockfile, or
  `node_modules`, and must build and execute with Bun auto-install disabled.
- Required-one queries bind absence explicitly with `missing:` to a failure
  declared by the enclosing action and derived from `NotFound`; they never
  convert absence into an untyped exception.
- Persistence adapters convert raw SQLite and PostgreSQL driver errors into the
  compiler-owned `PersistenceFault`, retaining the original error only as its
  internal cause.
- The Bun 1.2.20 PostgreSQL adapter uses `prepare: false`: parameter binding is
  retained, while named prepared statements and pipelining remain disabled
  until a pinned runtime upgrade passes the complete live rollback suite. This
  avoids an unresolved `SQL.begin()` promise after a prepared constraint error.
- Target generation fails for authenticated routes until the required-default
  authentication runtime exists. Missing security infrastructure never weakens
  a route to public access.
- IDE semantics should come from one compiler-backed LSP. TextMate and renderer
  grammars are presentation adapters and cannot become a second parser or type
  system.
- Editor navigation must not underline ordinary Jadpo source. Generated-
  artifact navigation is exposed through CodeLens or explicit commands, not
  document links whose decoration competes with diagnostic squiggles.

### Diagnostics and observability

- Diagnostic definitions live in one versioned compiler-owned catalogue. Call
  sites supply typed facts; they do not invent user-facing prose, repair advice,
  or editor-only variants.
- User-facing rule identifiers use readable lower-case dotted names such as
  `failure.must_be_declared`. Existing upper-case codes remain migration aliases
  only while fixtures and integrations move to the catalogue.
- A diagnostic is a guided repair protocol rather than an error string. It has
  a human-first `summary`, a short `reason`, one `recommendedNextStep`, bounded
  `alternatives`, a precise `location`, typed `context`, bounded `impact`, a
  `decisionOwner`, and a stable `helpId`.
- Every error classifies its next step as an automatic fix, a guided semantic
  choice, or a human-owned decision. The compiler marks a fix as preferred only
  when it can justify that preference, previews its behavioural/public-contract
  impact, and validates the edit against the diagnostic's source revision.
- The same semantic diagnostic object generates human CLI text, versioned agent
  JSON, LSP diagnostics and code actions, and reference documentation. Raw JSON
  is not the normal IDE presentation.
- IDEs show the plain summary in the Problems panel and inline squiggle, then
  show the reason, recommended step, alternatives, impact, and help link through
  hover and an expandable details view. Quick Fix lists the recommended verified
  edit first and cleanly separates other valid choices; multi-file edits receive
  a diff preview.
- Parser cascades and dependent consequences are grouped beneath one root cause.
  Pipeline summaries such as "type checking failed" are report status, not
  additional user problems.
- Public catalogue entries may not fall back to identifier-derived summaries,
  “compiler-enforced invariant” reasons, or “update the source” advice. Every
  public code has authored, fixture-backed repair guidance and passes the shared
  [diagnostic presentation contract](diagnostic-presentation-contract.md).
- An invalid route authentication value is one root diagnostic over the value,
  not a token cascade. Because `auth: none` disables the authenticated default,
  the diagnostic is a human-owned security decision: retain authentication by
  removing the item, or intentionally use the exact `none` opt-out. Neither
  choice is silently preferred or applied by an agent.
- Compiler diagnostics, public HTTP failures, operational telemetry, and
  agent incident packets are distinct audience-specific schemas. None can be
  serialized directly as another.
- Public HTTP failures contain only the declared stable code, static safe
  message, request identifier, and explicitly public details. They never expose
  source locations, repair advice, adapter/provider data, stacks, or internal
  context.
- Diagnostic and logging APIs accept compiler-approved safe value types rather
  than arbitrary formatted values. Secret values are non-renderable; `internal`
  means client-hidden, not automatically safe for logs, telemetry, or an LLM.
- Operational telemetry uses a redacted vendor-neutral event model compatible
  with structured JSON and OpenTelemetry correlation. Provider adapters may map
  safe fields into third-party grouping, tagging, release, and trace features,
  but application code does not call vendor SDKs directly.
- Production events carry stable semantic operation and source-revision IDs. A
  trusted local tool joins those IDs with the compiler graph to build a rich,
  bounded agent incident packet containing source location, rule, context,
  impact, and repair choices without sending customer values or secrets through
  third-party telemetry.

### Process

- Every compiler slice is preceded by canonical source and executable-style
  acceptance fixtures; implementation must not silently decide semantics.
- The basic semantic compiler starts from the Jadpo seed application.
- The first complete application is a todo backend, required before persistence
  and authentication expansion is considered complete.
- The second is a deliberately awkward order/payment backend.
- Maintain a language-design issue log rather than silently inventing syntax.
- Compare against a strong TypeScript baseline and try to falsify the idea.
- Preserve per-change tokens, time, attempts, defects, diagnostics, workarounds,
  and human interventions for that comparison. Every language-friction incident
  receives an explicit disposition; the experiment must not silently change the
  language to make it outperform the baseline.

## 2. Provisional directions

These are recommended but must survive the golden applications:

- braces, no required semicolons, and canonical non-semantic formatting;
- `UpperCamelCase` types/entities/events/failures and `lower_snake_case` values,
  fields, functions, actions, and jobs;
- explicit domain primitive types such as `Email`, `Money`, `Percentage`,
  `DateTime`, and `Duration`;
- constrained-type blocks;
- explicit `List<T>`/`Map<K,V>` style collection types;
- free collection functions such as `count(items)` rather than prototype magic;
- exhaustive `match` plus `if`/`else` as the main conditional forms;
- data-carrying enum variants as nominal tagged sums when variant-specific
  payloads make invalid states unrepresentable, subject to P12 pressure tests
  and explicit boundary, persistence, compatibility, and migration semantics;
- no enum methods, traits, automatic display labels, or customer-facing string
  derivation merely as part of adding enum support; named functions and
  exhaustive `match` remain the initial behaviour model;
- `for item in items` as the initial iteration form;
- pure functions separated from effectful actions;
- `fails` for the exhaustive recoverable-problem surface and `reject` for
  producing application failures;
- the exact arm and replacement-value grammar inside `attempt`;
- declarative `create`, `query`, `update`, and `delete` syntax;
- the exact field syntax for typed query, header, and body bindings inside the
  accepted route boundary;
- Rust for a serious compiler/toolchain;
- TypeScript/Bun as a pragmatic first executable target;
- a possible native binary target later;
- stable semantic node IDs plus source maps and `app.meta`;
- checkpoint formatting rather than formatting every agent save;
- Candidate A's elastic single-application tree is the P9–P11 project-structure
  prototype, with a root `app.jadpo` allowed to grow into feature slices;
  final layout and enforcement level remain open until the golden applications
  pressure-test it;
- authored multi-file applications opt into explicit stable logical `module`
  headers, selective imports, and private-by-default declarations exported with
  `public`; the first bounded core requires one module per file and globally
  unique declaration names while aliases, re-exports, relative imports,
  multi-file modules, and same-name namespaces await golden-application
  pressure;
- a deterministic static project scaffold before dynamic profiles; interactive
  creation should be a thin interface over the same finite, reproducible flags
  used by humans and agents;
- prohibiting raw representation primitives in ordinary application callable
  signatures, while retaining them beneath semantic types, in local
  computation, and in low-level standard-library facilities;
- the initial standard failure-kind catalogue and canonical HTTP envelope in
  the failure-model specification;
- a stable machine language identifier, separate from an eventual product
  name, shared by file associations, LSP, Markdown fences, and renderer grammar
  packages.
- explicit named imports for authored multi-file applications, replacing the
  prototype's ambient application-wide namespace once the golden todo is split;
  external dependencies remain separately declared and reviewed.
- the P10R candidate policy kernel's five-way judgement (`proved`, `rejected`,
  `requires human decision`, `unsupported`, and `cannot prove`) with absence of
  proof always failing closed; final acceptance depends on independent review
  of [the kernel](policy-proof-v0.1.md) and its fixtures;
- independent policy-weakening approval bound to canonical policy and semantic-
  graph digests through a protected CI/review attestation, with ordinary
  repository files unable to confer authority; final acceptance depends on the
  [approval protocol](approval-protocol.md) and comprehension experiment; and
- the [golden todo candidate](../examples/golden-todo/README.md) as the complete
  P10R pressure case, while its unsupported syntax remains design input rather
  than accepted grammar.

## 3. Rejected current alternatives

- **A loose semantic language.** Rejected because ambiguity gives agents more
  room to drift. Formatting may be forgiving; meaning is strict.
- **Cryptic token minimisation.** Rejected in favour of semantic density and
  readability.
- **Both `null` and `undefined`.** Rejected as needless ambiguity.
- **Missing fields inside concrete typed values.** Rejected; use projections,
  optional input shape, or `T?` explicitly.
- **Structural compatibility for domain values.** Rejected; two values do not
  become interchangeable merely because both are represented as `Text`, `Int`,
  or another primitive.
- **Prototype/magic collection properties such as `items.empty`.** Rejected as
  ambiguous between data, method, and intrinsic.
- **Entity-method persistence as the preferred model.** `Order.create` and
  `order.update` obscure that persistence is a language-understood effect;
  `create Order` and `update order` are preferred.
- **JavaScript ambient exceptions for domain flow.** Rejected because signatures
  hide expected failure.
- **Manual route-by-route status mapping.** Rejected because transport behaviour
  belongs to the standard failure kind and must remain consistent.
- **Client stack traces and raw provider/database messages.** Rejected as an
  information leak; public failure data is allowlisted.
- **Rust-style `Result<T,E>` plumbing throughout source.** The safety goal is
  accepted; exposing the wrapper mechanism on every call is not.
- **Audit as editable source of truth.** Rejected; policy is source, audit is
  derived.
- **Formatting errors as semantic/compiler errors.** Rejected; use a formatter.
- **Reformatting on every tiny agent save.** Rejected provisionally because it
  can invalidate position-based iterative edits.
- **Generated TypeScript as a debugging or review surface.** Rejected.
- **Native compilation as the first proof.** Rejected as solving the wrong risk.
- **A language name now.** Rejected until the hypothesis survives.

## 4. Open language questions

- file, module, import, namespace, visibility, and package semantics;
- comment, documentation, intent, rule, and decision syntax;
- exact primitive types and literal forms;
- whether authored low-level functions need a conspicuous escape hatch for raw
  primitive parameters and results;
- money/currency, decimal, time-zone, date, and duration semantics;
- generics and user-defined collection types;
- value-representation optimisation for large data and foreign buffers,
  including copy-on-write, uniqueness, zero-copy views, and builders; any
  caller-visible mutation contract requires new application evidence first;
- lambda/closure support;
- necessity and limits of `while` and recursion;
- resource and termination bounds;
- function effects and whether functions may query persistence;
- query cardinality and exact not-found syntax;
- pagination, aggregation, joins, and complex-query capabilities;
- isolation selection, savepoints, deliberate boundary splitting, and the
  interaction between database transactions and external effects;
- relationship, index, uniqueness, and lifecycle syntax;
- partial-update mechanics;
- exact cause-wrapping and `attempt` arm/recovery grammar;
- final built-in operational-problem boundary table and non-HTTP adapter
  mappings;
- whether application authors can explicitly `panic`;
- output `none` as JSON `null` versus omission controls;
- exact route query/header/body binding grammar, plus file and streaming
  semantics;
- authentication identity, roles, tenancy, and ownership proofs;
- policy storage, protection, and approval mechanism;
- external contract import/versioning/review;
- events, jobs, concurrency, ordering, retries, and idempotency;
- typed configuration declarations, environment binding/overlay rules, secret
  classification and rotation, startup/preflight validation, dependency
  readiness checks, and the deployment rollback signal;
- test grammar and generated/authored boundary;
- escape hatches and extension model.

## 5. Open implementation questions

- semantic graph and proof representation;
- source-map/metadata format and monitoring integration;
- Postgres schema-diff and migration strategy;
- target deployment environments;
- IDE/CLI and machine-readable diagnostics;
- how human approval is distinguished from agent edits.

## 6. Open validation and product questions

- Can agents work effectively in a language absent from pretraining?
- Can a TypeScript framework deliver comparable guarantees?
- How often do real applications need escape hatches?
- Is the source genuinely better for non-CRUD business logic?
- Are generated audits and behavioural diffs understandable to humans?
- What credible commercial model exists beyond an open-source core?
- Can adoption, support, ecosystem, and trust costs be justified?
