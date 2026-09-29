# CONFIG-001 configuration plan

**Status:** core implementation in progress; CONFIG-P0/P1/P3 complete, P2/P4/P6 core implemented, CONFIG-P5 and platform evidence pending

**Approved:** 2026-09-27

**Scope:** typed configuration, local secret entry, deployment bindings,
startup validation, liveness, and readiness for generated Bun services

**Out of scope:** hosted secret storage, cloud-specific deployment generation,
live configuration reload, authorization policy, and general orchestration

This contract is intentionally small. An application declares each typed value
and its deployment binding in one place. Jadpo checks the declaration, helps a
developer enter local secrets without exposing them to an LLM, and validates
real values automatically before a service starts.

## 1. Approved v0.1 contract

- Each configuration field, its type, classification, binding name, and
  optional default live together in the application source. There is no second
  Jadpo binding file to keep in sync.
- Field options use a structured body, not positional or space-separated
  modifiers. The exact grammar implemented by CONFIG-P1 follows this shape:

  ```jadpo
  config AppConfiguration {
      mailer_api_key: MailerApiKey {
          binding: "MAILER_API_KEY"
          secret: true
      }

      request_timeout: Duration {
          binding: "REQUEST_TIMEOUT"
          default: 5s
      }
  }
  ```

- A field is required unless it has a checked literal default. Secrets cannot
  have source defaults.
- Application behavior reads only typed `config.<field>` values. It cannot read
  the process environment, `.env.local`, or raw binding metadata.
- v0.1 bindings obtain values from the process environment. Secret managers and
  deployment platforms remain compatible by injecting those environment
  values; Jadpo does not need their SDKs.
- All v0.1 configuration is startup-bound. A change causes a controlled local
  restart or normal production restart/rolling deployment. There is no
  `restart` versus `reloadable` choice in the language.
- Jadpo uses no configuration-loading package. Generated Bun processes disable
  Bun's automatic `.env` discovery in production.
- Local development has exactly one conventional file: `.env.local`. Jadpo
  does not define `.env`, `.env.development`, `.env.test`, or layered overlay
  precedence. Tests receive configuration from the test harness.
- A missing or invalid required value cannot reach application code or a
  serving listener.
- Secret values cannot enter public output, logs, diagnostics, generated
  examples, manifests, health responses, or agent-readable command output.
- Liveness describes the local process. Readiness separately describes whether
  required dependencies can currently serve application traffic.

## 2. Approved decisions

| ID | v0.1 decision |
|---|---|
| CONFIG-D01 | Put the explicit environment binding beside its typed field in application source; do not generate or require a parallel deployment-binding file. |
| CONFIG-D02 | Use one local `.env.local` file and no implicit application-level environment overlays. Production receives the same binding names from its platform environment. |
| CONFIG-D03 | Treat the authored binding string as stable deployment API. A source-field rename does not silently rename it. |
| CONFIG-D04 | Make fields required by default or give them one checked literal default. Secrets cannot have defaults and an empty string is not optionality. |
| CONFIG-D05 | Make `secret` a checked information-flow classification on the typed value; only compiler-owned declared sinks may receive it. |
| CONFIG-D06 | Make every v0.1 value startup-bound. Local changes restart `dev`; production changes restart or roll the service. Defer live reload until a concrete need justifies its extra model. |
| CONFIG-D07 | Add a safe prompted `jadpo config set <field>` handoff and a value-safe `jadpo config check` for local configuration. |
| CONFIG-D08 | Keep `jadpo check` as the one authoritative static check. `build`, `test`, `watch`, and `dev` reuse it rather than introducing partial semantic checkers. |
| CONFIG-D09 | Validate real values automatically in `dev`, deployment integration, and runtime startup. Do not require developers to remember a separate preflight command. |
| CONFIG-D10 | Keep liveness local and dependency-free; make readiness bounded, secret-safe, and false while a required dependency is unavailable. |

## 3. Safe local secret handoff

The preferred LLM workflow is for the agent to name a command, not request the
secret:

```text
Run `jadpo config set mailer_api_key` in your terminal. It will prompt securely;
do not paste the value into chat.
```

Inside a project, the interaction is:

```text
$ jadpo config set mailer_api_key
Enter MAILER_API_KEY: [hidden]
✓ mailer_api_key configured locally
```

