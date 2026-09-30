import {launch,a} from './harness.mjs';
import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import os from 'node:os';
const results=[];
for(const target of ['native','bun']){
 const s=await launch(target);
 try{for(let i=0;i<20;i++)assert.deepEqual(await s.control('call',{operation:'Item.read',input:a.id}),{kind:'success',value:a});const stats=await s.control('stats'),rss=s.rss();assert(stats.maxRssBytes>rss*.8&&stats.maxRssBytes<rss*2);results.push({target,reportedPeakBytes:stats.maxRssBytes,externalRssBytes:rss,passed:true});}finally{await s.stop();}
}
const report={scope:'post-campaign memory-units-only correction; no performance scores replaced',machine:{cpus:os.cpus().map(x=>x.model),totalMemoryBytes:os.totalmem(),arch:os.arch(),release:os.release()},results};
writeFileSync('experiments/native-exp1/evidence/telemetry-fix.json',JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report));
