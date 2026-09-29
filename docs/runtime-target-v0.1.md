# Generated Bun target v0.1

**Status:** accepted for the P9 jadpo-seed slice  
**Target:** TypeScript executed by Bun 1.2 or later

`jadpo build <project>` first runs the complete syntax, semantic, nominal
type, failure, and effect checks. It then refreshes the seven P8 artifacts and
writes one disposable executable target:

```text
build/
  target/
    app.ts
```

The generated file has no package dependency or framework configuration. It
exports a Fetch-compatible `handleRequest(request)` function and starts
`Bun.serve` when executed as the main module.

## Closed runtime dependency contract

A generated application requires a Bun executable and nothing from a package
registry. It must never require `bun install`, `npm install`, or an equivalent
step.

Generated TypeScript may import only:

- `bun` and `bun:*` built-ins supplied by the Bun runtime; and
- compiler-owned relative TypeScript modules emitted in the same `build/`
  target set.

The generator rejects any bare third-party module with
`JADPO_TARGET_EXTERNAL_MODULE`. It also rejects generated `package.json`,
`bun.lock`, `bun.lockb`, or `node_modules` artifacts with
`JADPO_TARGET_DEPENDENCY_MANIFEST`. Authored source has no import or package
escape hatch that can bypass this check.

CI and acceptance commands use Bun's `--no-install` option. This matters because
Bun can otherwise auto-install an unresolved bare package during execution;
the disabled mode proves the target is closed over Bun's runtime and its own
generated files.

Applications use an explicit empty environment file, including the
compiler-owned `test` and `dev` launches. The pinned Bun 1.2.20 ignores the
unsupported `--no-env-file` flag and still discovers dotenv files. A production
launcher must retain both package and environment-loading boundaries:

```text
bun --no-install --env-file=/dev/null build/target/app.ts
```

The generated application reads only declared binding names from `Bun.env`,
decodes one complete typed snapshot, and exits before `Bun.serve` when any
required value is missing or invalid. Cloud-specific launch generation remains
outside this target contract; dropping the explicit empty-file argument is not
a supported production launch. Windows launches use `--env-file=NUL`.

P11 adds one narrow capability exception without changing this P9 seed
evidence: when JWT bearer validation is explicitly declared, the compiler may
add its exact pinned `jose` dependency plus a compiler-owned frozen
lock/integrity record. Applications without that declaration retain this closed
dependency-free contract. Authors cannot select, replace, or import the package,
and Bun auto-install remains disabled in both modes. See the
[AUTH-001 implementation plan](authentication-plan.md).

## Runtime boundaries

The v0.1 generator implements the already-checked core constructs needed by the
seed: semantic scalar types and constraints, closed input/output records,
actions and functions, conditionals, validated construction, returns, typed
rejection, and explicit public routes.

At the HTTP boundary it:

- generates a fresh `req_<uuid>` request ID and returns it in the body and
  `x-request-id` response header;
- parses JSON and validates the exact closed input shape before invoking the
  action;
- applies the same email, length, pattern, primitive, and record constraints as
  the compiler slice;
- validates and reconstructs the declared output, thereby serialising only its
  declared fields;
- maps declared domain failures through compiler-derived status, code, message,
  and public-field allowlists;
- never copies internal failure context into the response;
- converts malformed or constraint-invalid input to the generic 400
  `invalid_request` envelope; and
- contains output-contract failures, unknown generated-runtime failures, and
  other defects behind the generic 500 `internal_fault` envelope.

Input validation is handled locally inside the route branch. A validation error
after action invocation is therefore an internal contract defect, not a client
400. This distinction prevents generated or application defects from being
misclassified as caller mistakes.

Authentication is deliberately not stubbed. A route without the exact opt-out
`auth: none` produces `JADPO_TARGET_AUTH_NOT_IMPLEMENTED` until P11 provides
the required-default authentication runtime.

## Executable evidence

The [Bun acceptance suite](../tests/runtime/jadpo-seed.test.ts) starts a real
TCP listener on a bounded temporary localhost port and sends Fetch requests
through the generated handler. It proves:

- a valid registration returns the exact declared output;
- malformed email, constrained invite code, malformed JSON, and unknown fields
  all fail before action execution with a safe 400 response;
- a percent-decoded `Customer.email` path binding is validated before an inline
  route action, and an invalid segment receives the same safe 400 envelope;
- the exact reserved invite code maps automatically to the declared 422
  failure; and
- neither the internal `invite_code` field nor its `reserved` value appears in
  the public response.

`bun build --no-install` also bundles the generated file successfully as a
dependency-free Bun entry point. Generated target source remains disposable
and is not the normal review or debugging surface; authored source, derived
audits, OpenAPI, and semantic metadata retain those roles.
