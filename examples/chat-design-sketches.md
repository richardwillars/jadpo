# Historical design sketches from the source conversation

**Status:** historical evidence; syntax is not canonical

These examples preserve the concrete shapes that drove the design discussion.
Later decisions replaced several spellings, especially bindings, field
assignment, entity methods, and public-route syntax. Use the
[current syntax draft](../docs/syntax.md) for new work.

## 1. Initial entity and booking action

```text
entity User {
  id: id
  email: email unique
  name: text
}

entity Booking {
  id: id
  user: User
  starts_at: datetime
  status: enum[pending, confirmed, cancelled]
}

action create_booking(user, starts_at) -> Booking {
  require authenticated
  require user == current_user

  reject if Booking.exists(
    user = user,
    starts_at overlaps requested_slot
  )

  create Booking {
    user,
    starts_at,
    status: pending
  }

  emit booking_created
}
```

This established entities, domain actions, authentication/ownership checks,
overlap-aware queries, mutations, and events in one readable block.

## 2. Route sketch

```text
route POST /bookings
  uses create_booking

route GET /bookings/:id
  returns Booking
  authorize booking.user == current_user
```

This is historical shorthand. Current work expects explicit typed input/output,
safe authentication defaults, and policy conformance.

## 3. Event and scheduled work

```text
on booking_created(booking) {
  send_email booking.user
    template booking_confirmation
    data { booking }

  schedule 24h before booking.starts_at
    remind_user(booking)
}
```

This led to explicit `event` and `job` concepts and the requirement that retries,
delivery, scheduling, and idempotency be compiler-visible.

## 4. First external-service sketch

```text
service Stripe {
  secret STRIPE_SECRET

  operation create_checkout
    POST /v1/checkout/sessions
}

action purchase(order) {
  require authenticated
  require order.user == current_user

  payment = Stripe.create_checkout {
    amount: order.total
    currency: GBP
  }

  return payment.checkout_url
}
```

This introduced first-class services, secrets, typed operations, and auditable
external effects. The later design replaced bespoke libraries with imported or
reviewed contracts and normalised provider failures at the boundary.

## 5. Data lifecycle diagnostic

The source considered an underspecified destructive action:

```text
action delete_account(user) {
  delete user
}
```

Rather than guessing, compilation could fail with domain consequences:

```text
ERROR

User owns:
- 4 Booking references
- 1 Subscription
- 17 AuditEvent records

Specify lifecycle:
  cascade
  retain
  anonymize
  reject
```

This is the basis for explicit lifecycle and migration decisions.

## 6. Structured unresolved-requirements diagnostic

```text
BUILD FAILED

3 unresolved requirements:

1. create_booking can be called anonymously.
   Choose: authenticated | public

2. Booking.user may be deleted.
   Choose lifecycle policy.

3. send_confirmation has an external effect.
   Choose retry policy:
   once | at_least_once | best_effort
```

This example crystallised the compiler-as-reasoning-loop idea: errors are
small, semantic tasks rather than incidental framework traces.

## 7. Initial primitive vocabulary

```text
entity
value
query
action
route
event
job
service
policy
test
```

Later discussion added `app`, explicit `input`/`output`, functions, types, and a
possible rule/decision concept. The principle is a very small semantic surface.

## 8. Intent-bearing rule

```text
rule cancellation_window {
  intent:
    "Customers may cancel until 48 hours before their booking."

  applies Booking

  allow cancel
    when now < starts_at - 48h
}
```

The spelling remains open. The retained requirement is that source connects
behaviour with durable rationale so future agents see both what and why.

## 9. Conspicuously public route

```text
route GET /public-status {
    public: explicitly
    rate: 60/min
}
```

The syntax is historical, but the semantic requirement is accepted: removing
authentication must be a positive, visible statement.

## 10. Human/agent/compiler workflow

The conversation described this change sequence:

```text
Human:
  Add an endpoint for customers to cancel bookings.

Agent:
  Changes the implementation.

Compiler:
  This introduces a destructive mutation.
  Authentication is inherited as required.
  Which role may cancel?
  What is the cancellation policy?
  What happens to related payment records?

Human:
  Approves policy decisions.

Compiler:
  Generates and checks implementation, tests, docs, and audit.

CI:
  Fails if approved policy and implementation disagree.
```

## 11. Runtime source mapping sketch

```text
generated TS lines 145-163
    -> source: todos.backend:5
    -> node: route.GET.todos.id.authorization
```

An underlying target failure:

```text
TypeError: Cannot read properties of undefined
  at todoHandler (/app/generated/todos.ts:157:19)
```

should become a source-language diagnostic:

```text
TodoApp runtime error

GET /todos/:id

todos.backend:5
  require todo.owner = current_user
          ^

Todo lookup produced no value.
```

The later architecture generalises this through stable semantic node IDs and
`app.meta`.

## 12. Conceptual generated files

```text
app.js
app.js.map
app.meta
```

`app.meta` represents richer semantic mappings that ordinary source maps cannot
express, including generated behaviour with no literal one-to-one source line.

## 13. Error taxonomy sketches

Source-language compile error:

```text
ERROR AUTH003

GET /todos/:id returns Todo but does not prove
that the current user may read the selected record.
```

Generated-target failure:

```text
INTERNAL COMPILER ERROR

Compiler generated invalid target code.
Reference: COMPILER-TS-4821
```

Production runtime fault:

```text
ERROR

app: TodoApp
route: GET /todos/:id
operation: Todo.require
entity: Todo
source: todos.backend:14
request: req_28a91

cause:
  database connection timed out

duration:
  5000ms
```

These examples establish that target errors are either translated application
faults or compiler bugs; developers do not fix generated code.

## 14. Syntax evolution preserved elsewhere

The conversation also iterated through:

- method-style database calls to explicit language operations;
- `.empty` to `count(items) == 0`;
- `let`/`mut` to `var`/`var mut`;
- nullable wrapper concepts to `T?`/`none`;
- JavaScript exceptions and Rust `Result` to declared domain failures;
- loose object completeness to complete typed values plus projections.

Those alternatives and conclusions are recorded in the
[decision register](../docs/decision-register.md) and
[syntax draft](../docs/syntax.md).
