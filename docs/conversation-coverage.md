# Source conversation extraction and coverage

**Source:** [shared ChatGPT conversation](https://chatgpt.com/share/6ab5a5d4-8f44-83ed-8381-4a9b8518bf3d)  
**Conversation title:** “LLM Backend Language Idea”  
**Shape:** eighty user prompt/voice segments plus assistant responses  
**Purpose:** demonstrate that the documentation preserves the substantive
content, rationale, alternatives, and unresolved questions from the source

This is not a verbatim transcript. Voice fragments, acknowledgements, repeated
ideas, and half-finished sentences are consolidated into their complete meaning.
Historical syntax is preserved where it influenced a later decision. Current
authority remains the [decision register](decision-register.md).

## 1. Chronological extraction

### 1.1 Initial proposition

The conversation opened with a language written specifically for LLMs to build
website backends. The first answer reframed it away from “a nicer language that
LLMs happen to write well” toward a narrow mixture of:

- database schema;
- API contract;
- permissions;
- business rules;
- events/background work;
- external service declarations.

Early examples introduced entities, actions, routes, events, jobs, services,
policies, and tests. The key proposed advantage was eliminating entire classes
of choices: no default raw SQL, ORM selection, web framework selection, folder
architecture debate, or arbitrary library choice.

The compiler would make invalid architectures difficult by requiring
authorisation, declared side effects, safe secrets, route schemas,
retry/idempotency behaviour, generated migrations, lifecycle choices, resource
limits, and transactions.

This introduced the first major thesis: **the compiler becomes part of the
LLM's reasoning loop**, presenting unresolved decisions as structured tasks.

Captured in:

- [vision](vision.md), sections 2–4;
- [semantic model](semantic-model.md), sections 1–2;
- [assurance model](assurance-model.md), sections 2–3.

### 1.2 Earlier backend-language idea and LLM feasibility

The user explained this was an older idea: a prescribed backend language where
a route kept auth, title/description, tests, validation, and documentation
together. Inputs, outputs, and values going to the database would be validated.

It had previously seemed irrational to create a new language when TypeScript was
battle-tested and unusual requirements eventually need general-purpose code.
LLMs made implementation feel more feasible and changed the value proposition:
the language could be simpler, predictable, and potentially more token-efficient
for agent authorship.

The conversation recognised the tension between a very narrow CRUD language
and the need for unusual or algorithmic business logic. A possible extension or
handoff mechanism was raised but not resolved.

Captured in:

- [vision](vision.md), sections 1, 6, and 7;
- [validation plan](validation-plan.md), sections 10–12;
- [decision register](decision-register.md), open escape-hatch questions.

### 1.3 Related tools and positioning

The assistant noted adjacent backend DSLs, schema-driven generators, low-code
platforms, and AI-native languages, but characterised most AI-native work as
agent orchestration or full-stack generation rather than a deliberately boring,
constrained backend language.

No external comparison in the conversation established market uniqueness. The
documentation therefore preserves this as context, not evidence.

Captured in:

- [product strategy](product-strategy.md), sections 2–4.

### 1.4 Living documentation and provider independence

The user emphasised project documents that update as functionality is
implemented, carrying context and decisions in the repository. A new developer
or model should not have to reconstruct instructions. The language should work
across LLM providers rather than depend on Claude Code or any other single
agent.

This became the code/intent/policy/audit model and the repository-as-living-
specification workflow, related to the user's Project OS thinking.

Captured in:

- [vision](vision.md), section 5;
- [agent workflow](agent-workflow.md), sections 3 and 8;
- [semantic model](semantic-model.md), intent/rule/decision concept.

### 1.5 Safety nets enabled by default

The user proposed that protections such as authentication should be enabled by
default on every route and that an LLM must explicitly disable them. The system
should make it hard to go wrong.

The discussion added easy, generated audits of each route's security properties
and suggested checking those audits against a separate statement of intended
security.

Captured in:

- [charter](charter.md), accepted foundations;
- [assurance model](assurance-model.md), sections 2–6;
- [vision](vision.md), section 9.

### 1.6 Policy ownership versus derived audit

The conversation identified the “marking your own homework” problem: if the LLM
maintains both implementation and its check file, the check may be meaningless.
The resolution was:

- human-owned intent/auth policy is the source;
- implementation may be agent-authored;
- audit is compiler-derived;
- policy and implementation are compared in CI;
- the build fails when they disagree;
- an agent-assisted policy edit must still be explicitly driven/approved by the
  user.

Captured in:

- [assurance model](assurance-model.md), sections 1, 4, and 8;
- [POLICY-001 decision plan](policy-plan.md), especially the qualified scoped
  role model, membership/direct bindings, automatic database scoping, complete
  input-to-output validation chain, and semantic weakening approval;
- [agent workflow](agent-workflow.md), section 7;
- [decision register](decision-register.md), accepted and rejected items.

### 1.7 Syntax strictness and formatting

The user asked whether syntax should be strict or loose and whether formatting
mistakes such as two spaces should fail compilation. The conclusion was:

- semantic structure should be strict;
- syntax should be labelled, predictable, and readable rather than cryptic;
- humans must be able to inspect and sometimes write it;
- whitespace differences should not be semantic;
- a formatter should normalise source;
- formatting on every save risks disrupting an agent's iterative edits;
- formatting should therefore happen at explicit/checkpoint boundaries such as
  compile, pre-commit, pre-merge, or CI.

Captured in:

- [syntax](syntax.md), source form and formatting;
- [compiler/runtime](compiler-runtime.md), section 13;
- [decision register](decision-register.md), accepted/rejected alternatives.

### 1.8 Postgres and declarative data access

The user liked an opinionated Postgres model where source could not normally
write SQL, while recognising complex queries as a pressure point. The assistant
described the model as a hidden ORM with declarative or safe query blocks and a
deliberate audited escape hatch for the minority of unsupported cases.

Captured in:

- [semantic model](semantic-model.md), entity/query sections;
- [syntax](syntax.md), entities and persistence;
- [compiler/runtime](compiler-runtime.md), section 7.

### 1.9 External API contracts

The conversation rejected requiring every provider to build a bespoke library.
Instead, OpenAPI or provider documentation could produce a reviewed contract.
The compiler would then understand permitted operations, schemas,
authentication, errors, retries, and egress. Documentation-to-contract
conversion could use an LLM once, followed by review and commitment.

Captured in:

- [semantic model](semantic-model.md), service;
- [syntax](syntax.md), external services;
- [compiler/runtime](compiler-runtime.md), section 8.

### 1.10 Toolchain and target debate

Rust and Zig were raised for implementing the language. Rust was preferred for a
compiler and safety. The assistant suggested generating TypeScript initially to
avoid rebuilding the world, with Bun or similar as a runtime and Postgres as the
boring infrastructure.

The user worried that a TypeScript target weakened differentiation and
performance. The conclusion was:

- a native binary is a cleaner possible long-term outcome;
- an initial TypeScript target is pragmatic;
- performance is not the first win;
- TypeScript cannot be the product story;
- constraints and audit are the reason the compiler exists.

Captured in:

- [compiler/runtime](compiler-runtime.md), sections 2–4;
- [product strategy](product-strategy.md), sections 2–3.

### 1.11 First full synthesis

The idea was summarised as:

> A backend language where the compiler owns boring decisions, the human owns
> intent, and the LLM performs implementation work between them.

The strong combination was code + intent + policy + audit. Authentication,
validation, controlled data access, no raw SQL/arbitrary HTTP, safe secrets,
timeouts, transactions, rate limiting, and audit logging should default safely.

The conversation advised starting with the semantic model rather than the
grammar: entity, route, query, action, policy, event, job, service, and possibly
decision. A possible compiler pipeline was parser → AST → semantic model →
policy checker → audit graph → executable.

The source should make wrong solutions unrepresentable or compilation errors.
The optimisation target was refined to **minimum ambiguity per token**.

Captured across:

- [charter](charter.md);
- [vision](vision.md);
- [semantic model](semantic-model.md);
- [compiler/runtime](compiler-runtime.md).

### 1.12 Todo application as the first concrete test

The user proposed a todo list as the real test. The response sketched users,
todos, authentication, ownership, validation, filters, CRUD, tests, a policy
file, generated security tests, a route audit, and an overdue-reminder job.

The compiler could infer schema, migrations, validation, auth, 404/malformed-ID
behaviour, serialisation, OpenAPI, docs, route inventory, and security audit.

The historical syntax and artifacts are preserved in
[examples/chat-todo-sketch.md](../examples/chat-todo-sketch.md). Other early
examples are preserved in
[examples/chat-design-sketches.md](../examples/chat-design-sketches.md).

### 1.13 Worth-pursuing and kill-test discussion

The conclusion was “yes, as a small, deliberately brutal experiment,” not “yes,
build a language now.” The key research question was whether the new model makes
agents materially better than TypeScript.

The proposed experiment builds the same todo backend twice and gives both a
sequence of changes including ownership, sharing, admin, due dates, reminders,
soft delete, public routes, unsafe cross-user access, and schema evolution.

Measurements include context, tokens, security defects, touched files, docs and
test drift, compiler catches, and human understandability. Three explicit kill
conditions were repeated:

- ordinary SaaS constantly needs arbitrary escape hatches;
- agents are worse because they do not know the language;
- an opinionated TypeScript framework supplies equivalent guarantees.

Captured in:

- [validation plan](validation-plan.md);
- [product strategy](product-strategy.md), section 3.

### 1.14 Project OS save

The original chat recorded that the idea was saved to the user's Project OS as
“LLM-native backend programming language,” including human policy, audits/docs,
external contracts, Postgres defaults, provider independence, the todo
comparison, and continuation criteria.

This is historical context only; these repository documents now contain the
working detail.

### 1.15 Time, commercial value, and project prioritisation

The user expressed frustration with the fragmented web stack but also limited
time, many projects, and uncertainty about monetising an open-source language.
The response separated two ideas: building a new language and proving that
backend development can be radically simplified.

The advice was not to prioritise it as a near-term money project. Adoption,
distribution, support, and community are substantial work. Preserve it as a
small research bet; if a one-day/weekend todo spec is not strikingly good, stop.
AI can implement much of a prototype, but the concept still requires focused
human guidance.

Captured in:

- [product strategy](product-strategy.md), sections 5–6;
- [validation plan](validation-plan.md), section 14.

### 1.16 Renewed assurance framing

The user returned to the idea because AI-written websites were shipping without
meaningful review, with security delegated to optional agent skills. This
produced the sharpest framing:

- do not build a language that merely makes AI programming easier;
- build an environment in which AI is deliberately not trusted;
- implementation and guarantees must not both be probabilistic;
- as models produce more code, human review becomes less viable;
- the language/runtime should replace instruction soup with invariants;
- humans review intent, policy, and exceptions.

The discussion listed preventable classes including missing auth, IDOR,
arbitrary queries, private-field output, secret leaks, undeclared input,
unbounded external calls, and forgotten transactions, while acknowledging that
requirements, business logic, and external systems can still be wrong.

Captured in:

- [vision](vision.md), sections 2–3 and 10;
- [assurance model](assurance-model.md), sections 3, 5, and 11.

### 1.17 Debugging generated TypeScript

The user raised a detailed concern: production errors would mention generated
TypeScript, so how would they map back to original source? The response defined:

- generated TypeScript is never developer-facing source;
- ordinary source maps map target locations;
- richer semantic metadata maps generated code to stable AST/semantic node IDs;
- runtime errors are caught and translated into original language concepts;
- generated behaviour without a literal source line maps to the declaration and
  internal conceptual component;
- low-level traces remain for compiler/runtime maintainers;
- semantic logs integrate with Sentry/Datadog;
- deployment may include `app.js`, `app.js.map`, and `app.meta`;
- target compile failures are compiler bugs, not application fixes;
- agent-facing errors should be smaller and more actionable than target traces.

Captured in:

- [compiler/runtime](compiler-runtime.md), sections 9–12 and 14.

### 1.18 General programming and human-readable syntax

The user wanted to explore real business logic, not only routes and CRUD, and
required the language to remain readable/writable by humans. The assistant
proposed conventional TypeScript/Python/Rust-shaped syntax with novelty in
semantics rather than punctuation.

The language needs ordinary functions, values/structs, enums, lists/maps,
optionals, conditionals, exhaustive matches, iteration, and returns, while
excluding reflection, metaprogramming, runtime loading, raw sockets, `eval`, and
untyped dynamic objects.

Other points:

- immutable by default;
- domain-aware types such as email, money, percentage, datetime, and duration;
- exhaustive matches;
- explicit optional narrowing;
- route-local behaviour with extraction only when reuse justifies it;
- discourage controller/service/repository/factory/mapper proliferation;
- choose `if` and `match`, not multiple equivalent conditional forms;
- one error model, not exceptions + codes + result + callback + promise styles;
- test todo, bookings, checkout, admin, and an algorithmic shipping/scheduling
  case before implementation.

Captured in:

- [syntax](syntax.md), throughout;
- [semantic model](semantic-model.md), locality/abstraction;
- [validation plan](validation-plan.md), sections 11–12.

### 1.19 Variable clarity, collection operations, and error model

The user objected that the first syntax did not visually distinguish variables,
disliked ambiguous `.empty` prototype-like syntax, preferred immutability by
default, and wanted error handling better than both JavaScript exceptions and
Rust `Result` plumbing.

The evolved response proposed:

- explicit binding keyword, later settled as `var` / `var mut`;
- language/standard operations such as `collection.count(items)` rather than magic
  properties or prototype methods;
- visible compiler-understood `create`, `query`, `update`, `delete` rather than
  `Order.create`/`order.update`;
- declared `fails` and `reject` without wrapping every successful value;
- propagation only when the enclosing context declares/handles the same
  failures;
- a possible `attempt` mapping block;
- three categories: domain failure, infrastructure fault, bug/impossible state;
- `T?`/`none` instead of exposed `Option<T>` mechanics;
- no truthiness.

Captured in:

- [syntax](syntax.md), bindings, collections, persistence, and errors;
- [semantic model](semantic-model.md), failure model;
- [decision register](decision-register.md), accepted/rejected choices.

### 1.20 `var`, absence, omission, and service error normalisation

The user explicitly preferred `var total = ...` and `var mut retries = 0`, asked
about Stripe-style errors, and asked whether to support `undefined`, `null`, or
neither.

The final choices were:

- `var` immutable, `var mut` mutable;
- no `undefined`;
- no ambient `null`;
- one absence value, `none`;
- `T?` for a value that may be absent;
- omitted input field is different from a supplied `none`;
- `Text? optional` can represent patch “not supplied / set / clear” states;
- non-null database fields by default;
- database `NULL` maps to `none`;
- concrete objects have no missing fields;
- projections represent selected subsets;
- incoming JSON `null` may map to `none` and outgoing `none` provisionally maps
  to JSON `null`;
- service contracts map many provider errors into a smaller domain vocabulary
  or operational faults.

Captured in:

- [syntax](syntax.md), sections 4–6 and 12;
- [semantic model](semantic-model.md), trust and failure models;
- [decision register](decision-register.md).

### 1.21 Runtime type safety

The user explicitly confirmed the need for runtime safety. The response made
boundary validation a language property, not a remembered library call. It
listed HTTP input/output, database read/write, config, queues/events, external
responses, and caches.

Domain constraints should become static checks where possible, runtime
validators, database constraints, OpenAPI/docs, and tests. An invalid Postgres
row should be a hard contract failure. Output validation prevents returning a
full internal user where a public shape was declared.

The final promise was: **types are executable contracts**.

Captured in:

- [semantic model](semantic-model.md), sections 2–3;
- [compiler/runtime](compiler-runtime.md), sections 6–8;
- [charter](charter.md).

### 1.22 Agreed next step

The closing recommendation was not more isolated syntax exploration and not a
compiler. The sequence was:

1. freeze a tiny v0 charter;
2. write the complete golden todo backend;
3. maintain a language-design issue log;
4. write a deliberately difficult payment/order example;
5. only then design the final AST/semantic implementation;
6. sketch generated artifacts and repeatedly ask whether TypeScript habits are
   being copied unnecessarily.

The conversation advised resisting a language name until the hypothesis is
proven.

Captured in:

- [documentation index](README.md), current phase;
- [charter](charter.md), current test;
- [validation plan](validation-plan.md).

## 2. Topic-to-document matrix

| Source topic | Primary document | Supporting documents |
| --- | --- | --- |
| Core thesis and division of authority | [vision](vision.md) | [charter](charter.md), [agent workflow](agent-workflow.md) |
| Language concepts and guarantees | [semantic model](semantic-model.md) | [type system](type-system.md), [syntax](syntax.md) |
| Nominal types, field references, and validated construction | [type system](type-system.md) | [type-system acceptance cases](../examples/type-system-cases.md), [syntax](syntax.md) |
| Domain failures, HTTP mapping, disclosure, and stacks | [failure model](failure-model.md) | [failure-model acceptance cases](../examples/failure-model-cases.md), [syntax](syntax.md) |
| Concrete surface syntax | [syntax](syntax.md) | [decision register](decision-register.md) |
| Safe defaults and security | [assurance model](assurance-model.md) | [charter](charter.md), [semantic model](semantic-model.md) |
| Policy ownership and CI gate | [assurance model](assurance-model.md) | [agent workflow](agent-workflow.md) |
| Living docs / Project OS connection | [agent workflow](agent-workflow.md) | [vision](vision.md) |
| Postgres and data model | [semantic model](semantic-model.md) | [compiler/runtime](compiler-runtime.md), [syntax](syntax.md) |
| External API contracts | [semantic model](semantic-model.md) | [compiler/runtime](compiler-runtime.md), [syntax](syntax.md) |
| Runtime validation | [compiler/runtime](compiler-runtime.md) | [type system](type-system.md), [semantic model](semantic-model.md) |
| TypeScript/Bun/native target trade-off | [compiler/runtime](compiler-runtime.md) | [product strategy](product-strategy.md) |
| Source maps and semantic diagnostics | [compiler/runtime](compiler-runtime.md) | [agent workflow](agent-workflow.md) |
| Todo example | [historical todo sketch](../examples/chat-todo-sketch.md) | [validation plan](validation-plan.md) |
| Booking, service, lifecycle, and diagnostic examples | [historical design sketches](../examples/chat-design-sketches.md) | [semantic model](semantic-model.md), [assurance model](assurance-model.md) |
| Experiment, metrics, and kill criteria | [validation plan](validation-plan.md) | [product strategy](product-strategy.md) |
| Time, commercial, and adoption caveats | [product strategy](product-strategy.md) | [validation plan](validation-plan.md) |
| Decisions and unresolved questions | [decision register](decision-register.md) | all specifications |

## 3. Completeness note

The documentation intentionally does not preserve filler acknowledgements,
voice-transcription artefacts, repeated fragments, or a prior tool claim that
the idea was saved elsewhere. It does preserve every substantive design idea,
motivation, example class, recommendation, objection, trade-off, decision,
rejected alternative, implementation proposal, validation criterion, and
commercial caution identified in the second end-to-end reading.

Future work should cite and update these documents rather than relying on the
shared conversation. The conversation remains useful only as historical
provenance.
