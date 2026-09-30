// Sequential continuation: no build/test work overlaps any measurement campaign.
import {spawn} from 'node:child_process';
import {openSync,closeSync} from 'node:fs';
for(const [script,args,name] of [['measure.mjs',['--delete'],'delete'],['measure.mjs',['--micro'],'micro'],['measure.mjs',['--startup'],'startup'],['boundaries.mjs',[],'boundaries'],['correctness.mjs',[],'correctness-final']]){
 const fd=openSync(`build/native-exp1/${name}.log`,'w');
 const child=spawn(process.execPath,[`experiments/native-exp1/${script}`,...args],{stdio:['ignore',fd,fd]});
 const code=await new Promise((resolve,reject)=>{child.once('error',reject);child.once('exit',resolve)});closeSync(fd);
 if(code!==0)throw Error(`${name} failed (${code}); see retained log`);console.log(`${name} completed`);
}
