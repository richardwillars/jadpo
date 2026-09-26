# Failure-model acceptance cases

**Status:** normative design examples; unresolved handler arms remain illustrative
**Purpose:** define failure programs and boundary behaviours that a future
compiler and runtime must accept or reject

## Shared declarations

```text
failure CustomerNotFound {
    kind: NotFound
    code: "customer_not_found"
    message: "Customer not found."

    internal {
        customer_id: Customer.id
    }
}

failure CustomerNotVisible {
    kind: NotVisible
    code: "customer_not_found"
    message: "Customer not found."

    internal {
        customer_id: Customer.id
        policy_reason: PolicyReason
    }
}

failure EmailAlreadyUsed {
    kind: Conflict
    code: "email_already_used"
    message: "That email cannot be used."

    internal {
        email: Email
    }
}

failure QuantityUnavailable {
    kind: Conflict
    code: "quantity_unavailable"
    message: "The requested quantity is unavailable."

    public {
        available: Quantity
    }

    internal {
        product_id: Product.id
        requested: Quantity
    }
}

failure PaymentDeclined {
    kind: Rejected
    code: "payment_declined"
    message: "The payment was declined."
}

failure CheckoutUnavailable {
    kind: Unavailable
    code: "checkout_unavailable"
    message: "Checkout is temporarily unavailable."
}
```

`CustomerNotFound` and `CustomerNotVisible` deliberately share a public code and
shape so an unauthorised caller cannot distinguish concealment from absence.
This is a reviewed compatible alias, not a general permission for duplicate
codes.

## 1. Declared rejection

```text
action load_customer(id: Customer.id) -> Customer
    fails CustomerNotFound
{
    var customer = attempt query optional Customer {
        where: id == id
    }

    if customer == none {
        reject CustomerNotFound {
            customer_id: id
        }
    }

    return customer
}
```

**Expected:** compiles. The rejection is declared and all required diagnostic
context is supplied.

## 2. Undeclared rejection

```text
action load_customer(id: Customer.id) -> Customer {
    reject CustomerNotFound { customer_id: id }
}
```

**Expected:** compile error. `CustomerNotFound` must be listed in `fails`,
handled, or mapped.

## 3. Arbitrary string rejection

```text
reject "customer not found"
```

**Expected:** compile error. `reject` accepts only a declared failure type with a
standard kind.

## 4. Arbitrary exception

```text
throw Error("customer not found")
```

**Expected:** compile error. Application source has no arbitrary exception path
for expected outcomes.

## 5. Failure without a standard kind

```text
failure CustomerMissing {
    code: "customer_missing"
}
```

**Expected:** compile error. Every application failure derives from exactly one
standard kind.

## 6. Automatic HTTP mapping

```text
route GET /customers/{id} {
    path: { id: Customer.id }
    output: CustomerOutput
    run: load_customer(path.id)
}
```

**Expected:** compiles. `CustomerNotFound` of kind `NotFound` automatically generates a
404 response and public error schema. The route does not repeat the status.

## 7. Manual status mapping

```text
route GET /customers/{id} {
    CustomerNotFound => 404
}
```

**Expected:** compile error in ordinary route syntax. Status comes from the
standard kind, not a local integer mapping.

## 8. Safe default client response

Given `CustomerNotFound`, the HTTP body is equivalent to:

```json
{
  "error": {
    "code": "customer_not_found",
    "message": "Customer not found.",
    "request_id": "req_28a91"
  }
}
```

**Expected:** `customer_id`, source paths, stacks, query details, and database
information are absent.

## 9. Explicit public detail

```text
reject QuantityUnavailable {
    available: product.available_quantity
    product_id: product.id
    requested: input.quantity
}
```

**Expected:** compiles if policy permits disclosure of `available`. The client
receives it under `error.details`; internal fields remain absent.

## 10. Missing failure context

```text
reject QuantityUnavailable {
    available: product.available_quantity
}
```

**Expected:** compile error because required `product_id` and `requested`
internal fields are missing.

## 11. Extra failure context

```text
reject CustomerNotFound {
    customer_id: id
    sql: raw_sql
}
```

**Expected:** compile error because `sql` is not declared in the diagnostic
schema.

## 12. Secret in public payload

```text
failure ProviderFailed {
    kind: Unavailable
    code: "provider_failed"

    public {
        api_key: Secret<Text>
    }
}
```

**Expected:** compile error. Secret types cannot appear in public failure data.

## 13. Internal does not mean plaintext logging

```text
failure EmailAlreadyUsed {
    kind: Conflict
    code: "email_already_used"

    internal {
        email: Email
    }
}
```

**Expected:** compiles. The runtime applies the configured redaction policy to
`Email`; declaration as internal does not bypass data handling policy.

## 14. Failure propagation

```text
action create_order(input: CreateOrder) -> Order
    fails CustomerNotFound
{
    var customer = attempt load_customer(input.customer_id)
    return attempt create_order_for(customer, input)
}
```

**Expected:** compiles. The semantic trace records the rejection site in
`load_customer` and propagation through `create_order`.

## 15. Missing propagated failure

```text
action create_order(input: CreateOrder) -> Order {
    var customer = attempt load_customer(input.customer_id)
    return attempt create_order_for(customer, input)
}
```

**Expected:** compile error because `load_customer` can reject
`CustomerNotFound` and the caller neither declares nor handles it.

## 16. Exhaustive mapping

```text
action checkout(order: Order) -> Payment
    fails PaymentDeclined, CheckoutUnavailable
{
    return attempt PaymentProvider.charge(order) {
        CardDeclined => reject PaymentDeclined
        ProviderUnavailable => reject CheckoutUnavailable
    }
}
```

