export type Metadata={version:number;digest:string;records:{id:number;name:string;fields:string[]}[];entries:Record<string,number>};
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true,ignoreBOM:true});
const LIMIT=65536;
export function rowCodec(meta:Metadata){
 if(meta.version!==1||! /^[0-9a-f]{64}$/.test(meta.digest))throw Error('schema');
 const digest=Uint8Array.from(meta.digest.match(/../g)!.map(x=>parseInt(x,16)));
 const records=new Map(meta.records.map(r=>[r.id,{...r,keys:new Set(r.fields)}]));
 const names=new Map(meta.records.map(r=>[r.name,r.id]));
 const buffer=new Uint8Array(LIMIT),view=new DataView(buffer.buffer);
 return {
  schema:(name:unknown)=>typeof name==='string'?names.get(name):undefined,
  resultSchema:(entry:number)=>meta.entries[String(entry)],
  encode(id:number,row:any):Uint8Array{
   const schema=records.get(id);if(!schema||!row||typeof row!=='object'||Array.isArray(row)||Object.keys(row).some(k=>!schema.keys.has(k)))throw Error('row-shape');
   // Preserve the previous JSON envelope budget, even when binary fits more data.
   if(encoder.encode(JSON.stringify({kind:'success',value:row})).length>LIMIT)throw Error('row-json-size');
   buffer.set([74,82,87,49]);buffer.set(digest,4);view.setUint32(36,id,true);let pos=40;
   const room=(n:number)=>{if(pos+n>LIMIT)throw Error('row-size');};
   for(const key of schema.fields){
    room(1);if(!Object.hasOwn(row,key)){buffer[pos++]=0;continue;}
    const value=row[key];
    if(value===null){buffer[pos++]=1;continue;}
    if(typeof value==='string'){
     // UTF-8 replacement must not silently turn invalid surrogate input into data.
     if(!value.isWellFormed())throw Error('row-utf16');
     room(5);buffer[pos++]=2;const lengthOffset=pos;pos+=4;
     const {read,written}=encoder.encodeInto(value,buffer.subarray(pos));
     if(read!==value.length)throw Error('row-size');view.setUint32(lengthOffset,written,true);pos+=written;
    }else if(typeof value==='boolean')buffer[pos++]=value?4:3;
    else if(typeof value==='number'&&Number.isSafeInteger(value)){room(9);buffer[pos++]=5;view.setBigInt64(pos,BigInt(value),true);pos+=8;}
    else throw Error('row-field');
   }
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
