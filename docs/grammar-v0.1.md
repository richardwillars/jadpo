# Jadpo core grammar v0.1

**Status:** provisional implementation contract  
**Scope:** the first semantic compiler and Jadpo seed application  
**Not scope:** the complete language described by the broader syntax draft

This grammar freezes the smallest coherent source subset needed to build the
compiler front end. It intentionally excludes unresolved features rather than
letting parser code decide their semantics.

The semantic rules remain authoritative in the [semantic model](semantic-model.md),
[type system](type-system.md), and [failure model](failure-model.md). This
document controls spelling for the core compiler slice.

## 1. Source files and namespace

- Source files use the `.jadpo` extension.
- A project consists of all `.jadpo` files discovered beneath its source root.
- The core compiler has one application-wide namespace.
- Declaration order and file order do not affect name resolution.
- Duplicate names in the same namespace are errors.
- Modules, imports, visibility, and packages are deferred under `MOD-001`.

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

Naming shape is checked semantically rather than encoded into token kinds:

- named types, records, failures, and standard kinds use `UpperCamelCase`;
- fields, parameters, bindings, functions, and actions use `lower_snake_case`;
- language keywords are lowercase.

The lexer emits one `identifier` token so a misspelt case can receive a targeted
naming diagnostic rather than a confusing parse error.

`input`, `output`, and `value` are contextual in name positions. They retain
their keyword tokens for declaration dispatch but may be used as parameter,
binding, field, or expression names. The seed's `input: RegisterCustomer` is the
canonical example. This is a parser rule, not implicit string rewriting.

## 4. Compilation unit

```ebnf
source_file         = { declaration } ;

declaration         = type_declaration
                    | entity_declaration
                    | value_declaration
                    | input_declaration
                    | output_declaration
                    | failure_declaration
                    | function_declaration
                    | action_declaration
                    | route_declaration ;
```

The first slice has no `app` wrapper or import declarations.

## 5. Type expressions

```ebnf
type_expression     = primary_type, [ "?" ] ;

primary_type        = named_type
                    | field_type
                    | generic_type ;

named_type          = identifier ;
field_type          = identifier, ".", identifier ;
generic_type        = identifier, "<", type_expression,
                      { ",", type_expression }, ">" ;
```

`Customer.email` is parsed as a field type in type position and as member access
in expression position. Name resolution determines whether the owner declaration
and field exist.

The parser accepts generic type shape so built-in `List<T>`, `Set<T>`, and
`Map<K, V>` can be represented later. The first semantic slice may diagnose them
as unsupported under `TYPE-002`.

Applying `?` to an already nullable field type is a redundant-nullability
diagnostic rather than nested optionality.

## 6. Type and constraint declarations

```ebnf
type_declaration    = "type", identifier, "=", type_expression,
                      constraint_block ;

constraint_block    = "{", { constraint }, "}" ;

constraint          = "min", number_literal
                    | "max", number_literal
                    | "min_length", integer_literal
                    | "max_length", integer_literal
                    | "pattern", string_literal
                    | "format", identifier ;

number_literal      = integer_literal | decimal_literal ;
```

The core constraint vocabulary is `min`, `max`, `min_length`, `max_length`,
`pattern`, and `format email`. Unknown constraints are errors rather than
arbitrary callbacks.

Every `type` declaration creates a new nominal type. The syntax never means a
transparent alias.

## 7. Record declarations

```ebnf
entity_declaration  = "entity", identifier, record_body ;
value_declaration   = "value", identifier, record_body ;
input_declaration   = "input", identifier, record_body ;
output_declaration  = "output", identifier, record_body ;

record_body         = "{", { field_declaration | inverse_declaration }, "}" ;

field_declaration   = identifier, ":", type_expression,
                      [ constraint_block ],
                      { persistence_modifier },
                      [ reference_clause ], [ "optional" ] ;

persistence_modifier = "identity" | "unique" | "index" ;
reference_clause    = "references", qualified_name, [ "as", identifier ],
                      "on_delete", reference_delete_action ;
reference_delete_action = "restrict" | "cascade" | "set_null" ;
inverse_declaration = "inverse", identifier, ":", inverse_cardinality, identifier,
                      "via", qualified_name ;
inverse_cardinality = "many" | "optional" ;
```

