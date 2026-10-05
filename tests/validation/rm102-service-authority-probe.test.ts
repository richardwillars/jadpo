// Deliberately red RM-102 diagnostic, outside the supported passing gate.
// Run from repository root: bun --no-install --env-file=/dev/null test ./tests/validation/rm102-service-authority-probe.test.ts
// Remove this separate probe only after its obligations become registered runtime regressions.
import { afterAll, beforeEach, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { cpSync, existsSync, mkdtempSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

// RM-102/103 regression slice against unchanged migrated source. This is not
// execution of the frozen 44-case acceptance suite or the service lifecycle gate.
const root = mkdtempSync(join(tmpdir(), "jadpo-golden-protected-"));
const project = join(root, "application");
const source = resolve("examples/golden-todo-migration");
const previousSqlitePath = Bun.env.SQLITE_PATH;
const previousDatabaseUrl = Bun.env.DATABASE_URL;
let database: Database | undefined;
afterAll(() => {
  database?.close();
  if (previousSqlitePath === undefined) delete Bun.env.SQLITE_PATH;
  else Bun.env.SQLITE_PATH = previousSqlitePath;
  if (previousDatabaseUrl === undefined) delete Bun.env.DATABASE_URL;
  else Bun.env.DATABASE_URL = previousDatabaseUrl;
  rmSync(root, { recursive: true, force: true });
});
cpSync(source, project, { recursive: true, filter: path => path !== join(source, "build") });
const compiler = Bun.env.JADPO_BIN ?? resolve("jadpo/target/debug/jadpo");
const build = Bun.spawnSync([compiler, "build", project], { stdout: "pipe", stderr: "pipe" });
if (build.exitCode !== 0) throw new Error(`Golden protected fixture failed to build:\n${build.stdout}\n${build.stderr}`);
const dependency = resolve(Bun.env.JADPO_JWT_DEPENDENCY_DIR ?? "build/validation/jwt-dependencies");
if (!existsSync(join(dependency, "node_modules/jose/package.json"))) throw new Error("Install the pinned JWT dependency before running this suite");
symlinkSync(join(dependency, "node_modules"), join(project, "build/target/node_modules"), "dir");
Bun.env.SQLITE_PATH = join(root, "golden.sqlite");
delete Bun.env.DATABASE_URL;
const app = await import(pathToFileURL(join(project, "build/target/app.ts")).href);
database = new Database(Bun.env.SQLITE_PATH, { strict: true });
const db = database;
const owner = "00000000-0000-4000-8000-000000000001";
const otherOwner = "00000000-0000-4000-8000-000000000002";
const environment = {
  DATABASE_URL: "https://db.test/golden",
  SESSION_SIGNING_KEY: Buffer.alloc(32, 71).toString("base64url"),
  BROWSER_ORIGIN: "https://todo.test",
  OIDC_ISSUER: "https://issuer.test",
  OIDC_AUDIENCE: "todo",
  MAIL_API_KEY: Buffer.alloc(32, 72).toString("base64url"),
  MAIL_SENDER: "todo@example.test",
};
let now: number;
beforeEach(async () => {
  await app.initializeApplication(environment);
  db.exec('DELETE FROM todo; DELETE FROM "__jadpo_auth_sessions"; DELETE FROM "user";');
  now = Date.now();
  for (const [id, subject] of [[owner, "owner"], [otherOwner, "other-owner"]]) {
    db.prepare('INSERT INTO "user" (id, authentication_subject, email, status, created_at, disabled_at) VALUES (?, ?, ?, ?, ?, ?)')
      .run(id, subject, `${subject}@example.test`, "active", now, null);
  }
});
const host = () => app.authenticationHost();
const issue = (strategy = "api_bearer", subject = "owner") => host().issue(strategy, subject, now + 3_600_000, now);
const bearer = (credential: string) => ({ authorization: `Bearer ${credential}` });
const countTodos = () => (db.prepare("SELECT count(*) AS count FROM todo").get() as { count: number }).count;
function rawRequest(headers: Record<string, string>, body: unknown = { title: "Protected todo" }) {
  return new Request("https://todo.test/todos", {
    method: "POST", headers: { "content-type": "application/json", ...headers }, body: JSON.stringify(body),
  });
}
async function rejected(headers: Record<string, string>, status: number, code: string) {
  const before = countTodos();
  const response = await app.handleRequest(rawRequest(headers));
  expect(response.status).toBe(status);
  const text = await response.text();
  expect(JSON.parse(text).error.code).toBe(code);
  expect(countTodos()).toBe(before);
  expect(text).not.toContain(environment.SESSION_SIGNING_KEY);
  for (const value of Object.values(headers)) {
    if (value.startsWith("Bearer ")) expect(text).not.toContain(value.slice(7));
  }
}

// Diagnostic reproduction only: deliberately asserts the pending lifecycle gate.
test("pending golden declared service credential authority diagnostic", async () => {
  const service = "00000000-0000-4000-8000-000000000003";
  const credentialId = "00000000-0000-4000-8000-000000000004";
  const seedVerifier = "A".repeat(43);
  db.prepare('INSERT INTO service (id, owner_id, name, status, created_at, disabled_at) VALUES (?, ?, ?, ?, ?, ?)').run(service, owner, "reporter", "active", now, null);
  db.prepare('INSERT INTO service_credential (id, service_id, verifier, status, expires_at, created_at, revoked_at) VALUES (?, ?, ?, ?, ?, ?, ?)').run(credentialId, service, seedVerifier, "active", now + 3_600_000, now, null);
  const credential = await host().issueServiceCredential("api_bearer", seedVerifier, now + 3_600_000, now);
  const req = new Request("https://todo.test/todos", { headers: bearer(credential.credential) });
  const principal = await host().authenticate(req, false, now);
  const findings: Record<string, unknown> = { subjectEqualsDeclaredVerifier: principal.subject === seedVerifier, strength: principal.authenticationStrength, serviceIdCorrect: principal.values.service_id === service };
  for (const [name, sql, reset] of [
    ["disabledService", "UPDATE service SET status = 'disabled'", "UPDATE service SET status = 'active'"],
    ["expiredCredential", "UPDATE service_credential SET expires_at = 0", `UPDATE service_credential SET expires_at = ${now + 3_600_000}`],
    ["revokedCredentialTimestamp", `UPDATE service_credential SET revoked_at = ${now}`, "UPDATE service_credential SET revoked_at = NULL"],
    ["revokedCredentialStatus", "UPDATE service_credential SET status = 'revoked'", "UPDATE service_credential SET status = 'active'"],
  ]) {
    db.exec(sql);
    try { await host().authenticate(req, true, now); findings[name] = "accepted"; }
    catch (error: any) { findings[name] = error.code; }
    finally { db.exec(reset); }
  }
  console.log(JSON.stringify({ diagnostic: "pending-golden-service-authority", findings }));
  // Expected red: these obligations are not implemented, rather than skipped.
  expect(findings.subjectEqualsDeclaredVerifier).toBe(false);
});
