# First-party browser/API authentication milestone

This executable P11 example uses a signed browser cookie with a five-minute
revocation bound and an opaque API bearer credential with an authority check on
every request. `/fresh-identity` always checks the session and user authority.
The protected note routes exercise automatic ownership policy for reads and
writes. Missing authentication fails before input decoding or business work.

```sh
cargo run --manifest-path jadpo/Cargo.toml -p jadpo-cli -- build examples/first-party-authentication
bun --no-install test tests/runtime/first-party-authentication.test.ts
bash tests/runtime/first-party-authentication-postgres.sh
```

The tests use synthetic keys and isolated databases. The PostgreSQL runner
requires `initdb`, `pg_ctl`, and `createdb` on `PATH`; it starts a disposable
localhost cluster and removes it on exit, without using an existing database.
Both modes compile variants with immediate opaque-cookie authentication and
bounded signed-bearer authentication. SQLite mode ignores ambient `DATABASE_URL`.
The PostgreSQL test setting is reserved for disposable local test databases;
the suite creates additional databases for its variants. Run each runtime suite in its own process, as documented in
[the runtime test guide](../../tests/runtime/README.md).

## Configuration

Validators accept `secret`, optional `previous_secret`, `audience`, and (for
cookies) `origin`. Keys reference declared secret textual configuration; audience
and origin accept non-secret text literals or configuration. Unknown or duplicate
settings, literal keys, and secret values in audience/origin are compile errors.

Keys are 32 cryptographically random bytes encoded as **unpadded base64url**.
Use `jadpo config set signing_key` in the project directory for hidden local
entry; never put real keys in source, chat, or command-line arguments. The
example declares a second independent key to exercise rotation. A first
deployment can omit both the previous-key config field and the validators'
`previous_secret` settings. Empty or duplicate keys are invalid.

The cookie origin must be an exact HTTPS origin, without a path or trailing
slash. Cookies are host-only, `Secure`, `HttpOnly`, `SameSite=Strict`, and scoped
to `/`. Mutating requests require the exact configured `Origin` and the
session-bound `X-Jadpo-CSRF` header. Compiler-detected writes on GET routes also
require CSRF checks; a method name cannot bypass them. No CORS policy or public
login/refresh/logout route is generated.

## Credential lifecycle and integration boundary

The generated application's `initializeApplication(environment)` initializes
configuration and real adapters. `authenticationHost()` exposes **trusted host
integration** methods `issue`, `refresh`, and `revoke`. These are not callable
from authored Jadpo code or reachable over HTTP. A trusted login integration
must authenticate the human before calling `issue`; account creation, passwords,
magic links and login UI remain outside AUTH-001. The runtime independently
resolves an active user before issuing or refreshing a credential.

These internal methods consume an explicit operation timestamp in milliseconds.
`issue(strategy, subject, absoluteExpiry, operationNow)` returns the credential,
its expiry, a CSRF token, and a `Set-Cookie` value where applicable. Deliver these
only through the trusted transport; do not log their result. Signed refresh
requires a currently valid credential and live authority; it cannot extend the
absolute session expiry. An expired session needs the external login integration
again. Opaque credentials are reissued, not refreshed.

Only verifiers are stored for opaque secrets. Signed envelopes carry version,
audience, strategy namespace, session ID, subject, stable user ID and times;
kind and strength are fixed by the selected first-party validator. They never
carry roles, tenant permissions or profile data. Sessions live in the
compiler-owned `__jadpo_auth_sessions` table, unavailable to authored queries.

Revoking an opaque credential takes effect on the next request. A signed
credential may continue on ordinary routes until expiry, for at most the declared
bound; fresh routes and refresh check revocation immediately. Disabling a user
follows the same distinction. Authorization still uses the existing authoritative
policy machinery independently of credential validation.

Rotation uses a restart/rolling deployment with a new current key and the old
key as `previous_secret`; subsequent issuance/refresh uses the new key. Removing
the old key rejects its signed and opaque credentials. Opaque credentials require
reissuance before retiring their verifier key. No hot reload is implemented.

## Evidence and remaining gates

The checkpoint supports one first-party user validator per transport strategy,
a persisted unique subject authority, and an identity principal with `subject`,
`user_id` (UUID) and optionally textual `authentication_strength`. Unsupported
strategies, incomplete configuration and richer principal mappings still fail
protected target generation. Service keys/exchange and JWT remain later slices.

The generated authentication audit and OpenAPI list the configured transports,
revocation mode and protected/fresh routes. Evidence covers SQLite, real local
HTTP, expiry boundaries, rotation, revocation, CSRF, policy scoping, request
isolation, parser mutations and secret containment. The same 24-case suite passes
in SQLite and PostgreSQL modes, with all four credential/transport combinations
on each database. Separate-process restart/revocation and identity replacement
are covered. The two startup-failure subprocess cases use SQLite in both runs.
Concurrent initialization fails closed, and incompatible internal session tables
fail before the listener starts with a structured, secret-free diagnostic.

See [the retained completion evidence](../../tests/assurance/auth-first-party-2026-09-29/README.md).
This closes the scoped browser/API milestone. Long-running fuzz campaigns,
production deployment qualification, independent security review, and the other
authentication milestones remain open; this is not release-equivalent assurance.
