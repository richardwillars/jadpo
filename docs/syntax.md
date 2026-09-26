# Jadpo language syntax draft

**Status:** exploratory v0.1  
**Purpose:** turn the initial design conversation into a coherent syntax proposal  
**Audience:** language designers, human reviewers, and LLM coding agents

This document describes the proposed source language, not the generated
TypeScript or runtime implementation. It deliberately distinguishes decisions
that emerged clearly from the discussion from choices that still need to be
tested in complete example programs.

The examples are intended to be internally consistent, but this is not yet a
formal specification. Where earlier examples used conflicting spellings, this
draft chooses the form that best matches the principles below and records the
decision as provisional.

## 1. Design goals

The language is for web backends written primarily by LLMs and sometimes read or
edited by humans. It is not an alien or maximally terse "AI language".

The syntax should optimise for:

1. **Minimum ambiguity per token.** A longer, descriptive construct is better
   than compact syntax that can be interpreted in several ways.
2. **Human readability and writability.** A developer familiar with TypeScript,
   Python, or Rust should be able to understand ordinary code immediately.
3. **One canonical way to express common operations.** The language should not
   make an agent choose between several equivalent styles.
4. **Visible semantics.** Bindings, types, domain operations, database
   operations, and failures should be visually distinguishable.
5. **Strict meaning, forgiving formatting.** Whitespace is not semantic. A
   canonical formatter normalises source at deliberate checkpoints.
6. **Safe defaults.** Authentication, validation, constrained data access,
   output filtering, and declared external effects are normal language
   behaviour rather than optional libraries.
7. **Deterministic verification.** Requirements that can be compiler invariants
   should not remain prose instructions to an LLM.

The syntax should not optimise for the fewest characters. For example,
`calculate_discount` is preferable to `calc`, and an explicit public-access
override is worth its tokens because it documents a deliberate weakening of the
secure default.

## 2. Status vocabulary

This draft uses three labels:

- **Accepted** — strongly supported by the design discussion and used as the
  current default.
- **Provisional** — a recommended shape that must be exercised in the golden
  example programs.
- **Open** — intentionally unresolved.

## 3. Source form and naming

### 3.1 Familiar block syntax

**Provisional:** blocks use braces. Statements do not require semicolons.
Whitespace and indentation are formatted canonically but have no semantic
meaning.

```text
if total > customer.credit_limit {
    reject CreditLimitExceeded
}
```

This keeps source familiar without making small formatting mistakes into
compiler failures.

### 3.2 Naming

**Provisional:**

- types, entities, events, and named failures use `UpperCamelCase`;
- values, fields, functions, actions, and jobs use `lower_snake_case`;
- language keywords use lowercase.

```text
CreateOrder
OrderCreated
CreditLimitExceeded

current_user
calculate_total
place_order
```

Capitalisation supplies useful visual information without introducing special
sigils such as `@Order`.

### 3.3 Comments and documentation

**Open:** comment and documentation syntax was not decided. The language will
need both implementation comments and durable intent/decision documentation,
but intent must not be confused with a non-binding comment. A later document
should define which declarations are compiler-visible and which are explanatory.

## 4. Bindings and mutation

### 4.1 Immutable bindings

**Accepted:** `var` creates a binding, and bindings are immutable by default.

```text
var customer = current_user
var total = calculate_total(input.items)
var retries: Int = 0
```

Reassigning an immutable binding is a compile error:

```text
var total = 100
total = 200              // compile error
```

### 4.2 Mutable bindings

**Accepted:** mutation must be explicitly declared with `var mut`.

```text
var mut selected: Choice = initial
selected = replacement
```

`var mut` is intended to be unusual. Its presence tells a human or an LLM that
the local binding changes over time. Assigned values must be compatible with
the binding's established nominal type. Assignment targets only a bare local name; parameters, object fields, and
values reached through another binding cannot be assigned.

This is rebinding, not reference mutation. Ordinary parameters have immutable
value semantics, returned values are ordinary immutable values, and source code
cannot observe whether the compiler copies, moves, shares, or uses copy-on-write
storage. There is no `ref`, address, alias, or `inout` syntax in the accepted
language. An explicit caller-mutation contract will be considered only if a
golden application demonstrates a need that returning a value cannot express.

### 4.3 Constants

**Open:** the discussion did not establish a separate `const` declaration.
Ordinary `var` bindings are already immutable. Named compile-time constants may
still deserve a distinct declaration once configuration and modules are
designed.

## 5. Types

### 5.1 Type annotations

Types follow a colon. Local types may be inferred where there is one obvious
answer.

```text
var customer: Customer = current_user
var items: List<Item> = input.items
```

**Accepted:** a value that has a concrete object type is complete. It never has
undeclared or missing fields. If `customer` has type `Customer`, every field in
`Customer` exists; fields explicitly typed as optional values may contain
`none`.

### 5.2 Primitive and domain types

**Accepted and implemented:** the prelude contains a deliberately small set of
representation, time, container, and broadly unambiguous semantic types.

```text
Bool
Int
Decimal
Text
Bytes
Uuid
Date
Time
DateTime
Duration
Unit
Object
List<T>
Set<T>
Map<K, V>
Email
Url
IpAddress
```

`Email`, `Url`, and `IpAddress` carry compiler-owned validation at trust
boundaries. Policy-dependent concepts such as `Username`, `Slug`, `Phone`,
`Postcode`, `Money`, and `CountryCode` remain authored domain types rather than
built-ins whose meaning would be too vague.

**Accepted:** semantic types are nominal. Sharing a representation does not
make two types interchangeable. A `Username`, `Email`, and raw `Text` remain
different types even when all three are represented as text at runtime. There
is no implicit conversion merely because their underlying primitive matches.

### 5.3 Constrained types

**Provisional:** a type can refine a primitive with constraints.

```text
type Username = Text {
    min_length: 3
    max_length: 30
    pattern: "[a-z0-9_]+"
}

type StockLevel = Int {
    min: 0
}

type VatRate = Decimal {
    min: 0
    max: 1
}
```

Each declaration above introduces a distinct semantic type rather than a
transparent alias. Converting an untrusted primitive into one of these types
requires validation. A value that has already crossed a typed boundary needs no
revalidation inside trusted application code.

One declaration should drive static checks where possible, runtime validation,
database constraints, generated API schemas, documentation, and tests.

