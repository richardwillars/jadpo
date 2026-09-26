# Generated artifact contract

**Status:** accepted for the P8/P9 prototype  
**Schema version:** 1

`jadpo artifacts <project>` checks the complete frontend before writing
anything. A valid project produces exactly these compiler-owned files:

```text
build/
  app.meta.json
  inventory/
    routes.json
    callables.json
  audit/
    failures.json
  validators/
    plan.json
  compatibility/
    public-failure-codes.json
  openapi/
    openapi.json
```

`build/` is disposable output. Authored source, tests, configuration, secrets,
and deployment material never belong beneath it. Source discovery does not
traverse it, even if a target generator accidentally creates a file ending in
`.jadpo`.

## File responsibilities

- `app.meta.json` is the checked semantic manifest: stable nodes and source
  ranges, refinement and call edges, inferred expression types, failure
  contracts, and derived route failures. Source paths are project-relative so
  absolute and relative invocations produce identical bytes.
- `inventory/routes.json` records method, path, explicit/default authentication,
  boundary types, invoked callable, and derived public failures.
- `inventory/callables.json` records callable kind, nominal parameters, output,
  and declared closed failure set.
- `audit/failures.json` makes public versus internal disclosure visible and
  states that internal context cannot reach the client.
- `validators/plan.json` records named constraints, closed record shapes,
  optionality/nullability, and route boundary validation obligations. It is a
  plan for P9 generation, not executable validation code.
- `compatibility/public-failure-codes.json` is the current public failure
  contract snapshot and names the fields whose change is breaking. Until a
  versioned baseline is configured, its status is
  `baseline_not_configured`; it does not pretend to have performed a diff.
- `openapi/openapi.json` is the OpenAPI 3.1 subset derivable from the current
  grammar: request and success schemas, closed record shapes, scalar
  constraints, and reachable failure responses. It deliberately omits features
  not yet expressible in source.

Every JSON file ends with one newline. Arrays and object members derived from
semantic sets use canonical ordering. Repeating the command with the same
compiler and authored inputs is byte-identical. Output contains no timestamp,
machine path, random identifier, or environment-specific value.

The compatibility snapshot is not a stored baseline. A later versioning design
must put reviewed baselines outside `build/`, because disposable output cannot
be authoritative input to its own next generation.
