# Jadpo core grammar v0.1

**Status:** provisional implementation contract  
**Scope:** the first semantic compiler and Jadpo seed application  
**Not scope:** the complete language described by the broader syntax draft

This grammar freezes the smallest coherent source subset needed to build the
compiler front end. It intentionally excludes unresolved features rather than
letting parser code decide their semantics.

DATA-007, TX-001, CONSISTENCY-001, and WORKFLOW-001 now accept the [entity,
query, transaction, and cross-store consistency model](entity-query-model.md).
First-class `entity` dossiers are the accepted source model for identity-bearing
concepts; `type ... = Object` describes identity-free values. Legacy `type` plus
`persist` productions remain compatibility syntax, not the recommended model
for new applications. The bounded dossier/query/consistency surface below is
fixture-backed and executable; physical derived-store adapters and durable
workflow execution remain outside this grammar's runtime claim.

TIME-001 and TEST-001 accept the digest-pinned
[time and testing contract](time-testing-plan.md). The implemented prelude uses
`Instant`, `CalendarDate`, resolved zone-aware `Time`, fixed `Duration`, generated
`Zone`, bounded `Locale`, and presentation-classified text. Section 14 lists
these current names; the old `Date`/`DateTime` spellings are not alternatives for
new source. Authored tests and fixtures are described with the compilation unit below.

The semantic rules remain authoritative in the [semantic model](semantic-model.md),
[type system](type-system.md), and [failure model](failure-model.md). This
document controls spelling for the core compiler slice.

## 1. Source files and namespace

- Source files use the `.jadpo` extension.
- A project consists of all `.jadpo` files discovered beneath its source root.
- The core compiler has one application-wide namespace.
- Declaration order and file order do not affect name resolution.
- Duplicate names in the same namespace are errors.
- The bounded module extension supports one logical module per file, selective
  imports, and private-by-default declarations. Packages remain deferred.

## 2. Lexical grammar

The notation below is EBNF-like:

- `{ X }` means zero or more;
- `[ X ]` means optional;
- `( A | B )` means a choice;
- quoted text is a literal token.

```ebnf
letter              = "A".."Z" | "a".."z" ;
digit               = "0".."9" ;
identifier          = ( letter | "_" ), { letter | digit | "_" } ;
integer_literal     = [ "-" ], digit, { digit } ;
decimal_literal     = [ "-" ], digit, { digit }, ".", digit, { digit } ;
string_literal      = '"', { string_character | escape }, '"' ;
boolean_literal     = "true" | "false" ;
none_literal        = "none" ;

line_comment        = "//", { character_except_newline } ;
whitespace          = space | tab | newline | line_comment ;
```

Strings initially support `\"`, `\\`, `\n`, `\r`, and `\t`. Invalid UTF-8,
invalid escapes, and unterminated strings are lexical diagnostics.

Whitespace, newlines, and comments are non-semantic. The parser retains source
spans and comment trivia for diagnostics and future documentation tooling.

## 3. Naming diagnostics

Naming shape is checked semantically rather than encoded into token kinds. The
accepted [naming and qualification contract](naming-and-qualification.md) is
authoritative:

- nominal/type-like declarations use `UpperCamelCase`;
- fields, parameters, bindings, functions, actions, queries, jobs, capability
  instances, and variants use `lower_snake_case`;
- module segments and standard-library namespaces use `lower_snake_case`; and
- language keywords are lowercase.

The lexer emits one `identifier` token so a misspelt case can receive a targeted
naming diagnostic rather than a confusing parse error.

`input`, `output`, and `value` are contextual in name positions. The former
declaration spellings remain readable only for migration fixtures; canonical
source declares every shape with `type`. These words may still be used as
parameter, binding, field, or expression names. The seed's
`input: RegisterCustomer` is the canonical boundary-role example. This is a
parser rule, not implicit string rewriting.

## 4. Compilation unit

```ebnf
source_file         = [ module_declaration ], { import_declaration },
                      { [ "public" ], declaration } ;

declaration         = type_declaration
                    | application_declaration
                    | principal_declaration
                    | config_declaration
                    | entity_declaration
                    | persistence_declaration
                    | failure_declaration
                    | function_declaration
                    | action_declaration
                    | query_declaration
                    | route_declaration
                    | fixture_declaration
                    | test_declaration ;
```