The shorthand `Int >= 0` appeared in the initial exploration, but the block form
above is preferred provisionally because it scales to multiple constraints and
has less grammar magic.

### 5.4 Validated construction

**Accepted:** calling a semantic or field type constructs a value of that type
and runs its validation. It is never an unchecked cast.

```text
var support_email = Email("support@example.com")
var customer_email = Customer.email("person@example.com")
var quantity = Quantity(3)
```

When the argument is a constant literal, the compiler validates it during
compilation:

```text
var email = Email("dsfsd") // compile error: invalid Email
```

When the argument is dynamic, the same syntax validates it at runtime:

```text
var email = Email(raw_text)
```

Dynamic construction introduces a typed validation failure for the target type.
The containing callable must declare, map, or handle it; construction never
silently returns `none`, throws an ambient exception, or blesses an invalid
value. The precise failure payload and its `fails`/`attempt` mapping syntax are
open.

Construction can also express deliberate narrowing:

```text
var customer_email = Customer.email(email)
```

This revalidates any constraints added by `Customer.email`. The compiler may
elide checks already proven by `Email`, but the semantic transition remains
explicit in source and metadata.

### 5.5 Enumerations

**Accepted and implemented:** enums are closed, may carry variant-specific
payloads, and matches over them are exhaustive.

```text
type OrderStatus = Enum {
    pending
    paid
    shipped
    delivered
}
```

Variants are referenced with a qualified name such as `OrderStatus.paid` and
cross JSON boundaries as their declared string spelling. Adding a member causes
every now-incomplete `match` to fail compilation.

Enums may carry variant-specific data:

```text
type PaymentOutcome = Enum {
    pending {
        started_at: DateTime
    }

    paid {
        receipt_id: PaymentReceipt.id
        paid_at: DateTime
    }

    declined {
        reason: DeclineReason
    }
}
```

Such an enum is a closed tagged sum. A value contains exactly one variant and
exactly the fields declared by that variant. It does not contain a record with
all variant fields made nullable. A `pending` value therefore cannot carry a
receipt, and a `paid` value cannot omit `paid_at`.

Construction retains qualified variant syntax:

```text
var outcome = PaymentOutcome.paid {
    receipt_id: receipt.id
    paid_at: clock.now
}
```

An exhaustive `match` narrows each arm to its variant and binds only named
fields from that variant:

```text
match outcome {
    PaymentOutcome.pending { started_at } => {}
    PaymentOutcome.paid { receipt_id, paid_at } => {}
    PaymentOutcome.declined { reason } => {}
}
```

Adding a variant invalidates every match that becomes incomplete; changing a
payload invalidates affected constructors and patterns. JSON boundaries encode
tagged sums as closed objects with a compiler-owned `tag` field. Persistent
storage, database constraints, and migrations for tagged sums remain outside
the implemented storage subset; the declaration does not imply one ordinary
database enum column.

This direction does not introduce methods, traits, interfaces, automatic
display labels, or customer-facing string derivation. Reusable behaviour
remains a named function over the enum plus exhaustive `match`; localised or
contextual copy remains a separate presentation concern.

Enums represent a closed set of alternatives and are visibly distinct from
constrained text or numeric values. **Accepted:** authored source always
references a variant with qualified dot syntax:

```text
type InviteStatus = Enum {
    active
    blocked
    used
    expired
}

if invite.status == InviteStatus.blocked {
    reject InviteUnavailable
}
```

An enum is not constructed or parsed with ordinary call syntax. These forms are
invalid even when the string matches a declared variant:

```text
InviteStatus("blocked") // compile error: enums use qualified variants
InviteStatus(raw_text)  // compile error: no dynamic enum construction
```

Untrusted text becomes an enum only at a declared boundary decoder or through a
separate explicit parsing operation whose failure is typed. Application source
still receives or refers to the resulting value as `InviteStatus.blocked`; it
does not make an enum variant look like validated string construction.

By contrast, `InviteCode` is an open semantic text value. An exact reserved code
is constructed and named as a value rather than made to look like an enum
variant:

```text
var reserved_invite_code = InviteCode("reserved")

if (input.invite_code == reserved_invite_code) {
    reject InviteCodeRejected
}
```

Do not encode lifecycle state or a closed category as a magic string inside an
otherwise open semantic type. Model it as an enum, relationship, or other
explicit domain state. Enum declarations, qualified variants, payload
construction, and exhaustive `match` are implemented in the core grammar;
`EnumName("variant")` remains deliberately invalid.

### 5.6 Collections

**Provisional:** generic collection types use explicit names.

```text
List<Item>
Map<Text, Int>
Set<Id>
```

Collection operations should be language or standard-library functions rather
than magic properties or prototype-style methods:

```text
count(items)
contains(items, product)
sum(prices)
filter(items, is_available)
map(items, calculate_price)
```

The earlier `input.items.empty` form is rejected because `empty` could be a data
field, a method, a magic property, or a compiler intrinsic. Prefer:

```text
if count(input.items) == 0 {
    reject EmptyBasket
}
```

**Open:** lambda syntax and whether `map`/`filter` are needed in v0. Ordinary
`for` loops may cover the initial examples with less language complexity.

## 6. Absence, nullability, and omission

This is a firm part of the emerging design.

### 6.1 One absence model

**Accepted:** the language has neither JavaScript `undefined` nor an ambient
`null`. A value that may be absent has type `T?`; the absent value is `none`.

```text
var due_at: DateTime? = none
```

An optional value must be narrowed before use:

```text
if due_at exists {
    schedule(due_at)
}
```

or matched explicitly:

```text
match due_at {
    some date => schedule(date)
    none => skip
}
```

There is no general truthiness rule. A `DateTime?`, `Int`, or `Text` cannot be
used as a boolean.

### 6.2 Omission is not `none`

**Accepted:** whether an input field was supplied is part of the input object's
shape, not a third runtime value stored inside `T?`.

```text
type UpdateCustomer = Object {
    name: Text optional
    middle_name: Text? optional
}
```

The forms mean:

| Declaration | Field may be omitted | Supplied value may be `none` |
| --- | ---: | ---: |
| `name: Text` | no | no |
| `name: Text optional` | yes | no |
| `middle_name: Text?` | no | yes |
| `middle_name: Text? optional` | yes | yes |

For a patch input, `middle_name: Text? optional` therefore has three meaningful
input states:

