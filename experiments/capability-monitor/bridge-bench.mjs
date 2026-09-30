import {readFileSync} from 'node:fs';
import {performance} from 'node:perf_hooks';
import {Guest} from './monitor-driver.ts';

const seed=JSON.parse(readFileSync('build/capability-host/seed.json','utf8'));
const id=seed.notes[0].id,row=seed.notes[0];
const current=readFileSync(new URL('./application.wasm',import.meta.url));
const previousPath=process.env.PREVIOUS_WASM??'/tmp/jadpo-monitor-previous-application.wasm';
const previous=readFileSync(previousPath);
const iterations=Number(process.env.ITERATIONS??20000);

function run(bytes){
 const guest=new Guest(bytes);let frames=0,bytesRead=0,outputBytes=0,done=0;const started=performance.now();
 for(let n=1;n<=iterations;n++){
  let step=guest.start(46,{scope:n,input:{id}});
  outputBytes+=guest.exports.result_len();
  while(step.kind==='pending'){
   frames++;bytesRead+=Buffer.byteLength(JSON.stringify(step.args));
   step=guest.resume(step,{kind:'success',value:row});
   outputBytes+=guest.exports.result_len();
  }
  if(step.kind!=='success'||step.value.id!==id)throw Error('unexpected guest result');
  done++;
 }
 return {iterations:done,frames,bridgeArgBytes:bytesRead,bridgeOutputBytes:outputBytes,elapsedMs:performance.now()-started};
}
console.log(JSON.stringify({iterations,previous:run(previous),current:run(current)}));
