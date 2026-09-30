import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {readFileSync,writeFileSync,copyFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {request,Agent} from 'node:http';
const root=resolve('.'),out=resolve('build/capability-host'),seed=JSON.parse(readFileSync(out+'/seed.json','utf8'));
const smoke=process.argv.includes('--smoke'),variant=process.argv.includes('--variant');const results=[];
const clean=v=>{if(v?.error){const {request_id,...error}=v.error;return {error};}return v;};
async function start(target,journal){
 const dir=out+`/run-${target}-${journal}-${Date.now()}`;mkdirSync(dir,{recursive:true});const path=dir+'/state.sqlite';copyFileSync(out+'/seed.sqlite',path);
 const token='LOCAL_SYNTHETIC_CONTROL';
 const command=target==='native'?[out+'/target/release/jadpo-capability-host-native']:['bun','--no-install','--env-file=/dev/null',root+'/experiments/capability-host/bun-server.ts'];
 const child=spawn(command[0],command.slice(1),{env:{...process.env,...seed.configuration,SQLITE_PATH:path,DATABASE_URL:'',TEST_CONTROL_TOKEN:token,EXPERIMENT_TARGET:target,JOURNAL:journal,PORT:'0'},stdio:['ignore','pipe','pipe']});
 let stderr='',stdout='';child.stderr.on('data',b=>stderr+=b);
 const exit=new Promise((resolve,reject)=>{child.once('error',reject);child.once('exit',resolve)});
 let info;try{info=await new Promise((resolve,reject)=>{
  const timer=setTimeout(()=>reject(Error('startup timeout '+stderr)),20000);
  child.stdout.on('data',b=>{stdout+=b;if(stdout.includes('\n')){clearTimeout(timer);try{resolve(JSON.parse(stdout.split('\n')[0]))}catch(e){reject(e)}}});
  exit.then(code=>{clearTimeout(timer);reject(Error(`startup exit ${code} ${stderr}`))},reject);
 });}catch(e){child.kill();throw e;}
 for(const p of info.pragmas){assert.equal(p.journal_mode,journal.toLowerCase());assert.equal(p.synchronous,2);assert.equal(p.foreign_keys,1);assert.equal(p.busy_timeout,0);}
 const agent=new Agent({keepAlive:true,maxSockets:12});
 const send=(path,body,headers={})=>new Promise((resolve,reject)=>{
  const raw=typeof body==='string'?body:JSON.stringify(body);const req=request(info.url+path.slice(1),{method:'POST',agent,headers:{'content-type':'application/json','content-length':Buffer.byteLength(raw),...headers}},res=>{
   const chunks=[];res.on('data',b=>chunks.push(b));res.on('error',reject);res.on('end',()=>{try{const body=JSON.parse(Buffer.concat(chunks).toString());assert.equal(res.headers['cache-control'],'no-store');assert.ok(res.headers['x-request-id']);if(body?.error&&typeof body.error==='object')assert.equal(body.error.request_id,res.headers['x-request-id']);resolve({status:res.statusCode,body:clean(body)})}catch(e){reject(e)}});
  });req.on('error',reject);req.setTimeout(20000,()=>req.destroy(Error('request timeout')));req.end(raw);
 });
 const control=async body=>{const result=await send('/__control',body,{'x-experiment-control':token});assert.equal(result.status,200);assert.equal(result.body?.error,undefined,JSON.stringify(result));return result.body;};
 const sql=(sql,params=[])=>control({sql,params});
 const call=(path,body,user='alice',headers={})=>send(path,body,{...(user?{authorization:`Bearer ${seed.credentials[user]}`} :{}),...headers});
 const reset=async()=>{
  for(const note of seed.notes)await sql('UPDATE note SET owner_id=?,editor_id=?,title=?,private_note=? WHERE id=?',[note.owner_id,note.editor_id,note.title,note.private_note,note.id]);
  await sql('DELETE FROM user');for(const [subject,id] of Object.entries(seed.users))await sql('INSERT INTO user VALUES(?,?,1)',[id,subject]);
  await sql('DELETE FROM __jadpo_auth_sessions');for(const s of seed.sessions)await sql('INSERT INTO __jadpo_auth_sessions(id,data,revoked) VALUES(?,?,?)',[s.id,s.data,s.revoked]);
  await control({trace:true});
 };
 const snapshot=()=>sql('SELECT * FROM note ORDER BY id');
 return {info,call,send,sql,control,reset,snapshot,stop:async()=>{agent.destroy();child.kill();await exit;writeFileSync(dir+'/stderr.log',stderr);for(const secret of [...Object.values(seed.credentials),...Object.values(seed.configuration)])assert.ok(!stderr.includes(secret),'secret in runtime log');},dir};
}
const [a1,a2,c1,c2]=seed.notes.map(n=>n.id),absent='00000000-0000-4000-8000-000000000099';
const sessionId=user=>seed.credentials[user].split('.')[1];
for(const journal of smoke?['WAL']:['WAL','DELETE'])for(const target of ['bun','native','wasm']){
 const server=await start(target,journal);const records=[];
 try{
  async function check(name,work,status,code,changed=false){
   await server.reset();const before=await server.snapshot();const result=await work();
   assert.equal(result.status,status,`${target}/${journal}/${name}: ${JSON.stringify(result)}`);
   if(code)assert.equal(result.body.error?.code,code,`${name}: ${JSON.stringify(result)}`);
   const after=await server.snapshot();if(!changed)assert.deepEqual(after,before,`${name}: unexpected persisted write`);
   const trace=await server.control({trace:true});records.push({name,result,after,trace});
  }
  const read=(user='alice',id=a1,headers={})=>server.call('/notes/read',{id},user,headers);
  await check('owner read',()=>read(),200);
  await check('editor public projection',()=>read('bob'),200);
  assert.deepEqual(records.at(-1).result.body,{id:a1,title:'Alice first'});
  await check('outsider concealed',()=>read('charlie'),404,'note_missing');
  await check('missing concealed',()=>read('charlie',absent),404,'note_missing');
  assert.deepEqual(records.at(-1).result,records.at(-2).result);
  await check('owner private field',()=>server.call('/notes/private',{id:a1}),200);
  await check('editor private field denied',()=>server.call('/notes/private',{id:a1},'bob'),404,'note_missing');
  await check('owner private update',()=>server.call('/notes/secret',{id:a1,private_note:'Changed private'}),200,null,true);
  assert.equal(records.at(-1).after[0].private_note,'Changed private');
  await check('editor private update denied',()=>server.call('/notes/secret',{id:a1,private_note:'Forbidden'},'bob'),variant?200:404,variant?null:'note_missing',variant);
  await check('editor title update',()=>server.call('/notes/rename',{id:a1,title:'Editor title'},'bob'),200,null,true);
  assert.equal(records.at(-1).after[0].title,'Editor title');
  const pair=(second=a2,second_title='Second changed')=>({first:a1,second,title:'First changed',second_title});
  await check('atomic pair commit',()=>server.call('/notes/pair',pair()),200,null,true);
  assert.equal(records.at(-1).after[0].title,'First changed');assert.equal(records.at(-1).after[1].title,'Second changed');
  await check('second row denied rolls back first',()=>server.call('/notes/pair',pair(c1)),404,'note_missing');
  await check('second row absent rolls back first',()=>server.call('/notes/pair',pair(absent)),404,'note_missing');
  await check('second constraint rolls back first',()=>server.call('/notes/pair',pair(a2,'Charlie first')),409,'note_conflict');
  await check('authored rejection rolls back write',()=>server.call('/notes/reject',{id:a1,title:'Must roll back'}),422,'change_rejected');
  await check('valid previous key credential',()=>read('previous'),200);
  if(variant){
   await check('source refinement rejects three scalars',()=>server.call('/notes/rename',{id:a1,title:'abc'}),400,'invalid_request');
   await check('source refinement accepts five scalars',()=>server.call('/notes/rename',{id:a1,title:'abcde'}),200,null,true);
  }
  if(!smoke){
   await check('missing credential before malformed body',()=>server.call('/notes/rename','{',null),401,'authentication_required');
   await check('empty credential',()=>read(null,a1,{authorization:''}),401,'invalid_credentials');
   await check('wrong scheme',()=>read(null,a1,{authorization:'Basic fake'}),401,'invalid_credentials');
   await check('credential tampering',()=>read(null,a1,{authorization:`Bearer ${seed.credentials.alice.slice(0,-2)}AA`}),401,'invalid_credentials');
   await check('noncanonical base64',()=>read(null,a1,{authorization:`Bearer ${seed.credentials.alice}=`}),401,'invalid_credentials');
   await check('duplicate credentials',()=>read(null,a1,{authorization:`Bearer ${seed.credentials.alice}, Bearer ${seed.credentials.alice}`}),401,'ambiguous_credentials');
   await check('conflicting credentials before malformed body',()=>server.call('/notes/rename','{',null,{authorization:`Bearer ${seed.credentials.alice}, Bearer ${seed.credentials.bob}`}),401,'ambiguous_credentials');
   await check('forged principal header',()=>read(null,a1,{'x-principal':seed.users.alice}),401,'authentication_required');
   await check('body cannot supply principal',()=>server.call('/notes/read',{id:a1,principal:{user_id:seed.users.alice}},'charlie'),400,'invalid_request');
   await check('invalid body after auth',()=>server.call('/notes/rename','{'),400,'invalid_request');
   await check('lone surrogate rejected',()=>server.call('/notes/rename',{id:a1,title:'a\ud800b'}),400,'invalid_request');
   await check('UUID validation',()=>server.call('/notes/read',{id:'not-a-uuid'}),400,'invalid_request');
   await check('bound SQL input stays data',()=>server.call('/notes/rename',{id:a1,title:"x');DROP TABLE note;--"}),200,null,true);
   await check('owner clears restricted field',()=>server.call('/notes/secret',{id:a1,private_note:null}),200,null,true);
   await check('editor cannot clear restricted field',()=>server.call('/notes/secret',{id:a1,private_note:null},'bob'),404,'note_missing');
   await check('bearer scheme casing and tab',()=>read(null,a1,{authorization:`bEaReR\t${seed.credentials.alice}`}),200);
   await check('nominal title validation',()=>server.call('/notes/rename',{id:a1,title:'x'}),400,'invalid_request');
   await check('unicode scalar title',()=>server.call('/notes/rename',{id:a1,title:'😀😀😀'}),200,null,true);
   await check('unknown field',()=>server.call('/notes/rename',{id:a1,title:'Valid',extra:1}),400,'invalid_request');
   await check('revoked credential before malformed body',async()=>{await server.sql('UPDATE __jadpo_auth_sessions SET revoked=1 WHERE id=?',[sessionId('alice')]);return server.call('/notes/rename','{')},401,'invalid_credentials');
   const mutateSession=async change=>{const s=seed.sessions.find(s=>s.id===sessionId('alice'));await server.sql('UPDATE __jadpo_auth_sessions SET data=? WHERE id=?',[JSON.stringify({...JSON.parse(s.data),...change}),s.id]);};
   await check('expired session',async()=>{await mutateSession({expires:seed.now-1});return read()},401,'invalid_credentials');
   await check('unknown key',async()=>{await mutateSession({keyId:'unknown'});return read()},401,'invalid_credentials');
   await check('bad session verifier',async()=>{await mutateSession({verifier:Buffer.alloc(32).toString('base64url')});return read()},401,'invalid_credentials');
   await check('session absent',async()=>{await server.sql('DELETE FROM __jadpo_auth_sessions WHERE id=?',[sessionId('alice')]);return read()},401,'invalid_credentials');
   await check('disabled user before malformed body',async()=>{await server.sql('UPDATE user SET enabled=0 WHERE id=?',[seed.users.alice]);return server.call('/notes/rename','{')},422,'user_disabled');
   await check('removed authority',async()=>{await server.sql('DELETE FROM user WHERE id=?',[seed.users.alice]);return read()},401,'invalid_credentials');
   await check('authority identity changed',async()=>{await server.sql('UPDATE user SET id=? WHERE id=?',[absent,seed.users.alice]);return read()},503,'authentication_unavailable');
   await check('malformed authority',async()=>{await server.sql('UPDATE user SET enabled=7 WHERE id=?',[seed.users.alice]);return read()},503,'authentication_unavailable');
   await check('malformed stored session',async()=>{await server.sql('UPDATE __jadpo_auth_sessions SET data=? WHERE id=?',['{',sessionId('alice')]);return read()},503,'authentication_unavailable');
   await check('ownership fresh for old owner',async()=>{await server.sql('UPDATE note SET owner_id=? WHERE id=?',[seed.users.charlie,a1]);return read()},404,'note_missing',true);
   await check('ownership fresh for new owner',async()=>{await server.sql('UPDATE note SET owner_id=? WHERE id=?',[seed.users.charlie,a1]);return read('charlie')},200,null,true);
   await check('editor role removal fresh',async()=>{await server.sql('UPDATE note SET editor_id=? WHERE id=?',[seed.users.charlie,a1]);return read('bob')},404,'note_missing',true);
   await check('unauthorized cannot trigger uniqueness conflict',()=>server.call('/notes/rename',{id:c1,title:'Alice first'}),404,'note_missing');
   await check('trigger constraint normalization',async()=>{await server.sql("CREATE TRIGGER deny_second BEFORE UPDATE ON note WHEN OLD.id = '"+a2+"' BEGIN SELECT RAISE(ABORT,'denied'); END");try{return await server.call('/notes/pair',pair())}finally{await server.sql('DROP TRIGGER deny_second')}},409,'note_conflict');
   await check('SQL fault rolls back first write',async()=>{await server.sql("CREATE TRIGGER fail_second BEFORE UPDATE ON note WHEN OLD.id = '"+a2+"' BEGIN SELECT json('malformed-json'); END");try{return await server.call('/notes/pair',pair())}finally{await server.sql('DROP TRIGGER fail_second')}},500,'internal_fault');
   await check('oversized stored row prevents commit',async()=>{await server.sql("UPDATE note SET private_note=replace(hex(zeroblob(70000)), '00', 'x') WHERE id=?",[a1]);const result=await server.call('/notes/rename',{id:a1,title:'No commit'});assert.equal((await server.snapshot())[0].title,'Alice first');return result},500,'internal_fault',true);
   await check('unknown route',()=>server.call('/does-not-exist',{},null),404,'route_not_found');
   // Mixed principals share a server/connection; completed pairs must stay coherent.
   await server.reset();const concurrent=await Promise.all(Array.from({length:96},(_,i)=>{
    const user=i%2===0?'alice':'charlie';const own=user==='alice'?[a1,a2]:[c1,c2];const attack=i%3===0;
    return server.call('/notes/pair',{first:own[0],second:attack?(user==='alice'?c1:a1):own[1],title:`${user} first`,second_title:`${user} second`},user).then(result=>{assert.equal(result.status,attack?404:200);return {user,attack,result};});
   }));
   const after=await server.snapshot();for(const [index,expected] of ['alice first','alice second','charlie first','charlie second'].entries())assert.equal(after[index].title,expected);
   records.push({name:'96 mixed concurrent calls',concurrent,after});
  }
  results.push({target,journal,pragmas:server.info.pragmas,sqliteVersion:server.info.sqliteVersion,records});writeFileSync(out+'/results'+(variant?'-variant':smoke?'-smoke':'')+'.json',JSON.stringify(results,null,2)+'\n');
  console.log(`${target} ${journal}: ${records.length} checks passed`);
 }finally{await server.stop();}
}
// Compare stable public responses and committed application state, not target-specific SQL.
for(const journal of smoke?['WAL']:['WAL','DELETE']){
 const group=results.filter(r=>r.journal===journal),reference=group.find(r=>r.target==='bun');
 for(const result of group.filter(r=>r!==reference))for(let i=0;i<reference.records.length;i++){
  const expected=reference.records[i],actual=result.records[i];assert.equal(actual.name,expected.name);
  if(expected.result)assert.deepEqual(actual.result,expected.result,`${result.target} ${expected.name}: public parity`);
  assert.deepEqual(actual.after,expected.after,`${result.target} ${expected.name}: state parity`);
 }
}
console.log('All targets agree on public responses and committed application state.');