1. omitted — leave the stored value unchanged;
2. supplied with text — set the value;
3. supplied as `none` — clear the value.

Those are input states, not `undefined | null | string` values carried through
the application.

### 6.3 Boundary mapping

**Accepted:**

- database `NULL` maps to language `none`;
- incoming JSON `null` maps to `none` when the schema permits `T?`;
- an omitted JSON field remains omitted during input validation;
- fetched entity fields are never omitted;
- partial database selections return projection types, not half-populated
  entities.

**Provisional:** output `none` serialises as JSON `null` by default. A future
output declaration may explicitly request omission, but the language should have
one canonical default.

## 7. Structured values, inputs, and outputs

Every structured data shape uses `type Name = Object { ... }`. `input:` and
`output:` are route roles and may reference any declared type. Boundary use
derives the closed decoder or serializer; it does not change type identity.

### 7.1 Input types

Using a type as route input produces a runtime decoder and validator.

```text
type CreateUser = Object {
    email: Email
    age: Int {
        min: 18
    }
}
```

By the time ordinary application code receives `CreateUser`, its fields have
been validated. Code should not repeat boundary validation defensively.

### 7.2 Output types

Using a type as route output produces a runtime validator and closed serializer.

```text
type RegistrationAccepted = Object {
    email: Customer.email
}
```

Returning a richer entity or value never serialises undeclared fields. The
compiler must prove a safe projection or reject the result.

### 7.3 Value types

```text
type Address = Object {
    line_1: Text
    line_2: Text?
    city: Text
    postcode: Text
}
```

Object types may be reused as domain data and at either boundary. Nested closed
objects are supported directly, including `List<Object { ... }>` fields.

### 7.4 Field and object syntax

**Provisional:** declarations and object construction use `:` consistently.

```text
var address = Address {
    line_1: input.line_1
    line_2: none
    city: input.city
    postcode: input.postcode
}
```

`=` is reserved for binding and mutation; `==` and `!=` are comparisons. This
removes the earlier inconsistency between `field = value` and `field: value`.

## 8. Entities and persistence

Canonical source declares `type Name = Object { ... }` and opts into storage
separately with `persist Name { ... }`. A type without `persist` is ordinary
constructible application data and has no generated database operations.

### 8.1 Entity declaration

`persist` declares storage metadata for an existing object type. Postgres is
the opinionated initial database target.

```text
type Customer = Object {
    id: Uuid
    email: Email
    middle_name: Text?
    last_seen_at: DateTime?
}

persist Customer {
    identity: id
    unique: email
    index: last_seen_at
}

type Order = Object {
    id: Uuid
    owner_id: Customer.id
    status: OrderStatus
    total: Money
}

persist Order {
    identity: id
    references owner_id: Customer.id as: owner on_delete: restrict
}
```

**Accepted:** fields are non-nullable by default. A field is nullable only when
its type uses `?`.

**Accepted:** `identity:`, `unique:`, and `index:` belong inside `persist`. A
persisted type may have at most one non-nullable identity field. They describe
storage behavior and do not widen or narrow the field's nominal value type.

The exact colon-delimited spelling for named compound uniqueness remains open.
The former call-shaped spelling is not canonical. Defaults, generated values,
compound non-unique indexes, and migration identity remain provisional.

**Accepted:** an owning entity field declares a stored relationship explicitly:

```text
type Todo = Object {
    id: Uuid
    owner_id: User.id
    title: Text
    completed: Bool
}

persist Todo {
    identity: id
    references owner_id: User.id as: owner on_delete: cascade
}
```

The relationship target must be a non-nullable `identity` or `unique` field,
and the owner's field type must preserve that exact nominal target identity.
The optional `as owner` clause declares the logical relationship name used by
includes and nested outputs while `owner_id` remains the stored field. If `as`
is absent, the field name is used for both roles. The compiler does not derive
relationship names from `_id` or another naming convention.
`on_delete:` is mandatory and accepts `restrict`, `cascade`, or `set_null`;
`set_null` requires a nullable owner field. SQLite and PostgreSQL receive a
named foreign key and an automatically generated lookup index.

**Accepted for the first inverse-loading slice:** the parent declares a named
one-to-many inverse over an existing owning reference:

```text
persist User {
    identity: id
    inverse todos: many Todo via: Todo.owner_id
}
```

The inverse is relationship metadata rather than a stored field. It must name
the child entity in both `many Todo` and `via Todo.owner_id`, and that child
field must carry an owning reference back to the parent. Ordinary `User` access
does not load `todos`.

A parent can also declare an enforceable zero-or-one inverse:

```text
persist User {
    identity: id
    inverse profile: optional Profile via: Profile.user_id
}

persist Profile {
    identity: id
    unique: user_id
    references user_id: User.id on_delete: cascade
}
```

The owning `via` field must be `identity` or `unique`; a non-unique field is
rejected because it cannot support the declared at-most-one cardinality.
A required parent query loads it with
`include: profile optional into: UserProfile`, producing exactly
`parent: User` and `profile: Profile?`. The generated plan performs one parent
lookup and one optional child lookup. Missing children become `none`, and
ordinary entity access remains non-loading.

A required query opts into the relationship and a named nested output shape:

```text
type UserTodos = Object {
    parent: User
    todos: List<Todo>
}

attempt query required User {
    where: id == input.id
    include: todos into: UserTodos order_by: id asc limit: 100 offset: 0
    missing: UserNotFound
}
```

The compiler requires the output to contain exactly the validated parent and
the ordered child list. A positive literal `limit` and non-negative literal
`offset` make the to-many bound reviewable at compile time; the generated SQL
still binds both as parameters. The current plan is an explicit bounded batch
of two queries, exposed in generated metadata; it is never an implicit lazy
load.

The same include is accepted on a paginated many-parent query:

```text
attempt query many User {
    where: group == input.group
    order_by: id asc
    limit: 20 offset: 0
    include: todos into: UserTodos order_by: id asc limit: 100 offset: 0
}
```

Its type is `List<UserTodos>`. Parent ordering and pagination are applied in a
CTE before child expansion; ranked child rows are then joined and regrouped
into nested values. This one-include plan is a single query, preserves parents
with zero children, and cannot drop a parent merely because an earlier parent
has many children.

An owning reference may be traversed from one required child to its parent:

