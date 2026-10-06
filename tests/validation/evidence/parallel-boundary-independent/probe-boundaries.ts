import { strict as assert } from 'node:assert';
import { cpSync, mkdirSync, readFileSync, writeFileSync, symlinkSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { Database } from 'bun:sqlite';
const root = resolve('probes');
const compiler = resolve('cargo-target/debug/jadpo');
const observations: any[] = [];
const logs: string[] = [];
const originalError = console.error;
console.error = (...values) => logs.push(values.join(' '));
delete Bun.env.DATABASE_URL;
delete Bun.env.JADPO_DEBUG_TARGET_STACKS;
const canary = 'independent-boundary-canary-do-not-disclose';
async function inspect(name: string, response: Response, status: number, code?: string, expected?: any) {
  const body = await response.json();
  assert.equal(response.status, status, name);
  if (code) assert.equal(body.error.code, code, name);
  if (expected) assert.deepEqual(body, expected, name);
  assert.equal(JSON.stringify(body).includes(canary), false, name);
  assert.ok(/^req_/.test(response.headers.get('x-request-id') ?? ''), name);
  observations.push({ name, status: response.status, body, requestIdHeaderMatches: !body.error || body.error.request_id === response.headers.get('x-request-id') });
  return body;
}
function build(name: string, source: string) {
  const path = join(root, name); mkdirSync(path, { recursive: true }); writeFileSync(join(path, 'app.jadpo'), source);
  const result = Bun.spawnSync([compiler, 'build', path], { stdout:'pipe', stderr:'pipe' });
  writeFileSync(join('evidence', name + '-build.log'), result.stdout.toString() + result.stderr.toString());
  assert.equal(result.exitCode, 0, name + ': ' + result.stderr.toString());
  return path;
}
// Distinct input/path/query/header fixture; expected boundaries follow runtime spec.
const project = build('boundary', `
type Bounded = Text { min_length: 3 }
input Body { value: Bounded }
output Echo { value: Bounded }
input Query { size: Int { min: 1 } }
route POST /body { auth: none input: Body output: Echo action: { return Echo { value: input.value } } }
route GET /path/{value} { auth: none path: { value: Uuid } output: Uuid action: { return path.value } }
route GET /query { auth: none query: Query output: Query action: { return query } }
route GET /header { auth: none headers: { id: Uuid from "X-Identifier" } output: Uuid action: { return headers.id } }
`);
const app = await import(pathToFileURL(join(project, 'build/target/app.ts')).href);
const req = (body: any) => new Request('https://boundary.test/body', { method:'POST', body:JSON.stringify(body) });
await inspect('valid-body', await app.handleRequest(req({value:'valid'})), 200, undefined, {value:'valid'});
await inspect('malformed-json', await app.handleRequest(new Request('https://boundary.test/body', {method:'POST', body:'{'})), 400, 'invalid_request');
for (const body of [{value:'x'}, {value:7}, {}, {value:'valid', extra:canary}, null, []]) await inspect('declared-body-' + JSON.stringify(body), await app.handleRequest(req(body)), 422, 'invalid_input');
for (const failure of [new Error(canary), new TypeError(canary)]) {
  const incoming = req({}); Object.defineProperty(incoming, 'json', {value:async () => { throw failure; }});
  await inspect('unexpected-parser-' + failure.name, await app.handleRequest(incoming), 500, 'internal_fault');
}
for (const failure of [new Error(canary), new SyntaxError(canary)]) {
  const incoming = req({}); Object.defineProperty(incoming, 'json', {value:async () => new Proxy({}, {ownKeys() {throw failure;}})});
  await inspect('unexpected-validator-' + failure.name, await app.handleRequest(incoming), 500, 'internal_fault');
}
await inspect('declared-path', await app.handleRequest(new Request('https://boundary.test/path/not-a-uuid')), 422, 'invalid_value');
await inspect('query-syntax', await app.handleRequest(new Request('https://boundary.test/query?size=1&size=2')), 400, 'invalid_request');
await inspect('query-value', await app.handleRequest(new Request('https://boundary.test/query?size=0')), 422, 'invalid_value');
// Declared headers require the emitted Node adapter's raw-header metadata.
const server = app.createApplicationServer();
await new Promise<void>((resolve,reject) => {server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
const port=server.address().port;
await inspect('header-syntax', await fetch(`http://127.0.0.1:${port}/header`), 400, 'invalid_request');
await inspect('header-value', await fetch(`http://127.0.0.1:${port}/header`,{headers:{'X-Identifier':'bad'}}), 422, 'invalid_value');
await new Promise<void>((resolve) => server.close(resolve));
// Generated mutation probes fault classification at path/output validation without editing compiler source.
let generated = readFileSync(join(project, 'build/target/app.ts'),'utf8');
assert.ok(generated.includes('value: validateUuid(routePath1["value"], "path.value")'));
let pathMutation = generated.replace('value: validateUuid(routePath1["value"], "path.value")', `value: (() => { throw new SyntaxError("${canary}"); })()`);
writeFileSync(join(project, 'build/target/path-fault.ts'), pathMutation);
const pathFault = await import(pathToFileURL(join(project, 'build/target/path-fault.ts')).href);
await inspect('unexpected-path-validator', await pathFault.handleRequest(new Request('https://boundary.test/path/00000000-0000-4000-8000-000000000001')),500,'internal_fault');
const outputNeedle = 'return json(200, validate_Echo(';
assert.ok(generated.includes(outputNeedle));
const outputMutation = generated.replace(outputNeedle, `return json(200, (() => { throw new SyntaxError("${canary}"); })(`);
writeFileSync(join(project, 'build/target/output-fault.ts'), outputMutation);
const outputFault = await import(pathToFileURL(join(project, 'build/target/output-fault.ts')).href);
await inspect('unexpected-output-validator', await outputFault.handleRequest(req({value:'valid'})),500,'internal_fault');
// Active authentication and query tracing through migrated application.
const golden = join(root,'golden');
try { symlinkSync(resolve('build/validation/jwt-dependencies/node_modules'),join(golden,'build/target/node_modules'),'dir'); } catch (error:any) { if(error.code!=='EEXIST') throw error; }
const authenticationPath=join(golden,'build/target/first-party-authentication.ts');
const authenticationOriginal=readFileSync(authenticationPath,'utf8');
const authenticationEntry='async authenticate(request: Request, freshAuthority: boolean, operationNow: number, mutates = false): Promise<AuthPrincipal> {';
assert.equal(authenticationOriginal.split(authenticationEntry).length,2);
writeFileSync('evidence/original-first-party-authentication.ts',authenticationOriginal);
writeFileSync(authenticationPath,authenticationOriginal.replace(authenticationEntry,authenticationEntry+' (globalThis as any).__boundaryAuthAttempts++;'));
(globalThis as any).__boundaryAuthAttempts=0;
Bun.env.SQLITE_PATH = join(golden,'healthy.sqlite');
let queryCount = 0; let capture = false;
const prepare = Database.prototype.prepare;
(Database.prototype as any).prepare = function(sql: string,...args: any[]) {
  const statement = prepare.call(this,sql,...args);
  return new Proxy(statement,{get(target,property){const value=Reflect.get(target,property,target); if(typeof value!=='function') return value; if(!['all','get','run','values','iterate'].includes(String(property))) return value.bind(target); return (...arguments_:any[]) => {if(capture) queryCount++; return Reflect.apply(value,target,arguments_);};}});
};
const live = await import(pathToFileURL(join(golden,'build/target/app.ts')).href);
await live.initializeApplication({DATABASE_URL:'https://db.test/golden',SESSION_SIGNING_KEY:Buffer.alloc(32,71).toString('base64url'),BROWSER_ORIGIN:'https://todo.test',OIDC_ISSUER:'https://issuer.test',OIDC_AUDIENCE:'todo',MAIL_API_KEY:Buffer.alloc(32,72).toString('base64url'),MAIL_SENDER:'todo@example.test'});
const host = live.authenticationHost();
const incoming = new Request('https://todo.test/health/live', {headers:{authorization:'Bearer ' + canary,cookie:'jadpo_session=' + canary}});
Object.defineProperty(incoming,'json',{value:async () => {throw new Error('must-not-read-liveness-body');}});
capture=true; queryCount=0; (globalThis as any).__boundaryAuthAttempts=0;
await inspect('active-auth-invalid-credentials-liveness',await live.handleRequest(incoming),200,undefined,{status:'healthy'});
assert.equal(queryCount,0); assert.equal((globalThis as any).__boundaryAuthAttempts,0); observations.at(-1).databaseQueries=queryCount; observations.at(-1).authenticationAttempts=(globalThis as any).__boundaryAuthAttempts;
await inspect('protected-invalid-credentials-control',await live.handleRequest(new Request('https://todo.test/todos',{headers:{authorization:'Bearer ' + canary}})),401,'invalid_credentials');
assert.equal((globalThis as any).__boundaryAuthAttempts,1); observations.at(-1).authenticationAttempts=(globalThis as any).__boundaryAuthAttempts;
capture=false;
// Unavailable SQLite project, same enabled auth source, before initialization.
cpSync(golden,join(root,'outage'),{recursive:true,filter:path=>!path.endsWith('node_modules') && !path.endsWith('healthy.sqlite')});
symlinkSync(resolve('build/validation/jwt-dependencies/node_modules'),join(root,'outage/build/target/node_modules'),'dir');
Bun.env.SQLITE_PATH=join(root,'nonexistent-parent','outage.sqlite');
const outage = await import(pathToFileURL(join(root,'outage/build/target/app.ts')).href);
await inspect('outage-readiness-control',await outage.handleRequest(new Request('https://todo.test/health/ready')),503);
queryCount=0; capture=true; (globalThis as any).__boundaryAuthAttempts=0;
await inspect('active-auth-outage-liveness',await outage.handleRequest(new Request('https://todo.test/health/live',{headers:{authorization:'Bearer ' + canary,cookie:'jadpo_session=' + canary}})),200,undefined,{status:'healthy'});
assert.equal(queryCount,0); assert.equal((globalThis as any).__boundaryAuthAttempts,0); observations.at(-1).databaseQueries=queryCount; observations.at(-1).authenticationAttempts=(globalThis as any).__boundaryAuthAttempts;
capture=false;
// Output validation runs on constant liveness, defects remain a safe generic 500.
const goldenGenerated=readFileSync(join(golden,'build/target/app.ts'),'utf8');
assert.ok(goldenGenerated.includes('return json(200, validate_PublicHealth({ status: "healthy" }, "response.body"), requestId)'));
writeFileSync(join(golden,'build/target/liveness-output-fault.ts'),goldenGenerated.replace('validate_PublicHealth({ status: "healthy" }, "response.body")',`(() => { throw new SyntaxError("${canary}"); })()`));
Bun.env.SQLITE_PATH=join(golden,'healthy.sqlite');
const liveOutputFault=await import(pathToFileURL(join(golden,'build/target/liveness-output-fault.ts')).href);
await inspect('liveness-output-fault',await liveOutputFault.handleRequest(new Request('https://todo.test/health/live')),500,'internal_fault');
assert.equal(logs.join('\n').includes(canary),false);
writeFileSync('evidence/adversarial-observations.json',JSON.stringify({observations,operationalLogs:logs,canaryContainment:true},null,2)+'\n');
console.error=originalError;
console.log(`Independent probes passed: ${observations.length}; safe logs ${logs.length}`);
