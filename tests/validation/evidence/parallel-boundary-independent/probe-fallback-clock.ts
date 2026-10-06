import { strict as assert } from 'node:assert';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { resolve,join } from 'node:path';
const project=resolve('probes/fallback');mkdirSync(project,{recursive:true});
writeFileSync(join(project,'app.jadpo'),'enum HealthStatus { healthy }\noutput PublicHealth { status: HealthStatus }\nroute GET /ordinary { auth: none output: PublicHealth action: { return PublicHealth { status: HealthStatus.healthy } } }');
const result=Bun.spawnSync([resolve('cargo-target/debug/jadpo'),'build',project],{stdout:'pipe',stderr:'pipe'});
assert.equal(result.exitCode,0,result.stderr.toString());
const app=await import(pathToFileURL(join(project,'build/target/app.ts')).href);
const generic=await app.handleRequest(new Request('https://local.test/health/live'));
assert.equal(generic.status,200);const body=await generic.json();assert.deepEqual(body,{status:'live'});
const ordinary=await app.handleRequest(new Request('https://local.test/ordinary'));assert.equal(ordinary.status,200);assert.deepEqual(await ordinary.json(),{status:'healthy'});
const originalDate=globalThis.Date;
let ambientClockDisposition='unknown';
try {
  (globalThis as any).Date=class extends originalDate { constructor(...args:any[]){if(args.length===0) throw new Error('local-clock-probe');super(...args as any);} };
  try { await app.handleRequest(new Request('https://local.test/health/live'));ambientClockDisposition='response'; } catch { ambientClockDisposition='promise_rejection_before_handler_try'; }
} finally {globalThis.Date=originalDate;}
assert.equal(ambientClockDisposition,'promise_rejection_before_handler_try');
writeFileSync('evidence/fallback-local-clock-observations.json',JSON.stringify({genericFallback:{status:200,body},ordinaryAuthoredRoute:{status:200,body:{status:'healthy'}},ambientClockDisposition,classification:'Retained common local-host limitation; CONFIG-D10 does not require clock-fault immunity, and correction prohibits authored clock reads.'},null,2)+'\n');
console.log('Fallback probe passed; retained local-host clock failure recorded');