```text
type PatchItemReviewer = Object {
    parent: PatchItem
    reviewer_id: User?
}

attempt query required PatchItem {
    where: id == input.id
    include: reviewer_id optional into: PatchItemReviewer
    missing: PatchItemNotFound
}
```

The relationship name is the explicit `as` name when present and otherwise the
owning field. A nullable reference must use `optional` and the matching output
field is nullable. A non-nullable reference may use `required` with a
non-nullable parent output. The output must contain exactly the child as
`parent` and the named related value. Generation uses one
bounded child lookup followed by one parent lookup, validates both values, and
returns `none` for a null optional reference. No field access triggers an
implicit query.

The bounded nested form composes one non-nullable owning-parent hop with one
unique-backed optional inverse:

```text
type TodoOwnerProfile = Object {
    parent: Todo
    owner: UserProfile
}

attempt query required Todo {
    where: id == input.id
    include: owner.profile optional into: TodoOwnerProfile
    missing: TodoNotFound
}
```

`UserProfile` must itself be the exact output shape for the middle `User` and
optional `profile`. The compiler fixes the maximum path depth at two and emits
three bounded lookups, so source cannot accidentally introduce recursive or
per-row loading. Nullable first hops, to-many hops, repeated nested includes,
and deeper paths remain unsupported.

### 8.2 Declared field types

**Accepted:** the semantic type of a field on any named record-like declaration
can be referenced as `Type.field`. This includes entities, values, inputs,
outputs, events, and other declarations whose fields participate in the type
system.

```text
type CreateOrder = Object {
    customer_id: Customer.id
    receipt_email: Customer.email
}
```

These are not textual aliases that collapse to a representation primitive.
They retain the semantic identity and intrinsic validation rules of the fields
declared by `Customer`. Consequently, a raw `Text`, `Supplier.email`, or another
text-represented type cannot be passed where `Customer.email` is required.

Each declared field is a nominal refinement of the type written on that field.
It may safely widen to that declared type, but the reverse conversion and
conversion between sibling field types are not implicit. If both
`Customer.email` and `Supplier.email` refine `Email`, either can be passed to a
function accepting `Email`; neither can be substituted for the other when a
function asks for the specific field type.

The identifier case does not mean that the customer exists or that the current
actor may use it. Those stateful checks happen when application code loads the
entity under policy:

```text
action place_order(input: CreateOrder)
    fails CustomerNotFound, NotPermitted
    -> Order
{
    var customer = attempt query required Customer {
        where: id == input.customer_id
        missing: CustomerNotFound
    }

    require customer accessible_by current_user

    // ...
}
```

`Customer.id` appears in a type position. A lowercase value expression such as
`customer.id` accesses the identifier of a loaded `Customer`. The value has
type `Customer.id`, so it may be passed directly to a callable expecting that
field type. The same rule applies to every other field.

The form composes normally:

```text
List<Customer.id>
Customer.id?
Map<Customer.id, Customer.email>
```

A field type inherits value semantics: its underlying representation, nominal
identity, intrinsic validation, and nullability. Field presence or omission
belongs to the containing shape and is not part of the referenced value type.
A reference also does not inherit storage or stateful semantics such as a
default, index, uniqueness, foreign-key existence, authorisation, or permission
to read the field.

When several fields are deliberately interchangeable, give them a shared named
semantic type and accept that type rather than one entity's field type:

```text
type Customer = Object {
    email: Email
}

type Supplier = Object {
    email: Email
}

function send_message(to: Email) {
    // accepts validated email values from either entity
}
```

Using `Customer.email` in the function signature instead expresses a narrower
contract: the value must carry the semantic identity of that particular field.
This distinction is deliberate rather than inferred from matching structure.

### 8.3 Database operations are language constructs

**Accepted:** persistence operations must look like language operations, not
entity methods or arbitrary library calls.

Create:

```text
var order = attempt create Order {
    owner: current_user
    status: pending
    total: total
} conflict: OrderMutationConflict
```

The postfix `conflict:` binding is optional. When present, it must name a
`Conflict`-derived failure declared by the action and converts a normalised
database constraint violation into that domain failure. It uses the same
colon-separated binding form as required mutations.

Query:

```text
var orders = attempt query Order {
    where: owner == current_user
    and status == pending
    order_by: created_at desc
    limit: 50
}
```

Accepted ordered many-result form:

```text
var todos = attempt query many Todo {
    where: owner_id == input.owner_id
    order_by: id asc
    limit: 100 offset: 0
}
```

`query many` returns `List<Todo>`, requires explicit `asc`/`desc` ordering, and
currently requires the ordering field to be identity or unique. The generated
adapter uses a bound predicate, validates every row, and never chooses an
implicit database order. An optional positive literal `limit` plus
non-negative literal `offset` is emitted as parameters. Dynamic bounds and
compound predicates remain pending.

Required update:

```text
attempt update required Order {
    where: id == input.id
    set: {
        status: input.status
    }
    missing: OrderNotFound
    conflict: OrderMutationConflict
}
```

Required delete:

```text
attempt delete required Order {
    where: id == input.id
    missing: OrderNotFound
    conflict: OrderMutationConflict
}
```

This form is preferred to `Order.create(...)`, `Order.query(...)`, or
`order.update(...)` because `create`, `query`, `update`, and `delete` are visibly
compiler-understood operations.

Raw SQL is not part of ordinary application syntax. A future escape hatch, if
one exists, must be explicit, auditable, and conspicuous.

### 8.4 Query results and cardinality

**Accepted for the first read slice:** zero-or-one lookup uses an explicit
`query optional` expression:

```text
var customer = attempt query optional Customer {
    where: id == input.id
}
```

The expression has type `Customer?`. The single equality predicate must target
a known non-nullable entity field and compare a value with that field's nominal
identity. Zero rows produce `none`, one row produces a validated entity, and
multiple rows are an operational cardinality fault rather than an arbitrary
choice. This first form is action-only and compiles to a parameterised query
with a two-row limit on SQLite and PostgreSQL.

The required-one form is explicit about absence:

```text
var customer = attempt query required Customer {
    where: id == input.customer_id
    missing: CustomerNotFound {
        customer_id: input.customer_id
    }
}
```

Its expression type is `Customer`, and `CustomerNotFound` must be declared in
the enclosing action's `fails` clause and derive from `NotFound`. Missing rows
reject that typed failure; they never throw an untyped exception. The optional
failure body supplies the same exact flat context object as an explicit
`reject` statement; the declaration owns the public/internal split.