The `application` declaration owns project-wide capabilities; it is not a
wrapper around the other declarations. The bounded module/import extension is
specified in section 15.

### 4.1 Application authentication and principal

```ebnf
application_declaration
                    = "application", identifier, "{",
                      application_authentication,
                      "}" ;
application_authentication
                    = "authentication", "{",
                      "principal", ":", type_expression,
                      revocation_declaration,
                      "}" ;
revocation_declaration
                    = "revocation", "{",
                      "mode", ":", revocation_mode,
                      [ "maximum_delay", ":", duration_literal ],
                      "}" ;
revocation_mode     = "immediate" | "bounded" ;
duration_literal    = ( integer_literal | decimal_literal ),
                      ( "ms" | "s" | "m" | "h" | "d" ) ;

principal_declaration
                    = "principal", identifier, "{",
                      { principal_variant },
                      "}" ;
principal_variant   = ( "user" | "service" ), object_body ;
```

A project has at most one application and one principal declaration. The
principal is closed over exactly one `user` and one `service` variant. Bounded
revocation requires a positive `maximum_delay`; immediate revocation forbids
one. Strategy selection and typed principal resolution are implemented. Fully
configured first-party user strategies can generate protected routes; unsupported
strategies and principal mappings remain fail-closed.

```ebnf
authentication_strategy = "authentication", identifier, "{",
                          { authentication_item }, "}" ;
authentication_item = transport | validators | claims | resolution ;
transport           = "transport", "{",
                      ( "cookie", ":", string_literal
                      | "bearer", ":", "authorization_header" ), "}" ;
validators          = "validators", "{", { validator }, "}" ;
validator           = identifier, "{", "mode", ":", identifier,
                      "principal", ":", ( "user" | "service" ),
                      { validator_setting }, "}" ;
validator_setting   = identifier, ":", expression ;
claims              = "claims", "{", { authentication_mapping }, "}" ;
authentication_mapping = identifier, "->", qualified_name ;
resolution          = "resolution", ( "user" | "service" ), "{",
                      "authority", ":", qualified_name,
                      "active", ":", expression,
                      "mappings", "{", { authentication_mapping }, "}",
                      "inactive", ":", identifier, "}" ;
```

Each strategy has one transport and a nonempty validator set. First-party
validator settings are `secret`, optional `previous_secret`, `audience`, and
cookie `origin`. Keys must reference secret textual configuration; audience and
origin must be non-secret text literals or configuration references. Duplicate
or unsupported settings are errors. Signed credential lifetime uses the
application's bounded revocation limit; opaque session lifetime is supplied at
the trusted host issuance boundary. See the
[first-party example](../examples/first-party-authentication/README.md) for the
supported runtime subset and remaining gates.

### 4.2 Bounded entity dossier and named query extension

```ebnf
entity_declaration  = "entity", identifier, "{",
                      { entity_field | entity_dossier_item | entity_operation },
                      "}" ;
entity_field        = identifier, ":", type_expression ;
entity_dossier_item = "identity", ":", identifier
                    | persistence_capability
                    | representation_declaration ;
persistence_capability
                    = "persistence", "{",
                      "store", ":", identifier,
                      "role", ":", "authority",
                      { "unique", ":", identifier },
                      "}" ;
representation_declaration
                    = ( "cache" | "projection" ), identifier, "{",
                      "store", ":", identifier,
                      "from", ":", identifier,
                      [ "strategy", ":", identifier ],
                      "delivery", ":", "durable",
                      "}" ;
entity_operation    = function_declaration | action_declaration
                    | query_declaration ;
query_declaration   = "query", identifier, parameters,
                      "freshness", ":", freshness,
                      [ fails_clause ], "->", type_expression, block ;
freshness           = "authoritative" | "read_your_writes"
                    | "bounded_staleness" | "eventual" ;
```

An entity operation may use `self: ref` or `self: value`; the compiler lowers
these to `Entity.Ref` or the complete entity snapshot. A mutating value receiver
also requires `guard: reload` or `guard: revision`. Actions may declare
`consistency: atomic` or `consistency: durable_workflow`. Top-level named
queries use the same `query_declaration` form. Entity-centred operations are
indexed under the qualified name `Entity.operation`; dot calls are checked
qualified calls and never trigger an implicit load or save.

### Authored tests and fixtures

