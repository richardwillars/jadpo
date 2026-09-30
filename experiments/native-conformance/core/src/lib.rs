//! Experimental target-independent application runtime. All mutable execution
//! state belongs to an Invocation. No ABI pointers, database handles or globals.
#![forbid(unsafe_code)]
#![allow(unused_parens, unused_variables, unused_mut)]
use serde_json::{json, Value};
use std::{
    future::Future,
    io::{self, Write},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
};

pub const LIMIT: usize = 65_536;
const MAX_EFFECTS: u32 = 64;
#[derive(Debug)]
enum Fault {
    Domain(&'static str),
    Internal(u32),
    Invalid,
}
type Outcome = Result<AppValue, Fault>;
#[derive(Clone)]
enum AppValue {
    Json(Value),
    Row(Arc<Value>),
}
impl AppValue {
    fn checked_row(row: Value) -> Self {
        if row.as_object().map_or(false, |o| {
            o.values()
                .any(|v| v.as_str().map_or(false, |s| s.len() >= 1024))
        }) {
            Self::Row(Arc::new(row))
        } else {
            Self::Json(row)
        }
    }
    fn into_json(self) -> Value {
        match self {
            Self::Json(v) => v,
            Self::Row(v) => Arc::try_unwrap(v).unwrap_or_else(|v| (*v).clone()),
        }
    }
}
fn member(value: &AppValue, path: &[&str], operation: u32) -> Outcome {
    if path.is_empty() {
        return Ok(value.clone());
    }
    let mut value = match value {
        AppValue::Json(v) => v,
        AppValue::Row(v) => v.as_ref(),
    };
    for key in path {
        value = value.get(*key).ok_or(Fault::Internal(operation))?;
    }
    Ok(AppValue::Json(value.clone()))
}
fn uuid(value: &Value) -> bool {
    let Some(text) = value.as_str() else {
        return false;
    };
    let b = text.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                *c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
        && matches!(b[14], b'1'..=b'5')
        && matches!(b[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
}
// Count exactly the canonical JSON bytes without allocating an oversized output.
struct Budget(usize);
impl Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > LIMIT - self.0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "frame limit"));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub fn within_limit(value: &Value) -> bool {
    serde_json::to_writer(Budget(0), value).is_ok()
}
pub fn valid(name: &str, value: &Value) -> bool {
    validate(name, value)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub request: u32,
    pub operation: u32,
    pub semantic: u32,
    pub capability: &'static str,
    pub args: Value,
}
impl Pending {
    pub fn envelope(&self) -> Value {
        json!({"kind":"pending","requestId":self.request,"operationId":self.operation,"capability":self.capability,"args":self.args})
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Completion {
    pub value: Value,
    pub fault_operation: Option<u32>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Pending(Pending),
    Complete(Completion),
}
struct RequestState {
    request: u32,
    next: u32,
    semantic: u32,
    pending: Option<Pending>,
    response: Option<Value>,
}
#[derive(Clone)]
struct InvocationContext(Arc<Mutex<RequestState>>);
struct HostValue {
    value: Value,
}
struct HostEffect {
    ctx: InvocationContext,
    args: Option<Value>,
    semantic: u32,
    capability: &'static str,
    issued: bool,
}
impl Future for HostEffect {
    type Output = Result<HostValue, Fault>;
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.issued {
            self.issued = true;
            let args = self.args.take().unwrap();
            let mut state = self.ctx.0.lock().unwrap();
            if state.next >= MAX_EFFECTS || state.pending.is_some() {
                return Poll::Ready(Err(Fault::Internal(self.semantic)));
            }
            state.next += 1;
            state.semantic = self.semantic;
            let pending = Pending {
                request: state.request,
                operation: state.next,
                semantic: self.semantic,
                capability: self.capability,
                args,
            };
            if !within_limit(&pending.envelope()) {
                return Poll::Ready(Err(Fault::Internal(self.semantic)));
            }
            state.pending = Some(pending);
            return Poll::Pending;
        }
        let mut state = self.ctx.0.lock().unwrap();
        let Some(mut result) = state.response.take() else {
            return Poll::Pending;
        };
        state.pending = None;
        let Some(object) = result.as_object_mut() else {
            return Poll::Ready(Err(Fault::Internal(self.semantic)));
        };
        if object.len() != 2
            || object.get("kind").and_then(Value::as_str) != Some("success")
            || !object.contains_key("value")
        {
            return Poll::Ready(Err(Fault::Internal(self.semantic)));
        }
        Poll::Ready(Ok(HostValue {
            value: object.remove("value").unwrap(),
        }))
    }
}
fn host_read(ctx: InvocationContext, args: Value, semantic: u32) -> HostEffect {
    HostEffect {
        ctx,
        args: Some(args),
        semantic,
        capability: "storage.read",
        issued: false,
    }
}
fn host_update(ctx: InvocationContext, args: Value, semantic: u32) -> HostEffect {
    HostEffect {
        ctx,
        args: Some(args),
        semantic,
        capability: "storage.update",
        issued: false,
    }
}
struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

/// Owns its suspended future and all host/control state. Send allows a suspended
/// invocation to move between threads; poll/resume require exclusive access.
pub struct Invocation {
    ctx: InvocationContext,
    frame: Option<Pin<Box<dyn Future<Output = Outcome> + Send>>>,
    completed: Option<Completion>,
}
impl Invocation {
    pub fn new(request: u32, operation: i32, input: Value) -> Self {
        let ctx = InvocationContext(Arc::new(Mutex::new(RequestState {
            request,
            next: 0,
            semantic: operation as u32,
            pending: None,
            response: None,
        })));
        let frame = if request == 0 {
            Err(Fault::Invalid)
        } else {
            make_entry(operation, input, ctx.clone())
        };
        let mut invocation = Self {
            ctx,
            frame: None,
            completed: None,
        };
        match frame {
            Ok(frame) => invocation.frame = Some(frame),
            Err(fault) => {
                invocation.finish(Err(fault));
            }
        }
        invocation
    }
    fn finish(&mut self, result: Outcome) -> Step {
        self.frame = None;
        let mut state = self.ctx.0.lock().unwrap();
        state.pending = None;
        state.response = None;
        let (mut value, mut fault) = match result {
            Ok(value) => (json!({"kind":"success","value":value.into_json()}), None),
            Err(Fault::Domain(name)) => (json!({"kind":"domain","failure":name}), None),
            Err(Fault::Invalid) => (json!({"kind":"invalid"}), None),
            Err(Fault::Internal(id)) => (json!({"kind":"internal"}), Some(id)),
        };
        if !within_limit(&value) {
            value = json!({"kind":"internal"});
            fault = Some(state.semantic);
        }
        let completion = Completion {
            value,
            fault_operation: fault,
        };
        self.completed = Some(completion.clone());
        Step::Complete(completion)
    }
    pub fn poll(&mut self) -> Step {
        if let Some(result) = &self.completed {
            return Step::Complete(result.clone());
        }
        let waker = Waker::from(Arc::new(NoopWake));
        let result = self
            .frame
            .as_mut()
            .unwrap()
            .as_mut()
            .poll(&mut Context::from_waker(&waker));
        match result {
            Poll::Ready(result) => self.finish(result),
            Poll::Pending => {
                let state = self.ctx.0.lock().unwrap();
                let pending = state.pending.clone();
                let id = state.semantic;
                drop(state);
                match pending {
                    Some(p) => Step::Pending(p),
                    None => self.finish(Err(Fault::Internal(id))),
                }
            }
        }
    }
    pub fn resume(&mut self, request: u32, operation: u32, value: Value) -> Step {
        let mut state = self.ctx.0.lock().unwrap();
        let id = state.semantic;
        if self.completed.is_some()
            || !state
                .pending
                .as_ref()
                .map_or(false, |p| p.request == request && p.operation == operation)
            || state.response.is_some()
            || !within_limit(&value)
        {
            drop(state);
            return self.finish(Err(Fault::Internal(id)));
        }
        state.response = Some(value);
        drop(state);
        self.poll()
    }
    pub fn cancel(&mut self) -> Step {
        let id = self.ctx.0.lock().unwrap().semantic;
        self.finish(Err(Fault::Internal(id)))
    }
}
pub fn run(
    operation: i32,
    input: Value,
    mut host: impl FnMut(&str, &Value) -> Value,
) -> Completion {
    let mut invocation = Invocation::new(1, operation, input);
    let mut step = invocation.poll();
    loop {
        match step {
            Step::Complete(result) => return result,
            Step::Pending(p) => {
                let reply = host(p.capability, &p.args);
                step = invocation.resume(p.request, p.operation, reply);
            }
        }
    }
}
include!("../../build/application.rs");
#[cfg(test)]
mod tests;