**Open:** zero-or-more queries returning `List<T>` still need explicit ordering
and pagination semantics.

Earlier examples used `Order.require(...)`, but method syntax conflicts with the
decision to make database behaviour a language construct. Many-result queries
still need ordering/pagination rules.

### 8.5 Required mutations

The first mutation slice makes both affected-row cardinality and expected
database failures explicit:

```text
action update_customer(input: UpdateCustomer)
    fails CustomerNotFound, CustomerMutationConflict
    -> Customer
{
    return attempt update required Customer {
        where: id == input.id
        set: {
            email: input.email
        }
        missing: CustomerNotFound
        conflict: CustomerMutationConflict
    }
}
```

The predicate and replacement values carry the nominal identities of their
entity fields. `set:` accepts one or more distinct fields and every replacement
is checked independently. The fixed field set compiles to one parameterised
update statement, so a constraint failure cannot leave a subset committed.
Zero rows reject the declared `NotFound`-derived `missing:` failure; database
constraint violations reject the declared `Conflict`-derived `conflict:`
failure. A successful expression returns the validated updated `Customer`.

Omission-aware updates name a direct patch-input binding and an explicit empty
patch failure:

```text
type CustomerChanges = Object {
    name: Customer.name optional
    middle_name: Customer.middle_name optional
}

action patch_customer(id: Customer.id, changes: CustomerChanges)
    fails EmptyCustomerPatch, CustomerNotFound, CustomerMutationConflict
    -> Customer
{
    return attempt update required Customer {
        where: id == id
        patch: changes
        empty: EmptyCustomerPatch
        set: {
            updated_at: clock.now
            verification_sent_at: none when changes.name supplied
        }
        missing: CustomerNotFound
        conflict: CustomerMutationConflict
    }
}
```

The patch record must be non-empty, every field must be declared `optional`,
and every field must match a known entity field nominally. At runtime, an empty
object rejects the `InvalidValue`-derived `empty:` failure. Each possible field
has a fixed supplied-flag/value pair in one generated statement: omission keeps
the existing column, a supplied value replaces it, and supplied `none` clears a
nullable column. The compiler does not construct SQL dynamically from request
keys. A following `set:` block performs fixed derived writes in the same atomic
statement. A derived value with `when changes.name supplied` is applied only
when that patch key was present; unconditional derived values always apply once
the non-empty patch is accepted. Patch and derived writes may not target the
same entity field, avoiding ordering-dependent results.

When several constraints can fail, source can map compiler-owned identities
independently and retain one optional fallback:

```text
conflict Account.handle: AccountHandleTaken
conflict Account.tenant_owner: AccountTenantOwnerTaken
conflict: AccountMutationConflict
```

`Account.tenant_owner` comes from an entity declaration such as
`constraint tenant_owner: unique(tenant, owner_email)`. Specific mappings are
checked for existence and duplication. Generated application logic never
compares raw driver constraint names.

`delete required` has the same failure contract and returns the validated row
that was removed:

```text
attempt delete required Customer {
    where: id == input.id
    missing: CustomerNotFound
    conflict: CustomerMutationConflict
}
```

Both adapters select at most two matching rows and perform the mutation inside
a transaction. More than one match is an operational cardinality fault and the
transaction rolls back, so a failed cardinality check cannot partially update
or delete data.

### 8.6 Projections

Selecting only some fields does not return a partially populated entity.

```text
var customers = attempt query Customer {
    select id, email
}
```

Each result has an inferred complete projection equivalent to:

```text
{ id: Id, email: Email }
```

It is not a `Customer` and cannot accidentally expose or access unselected
fields.

## 9. Functions and actions

### 9.1 Functions

**Provisional:** `function` describes ordinary computation without persistent or
external effects.

```text
function calculate_discount(
    customer: Customer,
    basket: Basket
) -> Money {
    if customer.vip {
        return basket.total * 0.10
    }

    if basket.total > Money(100, GBP) {
        return basket.total * 0.05
    }

    return Money(0, GBP)
}
```

The precise `Money` literal/constructor syntax is open; it is written explicitly
here rather than relying on a currency-specific token such as `£100`.

**Provisional, strongly recommended:** ordinary application-defined callable
signatures must not accept or return raw representation primitives such as
`Text`, `Int`, or `Decimal`. Parameters and results instead use a named semantic
type or a declared field type:

```text
function send_receipt(to: Customer.email) -> DeliveryReceipt
function calculate_tax(amount: Order.subtotal, rate: VatRate) -> Money
```

This prevents primitive blindness: a username, email address, product code, and
free-form note cannot be exchanged merely because all are text. Primitives
remain available as the representation beneath semantic type declarations, for
local inference and operators, and to compiler or standard-library intrinsics.
The exact syntax for a conspicuous low-level escape hatch, if user code proves
to need one, is open.

### 9.2 Actions

**Provisional, strongly recommended:** `action` is a domain operation that may
read or mutate persistent state, emit events, or call declared services.

```text
action place_order(input: CreateOrder)
    fails EmptyBasket, CreditLimitExceeded
    -> Order
{
    if count(input.items) == 0 {
        reject EmptyBasket
    }

    var customer = current_user
    var total = calculate_total(input.items)

    if total > customer.credit_limit {
        reject CreditLimitExceeded
    }

    var order = attempt create Order {
        owner: customer
        items: input.items
        total: total
        status: pending
    }

    emit OrderCreated(order)

    return order
}
```

The compiler understands the action's database mutations, external effects,
possible domain failures, and transaction needs.

**Accepted signature order:** a fallible callable places `fails` before its
successful result arrow:

```text
action place_order(input: CreateOrder)
    fails EmptyBasket, CreditLimitExceeded
    -> Order
{
    // ...
}
```

This replaces `-> Order fails ...`, which reads as though the successful value
itself fails.

### 9.3 Effect rules

**Open:** the exact effect system has not been designed. At minimum, pure
functions should not be able to perform undeclared database writes, service
calls, event emission, or secret access. Whether functions can read the database
or whether all persistence belongs exclusively to actions must be decided from
real examples.

## 10. Control flow

### 10.1 Conditions

The language has `if` and `else`. Conditions must have type `Bool`.

```text
if total > customer.credit_limit {
    reject CreditLimitExceeded
} else {
    return approve_order(total)
}
```

