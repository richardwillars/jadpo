// Compiler-owned probe runtime. Application logic is appended by generate.py.
use serde_json::{json, Value};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[derive(Debug)]
enum Fault { Domain(&'static str), Internal(u32), Invalid }
type Outcome = Result<AppValue, Fault>;
struct HostValue { value: Value, json_bound: Option<usize>, typed: Option<CheckedRow>, origin: Option<RowOrigin> }
static mut HOST_TYPED_ROW: Option<CheckedRow> = None;
static mut HOST_ROW_ORIGIN: Option<RowOrigin> = None;
type HostOutcome = Result<HostValue, Fault>;
static mut HOST_JSON_BOUND: Option<usize> = None;
static mut FRAME: Option<Pin<Box<dyn Future<Output = Outcome>>>> = None;
static mut OUTPUT: Vec<u8> = Vec::new();
static mut ALLOCATIONS: Vec<Rc<Vec<u8>>> = Vec::new();
static mut STARTED: bool = false;
static mut REQUEST_ID: u32 = 1;
static mut PENDING: Option<(u32, u32)> = None;
static mut HOST_RESULT: Option<Value> = None;
static mut NEXT_OPERATION: u32 = 0;
static mut CURRENT_SEMANTIC_OPERATION: u32 = 0;
const LIMIT: usize = 65_536;

fn emit(value: Value, status: i32) -> i32 {
    unsafe { OUTPUT_FORMAT=0; }
    if status==0 && unsafe {ROW_FLAGS&2!=0 && RETURN_SCHEMA!=0} {
        if let (Some(schema),Some(row))=(row_schema(unsafe{RETURN_SCHEMA}),value.get("value")) {
            if row.as_object().map_or(false,|o|o.values().any(|v|v.as_str().map_or(false,|s|s.len()>=1024))) {
                if json_len(&value)>LIMIT {return fault(Fault::Internal(unsafe{CURRENT_SEMANTIC_OPERATION}));}
                if let Some(bytes)=encode_row(schema,row) {
                    unsafe {OUTPUT=bytes;OUTPUT_FORMAT=1;} return status;
                }
                // A very short schema can have a larger binary than JSON frame.
                // Keep a valid JSON-sized result working through the legacy path.
            }
        }
    }
    let bytes = serde_json::to_vec(&value).unwrap_or_default();
    let oversized = bytes.len() > LIMIT;
    unsafe {
        OUTPUT = if !oversized { bytes } else {
            br#"{"kind":"internal","operation":0}"#.to_vec()
        };
    }
    if oversized { 3 } else { status }
}
fn fault(error: Fault) -> i32 {
    match error {
        Fault::Domain(name) => emit(json!({"kind":"domain","failure":name}), 1),
        Fault::Internal(operation) => emit(json!({"kind":"internal","operation":operation}), 3),
        Fault::Invalid => emit(json!({"kind":"invalid"}), 4),
    }
}
fn allocated_bytes(pointer: i32, length: i32) -> Result<&'static [u8], Fault> {
    if pointer <= 0 || length < 0 || length as usize > LIMIT { return Err(Fault::Invalid); }
    let pointer = pointer as usize;
    let length = length as usize;
    let valid = unsafe { ALLOCATIONS.iter().any(|v| {
        let start = v.as_ptr() as usize;
        pointer >= start && pointer.checked_add(length).map_or(false, |end| end <= start + v.len())
    }) };
    if !valid { return Err(Fault::Invalid); }
    Ok(unsafe { std::slice::from_raw_parts(pointer as *const u8, length) })
}
// Input allocations are owned for the request. The driver finishes writing
// before resume and never mutates an accepted allocation; fields can share its
// backing storage. Rc keeps it alive even after the allocation registry clears.
fn allocated_owner(pointer:i32,length:i32)->Result<(Rc<Vec<u8>>,usize),Fault> {
    allocated_bytes(pointer,length)?;
    let pointer=pointer as usize;
    unsafe {ALLOCATIONS.iter().find_map(|v|{
        let start=v.as_ptr() as usize;
        if pointer>=start && pointer+length as usize<=start+v.len(){Some((v.clone(),pointer-start))}else{None}
    }).ok_or(Fault::Invalid)}
}
fn decode(pointer: i32, length: i32) -> Result<Value, Fault> {
    serde_json::from_slice(allocated_bytes(pointer,length)?).map_err(|_| Fault::Invalid)
}
#[no_mangle]
pub extern "C" fn alloc(length: i32) -> i32 {
    if length <= 0 || length as usize > LIMIT { return 0; }
    unsafe {
        if ALLOCATIONS.len() >= 64 { return 0; }
        let mut bytes = Vec::new();
        if bytes.try_reserve_exact(length as usize).is_err() { return 0; }
        bytes.resize(length as usize, 0);
        let pointer = bytes.as_mut_ptr() as i32;
        ALLOCATIONS.push(Rc::new(bytes));
        pointer
    }
}
#[no_mangle]
pub extern "C" fn result_ptr() -> i32 { unsafe { OUTPUT.as_ptr() as i32 } }
#[no_mangle]
pub extern "C" fn result_len() -> i32 { unsafe { OUTPUT.len() as i32 } }

fn raw_waker() -> RawWaker {
    unsafe fn clone(_: *const ()) -> RawWaker { raw_waker() }
    unsafe fn noop(_: *const ()) {}
    RawWaker::new(std::ptr::null(), &RawWakerVTable::new(clone, noop, noop, noop))
}
fn drive() -> i32 {
    let Some(mut frame) = (unsafe { FRAME.take() }) else { return fault(Fault::Internal(0)); };
    let waker = unsafe { Waker::from_raw(raw_waker()) };
    match frame.as_mut().poll(&mut Context::from_waker(&waker)) {
        Poll::Ready(Ok(value)) => { unsafe { PENDING = None; } {
            emit_app(value)
        } },
        Poll::Ready(Err(error)) => { unsafe { PENDING = None; } fault(error) },
        Poll::Pending => { unsafe { FRAME = Some(frame); } 2 },
    }
}
// ABI extension: reset only a completed request; pending frames cannot be reused.
// Bump request identity so handles from an earlier lease cannot resume this one.
#[no_mangle]
pub extern "C" fn reset() -> i32 {
    unsafe {
        if FRAME.is_some() || PENDING.is_some() || REQUEST_ID >= i32::MAX as u32 { return 0; }
        HOST_RESULT = None;
        HOST_JSON_BOUND = None;
        HOST_TYPED_ROW=None; HOST_ROW_ORIGIN=None;
        FRAME = None;
        OUTPUT.clear();
        ALLOCATIONS.clear();
        STARTED = false;
        NEXT_OPERATION = 0;
        CURRENT_SEMANTIC_OPERATION = 0;
        ROW_BUDGET_EXCEEDED=false;ROW_FLAGS=0;OUTPUT_FORMAT=0;RETURN_SCHEMA=0;PENDING_ROW_SCHEMA=0;
        REQUEST_ID += 1;
    }
    1
}
#[no_mangle]
pub extern "C" fn start(operation: i32, pointer: i32, length: i32) -> i32 {
    unsafe {
        if STARTED { FRAME = None; PENDING = None; return fault(Fault::Internal(operation as u32)); }
        STARTED = true;
        CURRENT_SEMANTIC_OPERATION = operation as u32;
        RETURN_SCHEMA=row_schema_for_entry(operation);
    }
    let input = match decode(pointer, length) { Ok(v) => v, Err(e) => return fault(e) };
    match make_entry(operation, input) {
        Ok(frame) => { unsafe { FRAME = Some(frame); } drive() },
        Err(error) => fault(error),
    }
}
#[no_mangle]
pub extern "C" fn resume(request: i32, operation: i32, pointer: i32, length: i32) -> i32 {
    resume_impl(request,operation,pointer,length,None)
}
#[no_mangle]
pub extern "C" fn resume_ref(request:i32,operation:i32,pointer:i32,length:i32)->i32 {
    if unsafe{ROW_FLAGS&4==0 || PENDING_ROW_SCHEMA==0} {return fault(Fault::Internal(unsafe{CURRENT_SEMANTIC_OPERATION}));}
    resume_impl(request,operation,pointer,length,Some(RowOrigin{request:request as u32,operation:operation as u32}))
}
fn resume_impl(request:i32,operation:i32,pointer:i32,length:i32,origin:Option<RowOrigin>)->i32 {
    unsafe {
        if PENDING != Some((request as u32, operation as u32)) || FRAME.is_none() {
            FRAME = None; PENDING = None; HOST_RESULT = None;
            return fault(Fault::Internal(CURRENT_SEMANTIC_OPERATION));
        }
    }
    let value = match decode(pointer, length) {
        Ok(v) => v,
        Err(_) => { unsafe { FRAME = None; PENDING = None; return fault(Fault::Internal(CURRENT_SEMANTIC_OPERATION)); } }
    };
    unsafe { HOST_RESULT = Some(value); HOST_JSON_BOUND = Some(length as usize); HOST_ROW_ORIGIN=origin;HOST_TYPED_ROW=None; }
    drive()
}

struct HostRead { args: Value, semantic: u32, issued: bool, capability: &'static str, compact: bool }
impl Future for HostRead {
    type Output = HostOutcome;
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<HostOutcome> {
        if !self.issued {
            self.issued = true;
            unsafe {
                NEXT_OPERATION += 1;
                CURRENT_SEMANTIC_OPERATION = self.semantic;
                PENDING = Some((REQUEST_ID, NEXT_OPERATION));
                PENDING_ROW_SCHEMA=if self.compact {compact_plan(self.semantic).map_or(0,|p|p.0)} else if self.capability=="storage.read" {
                    self.args.get("entity").and_then(Value::as_str).map(row_schema_for_name).unwrap_or(0)
                } else {0};
                if self.compact {
                    if emit_compact_read(&self.args,self.semantic,REQUEST_ID,NEXT_OPERATION).is_err() {
                        PENDING=None;return Poll::Ready(Err(Fault::Internal(self.semantic)));
                    }
                    return Poll::Pending;
                }
                let mut envelope = json!({"kind":"pending","requestId":REQUEST_ID,"operationId":NEXT_OPERATION,
                    "capability":self.capability});
                envelope["args"] = std::mem::take(&mut self.args);
                let status = emit(envelope, 2);
                if status != 2 {
                    PENDING = None;
                    return Poll::Ready(Err(Fault::Internal(self.semantic)));
                }
            }
            return Poll::Pending;
        }
        if let Some(mut row)=unsafe{HOST_TYPED_ROW.take()} {
            unsafe{PENDING=None;row.origin=HOST_ROW_ORIGIN.take();}
            return Poll::Ready(Ok(HostValue{value:Value::Null,json_bound:row.json_bound,typed:Some(row),origin:None}));
        }
        let Some(mut result) = (unsafe { HOST_RESULT.take() }) else { return Poll::Pending; };
        unsafe { PENDING = None; }
        let Some(object) = result.as_object_mut() else { return Poll::Ready(Err(Fault::Internal(self.semantic))); };
        // Raw host domain failures are not accepted: query absence is a null value,
        // and generated code selects the authored missing failure inside Wasm.
        if object.len() == 2 && object.get("kind").and_then(Value::as_str) == Some("success") && object.contains_key("value") {
            Poll::Ready(Ok(HostValue {value: object.remove("value").unwrap(), json_bound: if self.capability=="storage.read" {unsafe{HOST_JSON_BOUND.take()}} else {None}, typed:None, origin:unsafe{HOST_ROW_ORIGIN.take()}}))
        } else { Poll::Ready(Err(Fault::Internal(self.semantic))) }
    }
}
fn host_read(args: Value, semantic: u32) -> HostRead { HostRead { args, semantic, issued: false, capability: "storage.read", compact:false } }
fn host_read_compact(predicate:Value,semantic:u32)->HostRead {HostRead{args:predicate,semantic,issued:false,capability:"storage.read",compact:true}}
fn emit_compact_read(predicate:&Value,semantic:u32,request:u32,operation:u32)->Result<(),()> {
    let (_,base)=compact_plan(semantic).ok_or(())?;
    let bytes=serde_json::to_vec(predicate).map_err(|_|())?;
    // Preserve the exact legacy envelope limit, including escaped UTF-8 and IDs.
    if base-4+bytes.len()+request.to_string().len()-1+operation.to_string().len()-1>LIMIT || bytes.len()+48>LIMIT {return Err(());}
    let mut output=Vec::with_capacity(48+bytes.len());
    output.extend_from_slice(b"JRP1");output.extend_from_slice(&ROW_DIGEST);
    output.extend_from_slice(&request.to_le_bytes());output.extend_from_slice(&operation.to_le_bytes());
    output.extend_from_slice(&semantic.to_le_bytes());output.extend_from_slice(&bytes);
    unsafe{OUTPUT=output;OUTPUT_FORMAT=3;}Ok(())
}
fn host_update(args: Value, semantic: u32) -> HostRead { HostRead { args, semantic, issued: false, capability: "storage.update", compact:false } }
fn member(value: &AppValue, path: &[&str], operation: u32) -> Outcome {
    if path.is_empty() {return Ok(value.clone());}
    match value {
        AppValue::Json(value) => {
            let mut result=value;
            for key in path {result=result.get(*key).ok_or(Fault::Internal(operation))?;}
            Ok(AppValue::Json(result.clone()))
        },
        AppValue::Row(row) => {
            if path.len()!=1 {return Err(Fault::Internal(operation));}
            let i=row.schema.fields.iter().position(|key|*key==path[0]).ok_or(Fault::Internal(operation))?;
            Ok(AppValue::Json(row.fields[i].to_json().ok_or(Fault::Internal(operation))?))
        },
    }
}
fn uuid(value: &Value) -> bool {
    let Some(text) = value.as_str() else { return false; };
    let bytes = text.as_bytes();
    bytes.len() == 36 && bytes.iter().enumerate().all(|(i,b)| {
        if [8,13,18,23].contains(&i) { *b == b'-' } else { b.is_ascii_hexdigit() }
    }) && matches!(bytes[14], b'1'..=b'5') && matches!(bytes[19], b'8'|b'9'|b'a'|b'b'|b'A'|b'B')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_completion_reports_internal_status() {
        assert_eq!(emit(json!({"kind":"success","value":"x".repeat(LIMIT)}), 0), 3);
        let output: Value = unsafe { serde_json::from_slice(&OUTPUT).unwrap() };
        assert_eq!(output["kind"], "internal");
    }
    #[test]
    fn oversized_pending_completes_with_internal_fault() {
        let mut host = Box::pin(host_read(json!({"huge":"x".repeat(LIMIT)}), 26));
        let waker = unsafe { Waker::from_raw(raw_waker()) };
        assert!(matches!(host.as_mut().poll(&mut Context::from_waker(&waker)), Poll::Ready(Err(Fault::Internal(26)))));
        assert!(unsafe { PENDING.is_none() });
    }
}

#[cfg(test)]
mod compact_tests {
    use super::*;
    #[test]
    fn compact_budget_matches_exact_legacy_envelope_limit() {
        let id=(0..65536u32).find(|id|compact_plan(*id).is_some()).unwrap();
        let (_,base)=compact_plan(id).unwrap();
        for (request,operation) in [(1u32,1u32),(999,63)] {
            let digits=request.to_string().len()-1+operation.to_string().len()-1;
            let length=LIMIT-base+2-digits;
            let predicate=Value::String("x".repeat(length));
            assert!(emit_compact_read(&predicate,id,request,operation).is_ok());
            assert!(emit_compact_read(&Value::String("x".repeat(length+1)),id,request,operation).is_err());
            assert!(emit_compact_read(&Value::String("\n".repeat(length)),id,request,operation).is_err());
        }
        assert!(emit_compact_read(&Value::Null,0,1,1).is_err());
        unsafe{OUTPUT_FORMAT=0;OUTPUT.clear();}
    }
}
