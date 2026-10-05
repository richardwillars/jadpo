# Jadpo renderer fixtures

These fenced examples exercise the `jadpo` identifier in documentation and chat
renderers. The final example is deliberately incomplete so renderers can be
checked on malformed but readable source as well as valid-looking syntax.

## Declarations and strings

```jadpo
module rendering.examples

// Keep the user-supplied display name unchanged.
public type DisplayName = Text { min_length: 1, max_length: 80 }
public type Welcome = Object { message: Text }
```

## Constraints and literals

```jadpo
type RetryCount = Int { min: 0, max: 12 }
type ContactEmail = Email { format: "email" }
```

## Persistence and references

```jadpo
persist Todo {
    identity: id
    id: Uuid
    owner_id: User.id references User.id as owner on_delete restrict
    title: Text
}
```

## Failures and callables

```jadpo
failure TodoMissing {
    kind: NotFound
    public { message: Text }
}

action find_todo(id: Uuid) -> Todo fails TodoMissing {
    reject TodoMissing { message: "No matching todo" }
}
```

## Routes and comments

```jadpo
route GET /todos/{todo_id} {
    auth: none
    path: { todo_id: Uuid }
    output: Todo
    run: get_todo
}
```

## Malformed source stays readable

```jadpo
route GET /unfinished {
    auth: none
    output:
```