**Accepted:** do not add `unless`, ternaries, or another equivalent conditional
form in v0.

Expressions use conventional precedence: unary `not` and `-`; `*`, `/`, `%`;
`+`, `-`; `<`, `<=`, `>`, `>=`; `==`, `!=`; `and`; then `or`. Arithmetic is
numeric, except `Text + Text`; ordering accepts compatible non-null Text,
numeric, and DateTime representations. Operations return representation values,
so producing a constrained semantic type requires explicit construction.

### 10.2 Pattern matching

`match` handles closed alternatives and must be exhaustive.

```text
match order.status {
    OrderStatus.pending => {
        cancel(order)
    }
    OrderStatus.paid => {
        refund(order.payment)
        cancel(order)
    }
    OrderStatus.shipped => {
        reject AlreadyShipped
    }
    OrderStatus.delivered => {
        reject AlreadyDelivered
    }
}
```

The subject may be any typed expression, including a local, parameter, nested
child-field selection, grouped expression, equality expression, or callable
result. `Bool` is closed over `true` and `false`. Text, numeric, and other open
value spaces require a final `_` arm. Nullable values may match `none` and bind
the non-null value with `some(name)`; the binding retains its nominal type.

```text
match input.nickname {
    none => {}
    some(nickname) => {
        if nickname == Nickname("admin") {
            audit_admin_nickname()
        }
    }
}
```

### 10.3 Iteration

**Provisional:** one imperative iteration form is available.

```text
for item in items {
    process(item)
}
```

`while` appeared in the early candidate feature list but is not yet justified.
Loops, recursion, and resource limits need deliberate treatment because backend
code must remain bounded and auditable.

### 10.4 Skipping work

The early `match` example used `skip`, but its semantics are unclear.
**Open:** decide whether `skip` means a no-op value, loop continuation, or a
special scheduling operation. Until then, it is illustrative rather than
normative.

## 11. Errors and failures

The language should improve on untyped JavaScript exceptions without forcing
ordinary business code to manipulate `Result<T, E>` wrappers everywhere.

### 11.1 Domain failures

**Accepted:** expected business failures are typed, declared with `fails`, and
raised with `reject`.

```text
failure NotOwner {
    kind NotPermitted
    code "not_owner"
    message "You cannot modify this order."
}

failure AlreadyShipped {
    kind Conflict
    code "order_already_shipped"
    message "A shipped order cannot be cancelled."
}

action cancel_order(order: Order)
    fails NotOwner, AlreadyShipped
    -> Order
{
    if order.owner != current_user {
        reject NotOwner
    }

    if order.status == shipped {
        reject AlreadyShipped
    }

    return cancel(order)
}
```

**Accepted semantics:** every application failure derives from exactly one
standard kind. The kind determines default HTTP status, public disclosure,
retry classification, telemetry severity, and other boundary behaviour. The
domain failure supplies stable application meaning and a public code.

The explicit `kind` member and declaration-owned public/internal schemas are
accepted. See the
[failure model](failure-model.md) for the standard catalogue, disclosure
channels, operational faults, and non-HTTP mappings.

### 11.2 Propagation

**Accepted P10.6 direction:** every fallible expression is acknowledged with
`attempt`, including straightforward propagation. After local handling and
mapping, the callable's authored `fails` clause must exactly equal the closed
set of recoverable problems that can still escape. Missing declarations and
stale extra declarations are compile errors; the compiler reports the inferred
difference but never silently edits the contract.

```text
action cancel_order_by_id(id: Order.id)
    fails OrderNotFound, NotOwner, AlreadyShipped, Unavailable
    -> Order
{
    var order = attempt load_order(id)
    return attempt cancel_order(order)
}
```

The compiler calculates the transitive set through the call graph and checks it
against the written clause. A named action's route does not repeat that list:
the route derives its error surface from `run:`, and standard failure kinds
supply transport mapping without numeric statuses in application code.

For local handling or transformation, `attempt` grows a handler block:

```text
var order = attempt cancel_order(input.order_id) {
    NotFound => reject OrderNotFound
    NotOwner => reject Forbidden
    AlreadyShipped => reject CannotCancelOrder
}
```

The precise handler-arm grammar, replacement-value typing, and interaction with
`match` remain open.

### 11.3 Operational failures

Recoverable infrastructure conditions such as a database outage or network
timeout enter the same checked problem flow through source-agnostic built-ins
such as `Unavailable` and `TimedOut`. They must be handled, mapped, or written
in `fails`. Adapter names and raw driver failures remain internal, while true
defects stay outside catchable flow.

```text
service Stripe {
    timeout 5s
    retry 2
    on_failure fault Unavailable
}
```

Business code should not contain retry and timeout plumbing.

### 11.4 Bugs and impossible states

Bugs, violated invariants, and compiler/runtime defects are distinct from domain
failures. They produce enriched internal errors and a 500-class response at an
HTTP boundary.

The earlier discussion used `panic` as a possible spelling. **Open:** whether
application authors can invoke it directly. Ideally it is rare or absent from
normal source.

### 11.5 No ambient exceptions

**Accepted:** arbitrary, undeclared exceptions must not be an application-level
control-flow mechanism. A declaration's signature must expose its expected
domain failures.

Application code cannot use arbitrary `throw`, reject strings, manually return
numeric error statuses, or construct ad hoc error responses. Any unclassified
generated-target exception is treated as a compiler/runtime defect.

### 11.6 Public and internal failure information

By default a client receives only a stable failure code, safe static message,
and request ID. A failure may declare an explicit public detail schema. All
other context is internal and remains subject to redaction and retention policy.

Expected domain failures record a semantic rejection and propagation trace but
do not capture a native stack by default. Operational faults and defects retain
their low-level cause and stack internally. No stack is ever returned to a
public client.

## 12. External services

External network access is contract-driven and declared. Arbitrary HTTP calls
are not normal application code.

```text
service Stripe {
    secret STRIPE_SECRET

    operation create_payment
        POST /v1/payment_intents
        input StripeCreatePayment
        output StripePayment
        fails {
            card_declined => PaymentDeclined
            expired_card => PaymentDeclined
            rate_limit => PaymentTemporarilyUnavailable
            timeout => PaymentTemporarilyUnavailable
            authentication => fault Misconfigured
        }
}
```

Use in an action remains concise:

