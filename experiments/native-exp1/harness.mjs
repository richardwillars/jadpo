import {spawn,execFileSync} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {mkdirSync,readFileSync} from 'node:fs';
import {Agent,request} from 'node:http';
import {resolve} from 'node:path';
export const base=resolve('experiments/native-exp1'),out=resolve('build/native-exp1');mkdirSync(out,{recursive:true});
export const spec=JSON.parse(readFileSync('experiments/wasm-exp1/acceptance.json','utf8'));
export const [a,b,c]=spec.seeds.Item;
export const q=(xs,p)=>xs[Math.floor((xs.length-1)*p)];
let serial=0;
export async function launch(target,journal='WAL',trace=false,dbPath=null){
 const token=randomUUID(),db=dbPath??`${out}/${target}-${process.pid}-${serial++}.sqlite`,started=performance.now();
 const command=target==='native'?[`${out}/target/release/jadpo-native-exp1`,db,token,journal]:['bun','--no-install','--env-file=/dev/null',`${base}/bun-server.ts`,db,token,journal];
 const proc=spawn(command[0],command.slice(1),{stdio:['ignore','pipe','pipe'],env:{...process.env,DATABASE_URL:'',NATIVE_TRACE:trace?'1':'0'}});let stderr='',stdout='';proc.stderr.on('data',b=>stderr+=b);
 const exit=new Promise((res,rej)=>{proc.once('error',rej);proc.once('exit',res)});
 let info;
 try{info=await new Promise((res,rej)=>{const timer=setTimeout(()=>rej(Error('ready timeout '+stderr)),20000);proc.stdout.on('data',b=>{stdout+=b;if(stdout.includes('\n')){clearTimeout(timer);try{res(JSON.parse(stdout.split('\n')[0]));}catch(e){rej(e)}}});exit.then(code=>{clearTimeout(timer);rej(Error(`early exit ${code} ${stderr}`))},rej);});}catch(e){proc.kill();throw e;}
 const readyMs=performance.now()-started;
 const agent=new Agent({keepAlive:true,maxSockets:16});
 function send(path,body={},raw=false){return new Promise((res,rej)=>{
  const bytes=raw?body:JSON.stringify(body);const r=request(info.url+path,{method:'POST',agent,headers:{'content-type':'application/json','content-length':Buffer.byteLength(bytes),'x-experiment-token':token}},response=>{const chunks=[];response.on('data',b=>chunks.push(b));response.on('error',rej);response.on('end',()=>{try{res({status:response.statusCode,body:JSON.parse(Buffer.concat(chunks).toString())})}catch(e){rej(e)}})});r.on('error',rej);r.setTimeout(30000,()=>r.destroy(Error('request timeout')));r.end(bytes);
 });}
 return {info,readyMs,started,db,pid:proc.pid,stderr:()=>stderr,send,control:async(p,b={})=>(await send(p,b)).body,rss:()=>Number(execFileSync('ps',['-o','rss=','-p',String(proc.pid)],{encoding:'utf8'}).trim())*1024,stop:async()=>{agent.destroy();proc.kill('SIGTERM');await exit;}};
}
