// Exercise the selected sampled mode in the inherited adversarial suites.
import {configureRows as configure} from './hybrid-driver.ts';
import type {Metadata} from './codec.ts';
export {invoke,invokeSync,poolStats,boundedEncode,BUFFER_LIMIT,MEMORY_LIMIT} from './hybrid-driver.ts';
export function configureRows(module:WebAssembly.Module,meta:Metadata,flags=3){configure(module,meta,flags,true,true);}
