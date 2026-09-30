// Mutation challenge of the A01 assertion only; no compiler or SQL claim.
import {runFullSuite} from '../../full/suite.ts';
const spec=await Bun.file(new URL('../../acceptance.json',import.meta.url)).json();
const program=await Bun.file(new URL('../../compiler/build/projected/program.json',import.meta.url)).json();
const expected=spec.full_slice_cases.find((c:any)=>c.id==='A01').expect;
const host={
 reset:async()=>{},
 snapshot:async()=>({Item:structuredClone(spec.seeds.Item)}),
 reopen:async()=>({Item:structuredClone(spec.seeds.Item)}),
 invoke:async()=>({output:{...structuredClone(expected),unexpected:'extra'},hostCalls:0,calls:[],sql:[],events:[]}),
};
const suite=await runFullSuite(host,spec,program);
const actual=suite.results.find((c:any)=>c.id==='A01');
const report={scope:'Only A01 closed-envelope assertion challenged; other synthetic-host outcomes not interpreted',pass:actual?.status==='fail'&&actual.reason==='closed output',actual};
await Bun.write(new URL('../build/review-closed-output-after.json',import.meta.url),JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify(report));
if(!report.pass)process.exitCode=1;
