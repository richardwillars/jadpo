// Single-pass monitor: the trusted Rust core authenticates and validates the
// guest's checked effect sequence, while this adapter only runs the guest and
// supplies the existing SQLite driver. It never executes the application twice.
import {Guest} from '../capability-host/wasm-driver.ts';
import {sqliteHost} from '../capability-host/sqlite-host.ts';
import type {Database} from 'bun:sqlite';

type Reply=(capability:string,args:any)=>any;
export class MonitorHost {
 #monitor:Guest; #factory:()=>Guest; #sql:ReturnType<typeof sqliteHost>;
 #application:Guest|undefined; #pending:any; #scope=0; #started=false; #poisoned=false;
 constructor(monitor:Guest,factory:()=>Guest,db:Database){
  this.#monitor=monitor;this.#factory=factory;this.#sql=sqliteHost(db);
 }
 #deny():never{this.#poisoned=true;throw Error('invalid monitor protocol');}
 #reply(capability:string,args:any){
  if(this.#poisoned)this.#deny();
  if(capability==='guest.start'){
   const keys=Object.keys(args??{}).sort();
   if(this.#started||keys.join(',')!=='input,operation,scope'||!Number.isSafeInteger(args.scope)||args.scope<=0)this.#deny();
   this.#started=true;this.#scope=args.scope;this.#application=this.#factory();
   this.#pending=this.#application.start(args.operation,{scope:args.scope,input:args.input});
   return {kind:'success',value:this.#pending};
  }
  if(capability==='guest.resume'){
   const keys=Object.keys(args??{}).sort();
   if(!this.#started||!this.#application||keys.join(',')!=='operationId,requestId,result'||!this.#pending||this.#pending.kind!=='pending')this.#deny();
   if(args.requestId!==this.#pending.requestId||args.operationId!==this.#pending.operationId||args.requestId!==this.#scope)this.#deny();
   this.#pending=this.#application.resume(this.#pending,args.result);
   return {kind:'success',value:this.#pending};
  }
  if(capability.startsWith('guest.'))this.#deny();
  return this.#sql.reply(capability,args);
 }
 invoke(frame:any){
  this.#pending=undefined;this.#scope=0;this.#started=false;this.#poisoned=false;
  try{return this.#monitor.invoke(-1,frame,(capability,args)=>this.#reply(capability,args));}
  finally{try{this.#application?.cancel();}catch{}finally{this.#application=undefined;this.#sql.recover();}}
 }
}
