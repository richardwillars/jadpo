# Historical todo application sketch

**Status:** historical design evidence, not current canonical syntax  
**Source:** initial design conversation

This file preserves the first concrete application and generated-artifact ideas
from the conversation. It deliberately retains the earlier syntax so later
work can see what was being tested. The current language direction is defined by
the [syntax draft](../docs/syntax.md) and
[decision register](../docs/decision-register.md).

The repeated `auth required` lines below are preserved historical syntax. They
conflict with the accepted secure-default rule and must not be copied into the
golden application. In current canonical source, an ordinary route has no auth
line because authenticated access is inherited; only a public route carries an
explicit override.

## Application sketch

```text
app TodoApp

entity User {
    id        id
    email     email unique
    created   datetime default now
}

entity Todo {
    id        id
    owner     User
    title     text min 1 max 200
    done      bool default false
    due       datetime?
    created   datetime default now
}

route GET /todos {
    auth required

    query Todo
        where owner = current_user
        order created desc

    output Todo[]
}

route POST /todos {
    auth required

    input {
        title text min 1 max 200
        due   datetime?
    }

    create Todo {
        owner = current_user
        title = input.title
        due   = input.due
    }

    output Todo
}

route PATCH /todos/:id {
    auth required

    input {
        title text min 1 max 200 optional
        done  bool optional
        due   datetime? optional
    }

    todo = Todo.require(id = params.id)

    require todo.owner = current_user

    update todo from input

    output Todo
}

route DELETE /todos/:id {
    auth required

    todo = Todo.require(id = params.id)

    require todo.owner = current_user

    delete todo

    output none
}
```

## Behaviour implied by the sketch

The conversation observed that these declarations should already determine:

- Postgres schema;
- migrations;
- request validation;
- response validation;
- authentication;
- authorisation;
- 404 behaviour;
- malformed-ID behaviour;
- database serialization;
- OpenAPI;
- generated API documentation;
- route inventory;
- security audit.

## Co-located tests

```text
route DELETE /todos/:id {
    ...

    test "owner can delete" {
        as alice
        todo = fixture Todo(owner = alice)

        DELETE /todos/{todo.id}

        expect status 204
        expect todo deleted
    }

    test "cannot delete another user's todo" {
        as alice
        todo = fixture Todo(owner = bob)

        DELETE /todos/{todo.id}

        expect status 403
        expect todo exists
    }
}
```

## Tests the compiler could generate

From authentication, ownership, entity lookup, and the route contract, the
compiler could derive:

```text
unauthenticated request rejected
owner permitted
non-owner rejected
nonexistent Todo returns 404
malformed id rejected before database access
```

The conversation questioned whether authors should write basic tests the
compiler can already infer. The current answer is to generate structural tests
and reserve authored tests for business intent.

## Human-controlled policy sketch

```text
policy {
    Todo {
        read   owner
        create authenticated
        update owner
        delete owner
    }
}
```

If implementation became more permissive, the proposed failure was:

```text
POLICY VIOLATION

DELETE /todos/:id

Expected:
  Todo.delete = owner

Implementation permits:
  authenticated users

Build blocked.
```

## Generated audit sketch

```text
TodoApp security audit

GET /todos
  Auth        required
  Ownership   current user
  Input       validated
  Output      validated
  Rate limit  default
  Raw SQL     none
  Secrets     none

POST /todos
  Auth        required
  Ownership   forced owner=current_user
  Input       validated
  Output      validated

PATCH /todos/:id
  Auth        required
  Ownership   checked
  Input       validated
  Output      validated

DELETE /todos/:id
  Auth        required
  Ownership   checked
  Input       validated
  Output      validated

4 routes
0 warnings
0 policy violations
0 escape hatches
```

## Background job sketch

```text
job overdue_reminders every 15m {
    todos = Todo
        where done = false
        and due < now
        and reminder_sent = false

    for todo in todos {
        emit TodoOverdue(todo)
    }
}
```

This was added specifically to move the test beyond request → database →
response CRUD.

## What this sketch exposed

Even this small example requires decisions about:

- query cardinality and `Todo.require` replacement;
- ownership and policy proof;
- optional values versus omitted patch fields;
- update-from-input semantics;
- generated versus authored tests;
- job delivery and idempotency;
- email/external-service contracts;
- configuration and secrets;
- migrations and deletion lifecycle;
- route-local behaviour versus reusable actions;
- output `none` and HTTP status semantics.

Those gaps are design output, not flaws to hide. The future golden todo
application should resolve them using current syntax and record each decision.