The handler-arm spelling above is illustrative until its exact grammar is
accepted; the exhaustive mapping semantics are already fixed.

**Expected:** compiles when the provider operation declares exactly the handled
failure set. A newly added provider failure makes the mapping non-exhaustive and
breaks compilation.

## 17. Provider message leakage

```text
failure PaymentFailed {
    kind: Rejected
    code: "payment_failed"

    public {
        provider_message: Text
    }
}
```

**Expected:** compile error unless a reviewed provider contract supplies a safe
transformation and output policy explicitly permits it. Provider text is
internal by default.

## 18. Provider timeout

```text
var charge = attempt PaymentProvider.charge(order)
```

Assume the operation times out.

**Expected:** the service adapter creates a normalised `TimedOut` operational
fault, retaining the provider cause internally. It never exposes the provider
body or generated stack. A reviewed contract may instead map the condition to
declared `CheckoutUnavailable` when callers need that business outcome.

## 19. Known uniqueness conflict

```text
action register_customer(input: RegisterCustomer) -> Customer
    fails EmailAlreadyUsed
{
    return attempt create Customer {
        email: input.email
    }
}
```

**Expected:** compiles only when schema metadata maps the relevant customer
email uniqueness constraint to `EmailAlreadyUsed`. Raw constraint names and
SQLSTATE values remain internal.

## 20. Ambiguous database conflict

Assume the create operation can violate several unrelated unique constraints
without declared mappings.

**Expected:** compilation fails or requires explicit mappings. The runtime must
not guess which domain failure a driver error means.

## 21. Concealed resource

```text
action get_customer(id: Customer.id) -> Customer
    fails CustomerNotVisible
{
    var customer = attempt query visible Customer {
        where: id == id
    }

    if customer == none {
        reject CustomerNotVisible {
            customer_id: id
            policy_reason: current_policy_reason
        }
    }

    return customer
}
```

**Expected:** public 404 using code `customer_not_found`. Internal telemetry can
distinguish missing from policy-concealed without revealing the distinction to
the caller.

## 22. Expected failure has no physical stack

Trigger `CustomerNotFound` through an HTTP route.

**Expected:** client receives the safe 404 envelope. Internal telemetry contains
the failure declaration, rejection source node, and semantic propagation trace.
No native stack is captured by default.

## 23. Operational fault retains cause

Trigger database unavailability during the same route.

**Expected:** client receives a generic unavailable or internal response under
runtime policy. Authorised telemetry retains the driver cause chain, low-level
stack, semantic operation, request ID, and redacted context.

## 24. Defect containment

Assume generated target code throws an unclassified exception.

**Expected:** the runtime containment boundary returns generic
`internal_error`, attaches a request ID, records a compiler/runtime invariant
breach and full stack internally, and never asks application code to catch it.

## 25. Invalid literal

```text
var email = Email("invalid")
```

**Expected:** compile error. No runtime failure response exists because constant
construction is validated during compilation.

## 26. Dynamic validation

```text
failure InvalidContactEmail {
    kind: InvalidValue
    code: "invalid_contact_email"
    message: "Enter a valid contact email."
}

var email = attempt Email(raw_text)
```

**Expected:** dynamic construction creates a typed validation-failure
obligation. The containing callable must map it to `InvalidContactEmail`, allow
a standard boundary validation failure, or handle it. It cannot silently
produce `none`.

## 27. Not-found versus forbidden

Use `NotPermitted` when the client is allowed to know the resource exists but
may not perform an action. Use `NotVisible` when acknowledging existence would
leak information.

**Expected:** both choices are visible in policy and generated audit. The
compiler can require `NotVisible` for tenant-scoped lookups where policy demands
concealment.

## 28. Stable public code

Rename source declaration `EmailAlreadyUsed` to `CustomerEmailConflict` while
retaining code `email_already_used`.

**Expected:** no client contract change. Changing the code itself produces an
API compatibility warning or error according to versioning policy.

## 29. Duplicate incompatible code

Declare two publicly reachable failures with code `payment_failed` but different
statuses or public schemas.

**Expected:** compile error. A shared code is allowed only through an explicitly
reviewed compatible alias such as concealed and genuine not-found.

## 30. Job retry semantics

```text
job send_receipt(order: Order)
    fails CustomerNotFound
{
    // ...
}
```

**Expected:** `CustomerNotFound` is non-retryable by default. `RateLimited`,
`Unavailable`, and `TimedOut` follow the job's bounded retry policy. Behaviour
is based on kind, never message text.

## 31. Failure documentation

For every route, generated documentation lists reachable public codes, statuses,
messages, detail schemas, headers, and originating operations.

**Expected:** generated documentation comes from the semantic failure graph. A
handwritten route list cannot omit a transitively reachable domain failure.

## 32. No stack disclosure

Attempt to add a stack, source path, SQL message, or provider body to a public
failure schema.

**Expected:** compile error under the fixed public-envelope rules, regardless of
whether the source value is typed as ordinary `Text`.

## Compiler-test format to derive later

Once executable tooling exists, each case should become a fixture containing:

- source input;
- expected success or diagnostic code;
- expected reachable failure set;
- expected standard kind and boundary mapping;
- expected public envelope and headers;
- expected internal structured event and redaction plan;
- whether a semantic trace or low-level stack is present;
- expected retry/dead-letter classification;
- expected generated OpenAPI and audit fragments.

Tests must assert both intended disclosure and intended non-disclosure.