`optional` is valid only on input fields in the core. It is postfix and means
the field may be omitted from the input shape. It is distinct from `?`, which
means a supplied value may be `none`.

Every field declaration creates a nominal field type refining the written type.
Field constraints, if present, further refine intrinsic value validation.

`identity`, `unique`, and `index` are accepted only on entity fields. An entity
may declare at most one non-nullable `identity`; it becomes the generated
primary key. `unique` creates a stable named unique constraint, while `index`
creates a stable named non-unique index. These are storage properties and do
not become part of the field's nominal value type. Defaults, compound
constraints remain later P10 grammar extensions.

An owning entity field may declare
`references User.id as owner on_delete cascade` (or
`restrict`/`set_null`). `as owner` explicitly separates the logical traversal
name from a stored field such as `owner_id`; when it is omitted, the field name
is also the relationship name. The compiler never infers a name by stripping
an `_id` suffix. The referenced field must be a non-nullable identity or unique
field, and the owning field's nominal type must be that exact field type.
`set_null` additionally requires a nullable owner field. The compiler generates
a foreign key and lookup index, orders fresh-schema table creation by
dependency, and diagnoses cross-entity dependency cycles it cannot emit.
An entity may declare an inverse collection as
`inverse todos: many Todo via Todo.owner_id`, or a zero-or-one inverse as
`inverse profile: optional Profile via Profile.user_id`. The `via` field must
be an owning reference on the named child entity that points back to the
declaring parent. An optional inverse additionally requires that field to be
`identity` or `unique`, making the at-most-one claim enforceable in storage.
Inverse declarations are metadata, not stored entity fields and not implicit
lazy-loading accessors.

## 8. Failure declarations

```ebnf
failure_declaration = "failure", identifier, ":", identifier, "{",
                        "code", string_literal,
                        [ "message", string_literal ],
                        [ public_schema ],
                        [ internal_schema ],
                      "}" ;

public_schema       = "public", record_body ;
internal_schema     = "internal", record_body ;
```

The identifier after `:` must resolve to a standard failure kind. `public` and
`internal` schemas use field declarations but cannot use `optional` in the first
slice. Public codes are stable contract values.

Compatible-code alias syntax is deferred under `FAIL-002`; the seed does not
need it.

## 9. Callable declarations

```ebnf
function_declaration = "function", identifier, parameter_list,
                       "->", type_expression, block ;

action_declaration   = "action", identifier, parameter_list,
                       "->", type_expression,
                       [ fails_clause ], block ;

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

## 10. Statements

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

rejection_body      = "{", [ public_values ], [ internal_values ], "}" ;
public_values       = "public", object_body ;
internal_values     = "internal", object_body ;

if_statement        = "if", expression, block, [ "else", block ] ;
```

There are no semicolons. Assignment is distinguished by the single `=` after a
bare local name; equality uses `==`, and arbitrary expression statements are
not accepted. Loops, `match`, and `attempt` are outside the first slice.

`var` bindings and parameters are immutable. Only a local introduced with
`var mut` may be reassigned, and every assigned value must be compatible with
the binding's established nominal type. Assignment cannot target a parameter, field path, or
other value. This is local rebinding rather than observable reference or
interior mutation.

## 11. Expressions