`jadpo config set` must:

- resolve the authored field and binding from the checked configuration schema;
- accept the value only from an interactive prompt, never as a command-line
  argument;
- disable terminal echo for a secret and never reproduce its value;
- decode and validate the value before writing it;
- create `.env.local` with owner-only permissions when possible;
- update it atomically, preserve unrelated entries and comments, and refuse an
  unsafe target such as a symbolic link;
- detect a concurrent file change instead of overwriting it; and
- print only the field/binding identity, status, and safe validation errors.

The user may still edit `.env.local` manually. The command is the preferred
agent handoff because it keeps a secret out of chat, shell history, process
arguments, and agent-readable output. It is a local convenience, not a claim
that `.env.local` is a production secret vault.

`jadpo config check` checks whether the current project's local values are
present, decodable, and valid. It reports only field names and
set/missing/invalid status. It is deliberately narrower than `jadpo check`, but
it does not replace static checking.

## 4. Check, development, and deployment flow

| Command or boundary | Responsibility |
|---|---|
| `jadpo check` | Full-project syntax, type, configuration declaration, binding, and secret-flow checking. It never needs real values. |
| `jadpo build` | Runs the same full check, then generates deterministic output. A build never requires local or production secrets. |
| `jadpo test` | Runs the checked build, then tests with explicit harness-provided values. |
| `jadpo watch` | Repeats the same checked build after authored input changes. |
| `jadpo dev` | Runs the checked build, validates `.env.local`, starts only with valid values, and performs a controlled restart after a valid source or local-configuration change. |
| `jadpo config check` | Quickly reports local value presence and validity without building or starting the service. |
| deployment/startup | Validates the actual platform environment automatically before promotion/readiness and repeats validation before serving. |

Agent guidance is therefore short:

1. Run `jadpo check <project> --diagnostic-format=json` after a coherent edit
   when `dev` is not already reporting the same pipeline.
2. Ask the user to run `jadpo config set <field>` for a missing local secret.
3. Use `jadpo config check` only when diagnosing local values.
4. Finish with the relevant `build` or `test`; a separate check immediately
   beforehand is redundant.

A public `jadpo check config` mode is not added. Configuration references and
secret flow cross application boundaries, so a partial semantic check could
give false confidence. If full checks become slow, the compiler may cache and
incrementally recompute the affected graph without creating a weaker command.

## 5. Runtime and deployment boundary

Bun provides `Bun.env`/`process.env` and can disable automatic environment-file
loading, so Jadpo does not need `dotenv`. The compiler generates a closed loader
that reads only declared binding names, decodes them into nominal types, and
then removes the raw map from application reach.

Production launch disables automatic `.env` discovery. The deployment platform
supplies declared environment variables directly, including values injected by
its secret manager. Generated deployment integration validates them before a
new revision is promoted when the platform supports such a hook. Runtime
startup always repeats validation and never binds a public listener with
invalid configuration.

Valid configuration with an unavailable dependency is different from invalid
configuration. The process remains live, readiness is false, required probes
retry with bounded backoff, and application traffic is not served until the
dependency recovers. Probe output contains stable dependency names and states,
not endpoints, credentials, or native driver text.

## 6. Implementation sequence

### CONFIG-P0 — Propagate the approved contract

- update the decision register, roadmap, validation rules, threat model, golden
  source, expected audit, and comparison baseline;
- freeze diagnostic and safe-status vocabulary; and
- record the dependency-free Bun and explicit empty environment-file launch boundaries.

### CONFIG-P1 — Syntax and semantic graph

- parse the structured configuration declaration and field option bodies;
- assign stable field identities and record explicit binding names;
- type-check literal defaults and prohibit secret defaults;
- resolve every `config.<field>` use; and
- reject duplicate bindings, unknown options, raw environment reads, and
  dynamic configuration lookup.

### CONFIG-P2 — Secret flow and generated schema

- propagate secret classification through values, calls, structures, and
  failures;
- allow secrets only at compiler-owned adapter sinks;
- reject serialization, interpolation, logging, diagnostics, metrics, and
  generated examples containing secrets; and
- generate deterministic, value-free configuration audit and binding schemas.

### CONFIG-P3 — Local configuration commands