```text
action take_payment(order: Order)
    fails PaymentDeclined, PaymentTemporarilyUnavailable
    -> Payment
{
    var payment = Stripe.create_payment {
        amount: order.total
        currency: GBP
    }

    return payment
}
```

The integration boundary converts provider-specific failures into application
domain failures or operational faults. Raw provider error taxonomies do not
infect business code.

**Open:** the exact syntax for importing OpenAPI, recording a reviewed
LLM-derived contract, versioning a contract, and representing authentication.

## 13. Routes

Routes bind HTTP transport to typed application behaviour. Authentication and
all input/output validation are compiler/runtime concerns.

**Accepted:** every route requires authentication by default. Canonical source
does not repeat `auth required`; the secure behaviour is inherited from the
language and application policy.

```text
route POST /orders {
    input: CreateOrder

    run: place_order(input)

    output: Order
}
```

The route's `InvalidValue` and `Conflict` responses are derived from the failure
kinds reachable through `place_order` and appear automatically in generated
documentation.

**Accepted:** disabling authentication must be conspicuous and explicit.

```text
route GET /public-status {
    auth: none
    output: PublicStatus
}
```

The opt-out uses the same `name: value` form as every other route field. The
absence of `auth: none` retains authenticated access and never makes a route
public. `none` changes authentication only; it does not bypass route policy,
input validation, resource limits, or audit.

The current generated runtime has no authentication adapter yet. Consequently,
prototype acceptance routes use the explicit authentication opt-out, while
generation of a protected route fails rather than silently exposing it. This
temporary fixture repetition is not the intended application model.

**Accepted direction:** authentication strategies are configured outside route
business logic. Whether a request arrived through a session, OIDC/JWT, API key,
service identity, or another reviewed strategy, application code receives the
same typed actor context: authenticated identity, tenant where applicable,
allowlisted user information, permissions/capabilities, and authentication
strength. Routes inherit authentication and may eventually state a stronger
typed requirement or the conspicuous `none` exception; they do not name
provider SDKs or token
formats. Multiple enabled strategies require deterministic selection and may
not silently combine privileges. Exact configuration and strategy-declaration
syntax remains a P11 issue.

Path placeholders use braces in the route template and are typed together in a
`path: { ... }` group:

```text
route GET /customers/{customer_id}/orders/{order_id} {
    path: {
        customer_id: Customer.id
        order_id: Order.id
    }

    run: get_customer_order(path.customer_id, path.order_id)
    output: Order
}
```

The compiler requires exact correspondence: every template placeholder has one
typed path entry, every path entry appears in the template, and duplicate
placeholder names are invalid. Path values are decoded and nominally validated
before behaviour runs. Access through `path.name` prevents collisions with
query, header, and body fields.

Routes contain exactly one behaviour form: an inline `action` or a `run:`
invocation of a named action. Small, one-off behaviour remains local:

```text
route POST /todos {
    input: CreateTodo

    action: fails Unavailable {
        var todo = attempt create Todo {
            owner: current_user
            title: input.title
            due_at: input.due_at
        }

        return todo
    }

    output: Todo
}
```

An inline action has the same effect, transaction, policy, and exhaustive
recoverable-problem rules as a named action. The route-item colon separates
the `action:` key from the anonymous action value; its optional `fails` clause
still belongs to that action header. The body is delimited by braces and
indentation is never semantic.

Reusable behaviour, a stable domain command, or behaviour that deserves its own
testing, policy, or transaction boundary is extracted to a named action:

```text
action create_todo(input: CreateTodo, actor: Actor)
    fails Unavailable
    -> Todo
{
    return attempt create Todo {
        owner: actor
        title: input.title
        due_at: input.due_at
    }
}

route POST /todos {
    input: CreateTodo
    run: create_todo(input, current_actor)
    output: Todo
}
```

`run:` is only for this extracted form and keeps the mapping from typed
transport values to domain parameters visible. Abstraction remains available,
but the language should not encourage controller/service/repository/factory/
mapper layers for straightforward operations.

Query-string, ordinary-header, and body bindings also belong in the route and
are typed and validated before either behaviour form runs. Authentication
headers belong to the generated authentication boundary and are not ordinary
action inputs. Their exact binding-field grammar remains to be pressure-tested;
it will extend the existing brace-and-field route form rather than introduce
decorators, parameter annotations, or indentation-sensitive nesting.

**Open:** exact query/header/body binding fields, pagination, streaming, file
bodies, content negotiation, redirects, and route composition.

The compiler grammar and fixtures implement the accepted P10.6 route surface,
including `name: value` route items, inline `action:` blocks, named `run:`
invocations, and `auth: none`. The remaining open binding, pagination,
streaming, and composition questions above are not executable syntax.

## 14. Policies

Policy expresses human-approved intent independently enough for the compiler to
compare it with implementation behaviour.

```text
policy {
    Todo {
        read owner
        create authenticated
        update owner
        delete owner
    }
}
```

This syntax is illustrative. The important requirement is that an LLM must not
silently weaken human-owned policy to make its implementation compile. A policy
change requires an explicit human decision and appears clearly in review.

**Open:** whether policies live in the same files, a protected companion file,
or a separate signed/approved artifact; role and attribute syntax; record-level
proof rules; and how policy changes are authorised.

## 15. Events and jobs

External side effects and background work are declared so the compiler can audit
them and require retry/idempotency behaviour.

```text
event OrderCreated(order: Order)

action place_order(input: CreateOrder) -> Order {
    // ...
    emit OrderCreated(order)
    return order
}
```

```text
job overdue_reminders every 15m {
    var todos = attempt query Todo {
        where: done == false
        and due_at < now
        and reminder_sent == false
    }

    for todo in todos {
        emit TodoOverdue(todo)
    }
}
```

**Open:** event delivery guarantees, idempotency declarations, job concurrency,
transactions spanning event publication, scheduling syntax, and whether the
compiler can infer safe defaults.

## 16. Tests

Tests may live beside the behaviour they verify.

```text
test "addition preserves precedence" {
    var result = add(Number(2), Number(3 * 4))
    assert result == Number(14)
}
```

`assert` requires `Bool`. `jadpo test <project>` performs the authoritative
checked build, runs every authored test with Bun, emits one versioned compact
test report, and exits non-zero if any assertion fails. Failures contain source
byte ranges rather than host-language stack traces.

The compiler should generate obvious structural tests from known semantics—for
example, unauthenticated access is rejected, non-owners cannot access an
owner-only resource, malformed IDs fail before database access, and outputs
match their declared schemas.

