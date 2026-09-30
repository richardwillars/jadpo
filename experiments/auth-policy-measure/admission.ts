// Trusted-application mode only. The reviewed lock must be supplied by the host,
// never by the request or module. Hash equality is not independent authorization.
import {createHash} from 'node:crypto';
import {Guest} from '../auth-policy/wasm-driver.ts';
export type ArtifactLock={wasmSha256:string,contractSha256:string};
export function admitGuest(moduleBytes:Uint8Array,contractBytes:Uint8Array,lock:ArtifactLock){
 // Copy once, verify and instantiate those same bytes; do not reopen a checked path.
 const bytes=Uint8Array.from(moduleBytes),contract=Uint8Array.from(contractBytes);
 const digest=(v:Uint8Array)=>createHash('sha256').update(v).digest('hex');
 if(digest(bytes)!==lock.wasmSha256||digest(contract)!==lock.contractSha256)throw Error('unapproved application artifact');
 return {guest:new Guest(bytes),contract:JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(contract))};
}
