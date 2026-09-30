// One bounded, sequential conformance campaign; raw logs stay separate from exp1.
import {spawn} from 'node:child_process';
import {openSync,closeSync} from 'node:fs';
for(const name of ['correctness','boundaries','concurrency']){
 const fd=openSync(`build/native-conformance/${name}.log`,'w');const p=spawn(process.execPath,[`experiments/native-conformance/${name}.mjs`],{stdio:['ignore',fd,fd]});
 const code=await new Promise((resolve,reject)=>{p.once('error',reject);p.once('exit',resolve)});closeSync(fd);if(code!==0)throw Error(name+' failed; inspect retained log');console.log(name+' passed');
}
