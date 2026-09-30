//! Deliberately hostile application guest for local authority-boundary tests.
use serde_json::{json, Value};
use std::cell::RefCell;
#[derive(Default)]
struct State {
    input: Vec<u8>,
    output: Vec<u8>,
    mode: u32,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
#[no_mangle]
pub extern "C" fn attack_mode(mode: u32) {
    STATE.with(|s| s.borrow_mut().mode = mode);
}
#[no_mangle]
pub extern "C" fn alloc(n: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if n <= 0 || n > 65536 {
            return 0;
        }
        s.input = vec![0; n as usize];
        s.input.as_ptr() as u32
    })
}
#[no_mangle]
pub extern "C" fn result_ptr() -> u32 {
    STATE.with(|s| s.borrow().output.as_ptr() as u32)
}
#[no_mangle]
pub extern "C" fn result_len() -> u32 {
    STATE.with(|s| s.borrow().output.len() as u32)
}
#[no_mangle]
pub extern "C" fn start(_: i32, _: i32) -> i32 {
    STATE.with(|s|{
 let mut s=s.borrow_mut();let input:Value=serde_json::from_slice(&s.input).unwrap();
 let (cap,args)=match s.mode {
  1=>("sql.query",json!({"sql":"SELECT private_note FROM note WHERE id = ?","params":[input["input"]["id"]]})),
  2=>("transaction.commit",Value::Null),
  3=>("entity.read",json!({"plan":43,"key":input["input"]["id"]})),
  _=>("entity.read",json!({"plan":46,"key":input["input"]["id"],"principal":{"user_id":"forged"}})),
 };
 s.output=serde_json::to_vec(&json!({"kind":"pending","requestId":input["scope"],"operationId":1,"capability":cap,"args":args})).unwrap();2
})
}
#[no_mangle]
pub extern "C" fn resume(_: u32, _: u32, _: i32) -> i32 {
    cancel()
}
#[no_mangle]
pub extern "C" fn cancel() -> i32 {
    STATE.with(|s| s.borrow_mut().output = br#"{"kind":"internal"}"#.to_vec());
    3
}
