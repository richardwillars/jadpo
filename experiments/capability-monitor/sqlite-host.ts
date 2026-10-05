import type {Database} from 'bun:sqlite';

// Monitor-local SQLite adapter. The capability-host adapter remains the frozen
// reference path; this variant optionally reuses prepared statements so the
// monitor experiment can measure adapter overhead without changing semantics.
export function sqliteHost(db:Database,cacheStatements=false){
 const statements=new Map<string,any>();
 const prepare=(sql:string)=>{
  if(!cacheStatements)return db.prepare(sql);
  const existing=statements.get(sql);
  if(existing)return existing;
  const created=db.prepare(sql);
  statements.set(sql,created);
  return created;
 };
 const reply=(cap:string,args:any)=>{
  try{
   if(cap==='transaction.begin'){db.exec('BEGIN IMMEDIATE');return {kind:'success',value:null};}
   if(cap==='transaction.commit'){db.exec('COMMIT');return {kind:'success',value:null};}
   if(cap==='transaction.rollback'){db.exec('ROLLBACK');return {kind:'success',value:null};}
   if(!['sql.query','sql.update'].includes(cap)||cap==='sql.update'&&!db.inTransaction)return {kind:'internal'};
   try{return {kind:'success',value:prepare(args.sql).all(...args.params)};}
   catch(error:any){return {kind:'success',value:cap==='sql.update'&&String(error.code).startsWith('SQLITE_CONSTRAINT')?{constraint:true}:{error:'database'}};}
  }catch{return {kind:'internal'};}
 };
 return {reply,recover(){if(db.inTransaction)db.exec('ROLLBACK');}};
}