Handwritten tests remain necessary for business intent that cannot be inferred.

**Open:** fixtures, property tests, clocks, randomness, external-service fakes,
and the boundary between generated and authored tests.

## 17. Formatting and canonical source

Formatting differences do not change program meaning. `jadpo fmt <project>`
provides deterministic indentation, blank-line, comment, and final-newline
normalisation. `jadpo fmt <project> --check` is read-only and fails when a file
would change. The same algorithm backs VS Code formatting.

It should run at deliberate checkpoints such as an explicit format command,
before compilation, pre-commit, or CI. It should not constantly rewrite a file
while an agent is performing a multi-step edit, because changing positions
during an iteration can make tool-based edits unreliable.

The compiler should report semantic errors against canonical source locations.
If the language initially compiles to TypeScript/Bun, generated TypeScript is an
implementation artifact. Source maps plus semantic node metadata must translate
runtime faults back to the original declaration and operation. Developers and
agents should not need to inspect generated `.ts` files.

## 18. Deliberately excluded or constrained features

The initial language should not include:

- `undefined` and ambient `null`;
- truthiness;
- raw SQL in ordinary code;
- arbitrary undeclared HTTP/network access;
- `eval` or runtime code loading;
- untyped dynamic objects;
- reflection or general metaprogramming;
- several equivalent conditional, iteration, or error styles;
- ambient exceptions as business control flow;
- hidden mutation;
- unchecked access to secrets;
- framework-defined prototype magic.

Some may eventually exist as narrow, explicit escape hatches. Every escape hatch
must be visible in generated audits and policy review.

## 19. Consolidated example

This example demonstrates the current direction without pretending to solve all
open design questions.

```text
type OrderStatus = Enum {
    pending
    paid
    cancelled
}

type CreateOrder = Object {
    items: List<CreateOrderItem>
}

type Order = Object {
    id: Id
    owner: Customer
    items: List<OrderItem>
    total: Money
    status: OrderStatus
    created_at: DateTime
}

persist Order {
    identity: id
}

failure EmptyBasket {
    kind InvalidValue
    code "empty_basket"
    message "Add at least one item."
}

failure CreditLimitExceeded {
    kind Conflict
    code "credit_limit_exceeded"
    message "The order exceeds the available credit limit."
}

action place_order(input: CreateOrder)
    fails EmptyBasket, CreditLimitExceeded
    -> Order
{
    if count(input.items) == 0 {
        reject EmptyBasket
    }

    var customer = current_user
    var total = calculate_total(input.items)

    if total > customer.credit_limit {
        reject CreditLimitExceeded
    }

    var order = attempt create Order {
        owner: customer
        items: input.items
        total: total
        status: pending
    }

    emit OrderCreated(order)

    return order
}

route POST /orders {
    input: CreateOrder

    run: place_order(input)

    output: Order
}
```

## 20. Questions the golden programs must answer

The next syntax pass should come from writing complete applications, not from
isolated language examples. At minimum, the todo and order/payment examples must
resolve:

1. How are files, modules, imports, visibility, and namespacing expressed?
2. What is the exact syntax for zero/one/exactly-one database queries?
3. Are functions strictly pure? Which effects may actions perform?
4. How are transactions declared or inferred?
5. How do partial entity updates consume omission-aware input safely?
6. What is the exact handler-arm and replacement-value grammar after an
   `attempt` prefix?
7. What aliasing and cause-wrapping syntax is needed for mapped failures?
8. Which collection operations exist, and are lambdas necessary?
9. Are `while`, recursion, and unbounded collections permitted?
10. How are money, currency, decimal arithmetic, dates, time zones, and durations
    represented?
11. How are relationships, compound uniqueness/indexes, lifecycle rules, and
    migrations declared?
12. How are authentication identities, roles, tenancy, ownership, and policy
    proofs represented?
13. How are secrets introduced and prevented from reaching outputs or logs?
14. How do external contracts, retries, idempotency, and webhooks work?
15. How are configuration and environment values declared and validated?
16. What is the comment, documentation, intent, and decision-record syntax?
17. What belongs in authored tests versus compiler-generated tests?
18. Which escape hatches are genuinely required for ordinary SaaS backends?
19. Which concrete workload, if any, would justify adding an explicit
    caller-visible mutation contract beyond returning an ordinary value?
20. Can a non-trivial pricing or scheduling algorithm be expressed cleanly?
21. Does every common construct have one obvious canonical spelling?
22. Do payment outcomes, provider responses, or lifecycle states justify
    data-carrying enum variants, and can their boundary, persistence, and
    migration representations remain explicit and deterministic?

## 21. Current syntax decisions at a glance

| Concern | Current direction |
| --- | --- |
| Binding | `var name = value` |
| Mutable binding | `var mut name = value` |
| Reassignment | `name = value`, for type-compatible `var mut` locals only |
| Parameter passing | immutable value semantics; representation sharing is unobservable |
| Absence | `T?` and `none` |
| Missing input field | `optional`, separate from `none` |
| Complete object | Never contains missing declared fields |
| Type compatibility | nominal semantic types, not structural substitution |
| Field type reference | `Type.field`, such as `Customer.id` |
| Validated construction | `Type(value)`, checked at compile time or runtime |
| Primitive call parameters | prohibited in ordinary application callables |
| Conditionals | `if` / `else` |
| Closed alternatives | exhaustive `match` |
| Variant-specific data | implemented nominal tagged enums with exhaustive payload patterns; persistence remains deferred |
| Iteration | `for item in items` |
| Expected failure | declared `fails`, raised with `reject` |
| Failure transport | standard kind maps automatically; no route status numbers |
| Failure disclosure | safe code/message by default; public details are explicit |
| Fallible expression | explicit `attempt` prefix |
| Local failure mapping | handler block spelling remains unresolved |
| Persistence | `create`, `query`, `update`, `delete` language constructs |
| External effects | declared `service`, `event`, and `job` constructs |
| Route security | authenticated by default; exact opt-out is `auth: none` |
| Validation | automatic at every trust boundary |
| Formatting | non-semantic, canonical formatter at checkpoints |
| Generated target code | never normal developer-facing source |

This draft should now be tested by writing the complete golden todo backend. Any
place where that program needs a construct not defined here becomes a language
design issue rather than an invitation to improvise silently.