```ebnf
expression          = equality_expression ;

equality_expression = primary_expression,
                      { ( "==" | "!=" ), primary_expression } ;

primary_expression  = literal
                    | named_expression
                    | create_expression
                    | query_expression
                    | update_expression
                    | delete_expression
                    | "(", expression, ")" ;

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
failure_binding     = identifier,
                      [ "{", [ "public", object_body ],
                              [ "internal", object_body ], "}" ] ;
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
query required User {
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
query many User {
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
query required PatchItem {
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
query required User {
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
query required Todo {
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
cardinality and mutates inside the inferred action transaction, so multiple
matches or constraint failures leave no partially committed fields.

An entity may declare compound uniqueness with
`constraint name: unique(field_a, field_b)`. The name becomes the stable source
identity used by precise conflict mappings. Compound fields must be known,
distinct, and non-nullable; duplicate field sets are rejected.

The first expression grammar supports equality because the seed uses it.
Arithmetic, ordering, boolean operators, exhaustive `match`, and richer
precedence are added only through fixtures.

## 12. Route declarations

```ebnf
route_declaration   = "route", http_method, route_path, "{",
                        { route_item },
                      "}" ;

http_method         = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" ;
route_path          = path_token ;

route_item          = "auth", ":", "public", "explicitly"
                    | "input", ":", type_expression
                    | "output", ":", type_expression
                    | "run", ":", qualified_name, invocation_suffix ;
```

The lexer reads the non-whitespace token after the method as `path_token`.
Typed path parameters are deferred under `ROUTE-001`, so the seed uses a static
path.

Authentication is required when the `auth: public explicitly` item is absent.
The compiler records the inherited authentication rule even before an
authentication runtime exists.

Every route item uses `:` between its name and value. Space-only forms such as
`input CreateOrder` are syntax errors rather than alternate spellings.

A core route has at most one input, output, and run item. Their textual order is
not semantic; the formatter will eventually choose the canonical order shown by
the seed.

Reachable action failures are derived through `run`. Routes do not contain
numeric status mappings.

## 13. Core prelude

The compiler initially recognises these representation or standard semantic
types without user declarations:

```text
Bool
Int
Decimal
Text
Bytes
Uuid
```

It also recognises the initial standard failure kinds from the
[failure model](failure-model.md). Application declarations cannot shadow
prelude names.

Prelude types still obey semantic rules. For example, `Text` cannot be used as
an ordinary application callable parameter merely because it is in the prelude.

## 14. P10.5 module extension

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

## 15. Explicitly unsupported in the first slice

The parser or semantic checker produces a stable unsupported-feature diagnostic
for:

- import aliases, re-exports, relative imports, multi-file modules, same-name
  declarations in separate modules, and external packages;
- enums and `match`;
- loops and recursion;
- arbitrary assignment;
- `attempt` and local failure mapping;
- compound many-result predicates and general patch-condition expressions;
- services, events, and jobs;
- policies and `require`;
- path/query/header route bindings;
- defaults and compound non-unique indexes;
- relationship paths deeper than two, nested to-many paths without per-hop
  bounds, and required inverse-one declarations without an enforceable totality
  model or explicit domain absence path;
- generic user-defined types;
- transparent aliases;
- macros, reflection, and escape hatches.

Enums remain outside this grammar slice. Their future grammar must preserve the
accepted reference rule: variants use qualified dot syntax such as
`InviteStatus.blocked`; `InviteStatus("blocked")` and dynamic call-style enum
construction are invalid.

Unsupported does not mean rejected forever. It means the feature cannot enter
the implementation without a specification and fixture.

## 15. Seed coverage

The [Jadpo seed](../examples/jadpo-seed/app.jadpo) exercises:

- constrained nominal types;
- an entity and field type references;
- input and output record declarations;
- field refinement chains;
- a standard-kind domain failure with internal context;
- an action with `fails`, `if`, validated construction, `reject`, and `return`;
- an explicitly public static route;
- automatic failure-to-boundary metadata.

Every token in that file is covered by this grammar. Its semantic expectations
are recorded separately so parser success cannot be mistaken for compiler
correctness.
