import type {Database} from 'bun:sqlite';
// SQL/transaction driver only. The guest owns credential checks and policy predicates.
export function sqliteHost(db:Database){
 const reply=(cap:string,args:any)=>{
  try{
   if(cap==='transaction.begin'){db.exec('BEGIN IMMEDIATE');return {kind:'success',value:null};}
   if(cap==='transaction.commit'){db.exec('COMMIT');return {kind:'success',value:null};}
   if(cap==='transaction.rollback'){db.exec('ROLLBACK');return {kind:'success',value:null};}
   if(!['sql.query','sql.update'].includes(cap)||cap==='sql.update'&&!db.inTransaction)return {kind:'internal'};
   try{return {kind:'success',value:db.prepare(args.sql).all(...args.params)};}
   catch(error:any){return {kind:'success',value:cap==='sql.update'&&String(error.code).startsWith('SQLITE_CONSTRAINT')?{constraint:true}:{error:'database'}};}
  }catch{return {kind:'internal'};}
 };
 return {reply,recover(){if(db.inTransaction)db.exec('ROLLBACK');}};
}
