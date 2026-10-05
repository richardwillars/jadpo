
import { Database } from "bun:sqlite";
import { SQL } from "bun";
import { existsSync, mkdtempSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

// Component evidence only: the production renderer consumes the complete
// checked fixture and opaque binding, while public full-target lowering still
// refuses its job. No generated code is rewritten; invocation components use a
// disposable loopback reference provider, never a real mail account or scheduler.
const postgresUrl = Bun.env.JADPO_DELIVERY_HOOK_DATABASE_URL;
if (postgresUrl) {
  const url = new URL(postgresUrl);
  if (!["localhost", "127.0.0.1"].includes(url.hostname) || !/^\/jadpo_auth_test_[a-z0-9_]+$/u.test(url.pathname)) throw new Error("Requires a disposable local delivery component database");
}
delete Bun.env.DATABASE_URL;
delete Bun.env.SQLITE_PATH;
const root = mkdtempSync(join(tmpdir(), "jadpo-delivery-hook-components-"));
const repository = '/Users/richardwillars/Documents/Codex/LLM-coding';
const compiler = Bun.spawn(["cargo", "test", "--manifest-path", join(repository, "jadpo/Cargo.toml"), "-p", "jadpo-core", "--lib", "target::tests::delivery_schedule_hook_components_keep_full_execution_disabled", "--", "--exact"], {
  cwd: repository, env: { ...Bun.env, CARGO_INCREMENTAL: "0", JADPO_DELIVERY_HOOK_COMPONENT_OUTPUT: root }, stdout: "pipe", stderr: "pipe",
});
const [compileOut, compileErr, compileExit] = await Promise.all([new Response(compiler.stdout).text(), new Response(compiler.stderr).text(), compiler.exited]);
if (compileExit !== 0 || !compileOut.includes("1 passed")) throw new Error(`component generation failed: ${compileOut}\n${compileErr}`);
const sqlitePath = join(root, "application.sqlite");
if (postgresUrl) Bun.env.DATABASE_URL = postgresUrl;
else Bun.env.SQLITE_PATH = sqlitePath;
const persistencePath = join(root, "persistence.ts");
const dependency = join(repository, "build/validation/jwt-dependencies/node_modules");
if (!existsSync(join(dependency, "jose/package.json"))) throw new Error("Install the pinned JWT dependency before running native authentication components");
symlinkSync(dependency, join(root, "node_modules"), "dir");
const { persistence } = await import(pathToFileURL(persistencePath).href) as { persistence: any };
const { persistence: ordinary } = await import(pathToFileURL(join(root, "ordinary-persistence.ts")).href) as { persistence: any };
const app = await import(pathToFileURL(join(root, "app.ts")).href);
const { authenticationStorage, resolveAuthenticationAuthority } = await import(pathToFileURL(persistencePath).href);
const { createFirstPartyAuthentication } = await import(pathToFileURL(join(root, "first-party-authentication.ts")).href);
const { setReferenceMailEndpointForTesting } = await import(pathToFileURL(join(root, "service-adapter.ts")).href);
const sql = postgresUrl ? new SQL(postgresUrl, { max: 3, prepare: false }) : null;
const sqlite = sql === null ? new Database(sqlitePath) : null;

const instant = "2026-10-05T08:00:00.000Z";
const representationVariant = Bun.env.JADPO_DELIVERY_HOOK_COMPONENT_VARIANT === "completion-representation";
const changeField = representationVariant ? "modified_at" : "updated_at";
const dueA = "2026-10-06T08:00:00.000Z", dueB = "2026-10-07T08:00:00.000Z";
const owner = crypto.randomUUID();
// A test-owned user principal exercises unchanged row policy; it is not worker
// credential/authentication evidence or a generated reminder authority grant.
const client = persistence.withOperationTime(instant).withPolicy({ kind: "user", subject: owner, values: { user_id: owner } }, "Todo.patch_todo");
const method = "update_required_Todo_by_id_patch_title_and_status_and_due_at_set_reminder_sent_at";
const rows = async (statement: string, values: any[] = []) => sql ? await sql.unsafe(statement, values) : sqlite!.prepare(statement).all(...values);
const revision = (id: string) => client.transaction((tx: any) => tx.read_delivery_schedule_revision("Todo", id));
const patch = (id: string, value: Record<string, unknown>) => client[method](id, value, null);
const stored = (id: string) => rows('SELECT title, due_at, reminder_sent_at FROM "todo" WHERE id = $1', [id]);
const setSent = (id: string) => rows('UPDATE "todo" SET reminder_sent_at = $1 WHERE id = $2', [sql ? instant : Date.parse(instant), id]);
const todo = (id = crypto.randomUUID()) => ({ id, owner_id: owner, title: "component reminder", status: "open", due_at: dueA, reminder_sent_at: null, created_at: instant, [changeField]: instant, deleted_at: null });
const stamps = (id: string) => rows(`SELECT created_at,"${changeField}" AS updated_at FROM todo WHERE id=$1`, [id]);
// Raw fixture seed: authored User creation is deliberately not a granted API.
await rows('INSERT INTO "user" (id, authentication_subject, email, status, created_at, disabled_at) VALUES ($1,$2,$3,$4,$5,$6)', [owner, owner, `${owner}@example.invalid`, "active", sql ? instant : Date.parse(instant), null]);

const bindingIdentity = JSON.parse(readFileSync(join(root, "binding.json"), "utf8")).binding;
const authEnvironment = {
  DATABASE_URL: "https://db.example.invalid/component",
  SESSION_SIGNING_KEY: Buffer.alloc(32, 71).toString("base64url"), BROWSER_ORIGIN: "https://todo.example.invalid",
  OIDC_ISSUER: "https://issuer.example.invalid", OIDC_AUDIENCE: "todo",
  MAIL_API_KEY: Buffer.alloc(32, 72).toString("base64url"), MAIL_SENDER: "todo@example.invalid",
};
const dbInstant = (milliseconds: number) => sql ? new Date(milliseconds).toISOString() : milliseconds;
async function authorityFixture() {
  await app.initializeApplication(authEnvironment);
  const service = crypto.randomUUID(), membership = crypto.randomUUID(), subject = `component-${service}`;
  const now = Date.now();
  await rows('INSERT INTO "service" (id, owner_id, name, status, created_at, disabled_at) VALUES ($1,$2,$3,$4,$5,$6)', [service, owner, subject, "active", dbInstant(now), null]);
  await rows('INSERT INTO "reminder_service_membership" (id, service_id, role) VALUES ($1,$2,$3)', [membership, service, "sender"]);
  const host = app.authenticationHost();
  const issued = await host.issueServiceCredential("api_bearer", subject, now + 3_600_000, now);
  const proof = await host.prepareDeliveryCredential("api_bearer", issued.credential, now);
  return { host, issued, proof, service, membership, subject, now };
}
const authority = (proof: unknown, binding = bindingIdentity) => persistence.transaction((tx: any) => tx.check_delivery_authority(binding, proof));
const scanTime = "2026-10-08T08:00:00.000Z";
const scan = (proof: unknown, after: unknown = null, at = scanTime) => persistence.withOperationTime(at).transaction((tx: any) => tx.select_delivery_intents(bindingIdentity, proof, after));
const clearScanTodos = () => rows('DELETE FROM "todo"'); // This process's disposable component fixtures only.
const intentRows = (id: string) => rows('SELECT intent_id, payload FROM "__jadpo_deliveries_v1" WHERE source_entity_id=$1', [id]);
async function unpublishedHost() {
  // Real configured crypto, exact shared declared storage and actual generated
  // authority queries. Its only missing fact is publication by the app.
  return createFirstPartyAuthentication([{ name: "api_bearer", principal: "service", mode: "api_key", cookie: null,
    audience: "todo-service-key", origin: null, secret: authEnvironment.SESSION_SIGNING_KEY, maximumDelayMs: 300_000 }],
    authenticationStorage, async (strategy: string, subject: string) => {
      const actual = await resolveAuthenticationAuthority(strategy, subject, "service");
      if (actual.length !== 1 || actual[0].status !== "active") return { kind: "missing" };
      return { kind: "active", principal: { kind: "service", subject: actual[0].name, authenticationStrength: "primary", values: { service_id: actual[0].id } } };
    }, undefined, () => {});
}


const f = await authorityFixture();
const profile = { executionMs:120000, leaseMs:90000 };
const value=todo(); await client.create_Todo(value); const intent=(await scan(f.proof)).intents[0];
let calls=0; Bun.env.NODE_ENV="test";
const server=Bun.serve({hostname:"127.0.0.1",port:0,async fetch(request){ const body=await request.json(); calls++; return Response.json({accepted_at:"2099-01-01T00:00:00.000Z"},{status:202,headers:{"Idempotency-Key":body.idempotency_key}}); }});
setReferenceMailEndpointForTesting(`http://127.0.0.1:${server.port}`);
await persistence.transaction(async (tx:any)=>{
  await tx.tick_delivery_schedule(bindingIdentity,900000,scanTime);
  const handle=await tx.claim_delivery_activation(bindingIdentity,scanTime,profile);
  await tx.stage_delivery_activation_page(handle,[intent.intentId],{ binding:"wrong-binding",operationTime:scanTime,dueAt:dueA,id:crypto.randomUUID() });
});
await rows('UPDATE "__jadpo_delivery_activations_v1" SET lease_until=$1 WHERE binding=$2',["2000-01-01T00:00:00.000Z",bindingIdentity]);
const before=await rows('SELECT * FROM "__jadpo_delivery_activations_v1" WHERE binding=$1',[bindingIdentity]);
let result:any,error:any=null;
try { result=await app.runDeliveryScheduleActivation(bindingIdentity,f.proof,profile,scanTime); } catch(e:any) { error={name:e.name,message:e.message,operation:e.operation}; }
const after=await rows('SELECT * FROM "__jadpo_delivery_activations_v1" WHERE binding=$1',[bindingIdentity]);
console.log(JSON.stringify({ generated:root,result,error,calls,sent:(await stored(value.id))[0].reminder_sent_at,unchanged:JSON.stringify(before)===JSON.stringify(after),before,after }));
setReferenceMailEndpointForTesting(null); server.stop(true); if(sql)await sql.close(); if(sqlite)sqlite.close();
process.exit(error!==null && calls===0 && JSON.stringify(before)===JSON.stringify(after) ? 0 : 2);
