import {environment,originalRows} from './environment.ts';
const [target,path,token]=Bun.argv.slice(2);if(!token)throw Error('token');
const env=await environment(target,path);env.reset();
const server=Bun.serve({hostname:'127.0.0.1',port:0,async fetch(req){
 if(req.headers.get('x-token')!==token)return new Response('',{status:404});
 if(new URL(req.url).pathname==='/stop'){setTimeout(()=>{server.stop(true);env.close();process.exit(0);},5);return Response.json({ok:true});}
 return Response.json(await env.call('Item.read',originalRows[0].id));
}});
console.log(JSON.stringify({url:server.url.href}));
