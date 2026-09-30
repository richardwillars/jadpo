import type {Database} from 'bun:sqlite';
import {bunSqlite} from '../optimization/cached-sqlite.ts';

// Diagnostic only. Keep Bun's existing transaction implementation and durability.
// The intervals bracket the callback: begin/commit include native wrapper cost.
export function timedSqlite(db:Database) {
 const base=bunSqlite(db);
 type Sample={totalMs:number,beginMs:number,bodyMs:number,finishMs:number,sqlMs:number,rollback:boolean,calls:{verb:string,ms:number}[]};
 let samples:Sample[]=[],active:{verb:string,ms:number}[]|undefined;
 const adapter={...base,
  rows(sql:string,parameters:any[]){const start=performance.now();try{return base.rows(sql,parameters);}finally{active?.push({verb:sql.split(' ',1)[0],ms:performance.now()-start});}},
  transactionSync<T>(work:()=>T):T {
   if(active)throw Error('diagnostic does not support nested transactions');
   const calls:{verb:string,ms:number}[]=[];active=calls;
   const start=performance.now();let entered=start,finished=start,rollback=true;
   try {const result=base.transactionSync(()=>{entered=performance.now();try{return work();}finally{finished=performance.now();}});rollback=false;return result;}
   finally{const end=performance.now();samples.push({totalMs:end-start,beginMs:entered-start,bodyMs:finished-entered,finishMs:end-finished,sqlMs:calls.reduce((n,c)=>n+c.ms,0),rollback,calls});active=undefined;}
  },
 };
 return {adapter,reset(){samples=[];},snapshot(){return samples.slice();},summary(){
  const q=(xs:number[],p:number)=>xs.slice().sort((a,b)=>a-b)[Math.floor((xs.length-1)*p)]??null;
  const slow=samples.filter(s=>s.totalMs>=10);
  return {count:samples.length,stats:Object.fromEntries(['totalMs','beginMs','bodyMs','finishMs','sqlMs'].map(k=>[k,{p50:q(samples.map(s=>(s as any)[k]),.5),p95:q(samples.map(s=>(s as any)[k]),.95),p99:q(samples.map(s=>(s as any)[k]),.99),max:samples.length?Math.max(...samples.map(s=>(s as any)[k])):null}])),slowCount:slow.length,
   slowStageTotals:slow.reduce((a,s)=>({beginMs:a.beginMs+s.beginMs,bodyMs:a.bodyMs+s.bodyMs,finishMs:a.finishMs+s.finishMs,sqlMs:a.sqlMs+s.sqlMs}),{beginMs:0,bodyMs:0,finishMs:0,sqlMs:0}),
   slowest:samples.slice().sort((a,b)=>b.totalMs-a.totalMs).slice(0,12)};
 }};
}
