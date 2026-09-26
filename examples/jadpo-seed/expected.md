# Jadpo seed expectations

**Status:** normative expectations for the first compiler slice

The source is [app.jadpo](app.jadpo). Parsing it successfully is necessary but
not sufficient; `jadpo inspect` and `jadpo check` must establish the facts
below.

## 1. Expected result

The program compiles without diagnostics under the core grammar and semantic
rules and executes through the generated P9 Bun target.

It does not yet access a database or configure a production authentication
provider. Both routes use the exact `auth: none` opt-out, so neither relies on
the future default-authentication runtime.

## 2. Declaration nodes

The semantic graph contains nodes for:

```text
Email
InviteCode
Customer
Customer.id
Customer.email
RegisterCustomer
RegisterCustomer.email
RegisterCustomer.invite_code
RegistrationAccepted
RegistrationAccepted.email
InviteCodeRejected
InviteCodeRejected.internal.invite_code
register_customer
POST /registrations
GET /registrations/{email}
```

Prelude nodes referenced by the program include:

```text
Text
Uuid
Rejected
```

## 3. Refinement edges

The compiler emits at least these edges:

```text
Email                              <: Text
InviteCode                         <: Text
Customer.id                        <: Uuid
Customer.email                     <: Email
RegisterCustomer.email             <: Customer.email
RegisterCustomer.invite_code       <: InviteCode
RegistrationAccepted.email         <: Customer.email
InviteCodeRejected.internal.invite_code <: InviteCode
```

No edge is inferred merely from matching representation or structure.

## 4. Constraint facts

```text
Email:
  compiler-owned `Email` validation
  maximum length 254

InviteCode:
  min_length: 6
  max_length: 32
  pattern: "[a-z0-9_]+"
```

`InviteCode("reserved")` is validated during compilation and bound as the
named semantic value `reserved_invite_code`. The subsequent equality compares
two `InviteCode` values. It does not use a string sentinel as an enum member.

## 5. Action facts

`register_customer`:

- accepts exactly `RegisterCustomer`;
- returns exactly `RegistrationAccepted`;
- may reject exactly `InviteCodeRejected`;
- has no persistence or external effects in the core;
- constructs the exact reserved code as a named `InviteCode` value;
- compares `RegisterCustomer.invite_code` after widening it to `InviteCode`;
- supplies all required internal failure context;
- constructs every required output field;
- widens `RegisterCustomer.email` through `Customer.email` for output
  construction.

The action has no implicit primitive parameters or result.

## 6. Failure facts

`InviteCodeRejected`:

```text
kind: Rejected
public code: invite_code_rejected
public message: That invite code cannot be used.
public fields: none
internal fields:
  invite_code: InviteCode
default HTTP status: 422
expected native stack: none
expected semantic trace: yes
```

The internal invite code never appears in the public response.

## 7. Route facts

`POST /registrations`:

- is public only because `auth: none` appears;
- validates incoming data as `RegisterCustomer`;
- invokes `register_customer`;
- validates and serialises exactly `RegistrationAccepted`;
- can return the public `invite_code_rejected` failure with status 422;
- derives the failure response through the call graph rather than a local status
  mapping;
- rejects unknown input fields under the boundary's canonical closed-shape
  policy;
- emits no internal diagnostic field to the client.

`GET /registrations/{email}`:

- binds the placeholder one-to-one to `path.email: Customer.email`;
- percent-decodes and validates the segment before authored behaviour runs;
- exposes the validated value only as `path.email`; and
- executes an inline action that returns the declared output shape.

## 8. Generated HTTP behaviour

Valid request:

```json
{
  "email": "person@example.com",
  "invite_code": "welcome_1"
}
```

Expected success body:

```json
{
  "email": "person@example.com"
}
```

Rejected request:

```json
{
  "email": "person@example.com",
  "invite_code": "reserved"
}
```

Expected failure shape:

```json
{
  "error": {
    "code": "invite_code_rejected",
    "message": "That invite code cannot be used.",
    "request_id": "<generated>"
  }
}
```

Malformed email, invalid typed path values, and constraint-invalid invite codes
fail at boundary validation before the action runs.

The executable evidence is the [jadpo-seed Bun acceptance
suite](../../tests/runtime/jadpo-seed.test.ts).

## 9. Mutations that must fail compilation

Later fixtures should derive at least these failures from the seed:

- change the valid reserved-code literal to `InviteCode("x")`;
- pass `Email` directly where `Customer.email` is required without explicit
  narrowing;
- remove `InviteCodeRejected` from the action's `fails` clause;
- omit `invite_code` from the flat rejection object;
- add the internal invite code to an undeclared public response field;
- replace the declared failure with a string or arbitrary exception;
- add a numeric route status mapping;
- remove `auth: none` and then assert the route is public.