```ebnf
test_declaration    = "test", string_literal, [ "using", identifier ], block ;
fixture_declaration = "fixture", identifier, "{", { fixture_setting }, "}" ;
fixture_setting     = "clock", ":", "fixed", expression
                    | "config", "{", { fixture_config_value }, "}" ;
fixture_config_value = identifier, ":", expression
                     | identifier, ":", "secret", "(", expression, ")" ;
assert_statement    = "assert", expression ;
advance_statement   = "advance", "clock", "by", expression ;
```

An assertion must evaluate to `Bool`. Tests may use ordinary checked statements,
assertions and the supported test-only operations. `advance clock by` requires
a fixture clock and a fixed duration. Fixture configuration is checked against
the application's `config` declaration; secret fields use `secret(...)`.
The [time/testing plan](time-testing-plan.md) defines the detailed isolation and
capability boundaries.

A minimal executable test needs no HTTP service:

```jadpo
type CardTitle = Text { min_length: 3 max_length: 12 }

test "a valid title is preserved" {
    var title = CardTitle("First card")
    assert title == CardTitle("First card")
}
```

Run `jadpo test <project>`. A project with no authored `test` declarations reports
`TEST_NO_TESTS`; a successful `check` or `build` alone does not run tests. Each
fixture-backed test receives fresh supported test state. Root `app.jadpo` may
contain shared failures and tests; recognised role directories constrain the
kinds of declarations they contain, as described in the
[project-role contract](entity-query-model.md#10-project-roles).

## 5. Type expressions

```ebnf
type_expression     = primary_type, [ "?" ] ;

primary_type        = named_type
                    | field_type
                    | generic_type
                    | inline_object_type ;

named_type          = identifier ;
field_type          = identifier, ".", identifier ;
generic_type        = identifier, "<", type_expression,
                      { ",", type_expression }, ">" ;
inline_object_type  = "Object", object_body ;
```

`Customer.email` is parsed as a field type in type position and as member access
in expression position. Name resolution determines whether the owner declaration
and field exist.

The parser and runtime support the prelude containers `List<T>`, `Set<T>`, and
`Map<K, V>`. User-defined generic declarations remain unsupported.

Applying `?` to an already nullable field type is a redundant-nullability
diagnostic rather than nested optionality.

## 6. Type and constraint declarations

```ebnf
type_declaration    = "type", identifier, "=",
                      ( scalar_type_body | object_type_body | enum_type_body ) ;

scalar_type_body    = type_expression, constraint_block ;
object_type_body    = "Object", object_body ;
enum_type_body      = "Enum", "{", enum_variant, { enum_variant }, "}" ;
enum_variant        = identifier, [ object_body ] ;

constraint_block    = "{", { constraint }, "}" ;

constraint          = "min", ":", number_literal
                    | "max", ":", number_literal
                    | "min_length", ":", integer_literal
                    | "max_length", ":", integer_literal
                    | "pattern", ":", string_literal
                    | "format", ":", identifier ;

number_literal      = integer_literal | decimal_literal ;
```

The core constraint vocabulary is `min`, `max`, `min_length`, `max_length`,
`pattern`, and `format: email`. Unknown constraints are errors rather than
arbitrary callbacks.

Every `type` declaration creates a new nominal type. The syntax never means a
transparent alias.

## 7. Object types and persistence

```ebnf
object_body         = "{", { field_declaration }, "}" ;

field_declaration   = identifier, ":", type_expression,
                      [ constraint_block ], [ "optional" ] ;

persistence_declaration = "persist", identifier, "{",
                            { persistence_item },
                          "}" ;
persistence_item    = "identity", ":", identifier
                    | "unique", ":", identifier
                    | "index", ":", identifier
                    | reference_item
                    | inverse_declaration ;
reference_item      = "references", identifier, ":", qualified_name,
                      [ "as", ":", identifier ],
                      "on_delete", ":", reference_delete_action ;
reference_delete_action = "restrict" | "cascade" | "set_null" ;
inverse_declaration = "inverse", identifier, ":", inverse_cardinality, identifier,
                      "via", ":", qualified_name ;
inverse_cardinality = "many" | "optional" ;
```

`optional` is meaningful when an object is used as route input. It means the
field may be omitted from that boundary shape and is distinct from `?`, which
means a supplied value may be `none`.

Every field declaration creates a nominal field type refining the written type.
Field constraints, if present, further refine intrinsic value validation.

`persist Name { ... }` is the only storage opt-in. A persisted object may
declare at most one non-nullable `identity`; it becomes the generated primary
key. `unique` creates a stable named unique constraint, while `index` creates a
stable named non-unique index. These are storage properties and do not become
part of the field's nominal value type. Without `persist`, an object type is
ordinary non-persistent application data.

An owning field on a persisted object may declare
`references owner_id: User.id as: owner on_delete: cascade` (or
`restrict`/`set_null`). `as owner` explicitly separates the logical traversal
name from a stored field such as `owner_id`; when it is omitted, the field name
is also the relationship name. The compiler never infers a name by stripping
an `_id` suffix. The referenced field must be a non-nullable identity or unique
field, and the owning field's nominal type must be that exact field type.
`set_null` additionally requires a nullable owner field. The compiler generates
a foreign key and lookup index, orders fresh-schema table creation by
dependency, and diagnoses cross-object dependency cycles it cannot emit.
A persisted object may declare an inverse collection as
`inverse todos: many Todo via: Todo.owner_id`, or a zero-or-one inverse as
`inverse profile: optional Profile via: Profile.user_id`. The `via` field must
be an owning reference on the named child entity that points back to the
declaring parent. An optional inverse additionally requires that field to be
`identity` or `unique`, making the at-most-one claim enforceable in storage.
Inverse declarations are metadata, not stored entity fields and not implicit
lazy-loading accessors.

## 8. Configuration declarations

```ebnf
config_declaration  = "config", identifier, "{",
                        { config_field },
                      "}" ;

config_field        = identifier, ":", type_expression, "{",
                        config_binding,
                        [ config_secret ],
                        [ config_default ],
                      "}" ;

config_binding      = "binding", ":", string_literal ;
config_secret       = "secret", ":", boolean_literal ;
config_default      = "default", ":",
                      ( string_literal | integer_literal | decimal_literal
                      | boolean_literal | duration_literal ) ;
duration_literal    = number, ( "ms" | "s" | "m" | "h" | "d" ) ;
```

There is exactly one application configuration declaration. Options are
structured and may be written in any order, but each may appear at most once.
Every field has one non-empty, portable environment `binding`; fields are
required unless they have a checked literal `default`. A `secret: true` field
cannot have a source default and may flow only to a compiler-owned declared
secret sink. All v0.1 fields are startup-bound. Authored behavior reads typed
values through `config.<field>` and cannot inspect the process environment.

Local development uses `.env.local`; it is not part of this source grammar.
Generated production startup reads only declared binding names and disables
automatic environment-file discovery.

## 9. Failure declarations

```ebnf
failure_declaration = "failure", identifier, "{",
                        "kind", ":", identifier,
                        "code", ":", string_literal,
                        [ "message", ":", string_literal ],
                        [ public_schema ],
                        [ internal_schema ],
                      "}" ;

public_schema       = "public", record_body ;
internal_schema     = "internal", record_body ;
```

The identifier after `kind` must resolve to a standard failure kind. `public`
and `internal` schemas use field declarations but cannot use `optional` in the
first slice. Their field names must be disjoint, and public codes are stable
contract values. Rejection sites supply one flat object containing exactly the
combined public and internal fields; the declaration remains the sole owner of
which values may cross the public boundary.

Compatible-code alias syntax is deferred under `FAIL-002`; the seed does not
need it.

## 10. Callable declarations

```ebnf
function_declaration = "function", identifier, parameter_list,
                       [ fails_clause ], "->", type_expression, block ;

action_declaration   = "action", identifier, parameter_list,
                       [ fails_clause ], "->", type_expression, block ;

parameter_list       = "(", [ parameter,
                         { ",", parameter } ], ")" ;
parameter            = identifier, ":", type_expression ;

fails_clause         = "fails", identifier, { ",", identifier } ;
```

Functions are pure in the core. Actions may reject declared domain failures.
The P10 extension permits `create`, `query optional`, `query required`,
`update required`, and `delete required` only inside actions; other database
and external effects remain unavailable until their compiler phases.

Raw representation primitives in application callable parameter or return
positions are semantic errors, not parse errors.

## 11. Statements

```ebnf
block               = "{", { statement }, "}" ;

statement           = binding_statement
                    | assignment_statement
                    | return_statement
                    | reject_statement
                    | if_statement ;

binding_statement   = "var", [ "mut" ], identifier,
                      [ ":", type_expression ], "=", expression ;

assignment_statement = identifier, "=", expression ;

return_statement    = "return", expression ;

reject_statement    = "reject", identifier, [ rejection_body ] ;

rejection_body      = object_body ;

if_statement        = "if", expression, block, [ "else", block ] ;
```

There are no semicolons. Assignment is distinguished by the single `=` after a
bare local name; equality uses `==`, and arbitrary expression statements are
not accepted. Loops remain outside the first slice. `attempt` is an expression
prefix, not a statement or handler form.

`var` bindings and parameters are immutable. Only a local introduced with
`var mut` may be reassigned, and every assigned value must be compatible with
the binding's established nominal type. Assignment cannot target a parameter, field path, or
other value. This is local rebinding rather than observable reference or
interior mutation.

## 12. Expressions

```ebnf
expression          = equality_expression ;

equality_expression = prefix_expression,
                      { ( "==" | "!=" ), prefix_expression } ;

prefix_expression   = [ "attempt" ], primary_expression ;

primary_expression  = literal
                    | named_expression
                    | outcome_match_expression
                    | create_expression
                    | query_expression
                    | update_expression
                    | delete_expression
                    | "(", expression, ")" ;

outcome_match_expression
                    = "match", qualified_name, invocation_suffix, "{",
                      { outcome_match_arm },
                      "}" ;
outcome_match_arm   = outcome_success_arm | outcome_failure_arm ;
outcome_success_arm = "success", "(", identifier, ")", "=>", expression ;
outcome_failure_arm = "failure", identifier, "=>", outcome_arm_body ;
outcome_arm_body    = expression
                    | reject_statement
                    | "propagate" ;

create_expression   = "create", identifier, object_body,
                      [ "conflict", ":", failure_binding ] ;
query_expression    = "query", query_cardinality, identifier, "{",
                      "where", ":", identifier, "==", expression,
                      [ "order_by", ":", identifier, order_direction ],
                      [ pagination_clause ],
                      { include_clause },
                      [ "missing", ":", failure_binding ],
                      "}" ;
include_clause      = inverse_include_clause | parent_include_clause ;
inverse_include_clause
                    = "include", ":", identifier, "into", ":", identifier,
                      "order_by", ":", identifier, order_direction,
                      pagination_clause ;
parent_include_clause
                    = "include", ":", relationship_path,
                      parent_include_cardinality,
                      "into", ":", identifier ;
relationship_path   = identifier, [ ".", identifier ] ;
parent_include_cardinality
                    = "required" | "optional" ;
pagination_clause   = "limit", ":", integer_literal,
                      "offset", ":", integer_literal ;
query_cardinality   = "optional" | "required" | "many" ;
order_direction     = "asc" | "desc" ;
failure_binding     = identifier, [ object_body ] ;
update_expression   = "update", "required", identifier, "{",
                      "where", ":", identifier, "==", expression,
                      ( "set", ":", object_body
                      | "patch", ":", qualified_name,
                        "empty", ":", failure_binding,
                        [ "set", ":", patch_set_body ] ),
                      "missing", ":", failure_binding,
                      "conflict", ":", failure_binding,
                      "}" ;
patch_set_body      = "{", { identifier, ":", expression,
                        [ "when", qualified_name, "supplied" ] }, "}" ;
delete_expression   = "delete", "required", identifier, "{",
                      "where", ":", identifier, "==", expression,
                      "missing", ":", failure_binding,
                      "conflict", ":", failure_binding,
                      "}" ;

literal             = string_literal
                    | integer_literal
                    | decimal_literal
                    | boolean_literal
                    | none_literal ;

named_expression    = qualified_name,
                      [ invocation_suffix | construction_suffix ] ;
qualified_name      = identifier, { ".", identifier } ;
invocation_suffix   = "(", [ argument, { ",", argument } ], ")" ;
construction_suffix = object_body ;
argument            = expression ;
object_body         = "{", { field_initialiser }, "}" ;
field_initialiser   = identifier, ":", expression ;
```

`Email(value)` and `register_customer(input)` share the same invocation syntax.
Name resolution distinguishes validated type construction from callable
invocation. A qualified name without a suffix represents a binding or field
access. A qualified name followed by an object body is record construction.

The accepted callable-ownership rule additionally reserves lowercase
compiler-owned standard-library namespaces such as `temporal` and `collection`.
Their operations use one mandatory qualified spelling. They cannot be opened by
an import, shadowed, aliased, or mirrored as receiver methods. Authored free
callables remain unqualified within their module or selective-import scope;
entity-owned operations retain their existing entity-qualified or receiver-dot
forms. The grammar already accepts the shared qualified-name shape; individual
standard-library families enter the executable prelude with their owning
implementation milestone.

Every invocation or persistence expression that can fail must be acknowledged
with `attempt` or, for a direct callable invocation, an exhaustive outcome
match. `attempt` propagates the exact failure set. An outcome match contains
exactly one `success(value)` arm and one named arm for every callable failure;
wildcards are rejected. A failure arm returns a compatible replacement value,
rejects a mapped failure, or uses `propagate`. The callable's `fails` clause
must equal the complete reachable, unhandled set after those decisions.
Failure-context binding is not part of this executable grammar.

`RegistrationAccepted { email: input.email }` constructs a complete record.
`create Customer { ... }` is the P10 persistent-create expression. Its target
must be an entity, its object body must be complete, and the expression has the
entity's nominal type. A function using it is an effect error.
Each initialiser is checked against the field's nominal identity, such as
`Customer.id`, rather than only its declared representation parent.
An optional `conflict:` binding translates a normalised database constraint
violation into a failure declared by the action and derived from `Conflict`.
Without that binding, a constraint violation remains an operational fault.
Specific bindings use a compiler-owned identity, for example
`conflict Account.handle: AccountHandleTaken`; several specific bindings may
precede one fallback. Unknown or duplicate mappings are rejected.

`query optional Customer { where: id == input.id }` has type `Customer?`: zero
rows produce `none`. `query required` has the non-nullable entity type and must
include `missing: FailureName`; that failure must be declared by the action and
derive from `NotFound`. The binding may supply the same typed `public` and
`internal` context as `reject`. Both forms validate one returned row and treat
multiple rows as a contained operational cardinality fault. Query targets must
be entities, predicate fields must be known and non-nullable, and values must
carry the field's nominal identity. Queries are action-only. Many-result,
compound predicates and projections remain outside this slice.

`query many Todo { ... order_by: id asc limit: 100 offset: 0 }` has type
`List<Todo>`. Ordering is
mandatory, and the ordering field must currently be an identity or unique key
so results are deterministic without an implicit tie-breaker. Every returned
row is validated as the entity before it enters the collection. Literal
pagination is optional for a direct many query and parameterised when present;
compound predicates remain outside this slice.

A required parent query may explicitly include one or more inverse collections:

```text
attempt query required User {
    where: id == input.id
    include: todos into: UserActivity order_by: id asc limit: 100 offset: 0
    include: notes into: UserActivity order_by: id desc limit: 20 offset: 0
    missing: UserNotFound
}
```

The named output must contain exactly `parent: User` and one field matching
each included inverse name, such as `todos: List<Todo>` and
`notes: List<Note>`. Repeated includes must use the same output shape; duplicate
relationships are rejected. Each child ordering key must be identity or
unique. `limit` must be a positive integer literal and `offset` a
non-negative integer literal in this first bounded slice. Both become SQL
parameters. The compiler currently chooses a bounded two-query batch, validates
the parent and every child before constructing the output, and records that
choice in persistence query-plan metadata. Ordinary entity queries never load
relationships implicitly.

A `many` parent query may include the same nested shape only when the parent
query itself has deterministic ordering and explicit pagination:

```text
attempt query many User {
    where: group == input.group
    order_by: id asc
    limit: 20 offset: 0
    include: todos into: UserTodos order_by: id asc limit: 100 offset: 0
}
```

This form has type `List<UserTodos>`. The generated plan pages parents in a CTE
before joining ranked child rows, so child expansion cannot change which
parents belong to the page. The one-include plan executes as one query and is
recorded as `parent_page_join` metadata. With repeated includes, the compiler
executes one independently bounded parent-page join per relationship and merges
them by the deterministic parent key, avoiding both Cartesian multiplication
and N+1 execution.

A required child query may instead traverse one owning reference to its parent:

```text
attempt query required PatchItem {
    where: id == input.id
    include: reviewer_id optional into: PatchItemReviewer
    missing: PatchItemNotFound
}
```

The included name is the stored reference field. `optional` is required when
that field is nullable and produces an output field of type `Parent?`;
`required` is accepted only for a non-nullable reference and produces
`Parent`. The output contains exactly `parent: PatchItem` plus the included
field. The generated bounded plan reads the child and then the referenced
parent, performs no lazy access, and records `bounded_parent_lookup` with a
query count of two.

A required parent query may load a declared optional inverse:

```text
attempt query required User {
    where: id == input.id
    include: profile optional into: UserProfile
    missing: UserNotFound
}
```

The output contains exactly `parent: User` and `profile: Profile?`. The child
owning reference must be unique, and the generated two-query
`bounded_optional_inverse` plan returns `none` when no child exists.

The first nested slice accepts exactly two hops: a non-nullable owning reference
followed by a declared optional inverse:

```text
attempt query required Todo {
    where: id == input.id
    include: owner.profile optional into: TodoOwnerProfile
    missing: TodoNotFound
}
```

The outer output contains `parent: Todo` and `owner: UserProfile`; the inner
output contains `parent: User` and `profile: Profile?`. The compiler rejects
other hop/cardinality combinations and records a `bounded_nested_lookup` with
`maximum_depth: 2` and `query_count: 3`.

`update required` and `delete required` each match exactly one entity or reject
their declared `missing:` failure, which must derive from `NotFound`. Database
constraint violations reject the declared `conflict:` failure, which must
derive from `Conflict`. Both expressions return the validated affected entity.
An update either requires one or more distinct fields in `set:`, or names a
direct input binding in `patch:`. Every replacement is checked against its
entity field's nominal identity. A patch input is a non-empty record whose
fields are all declared `optional` and correspond to known target fields. If no
patch field was supplied, `empty:` rejects a declared `InvalidValue` failure.
For each patch field, one fixed parameterised statement binds a supplied flag
and a value; omitted fields retain their stored values, while an explicitly
supplied `none` clears a nullable field. An optional following `set:` block may
add fixed derived writes; `value when patch.field supplied` applies only when
that exact patch field was present. A field cannot be written by both `patch:`
and derived `set:`. The generated adapter preflights
cardinality and mutates inside the current prototype's inferred action
transaction, so multiple matches or constraint failures leave no partially
committed fields. The accepted entity/query model retains failure atomicity but
requires explicit intent before several entity mutation scopes are combined.

The exact colon-delimited spelling for named compound uniqueness remains open.
The former call-shaped `constraint name: unique(field_a, field_b)` spelling is
not canonical and must not be copied into new source.

The expression grammar implements conventional precedence for unary `not` and
`-`; multiplicative `*`, `/`, and `%`; additive `+` and `-`; ordering `<`,
`<=`, `>`, and `>=`; equality `==` and `!=`; `and`; then `or`. Arithmetic is
numeric except for `Text + Text`, and `match` is exhaustive for closed domains.

## 13. Route declarations

```ebnf
route_declaration   = "route", http_method, route_path, "{",
                        { route_item },
                      "}" ;

http_method         = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" ;
route_path          = path_token ;

route_item          = "auth", ":", "none"
                    | "path", ":", route_path_schema
                    | "input", ":", type_expression
                    | "output", ":", type_expression
                    | "run", ":", qualified_name, invocation_suffix
                    | inline_action ;

route_path_schema   = "{", { field_declaration }, "}" ;
inline_action       = "action", ":", [ fails_clause ], block ;
```

The first route item above is spelled `auth: none`; the older
`auth: public explicitly` form is not accepted. Authentication is required when
`auth: none` is absent. Any other authored value produces the human-owned
`route.auth_value_invalid` diagnostic over that value. The diagnostic does not
guess whether authentication should be retained or disabled: it offers removal
of the item and replacement with `none` as two non-preferred choices, reports
their distinct public-security effects, and recovers at the next route item so
valid `input:`, `output:`, and `run:` members do not become cascade errors.

The lexer reads the non-whitespace token after the method as `path_token`.
Every `{name}` placeholder must have exactly one same-named typed field in the
`path: { ... }` block, and every path field must have exactly one placeholder.
Path fields contain only a required semantic type; nullable/optional fields,
constraints, references, and persistence modifiers are invalid there.
Handler expressions access those bindings as `path.name`. Matching is exact;
each matched segment is percent-decoded and nominally validated before the
handler runs.

Every route item uses `:` between its name and value. Space-only forms such as
`input CreateOrder` are syntax errors rather than alternate spellings.

A core route has at most one path, input, and output item and exactly one
behaviour: either `run:` or one inline `action:`, never both. Their textual order
is not semantic; the formatter chooses a canonical order.

Reachable failures are derived through `run` or the inline action's exact
`fails` set. Routes do not contain numeric status mappings. Query, header, and
body binding grammar remains unresolved and is not inferred from the path form.

## 14. Core prelude

This section describes the current implemented prelude, including TIME-001.

The compiler initially recognises these representation or standard semantic
types without user declarations:

```text
Bool
Int
Decimal
Text
Bytes
Uuid
Instant
CalendarDate
Time
Duration
Zone
Locale
PresentationText
InstantRange
LocalOverlap
LocalGap
InvalidDay
Weekday
TimeFormat
FriendlyTimeFormat
Unit
Object
List<T>
Set<T>
Map<K, V>
Email
Url
IpAddress
```

It also recognises the initial standard failure kinds, including
`RateLimited`, `Unavailable`, `TimedOut`, and `OutcomeUnknown`, from the
[failure model](failure-model.md). Application declarations cannot shadow
prelude names.

Prelude types still obey semantic rules. For example, `Text` cannot be used as
an ordinary application callable parameter merely because it is in the prelude.

## 15. P10.5 module extension

The optional module grammar is:

```text
module-declaration ::= "module" qualified-name
import-declaration ::= "import" qualified-name "{" name ("," name)* "}"
public-declaration ::= "public" declaration
```

A modular source file begins with exactly one `module` declaration, followed by
zero or more selective imports and then declarations. Once any source file in a
project declares a module, every source file must declare one. Module names are
explicit stable logical identities; directory names do not silently become
semantic namespaces.

Declarations are private to their module unless prefixed with `public`.
Imports name a module and a non-empty set of its public declarations. Importing
a private or unknown declaration, using a cross-file declaration without an
import, importing the same local name twice, colliding with a local declaration,
declaring the same module in two files, mixing modular and ambient files, or
forming an import cycle is an error. Routes cannot use the module `public`
prefix because their external visibility is governed by route and policy
semantics.

The checked manifest records canonical module names, source files, selective
imports, and exports. The bounded first implementation retains globally unique
declaration names internally; aliases, re-exports, same-name declarations in
separate modules, relative imports, multi-file modules, and external packages
remain unsupported rather than receiving implicit semantics.

Legacy projects with no module headers retain the original single ambient
namespace so existing core fixtures and the Jadpo seed remain valid.

## 16. Explicitly unsupported in the first slice

The parser or semantic checker produces a stable unsupported-feature diagnostic
for:

- import aliases, re-exports, relative imports, multi-file modules, same-name
  declarations in separate modules, and external packages;
- loops and recursion;
- assignment outside a bare, fixed-type `var mut` local;
- failure-context binding in outcome arms;
- compound many-result predicates and general patch-condition expressions;
- services, events, and jobs;
- policies and `require`;
- query/header/body route bindings;
- defaults and compound non-unique indexes;
- relationship paths deeper than two, nested to-many paths without per-hop
  bounds, and required inverse-one declarations without an enforceable totality
  model or explicit domain absence path;
- generic user-defined types;
- transparent aliases;
- macros, reflection, and escape hatches.

Enum variants use qualified dot syntax such as
`InviteStatus.blocked`; `InviteStatus("blocked")` and dynamic call-style enum
construction are invalid.

Unsupported does not mean rejected forever. It means the feature cannot enter
the implementation without a specification and fixture.

## 17. Seed coverage

The [Jadpo seed](../examples/jadpo-seed/app.jadpo) exercises:

- constrained nominal types;
- an object type, a separate persistence declaration, and field type references;
- route input and output roles using ordinary object types;
- field refinement chains;
- a standard-kind domain failure with flat rejection context and a declaration-
  owned internal field;
- an action with exact `fails`, `if`, validated construction, `reject`,
  `attempt`, and `return`;
- explicit `auth: none` routes using named and inline behaviour, including a
  typed path binding;
- automatic failure-to-boundary metadata.

Every token in that file is covered by this grammar. Its semantic expectations
are recorded separately so parser success cannot be mistaken for compiler
correctness.
