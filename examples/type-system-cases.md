# Type-system acceptance cases

**Status:** normative design examples; surface spelling remains provisional  
**Purpose:** give a future compiler executable-style positive and negative tests

Each case states whether it must compile. `Type(value)` is the accepted
validated-constructor spelling. Other surface syntax and the exact
failure-mapping grammar may change without changing the expected result.

## Shared declarations

```text
type Email = Text {
    format: email
    max_length: 254
}

type Username = Text {
    min_length: 3
    max_length: 30
    pattern: "[a-z0-9_]+"
}

type Quantity = Int {
    min: 1
    max: 1000
}

entity Customer {
    id: Uuid
    email: Email
    username: Username
    referrer_id: Customer.id?
}

entity Supplier {
    id: Uuid
    email: Email
}

entity Order {
    id: Uuid
    customer_id: Customer.id
    quantity: Quantity
}

input RegisterCustomer {
    email: Customer.email
    username: Customer.username
    referrer_id: Customer.id? optional
}
```

The declarations imply these refinement edges:

```text
Customer.id                 <: Uuid
Customer.email              <: Email
Customer.username           <: Username
Supplier.id                 <: Uuid
Supplier.email              <: Email
Order.id                    <: Uuid
Order.customer_id           <: Customer.id
Order.quantity              <: Quantity
RegisterCustomer.email      <: Customer.email
RegisterCustomer.username   <: Customer.username
```

The edge `Customer.id <: Uuid` does not imply that a `Uuid` can become a
`Customer.id`, or that another UUID-backed field can substitute for it.

## 1. Exact field type

```text
function load_customer(id: Customer.id) -> Customer

var customer = load_customer(order.customer_id)
```

**Expected:** compiles. `Order.customer_id` refines `Customer.id` and widens to
the requested parameter type.

## 2. Wrong identifier domain

```text
function load_customer(id: Customer.id) -> Customer

var customer = load_customer(order.id)
```

**Expected:** compile error. `Order.id` and `Customer.id` are sibling refinements
of `Uuid`; the shared representation does not make them interchangeable.

## 3. Raw identifier

```text
var raw: Uuid = external_value
var customer = load_customer(raw)
```

**Expected:** compile error. A base type cannot implicitly narrow to a field
type. The code must explicitly validate or construct `Customer.id`.

## 4. Shared semantic email

```text
function send_email(to: Email) -> DeliveryReceipt

send_email(customer.email)
send_email(supplier.email)
```

**Expected:** both calls compile. Both field types widen to the intentionally
shared `Email` type.

## 5. Specific email field

```text
function send_customer_receipt(to: Customer.email) -> DeliveryReceipt

send_customer_receipt(customer.email)
send_customer_receipt(supplier.email)
```

**Expected:** the first call compiles. The second is a compile error because a
sibling field type cannot substitute for `Customer.email`.

## 6. Raw text is not email

```text
var text: Text = "person@example.com"
send_email(text)
```

**Expected:** compile error even though the contents happen to look like an
email address. Runtime contents do not change a variable's static type.

## 7. Checked literal construction

```text
var support = Email("support@example.com")
var broken = Email("not an email")
```

**Expected:** the first declaration compiles. The second fails during
compilation because the literal violates `Email`.

## 8. Explicit dynamic validation

```text
var raw: Text = external_value
var email = Email(raw)

send_email(email)
```

**Expected:** compiles only when the containing boundary or callable handles the
typed validation failure. Successful validation produces `Email`; failure must
not produce `none`, an ambient exception, or an invalid value.

## 9. Explicit field narrowing

```text
var email: Email = validated_email
var customer_email = Customer.email(email)

send_customer_receipt(customer_email)
```

**Expected:** compiles with explicit validation failure handling. Even if
`Customer.email` currently adds no constraint beyond `Email`, the narrowing
must remain visible so a future field constraint cannot change old code
silently.

## 10. Input boundary construction

```text
route POST /customers {
    input: RegisterCustomer
    output: Customer
    run: register_customer(input)
}

action register_customer(input: RegisterCustomer) -> Customer {
    return attempt create Customer {
        email: input.email
        username: input.username
        referrer_id: input.referrer_id
    }
}
```

**Expected:** compiles. The route boundary validates wire values into
`RegisterCustomer` field types. Those types widen along their declared edges to
the corresponding `Customer` field types.

Authentication is not repeated in the route because required authentication is
inherited by default.

## 11. Validation is not existence

```text
input GetCustomer {
    id: Customer.id
}

action get_customer(input: GetCustomer) -> Customer
    fails CustomerNotFound, NotPermitted
{
    var customer = attempt query required Customer {
        where: id == input.id
        missing: CustomerNotFound
    }

    require customer accessible_by current_user
    return customer
}
```

**Expected:** compiles. Boundary validation proves that `input.id` is a valid
customer identifier value. The query proves existence; policy evaluation proves
access. None of those facts substitutes for another.

## 12. Nullable field widening

```text
var referrer: Customer.id? = customer.referrer_id
var raw_id: Uuid? = customer.referrer_id
```

**Expected:** the first declaration compiles. The second is a compile error
because semantic identifiers do not implicitly unwrap to their representation,
even through `?`.

## 13. Optional shape is not nullable value

```text
input PatchCustomer {
    email: Customer.email optional
}
```

