//! Transport adapter only. One ABI slot per instance; application state lives in
//! the shared Invocation. Native address/pointer APIs never enter the core crate.
use jadpo_capability_monitor_core::{Invocation, Step, LIMIT};
use serde_json::{json, Value};
use std::cell::RefCell;
#[derive(Default)]
struct Slot {
    input: Vec<u8>,
    output: Vec<u8>,
    active: Option<Invocation>,
    next_request: u32,
    status: i32,
    request: u32,
    operation: u32,
    capability: u32,
}
thread_local! {static SLOT:RefCell<Slot>=RefCell::new(Slot::default());}
fn capability_code(capability: &str) -> u32 {
    match capability {
        "entity.read" => 1,
        "entity.update" => 2,
        "sql.query" => 3,
        "sql.update" => 4,
        "transaction.begin" => 5,
        "transaction.commit" => 6,
        "transaction.rollback" => 7,
        "guest.start" => 8,
        "guest.resume" => 9,
        "guest.check" => 10,
        "guest.complete" => 11,
        _ => 0,
    }
}
impl Slot {
    fn publish_payload(
        &mut self,
        status: i32,
        request: u32,
        operation: u32,
        capability: u32,
        payload: &Value,
    ) -> i32 {
        self.status = status;
        self.request = request;
        self.operation = operation;
        self.capability = capability;
        self.output.clear();
        serde_json::to_writer(&mut self.output, payload).unwrap();
        status
    }
    fn publish(&mut self, step: Step) -> i32 {
        match step {
            Step::Pending(p) => self.publish_payload(
                2,
                p.request,
                p.operation,
                capability_code(p.capability),
                &p.args,
            ),
            Step::Complete(c) => {
                self.active = None;
                let value = c.value;
                match value["kind"].as_str() {
                    Some("success") => self.publish_payload(
                        0,
                        0,
                        0,
                        0,
                        value.get("value").unwrap_or(&Value::Null),
                    ),
                    Some("domain") => self.publish_payload(
                        1,
                        0,
                        0,
                        0,
                        value.get("failure").unwrap_or(&Value::Null),
                    ),
                    Some("invalid") => self.publish_payload(4, 0, 0, 0, &Value::Null),
                    _ => self.publish_payload(3, 0, 0, 0, &Value::Null),
                }
            }
        }
    }
    fn invalid_host(&mut self) -> i32 {
        let step = match self.active.as_mut() {
            Some(i) => i.cancel(),
            None => Step::Complete(jadpo_capability_monitor_core::Completion {
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
        if n <= 0 || n as usize > LIMIT * 2 {
            s.input.clear();
            return 0;
        }
        s.input.clear();
        s.input.resize(n as usize, 0);
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
pub extern "C" fn result_status() -> i32 {
    SLOT.with(|s| s.borrow().status)
}
#[no_mangle]
pub extern "C" fn result_request() -> u32 {
    SLOT.with(|s| s.borrow().request)
}
#[no_mangle]
pub extern "C" fn result_operation() -> u32 {
    SLOT.with(|s| s.borrow().operation)
}
#[no_mangle]
pub extern "C" fn result_capability() -> u32 {
    SLOT.with(|s| s.borrow().capability)
}
#[no_mangle]
pub extern "C" fn start(operation: i32, n: i32) -> i32 {
    SLOT.with(|slot| {
        let mut s = slot.borrow_mut();
        if s.active.is_some() || s.next_request == u32::MAX {
            return s.invalid_host();
        }
        let Some(input) = s.decode(n) else {
            return s.publish_payload(4, 0, 0, 0, &Value::Null);
        };
        s.next_request += 1;
        let mut invocation = Invocation::monitor(s.next_request, operation, input);
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
