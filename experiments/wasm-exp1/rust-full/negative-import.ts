import wabtFactory from '../tooling/node_modules/wabt/index.js';
const wabt=await wabtFactory();
const wat='(module (import "forbidden" "effect" (func)))';
const parsed=wabt.parseWat('unapproved-import.wat',wat);parsed.validate();
const bytes=parsed.toBinary({canonicalize_lebs:true}).buffer;parsed.destroy();
const path=new URL('build/negative/unapproved-import.wasm',import.meta.url).pathname;
await Bun.write(path,bytes);
const command=['bun','--no-install','--env-file=/dev/null','experiments/wasm-exp1/full/run-local.ts',path];
const child=Bun.spawn(command,{stdout:'pipe',stderr:'pipe'});
const [stdout,stderr,exitCode]=await Promise.all([new Response(child.stdout).text(),new Response(child.stderr).text(),child.exited]);
const pass=exitCode!==0&&stderr.includes('Unapproved application host import');
const result={case:'unapproved-host-import',pass,scope:'Infrastructure negative Wasm fixture, not a compiler-generated application.',command,exitCode,stdout,stderr,
  imports:WebAssembly.Module.imports(new WebAssembly.Module(bytes)),sha256:new Bun.CryptoHasher('sha256').update(bytes).digest('hex')};
await Bun.write(new URL('build/negative/import-result.json',import.meta.url),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({case:result.case,pass,exitCode}));
if(!pass)process.exitCode=1;