**Expected:** the field may be omitted. When present, its value is a refinement
of `Customer.email`; it is not `Customer.email?` and cannot contain `none`.

```text
input PatchCustomer {
    email: Customer.email? optional
}
```

**Expected:** the field may be omitted; when present, it may contain
`Customer.email` or `none`. Omission and `none` remain distinct.

## 14. Collections preserve identity

```text
function notify_customers(recipients: List<Customer.email>)
function notify_any(recipients: List<Email>)

var customers: List<Customer.email> = customer_emails

notify_customers(customers)
notify_any(customers)
```

**Expected under the initial invariant-collection rule:** the first call
compiles and the second is a compile error. Use an explicit `map` to widen every
element to `Email`. This expectation may change only if immutable collections
gain sound covariance.

## 15. Named records are not structurally interchangeable

```text
value PostalAddress {
    line_1: Text
    postcode: Text
}

value WarehouseAddress {
    line_1: Text
    postcode: Text
}

function ship_to(address: PostalAddress)

ship_to(warehouse.address)
```

**Expected:** compile error if `warehouse.address` has type `WarehouseAddress`.
Matching field names and representations do not establish semantic identity.
An explicit mapping or shared named type is required.

## 16. Primitive callable parameters

```text
function find_customer(email: Text) -> Customer?
```

**Expected under the provisional application-signature restriction:** compile
error. The parameter must be `Email`, `Customer.email`, or another deliberate
semantic type.

```text
function find_customer(email: Email) -> Customer?
```

**Expected:** compiles, subject to the separate decision about whether pure
functions may query persistence.

## 17. Primitive boolean flags

```text
action set_subscription(active: Bool)
```

**Expected under the provisional application-signature restriction:** compile
error. Prefer an enum with domain meaning or separate activate/deactivate
actions unless the boolean is itself wrapped in a deliberate semantic type.

## 18. Cross-field invariants do not project

```text
value DateRange {
    starts_at: DateTime
    ends_at: DateTime

    require starts_at < ends_at
}

function inspect_start(value: DateRange.starts_at)
```

**Expected:** `DateRange.starts_at` carries the field's intrinsic value
constraints. It does not prove that an `ends_at` exists alongside it or that a
complete `DateRange` invariant has been checked.

## 19. Database properties do not become value types

```text
entity Customer {
    email: Email unique
}

input RegisterCustomer {
    email: Customer.email
}
```

**Expected:** input validation checks the email representation and intrinsic
constraints. It does not claim that the email is unique. Uniqueness is checked
transactionally when the entity is created or updated.

## 20. Authorisation does not follow a field reference

```text
function audit_email(email: Customer.email)
```

**Expected:** the type establishes value validity only. Possessing or naming
the type does not grant permission to read a customer's email. Field-level
access remains a policy question checked where data flows are established.

## 21. Provider data crosses a trust boundary

```text
service MailingProvider {
    operation lookup_recipient(...) -> ProviderRecipient
}

var response = MailingProvider.lookup_recipient(...)
send_email(response.email)
```

**Expected:** the call compiles only if the reviewed provider contract declares
`response.email` as refining `Email` and the runtime validates the provider
response. A provider-supplied raw `Text` cannot be passed directly.

## 22. No unchecked cast

```text
var email = raw_text as Email
```

**Expected:** compile error. Ordinary source has no unchecked nominal cast.
Use explicit validation and handle its typed failure.

## 23. Projection retains source field identity

```text
var recipients = attempt query Customer {
    select id, email
}

for recipient in recipients {
    send_customer_receipt(recipient.email)
}
```

**Expected:** compiles. The projection is not a `Customer`, but its selected
fields retain types `Customer.id` and `Customer.email`.

## 24. Refinement chains remain visible

```text
function accepts_customer_email(value: Customer.email)
function accepts_email(value: Email)

accepts_customer_email(input.email)
accepts_email(input.email)
```

**Expected:** both compile because:

```text
RegisterCustomer.email <: Customer.email <: Email
```

The compiler should record the chosen widening path in diagnostics and semantic
metadata rather than treating the types as structurally equal.

## 25. Nested structured fields use their declared record contract

```text
type PostalCode = Text {
    min_length: 3
    max_length: 12
}

value Address {
    postal_code: PostalCode
}

entity Customer {
    billing_address: Address
    shipping_address: Address
}

function accept_postal_code(postal_code: Address.postal_code) -> Address.postal_code {
    return postal_code
}

function billing_postal_code(customer: Customer) -> Address.postal_code {
    return accept_postal_code(customer.billing_address.postal_code)
}
```

**Expected:** compiles. `Customer.billing_address` is a nominal refinement of
`Address`, but selecting through it uses the declared `Address` record shape.
The selected value therefore has type `Address.postal_code`, not a synthetic
`Customer.billing_address.postal_code`. Reusing `Address` for shipping shares
that nested contract; distinct nested identities require distinct named value
types. Selection through an `Address?` requires handling `none` first.

## Compiler-test format to derive later

Once a parser exists, these cases should move into machine-readable fixtures
with:

- source input;
- expected success or diagnostic code;
- expected inferred type at selected expressions;
- expected widening/refinement path;
- expected boundary validators;
- expected absence of unintended conversions;
- stable source spans and semantic node IDs.

The fixtures should test semantics directly before testing generated
TypeScript, because TypeScript's structural type checker is not the authority
for this language.
