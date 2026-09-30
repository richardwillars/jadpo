//! Transport adapter only. One ABI slot per instance; application state lives in
//! the shared Invocation. Native address/pointer APIs never enter the core crate.
use jadpo_auth_policy_core::{Invocation, Step, LIMIT};
use serde_json::{json, Value};
use std::cell::RefCell;
#[derive(Default)]
struct Slot {
    input: Vec<u8>,
    output: Vec<u8>,
    active: Option<Invocation>,
    next_request: u32,
}
thread_local! {static SLOT:RefCell<Slot>=RefCell::new(Slot::default());}
impl Slot {
    fn publish(&mut self, step: Step) -> i32 {
        let value = match step {
            Step::Pending(p) => p.envelope(),
            Step::Complete(c) => {
                self.active = None;
                c.value
            }
        };
        let status = match value["kind"].as_str() {
            Some("success") => 0,
            Some("domain") => 1,
            Some("pending") => 2,
            Some("invalid") => 4,
            _ => 3,
        };
        self.output = serde_json::to_vec(&value).unwrap();
        status
    }
    fn invalid_host(&mut self) -> i32 {
        let step = match self.active.as_mut() {
            Some(i) => i.cancel(),
            None => Step::Complete(jadpo_auth_policy_core::Completion {
                value: json!({"kind":"internal"}),
                fault_operation: None,
            }),
        };
        self.publish(step)
    }
    fn decode(&self, n: i32) -> Option<Value> {
        if n < 0 || n as usize != self.input.len() {
            None
        } else {
            serde_json::from_slice(&self.input).ok()
        }
    }
}
#[no_mangle]
pub extern "C" fn alloc(n: i32) -> u32 {
    SLOT.with(|slot| {
        let mut s = slot.borrow_mut();
        if n <= 0 || n as usize > LIMIT {
            s.input.clear();
            return 0;
        }
        s.input = vec![0; n as usize];
        s.input.as_ptr() as u32
    })
}
#[no_mangle]
pub extern "C" fn result_ptr() -> u32 {
    SLOT.with(|s| s.borrow().output.as_ptr() as u32)
}
#[no_mangle]
pub extern "C" fn result_len() -> u32 {
    SLOT.with(|s| s.borrow().output.len() as u32)
}
#[no_mangle]
pub extern "C" fn start(operation: i32, n: i32) -> i32 {
    SLOT.with(|slot| {
        let mut s = slot.borrow_mut();
        if s.active.is_some() || s.next_request == u32::MAX {
            return s.invalid_host();
        }
        let Some(input) = s.decode(n) else {
            s.output = br#"{"kind":"invalid"}"#.to_vec();
            return 4;
        };
        s.next_request += 1;
        let mut invocation = Invocation::new(s.next_request, operation, input);
        let step = invocation.poll();
        s.active = Some(invocation);
        s.publish(step)
    })
}
#[no_mangle]
pub extern "C" fn resume(request: u32, operation: u32, n: i32) -> i32 {
    SLOT.with(|slot| {
        let mut s = slot.borrow_mut();
        let Some(value) = s.decode(n) else {
            return s.invalid_host();
        };
        let Some(i) = s.active.as_mut() else {
            return s.invalid_host();
        };
        let step = i.resume(request, operation, value);
        s.publish(step)
    })
}
#[no_mangle]
pub extern "C" fn cancel() -> i32 {
    SLOT.with(|slot| slot.borrow_mut().invalid_host())
}
