"""Build a diagnostic-only module; never replace the measured application module."""
from pathlib import Path
import shutil,subprocess,os
root=Path(__file__).resolve().parents[3];source=Path(__file__).resolve().parent/'compiler'
out=root/'build/wasm-exp1/row-transport/validation';out.mkdir(parents=True,exist_ok=True)
manifest=(source/'Cargo.toml').read_text().replace('path = "build/generated.rs"','path = "generated.rs"')
(out/'Cargo.toml').write_text(manifest);shutil.copyfile(source/'Cargo.lock',out/'Cargo.lock')
hooks='''
static mut VALIDATION_SAMPLE: Option<Value> = None;
static mut VALIDATION_KIND: i32 = 0;
#[no_mangle]
pub extern "C" fn validation_load(kind:i32,p:i32,n:i32)->i32 {
    match decode(p,n) {Ok(value)=>{unsafe {VALIDATION_SAMPLE=Some(value);VALIDATION_KIND=kind;} 1},Err(_)=>0}
}
#[no_mangle]
pub extern "C" fn validation_run(repetitions:u32)->u32 {
    let name=match unsafe{VALIDATION_KIND} {0=>"Item",1=>"ItemTitle",2=>"User",_=>return 0};
    let mut accepted=0;
    for _ in 0..repetitions {
        if validate(name,std::hint::black_box(unsafe{VALIDATION_SAMPLE.as_ref().unwrap()})){accepted+=1;}
    }
    accepted
}
'''
(out/'generated.rs').write_text((source/'build/generated.rs').read_text()+hooks)
flags='-C target-feature=-simd128,-atomics,-bulk-memory,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144'
subprocess.run(['cargo','build','--offline','--locked','--release','--target','wasm32-unknown-unknown','--manifest-path',str(out/'Cargo.toml'),'--target-dir',str(out/'target')],cwd=root,env={**os.environ,'RUSTFLAGS':flags},check=True)
shutil.copyfile(out/'target/wasm32-unknown-unknown/release/jadpo_wasm_exp1_rust_full.wasm',out/'diagnostic.wasm')
