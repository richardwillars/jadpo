// Trusted coordinator. The application instance never receives this object's
// SQL closure, authority instance, configuration or HTTP credential envelope.
import {isDeepStrictEqual} from 'node:util';
import {createHash} from 'node:crypto';
import {Guest} from './wasm-driver.ts';
import {sqliteHost} from './sqlite-host.ts';
import type {Database} from 'bun:sqlite';
export type ApplicationGuest=Pick<Guest,'start'|'resume'|'cancel'>;
export function authorityGuest(bytes:Uint8Array,sha256:string){
 const copy=Uint8Array.from(bytes);
 if(createHash('sha256').update(copy).digest('hex')!==sha256)throw Error('unapproved authority artifact');
 return new Guest(copy);
}
export class AuthorityHost {
 #authority:Guest; #application:ApplicationGuest|undefined; #factory:()=>ApplicationGuest; #scope=0; #sql:ReturnType<typeof sqliteHost>;
 #step:any; #started=false; #checked=false; #finished=false; #poisoned=false;
 #request:number|undefined; #next=1; #lastRequest=0;
 constructor(authority:Guest,factory:()=>ApplicationGuest,db:Database){
  this.#authority=authority;this.#factory=factory;this.#sql=sqliteHost(db);
 }
 #deny(){this.#poisoned=true;throw Error('invalid application capability protocol');}
 #reply(cap:string,args:any){
  // Only the trusted authority's pending effects enter here. No application
  // effect is ever passed to sqliteHost, including one named sql.query.
  if(cap.startsWith('guest.')){
   if(this.#poisoned)this.#deny();
   if(cap==='guest.start'){
    if(this.#started)this.#deny();this.#started=true;
    this.#scope=args.scope;this.#application=this.#factory();
    this.#step=this.#application.start(args.operation,{scope:args.scope,input:args.input});
   }else if(cap==='guest.check'){
    const p=this.#step;
    if(!this.#started||this.#finished||this.#checked||p?.kind!=='pending')this.#deny();
    if(!Number.isSafeInteger(p.requestId)||p.requestId<=0||!Number.isSafeInteger(p.operationId)||p.operationId!==this.#next)this.#deny();
    if(p.requestId!==this.#scope)this.#deny();
    if(this.#request===undefined){if(p.requestId<=this.#lastRequest)this.#deny();this.#request=p.requestId;this.#lastRequest=p.requestId;}
    if(p.requestId!==this.#request||p.requestId!==this.#scope||!isDeepStrictEqual(Object.keys(p).sort(),['args','capability','kind','operationId','requestId']))this.#deny();
    if(!isDeepStrictEqual({capability:p.capability,args:p.args},args))this.#deny();
    this.#checked=true;
   }else if(cap==='guest.resume'){
    if(!this.#checked||this.#finished)this.#deny();
    this.#checked=false;this.#next++;
    this.#step=this.#application!.resume(this.#step,args);
   }else if(cap==='guest.complete'){
    if(!this.#started||this.#finished||this.#checked||!isDeepStrictEqual(this.#step,args))this.#deny();
    this.#finished=true;
   }else this.#deny();
   return {kind:'success',value:null};
  }
  if(cap==='transaction.commit'&&(!this.#finished||this.#poisoned))this.#deny();
  return this.#sql.reply(cap,args);
 }
 invoke(frame:any){
  this.#step=undefined;this.#request=undefined;this.#next=1;
  this.#started=this.#checked=this.#finished=this.#poisoned=false;
  try{return this.#authority.invoke(-1,frame,(cap,args)=>this.#reply(cap,args));}
  finally{try{this.#application?.cancel();}catch{}finally{this.#sql.recover();this.#step=undefined;this.#application=undefined;}}
 }
}
