import { describe, expect, test } from "bun:test";

import {
  authenticateRequest,
  AuthenticationFault,
  type AuthPrincipal,
  type AuthenticationAdapter,
  type ResolutionResult,
  type ValidationResult,
} from "../../examples/authentication-selector/build/target/authentication.ts";

const principal: AuthPrincipal = {
  kind: "user",
  subject: "user-1",
  authenticationStrength: "signed",
  values: { user_id: "00000000-0000-4000-8000-000000000001" },
};

const servicePrincipal: AuthPrincipal = {
  kind: "service",
  subject: "service-1",
  authenticationStrength: "api_key",
  values: { service_id: "00000000-0000-4000-8000-000000000002" },
};

function request(headers: Record<string, string> = {}): Request {
  return new Request("http://127.0.0.1/private", { headers });
}

function fakeAdapter(
  validation: ValidationResult,
  resolution: ResolutionResult = { kind: "active", principal },
) {
  const calls = { validate: 0, resolve: 0 };
  const adapter: AuthenticationAdapter = {
    async validate() {
      calls.validate += 1;
      return validation;
    },
    async resolve() {
      calls.resolve += 1;
      return resolution;
    },
  };
  return { adapter, calls };
}

async function faultCode(operation: Promise<unknown>): Promise<string> {
  try {
    await operation;
    throw new Error("expected authentication failure");
  } catch (error) {
    expect(error).toBeInstanceOf(AuthenticationFault);
    return (error as AuthenticationFault).code;
  }
}

