// Trusted adapter for frozen synthetic full-slice cases. No authored control flow.
import { createStorage, type SqlAdapter } from './adapter.ts';
import { invokeSync } from '../optimization/driver.ts';
type J=any;
export function createSliceHost(module:WebAssembly.Module,program:J,acceptance:J,sql:SqlAdapter,
  options:{reopen:()=>Promise<J>;trapModule?:WebAssembly.Module;emitEvent?:(event:J)=>void}) {
  let current:J=null;
  const traced:SqlAdapter={...sql,rows:(query,parameters)=>{current?.sql.push({sql:query,parameters:[...parameters]});return sql.rows(query,parameters);}};
  const storage=createStorage(program,traced);storage.setup();
  const seedRows=Object.fromEntries(program.entities.filter((e:J)=>e.persistent).map((e:J)=>[e.name,acceptance.seeds[e.name]]));
  let request=0;
  return {
    reset:async()=>{storage.resetFixture(seedRows);},snapshot:async()=>storage.snapshot(),reopen:options.reopen,
    async invoke(operation:string,input:J,principalName:string,control:J={}) {
      // Any delay is before entering the atomic authority. No await occurs within.
      if(control.delay_ms)await new Promise<void>(r=>setTimeout(r,control.delay_ms));
      const decl=program.declarations.find((d:J)=>d.kind==='callable'&&d.name===operation);
      if(!decl)throw new Error('Unknown fixture operation');
      const fixture=acceptance.seeds.principals[principalName];
      const entity=program.entities.find((e:J)=>e.name===fixture?.entity);
      if(!entity)throw new Error('Unknown trusted fixture principal');
      const principal={entity:entity.name,values:{[entity.identity]:fixture.id,...(control.privateContext===undefined?{}:{private_experiment_context:control.privateContext})}};
      const requestId=`slice-${++request}`;
      const trace:J={requestId,transaction:requestId,hostCalls:0,calls:[],sql:[],events:[]};
      current=trace;
      try {
        if(control.trap&&!options.trapModule)throw new Error('Dedicated ABI trap fixture missing');
        const selected=control.trap?options.trapModule!:module;
        const wireInput=decl.parameters.length===1&&Array.isArray(input)&&input.length===1?input[0]:input;
        const execute=()=>invokeSync(selected,decl.semanticId,wireInput,(capability,args)=>{
          trace.hostCalls++;const call:J={sequence:trace.hostCalls,transaction:trace.transaction,capability,semanticOperationId:(args as J)?.semanticOperationId,operation:(args as J)?.operation,completed:false};trace.calls.push(call);
          if(control.fault){call.failed=true;throw new Error('WASM_EXP1_SECRET_SENTINEL');}
          let result;
          if(capability==='storage.read')result=storage.read(args,principal);
          else if(capability==='storage.update')result=storage.update(args,principal);
          else throw new Error('Unsupported capability');
          call.completed=true;return Object.hasOwn(control,'result')?structuredClone(control.result):result;
        });
        const output=storage.runAtomic(execute);
        if(output.kind==='internal'){
          const operationId=Number(output.operation??decl.semanticId);
          const mapping=program.declarations.find((d:J)=>d.semanticId===operationId);
          const event={kind:'internal',semanticOperationId:operationId,checkedRevision:program.checkedRevision,span:mapping?.span??null,mappingOrigin:output.operation===undefined?'entrypoint_fallback':'core'};
          trace.events.push(event);options.emitEvent?.(event);
        }
        return {...trace,output};
      } finally {current=null;}
    },
  };
}
