const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const capabilities=['','entity.read','entity.update','sql.query','sql.update','transaction.begin','transaction.commit','transaction.rollback','guest.start','guest.resume','guest.check','guest.complete'];
export class Guest {
 exports:any;
 constructor(bytes:Uint8Array|WebAssembly.Module){this.exports=new WebAssembly.Instance(bytes instanceof WebAssembly.Module?bytes:new WebAssembly.Module(bytes)).exports;}
 input(value:any){const bytes=encoder.encode(JSON.stringify(value));const ptr=this.exports.alloc(bytes.length);if(!ptr)throw Error('input budget');new Uint8Array(this.exports.memory.buffer,ptr,bytes.length).set(bytes);return bytes.length;}
 output(){
  if(typeof this.exports.result_status!=='function'){
   const length=this.exports.result_len();if(length>131072)throw Error('output budget');
   return JSON.parse(decoder.decode(new Uint8Array(this.exports.memory.buffer,this.exports.result_ptr(),length)));
  }
  const status=this.exports.result_status(),length=this.exports.result_len();
  if(length>131072)throw Error('output budget');
  const payload=JSON.parse(decoder.decode(new Uint8Array(this.exports.memory.buffer,this.exports.result_ptr(),length)));
  if(status===0)return {kind:'success',value:payload};
  if(status===1){if(typeof payload!=='string')throw Error('invalid domain frame');return {kind:'domain',failure:payload};}
  if(status===2){const capability=capabilities[this.exports.result_capability()];if(!capability)throw Error('invalid capability frame');return {kind:'pending',requestId:this.exports.result_request(),operationId:this.exports.result_operation(),capability,args:payload};}
  if(status===4)return {kind:'invalid'};
  return {kind:'internal'};
 }
 start(op:number,input:any){this.exports.start(op,this.input(input??null));return this.output();}
 resume(pending:any,result:any){let length:number;try{length=this.input(result);}catch{this.exports.cancel();return this.output();}this.exports.resume(pending.requestId,pending.operationId,length);return this.output();}
 cancel(){this.exports.cancel();return this.output();}
 drive(result:any,host:(cap:string,args:any)=>any){try{while(result.kind==='pending'){let reply;try{reply=host(result.capability,result.args);}catch{reply={kind:'internal'};}result=this.resume(result,reply);}return result;}catch{this.cancel();return {kind:'internal'};}}
 invoke(op:number,input:any,host:(cap:string,args:any)=>any){try{return this.drive(this.start(op,input),host);}catch{this.cancel();return {kind:'invalid'};}}
}
