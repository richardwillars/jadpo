// Local-only infrastructure test. Not generated application code or a deployment target.
import { DurableObject } from "cloudflare:workers";
import { createStorage } from "./adapter.ts";
import { cloudflareSqlite } from "./cloudflare-sqlite.ts";
import program from "../compiler/build/projected/program.json";
import acceptance from "../acceptance.json";
const equal=(a:unknown,b:unknown)=>{if(JSON.stringify(a)!==JSON.stringify(b))throw new Error(`Local storage assertion failed: ${JSON.stringify(a)}`);};
export class StorageSmoke extends DurableObject {
  run() {
    const events:unknown[]=[];
    const sql=cloudflareSqlite(this.ctx.storage);
    const storage=createStorage(program,{...sql,rows:(query,parameters)=>{events.push({query,parameters});return sql.rows(query,parameters);}});
    storage.setup();storage.resetFixture({Item:acceptance.seeds.Item});
    const owner={entity:"User",values:{id:acceptance.seeds.principals.owner.id}};
    const other={entity:"User",values:{id:acceptance.seeds.principals.other.id}};
    const row=acceptance.seeds.Item[0];
    const args=(operation="Item.read",changes?:Record<string,unknown>)=>({entity:"Item",operation,semanticOperationId:program.declarations.find(d=>d.name===operation)!.semanticId,freshness:"authoritative",policy:storage.descriptor(operation),predicate:{field:"id",operator:"equal",value:row.id},...(changes===undefined?{}:{changes})});
    equal(storage.read(args(),owner),{kind:"success",value:row});
    equal(storage.read(args(),other),{kind:"success",value:null});
    equal(storage.runAtomic(()=>{storage.update(args("Item.rename",{title:"rolled-back"}),owner);return {kind:"domain"};}),{kind:"domain"});
    equal(storage.snapshot().Item,acceptance.seeds.Item);
    equal(storage.runAtomic(()=>storage.update(args("Item.rename",{title:"bravo"}),owner)),{kind:"success",value:{status:"conflict"}});
    equal(storage.snapshot().Item,acceptance.seeds.Item);
    const changed=storage.runAtomic(()=>storage.update(args("Item.change",{title:"committed",note:null}),owner));
    equal(changed,{kind:"success",value:{status:"found",row:{...row,title:"committed",note:null}}});
    return {status:"pass",checks:7,events};
  }
  persisted() {
    const storage=createStorage(program,cloudflareSqlite(this.ctx.storage));
    return storage.snapshot();
  }
}
export default {
  async fetch(_request:Request,env:Env) {
    const stub=env.STORAGE_SMOKE.getByName("local-storage-smoke");
    const result=await stub.run();
    const persisted=await stub.persisted();
    equal(persisted.Item[0].title,"committed");equal(persisted.Item[0].note,null);
    return Response.json({...result,checks:result.checks+2,persisted});
  },
};