describe("compiler-owned authentication selector", () => {
  test("verified external identities use one request-local authority result even on fresh routes", async () => {
    for (const freshAuthority of [false, true]) {
      const { adapter, calls } = fakeAdapter({
        kind: "authoritative", principalKind: "user", subject: principal.subject,
        resolution: { kind: "active", principal: { ...principal, authenticationStrength: "jwt" } },
      });
      expect(await authenticateRequest(request({ authorization: "Bearer a.b.c" }), { freshAuthority }, adapter))
        .toEqual({ ...principal, authenticationStrength: "jwt" });
      expect(calls).toEqual({ validate: 1, resolve: 0 });
    }
  });

  test("external authority outcomes preserve failures and reject identity substitution", async () => {
    for (const [resolution, expected] of [
      [{ kind: "inactive", failureName: "PrincipalInactive" }, "principal_inactive"],
      [{ kind: "missing" }, "invalid_credentials"],
      [{ kind: "duplicate" }, "authority_invariant"],
      [{ kind: "unavailable" }, "authentication_unavailable"],
      [{ kind: "active", principal: { ...principal, subject: "another-user" } }, "authority_invariant"],
      [{ kind: "active", principal: servicePrincipal }, "authority_invariant"],
      [{ kind: "active", principal, claims: { admin: true } }, "authority_invariant"],
    ] as const) {
      const { adapter, calls } = fakeAdapter({ kind: "authoritative", principalKind: "user", subject: principal.subject, resolution } as ValidationResult);
      expect(await faultCode(authenticateRequest(request({ authorization: "Bearer a.b.c" }), { freshAuthority: true }, adapter))).toBe(expected);
      expect(calls).toEqual({ validate: 1, resolve: 0 });
    }
  });

  test("a pre-resolved identity cannot bypass its configured validator or closed envelope", async () => {
    const base = { kind: "authoritative", principalKind: "user", subject: principal.subject, resolution: { kind: "active", principal } };
    for (const [headers, validation] of [
      [{ cookie: "selector_session=a.b.c" }, base],
      [{ authorization: "Bearer a.b.c" }, { ...base, subject: "" }],
      [{ authorization: "Bearer a.b.c" }, { ...base, claims: { admin: true } }],
    ] as const) {
      const { adapter } = fakeAdapter(validation as ValidationResult);
      expect(await faultCode(authenticateRequest(request(headers), { freshAuthority: false }, adapter))).toBe("authority_invariant");
    }
  });

  test("zero credentials fails before adapter work", async () => {
    const { adapter, calls } = fakeAdapter({
      kind: "valid",
      identity: { principal, authorityRequired: false },
    });
    expect(
      await faultCode(authenticateRequest(request(), { freshAuthority: false }, adapter)),
    ).toBe("authentication_required");
    expect(calls).toEqual({ validate: 0, resolve: 0 });
  });

  test("one bounded credential returns its normalised principal", async () => {
    const { adapter, calls } = fakeAdapter({
      kind: "valid",
      identity: { principal, authorityRequired: false },
    });
    const actual = await authenticateRequest(
      request({ cookie: "selector_session=opaque-value" }),
      { freshAuthority: false },
      adapter,
    );
    expect(actual).toEqual(principal);
    expect(calls).toEqual({ validate: 1, resolve: 0 });
  });

  test("fresh authority resolves after validation", async () => {
    const { adapter, calls } = fakeAdapter({
      kind: "valid",
      identity: { principal, authorityRequired: false },
    });
    await authenticateRequest(
      request({ authorization: "Bearer opaque-value" }),
      { freshAuthority: true },
      adapter,
    );
    expect(calls).toEqual({ validate: 1, resolve: 1 });
  });

  test("duplicates and competing locations are ambiguous without validation", async () => {
    for (const headers of [
      { cookie: "selector_session=one; selector_session=two" },
      { cookie: "selector_session=one", authorization: "Bearer two" },
      { authorization: "Bearer one, Bearer two" },
    ]) {
      const { adapter, calls } = fakeAdapter({ kind: "invalid" });
      expect(
        await faultCode(
          authenticateRequest(request(headers), { freshAuthority: false }, adapter),
        ),
      ).toBe("ambiguous_credentials");
      expect(calls).toEqual({ validate: 0, resolve: 0 });
    }
  });

  test("malformed, inactive, unavailable and duplicate authority outcomes stay classified", async () => {
    const malformed = fakeAdapter({
      kind: "valid",
      identity: { principal, authorityRequired: false },
    });
    expect(
      await faultCode(
        authenticateRequest(
          request({ authorization: "Basic unsupported" }),
          { freshAuthority: false },
          malformed.adapter,
        ),
      ),
    ).toBe("invalid_credentials");
    expect(malformed.calls.validate).toBe(0);

    for (const [resolution, expected] of [
      [{ kind: "inactive", failureName: "PrincipalInactive" }, "principal_inactive"],
      [{ kind: "missing" }, "invalid_credentials"],
      [{ kind: "unavailable" }, "authentication_unavailable"],
      [{ kind: "duplicate" }, "authority_invariant"],
    ] as const) {
      const { adapter } = fakeAdapter(
        { kind: "valid", identity: { principal, authorityRequired: true } },
        resolution,
      );
      expect(
        await faultCode(
          authenticateRequest(
            request({ authorization: "Bearer value" }),
            { freshAuthority: false },
            adapter,
          ),
        ),
      ).toBe(expected);
    }
  });

  test("a strategy cannot return a principal kind it does not declare", async () => {
    const { adapter } = fakeAdapter({
      kind: "valid",
      identity: { principal: servicePrincipal, authorityRequired: false },
    });
    expect(
      await faultCode(
        authenticateRequest(
          request({ cookie: "selector_session=opaque-value" }),
          { freshAuthority: false },
          adapter,
        ),
      ),
    ).toBe("invalid_credentials");
  });

  test("malformed or widened adapter principals fail as authority invariants", async () => {
    for (const malformedPrincipal of [
      { ...principal, values: { user_id: "not-a-uuid" } },
      { ...principal, values: { ...principal.values, permission: "admin" } },
      { ...principal, rawClaims: { permission: "admin" } },
    ]) {
      const { adapter } = fakeAdapter({
        kind: "valid",
        identity: {
          principal: malformedPrincipal as AuthPrincipal,
          authorityRequired: false,
        },
      });
      expect(
        await faultCode(
          authenticateRequest(
            request({ authorization: "Bearer opaque-value" }),
            { freshAuthority: false },
            adapter,
          ),
        ),
      ).toBe("authority_invariant");
    }
  });

  test("authoritative resolution cannot switch principal identity", async () => {
    const changedSubject: AuthPrincipal = { ...principal, subject: "user-2" };
    const { adapter } = fakeAdapter(
      { kind: "valid", identity: { principal, authorityRequired: true } },
      { kind: "active", principal: changedSubject },
    );
    expect(
      await faultCode(
        authenticateRequest(
          request({ authorization: "Bearer opaque-value" }),
          { freshAuthority: false },
          adapter,
        ),
      ),
    ).toBe("authority_invariant");
  });

  test("malformed adapter outcomes are contained at the authentication boundary", async () => {
    for (const validation of [null, { kind: "valid" }, { kind: "invalid", extra: true }]) {
      const { adapter } = fakeAdapter(validation as unknown as ValidationResult);
      expect(
        await faultCode(
          authenticateRequest(
            request({ authorization: "Bearer opaque-value" }),
            { freshAuthority: false },
            adapter,
          ),
        ),
      ).toBe("authority_invariant");
    }

    for (const resolution of [
      null,
      { kind: "active" },
      { kind: "inactive", failureName: { raw: "provider-object" } },
    ]) {
      const { adapter } = fakeAdapter(
        { kind: "valid", identity: { principal, authorityRequired: true } },
        resolution as unknown as ResolutionResult,
      );
      expect(
        await faultCode(
          authenticateRequest(
            request({ authorization: "Bearer opaque-value" }),
            { freshAuthority: false },
            adapter,
          ),
        ),
      ).toBe("authority_invariant");
    }
  });

  test("thrown adapter faults become availability failures", async () => {
    const adapter: AuthenticationAdapter = {
      async validate() {
        throw new Error("provider detail must not escape");
      },
      async resolve() {
        throw new Error("not reached");
      },
    };
    expect(
      await faultCode(
        authenticateRequest(
          request({ authorization: "Bearer opaque-value" }),
          { freshAuthority: false },
          adapter,
        ),
      ),
    ).toBe("authentication_unavailable");
  });
});
