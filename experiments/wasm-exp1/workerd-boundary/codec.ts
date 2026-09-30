export type Metadata={version:number;digest:string;records:{id:number;name:string;fields:string[]}[];entries:Record<string,number>;readPlans?:{id:number;schema:number;jsonBase:number;args:any}[]};
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true,ignoreBOM:true});
const LIMIT=65536;
export function rowCodec(meta:Metadata){
 if(meta.version!==1||! /^[0-9a-f]{64}$/.test(meta.digest))throw Error('schema');
 const digest=Uint8Array.from(meta.digest.match(/../g)!.map(x=>parseInt(x,16)));
 const records=new Map(meta.records.map(r=>[r.id,{...r,keys:new Set(r.fields),keyBytes:new Map(r.fields.map(k=>[k,encoder.encode(JSON.stringify(k)).length]))}]));
 const names=new Map(meta.records.map(r=>[r.name,r.id]));
 // Clone then freeze compiler-owned descriptors; callers cannot mutate a plan.
 const freeze=(value:any):any=>{if(value&&typeof value==='object'){for(const v of Object.values(value))freeze(v);Object.freeze(value);}return value;};
 const plans=new Map((meta.readPlans??[]).map(p=>[p.id,freeze(structuredClone(p))]));
 const buffer=new Uint8Array(LIMIT),view=new DataView(buffer.buffer);
 return {
  pending(bytes:Uint8Array):any {
   if(bytes.length<49||bytes.length>LIMIT||bytes[0]!==74||bytes[1]!==82||bytes[2]!==80||bytes[3]!==49||digest.some((v,i)=>bytes[4+i]!==v))throw Error('pending-header');
   const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
   const requestId=view.getUint32(36,true),operationId=view.getUint32(40,true),id=view.getUint32(44,true),plan=plans.get(id);
   if(!requestId||requestId>2147483647||!operationId||operationId>64||!plan||!records.has(plan.schema))throw Error('pending-identity');
   const value=JSON.parse(decoder.decode(bytes.subarray(48)));
   // The canonical reconstructed envelope must respect the same JSON byte bound.
   if(plan.jsonBase-4+encoder.encode(JSON.stringify(value)).length+String(requestId).length-1+String(operationId).length-1>LIMIT)throw Error('pending-size');
   return {kind:'pending',requestId,operationId,capability:'storage.read',args:{...plan.args,predicate:{...plan.args.predicate,value}}};
  },
  snapshot(id:number,row:any):any {
   const schema=records.get(id);
   if(!schema||!row||typeof row!=='object'||Array.isArray(row))throw Error('row-shape');
   // Spread reads accessors once into a private object with ordinary data fields.
   // Only immutable scalar values are eligible; no dispatch-owned alias survives.
   const owned:any={...row};
   for(const key of Object.keys(owned)){
    if(!schema.keys.has(key))throw Error('row-shape');
    const value=owned[key];
    if(value!==null&&typeof value!=='string'&&typeof value!=='boolean'&&!(typeof value==='number'&&Number.isSafeInteger(value)))throw Error('row-field');
    if(Object.is(value,-0))owned[key]=0;
   }
   return owned; // Private to the invocation; only a fresh copy is exposed.
  },
  reference(expected:number|undefined,bytes:Uint8Array,rows:Map<string,{schema:number,row:any}>):any {
   if(bytes.length!==48||bytes[0]!==74||bytes[1]!==82||bytes[2]!==82||bytes[3]!==49||digest.some((v,i)=>bytes[4+i]!==v))throw Error('reference-header');
   const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),schema=view.getUint32(36,true),request=view.getUint32(40,true),operation=view.getUint32(44,true);
   const found=rows.get(request+':'+operation);
   if(!records.has(schema)||schema!==expected||!found||found.schema!==schema||request===0||operation===0)throw Error('reference-identity');
   // Fresh ordinary object, immutable scalar values; no alias to the dispatch row.
   return {...found.row};
  },
  schema:(name:unknown)=>typeof name==='string'?names.get(name):undefined,
  resultSchema:(entry:number)=>meta.entries[String(entry)],
  encode(id:number,row:any):Uint8Array{
   const schema=records.get(id);if(!schema||!row||typeof row!=='object'||Array.isArray(row)||Object.keys(row).some(k=>!schema.keys.has(k)))throw Error('row-shape');
   // Derive the exact original JSON envelope budget while encoding. A native
   // regexp tests for escapes; only escaped strings need JSON spelling. UTF-8
   // byte counts come from encodeInto, so no whole-row JSON/UTF-8 buffer exists.
   let jsonBound=29,present=0;
   buffer.set([74,82,87,49]);buffer.set(digest,4);view.setUint32(36,id,true);let pos=40;
   const room=(n:number)=>{if(pos+n>LIMIT)throw Error('row-size');};
   for(const key of schema.fields){
    room(1);if(!Object.hasOwn(row,key)){buffer[pos++]=0;continue;}
    const value=row[key];
    jsonBound+=schema.keyBytes.get(key)!+1+Number(present++>0);
    if(value===null){jsonBound+=4;buffer[pos++]=1;continue;}
    if(typeof value==='string'){
     // UTF-8 replacement must not silently turn invalid surrogate input into data.
     if(!value.isWellFormed())throw Error('row-utf16');
     room(5);buffer[pos++]=2;const lengthOffset=pos;pos+=4;
     const {read,written}=encoder.encodeInto(value,buffer.subarray(pos));
     if(read!==value.length)throw Error('row-size');view.setUint32(lengthOffset,written,true);pos+=written;
     jsonBound+=written+2+(/[\"\\\x00-\x1f]/.test(value)?JSON.stringify(value).length-value.length-2:0);
    }else if(typeof value==='boolean'){buffer[pos++]=value?4:3;jsonBound+=value?4:5;}
    else if(typeof value==='number'&&Number.isSafeInteger(value)){room(9);buffer[pos++]=5;view.setBigInt64(pos,BigInt(value),true);pos+=8;jsonBound+=String(value).length;}
    else throw Error('row-field');
   }
   if(jsonBound>LIMIT)throw Error('row-json-size');
   // Borrowed only until the caller copies to guest memory; never held over await.
   return buffer.subarray(0,pos);
  },
  decode(expected:number|undefined,bytes:Uint8Array):any{
   if(bytes.length<40||bytes.length>LIMIT||bytes[0]!==74||bytes[1]!==82||bytes[2]!==87||bytes[3]!==49||digest.some((v,i)=>bytes[4+i]!==v))throw Error('row-header');
   const dv=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),id=dv.getUint32(36,true);
   const schema=records.get(id);if(id!==expected||!schema)throw Error('row-schema');
   let pos=40;const row:any={};const room=(n:number)=>{if(pos+n>bytes.length)throw Error('row-truncated');};
   for(const key of schema.fields){
    room(1);const tag=bytes[pos++];let value:any;
    if(tag===0)continue;
    if(tag===1)value=null;
    else if(tag===3||tag===4)value=tag===4;
    else if(tag===2){room(4);const n=dv.getUint32(pos,true);pos+=4;room(n);value=decoder.decode(bytes.subarray(pos,pos+n));pos+=n;}
    else if(tag===5){room(8);value=Number(dv.getBigInt64(pos,true));pos+=8;if(!Number.isSafeInteger(value))throw Error('row-integer');}
    else throw Error('row-tag');
    Object.defineProperty(row,key,{value,enumerable:true,writable:true,configurable:true});
   }
   if(pos!==bytes.length)throw Error('row-trailing');return row;
  }
 };
}