- implement `jadpo config set` with hidden TTY input and safe atomic updates;
- implement value-safe `jadpo config check`;
- generate and ignore `.env.local` without adding other environment variants;
  and
- make `dev` validate and safely restart after valid local changes.

### CONFIG-P4 — Startup and deployment validation

- generate the closed Bun environment loader without a package dependency;
- disable automatic `.env` loading in generated production launch;
- validate a complete typed snapshot before adapter initialization or listen;
- emit stable secret-safe startup failures; and
- expose the same validation operation to generated deployment integration.

### CONFIG-P5 — Liveness and readiness

- generate distinct compiler-owned health contracts;
- enforce probe timeouts, concurrency bounds, and bounded retry/backoff;
- classify required and advisory dependencies;
- recover readiness without restarting after a dependency recovers; and
- prove ordinary routes cannot acquire deployment-plane details.

### CONFIG-P6 — Tooling, golden, and adversarial exit run

- add LSP hover/completion/diagnostics and agent-oriented JSON;
- add `audit/configuration.json` with declarations, classifications, bindings,
  sinks, probes, and source spans but no values;
- migrate the golden todo and comparison baseline; and
- run missing, malformed, duplicate, redaction, startup, restart, readiness,
  recovery, concurrent-write, unsafe-file, and secret-canary cases.

### Implementation record

As of 2026-09-27, the compiler parses and checks the configuration declaration,
resolves typed `config.<field>` access, tracks secret values, emits a value-free
configuration audit, and generates a closed package-free Bun loader that
validates before `Bun.serve`. The CLI implements hidden prompted
`config set`, value-safe `config check`, `.env.local` validation, declared-only
environment forwarding, and last-known-good `dev` restart behavior. The LSP,
VS Code grammar/snippet, compile fixtures, CLI integration cases, and a runnable
configuration example cover the new surface.

The remaining work is deliberately narrower: AUTH-001 and SERVICE-001 must
provide real compiler-owned secret sinks; SERVICE-001 must define dependency
probe semantics while the approved [TIME-001/TEST-001 contract](time-testing-plan.md)
now supplies monotonic deadlines, stable operation instants, deterministic
clock control, and test isolation; cloud-specific deployment hooks remain out
of scope unless separately approved; and the golden todo plus the full P6
adversarial matrix wait for those consumers. Startup validation itself is
implemented and fail-closed.

## 7. Required evidence

| Area | Required positive and adversarial evidence |
|---|---|
| Static contract | Typed required/default fields; reject unknown options, duplicate bindings, invalid/secret defaults, direct environment access, and dynamic lookup. |
| Local handoff | Hidden interactive entry and atomic preservation; reject argument values, non-interactive secret entry, unsafe targets, concurrent changes, and all value disclosure. |
| Secrets | Reach only declared adapter sinks; canaries never enter output, logs, diagnostics, metrics, manifests, health, URLs, or provider errors. |
| Build | Identical artifacts regardless of actual values; no secret access, external package, or implicit environment-file load. |
| Startup | Valid values construct one snapshot; missing or malformed values never bind a listener or expose native errors. |
| Restart | Valid local changes restart `dev`; invalid changes retain the last ready revision and visibly report stale state. |
| Readiness | Required/advisory classification, bounded checks, outage behavior, and recovery without a liveness restart loop. |
| Deployment | Invalid values block promotion/serving automatically; a healthy prior revision remains available where the platform supports rolling deployment. |

## 8. Dependencies and stop conditions

- **AUTH-001** consumes signing keys, issuer/audience values, and credential
  lifetimes through this configuration boundary.
- **SERVICE-001** consumes provider endpoints and secrets through declared
  sinks; CONFIG-001 does not authorize network egress.
- **TIME-001/TEST-001** now supplies the approved clock, monotonic-deadline,
  evidence-timestamp, deterministic-test, and fake-capability foundation.
  SERVICE-001/ASYNC-001 still own retry/backoff policy.
- **DX0.5** supplies the checked local restart and last-known-good harness that
  CONFIG-001 extends.

Return to the owner before adding a new source kind, implicit overlay, live
reload mode, general secret sink, hosted secret SDK, configuration package
dependency, or cloud-specific deployment contract. Do not weaken automatic
startup/deployment validation merely to make a deployment pass.
