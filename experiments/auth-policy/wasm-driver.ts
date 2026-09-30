import {readFileSync} from 'node:fs';
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
export class Guest {
 exports:any;
 constructor(bytes:Uint8Array){this.exports=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;}
 input(value:any){const bytes=encoder.encode(JSON.stringify(value));const ptr=this.exports.alloc(bytes.length);if(!ptr)throw Error('input budget');new Uint8Array(this.exports.memory.buffer,ptr,bytes.length).set(bytes);return bytes.length;}
 output(){const length=this.exports.result_len();if(length>65536)throw Error('output budget');return JSON.parse(decoder.decode(new Uint8Array(this.exports.memory.buffer,this.exports.result_ptr(),length)));}
 start(op:number,input:any){this.exports.start(op,this.input(input??null));return this.output();}
 resume(pending:any,result:any){let length:number;try{length=this.input(result);}catch{this.exports.cancel();return this.output();}this.exports.resume(pending.requestId,pending.operationId,length);return this.output();}
 cancel(){this.exports.cancel();return this.output();}
 drive(result:any,host:(cap:string,args:any)=>any){try{while(result.kind==='pending'){let reply;try{reply=host(result.capability,result.args);}catch{reply={kind:'internal'};}result=this.resume(result,reply);}return result;}catch{this.cancel();return {kind:'internal'};}}
 invoke(op:number,input:any,host:(cap:string,args:any)=>any){try{return this.drive(this.start(op,input),host);}catch{this.cancel();return {kind:'invalid'};}}
}
export function loadGuest(path=import.meta.dir+'/build/application.wasm'){return new Guest(readFileSync(path));}
