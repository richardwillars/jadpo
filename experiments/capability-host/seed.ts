// Test-only issuance through the existing generated first-party authentication host.
import {Database} from 'bun:sqlite';
import {mkdirSync,writeFileSync,rmSync} from 'node:fs';
import {resolve} from 'node:path';
const out=resolve('build/capability-host');mkdirSync(out,{recursive:true});
const path=out+'/seed.sqlite';for(const suffix of ['', '-wal','-shm'])rmSync(path+suffix,{force:true});
delete Bun.env.DATABASE_URL;Bun.env.SQLITE_PATH=path;
const configuration={AUTH_SIGNING_KEY:Buffer.alloc(32,71).toString('base64url'),AUTH_PREVIOUS_SIGNING_KEY:Buffer.alloc(32,72).toString('base64url')};
Object.assign(Bun.env,configuration);
const app=await import('./build/projected/bun/target/app.ts');
const db=new Database(path,{strict:true});
const id=(n:number)=>`00000000-0000-4000-8000-${n.toString().padStart(12,'0')}`;
const users={alice:id(1),bob:id(2),charlie:id(3)};
for(const [subject,id] of Object.entries(users))db.prepare('INSERT INTO user(id,authentication_subject,enabled) VALUES(?,?,1)').run(id,subject);
const notes=[{id:id(11),owner_id:users.alice,editor_id:users.bob,title:'Alice first',private_note:'ALICE_PRIVATE_1'},
 {id:id(12),owner_id:users.alice,editor_id:users.bob,title:'Alice second',private_note:'ALICE_PRIVATE_2'},
 {id:id(21),owner_id:users.charlie,editor_id:users.charlie,title:'Charlie first',private_note:'CHARLIE_PRIVATE'},
 {id:id(22),owner_id:users.charlie,editor_id:users.charlie,title:'Charlie second',private_note:null}];
for(const row of notes)db.prepare('INSERT INTO note(id,owner_id,editor_id,title,private_note) VALUES(?,?,?,?,?)').run(row.id,row.owner_id,row.editor_id,row.title,row.private_note);
await app.initializeApplication(configuration);
const credentials:any={};const now=Date.now();
for(const subject of Object.keys(users))credentials[subject]=(await app.authenticationHost().issue('api_bearer',subject,now+3600000,now)).credential;
await app.initializeApplication({AUTH_SIGNING_KEY:configuration.AUTH_PREVIOUS_SIGNING_KEY,AUTH_PREVIOUS_SIGNING_KEY:configuration.AUTH_SIGNING_KEY});
credentials.previous=(await app.authenticationHost().issue('api_bearer','alice',now+3600000,now)).credential;
const sessions=db.prepare('SELECT * FROM __jadpo_auth_sessions ORDER BY id').all();
writeFileSync(out+'/seed.json',JSON.stringify({configuration,credentials,users,notes,sessions,now},null,2)+'\n');
db.exec('PRAGMA wal_checkpoint(TRUNCATE)');db.close();
console.log('Created disposable SQLite fixture and synthetic opaque credentials through generated Bun authentication');
