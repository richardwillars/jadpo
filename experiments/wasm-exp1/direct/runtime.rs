// Generic bounded JSON/value support. This module has no application operations,
// names, query semantics, recovery branches, continuation state, or IR evaluator.
use serde_json::{json, Value};
const LIMIT: usize = 65_536;
static mut CONSTANTS: [u8; LIMIT] = [0; LIMIT];
static mut VALUES: Vec<Value> = Vec::new();
static mut ALLOCATIONS: Vec<Vec<u8>> = Vec::new();
static mut OUTPUT: Vec<u8> = Vec::new();
fn put(value: Value) -> i32 {
    unsafe { if VALUES.len() >= 4096 { return 0; } VALUES.push(value); VALUES.len() as i32 }
}
fn get(handle: i32) -> Option<&'static Value> {
    if handle <= 0 { return None; }
    unsafe { VALUES.get(handle as usize - 1) }
}
#[no_mangle] pub extern "C" fn rt_constant_base() -> i32 { unsafe { CONSTANTS.as_ptr() as i32 } }
#[no_mangle] pub extern "C" fn rt_constant(offset: i32, length: i32) -> i32 {
    if offset < 0 || length < 0 || offset as usize + length as usize > LIMIT { return 0; }
    let bytes = unsafe { &CONSTANTS[offset as usize..offset as usize + length as usize] };
    serde_json::from_slice(bytes).map(put).unwrap_or(0)
}
#[no_mangle] pub extern "C" fn alloc(length: i32) -> i32 {
    if length <= 0 || length as usize > LIMIT { return 0; }
    unsafe {
        if ALLOCATIONS.len() >= 64 { return 0; }
        let mut bytes = Vec::new();
        if bytes.try_reserve_exact(length as usize).is_err() { return 0; }
        bytes.resize(length as usize, 0);
        let pointer = bytes.as_mut_ptr() as i32;
        ALLOCATIONS.push(bytes); pointer
    }
}
#[no_mangle] pub extern "C" fn rt_decode(pointer: i32, length: i32) -> i32 {
    if pointer <= 0 || length < 0 || length as usize > LIMIT { return 0; }
    let p = pointer as usize; let len = length as usize;
    let valid = unsafe { ALLOCATIONS.iter().any(|v| {
        p >= v.as_ptr() as usize && p.checked_add(len).map_or(false, |end| end <= v.as_ptr() as usize + v.len())
    }) };
    if !valid { return 0; }
    let bytes = unsafe { std::slice::from_raw_parts(p as *const u8, len) };
    serde_json::from_slice(bytes).map(put).unwrap_or(0)
}
#[no_mangle] pub extern "C" fn rt_member(value: i32, key: i32) -> i32 {
    get(key).and_then(Value::as_str).and_then(|k| get(value)?.get(k)).cloned().map(put).unwrap_or(0)
}
#[no_mangle] pub extern "C" fn rt_set(value: i32, key: i32, item: i32) -> i32 {
    let Some(k) = get(key).and_then(Value::as_str).map(str::to_owned) else { return 0; };
    let Some(v) = get(item).cloned() else { return 0; };
    if value <= 0 { return 0; }
    unsafe {
        let Some(object) = VALUES.get_mut(value as usize - 1).and_then(Value::as_object_mut) else { return 0; };
        object.insert(k, v); value
    }
}
#[no_mangle] pub extern "C" fn rt_is_null(value: i32) -> i32 { get(value).map_or(false, Value::is_null) as i32 }
#[no_mangle] pub extern "C" fn rt_equal(left: i32, right: i32) -> i32 {
    matches!((get(left), get(right)), (Some(a), Some(b)) if a == b) as i32
}
fn uuid(value: &Value) -> bool {
    let Some(text) = value.as_str() else { return false; };
    let bytes = text.as_bytes();
    bytes.len() == 36 && bytes.iter().enumerate().all(|(i,b)| {
        if [8,13,18,23].contains(&i) { *b == b'-' } else { b.is_ascii_hexdigit() }
    }) && matches!(bytes[14], b'1'..=b'5') && matches!(bytes[19], b'8'|b'9'|b'a'|b'b'|b'A'|b'B')
}
fn validate(value: &Value, descriptor: &Value, depth: usize) -> bool {
    if depth > 32 { return false; }
    if value.is_null() { return descriptor["nullable"] == true; }
    let base = match descriptor["kind"].as_str() {
        Some("Text") => value.is_string(), Some("Uuid") => uuid(value),
        Some("Int") => value.as_i64().is_some(), Some("Bool") => value.is_boolean(),
        Some("Float") => value.is_number(),
        Some("enum") => descriptor["variants"].as_array().map_or(false, |xs| xs.contains(value)),
        Some("record") => {
            let (Some(object), Some(fields)) = (value.as_object(), descriptor["fields"].as_object()) else { return false; };
            object.keys().all(|k| fields.contains_key(k)) && fields.iter().all(|(k,d)| match object.get(k) {
                Some(v) => validate(v, d, depth + 1), None => d["optional"] == true,
            })
        },
        _ => false,
    };
    base && descriptor["constraints"].as_array().map_or(true, |rules| rules.iter().all(|rule| {
        match rule["kind"].as_str() {
            Some("MinLength") => value.as_str().zip(rule["value"].as_u64()).map_or(false, |(s,n)| s.chars().count() >= n as usize),
            Some("MaxLength") => value.as_str().zip(rule["value"].as_u64()).map_or(false, |(s,n)| s.chars().count() <= n as usize),
            Some("Min") => value.as_f64().zip(rule["value"].as_f64()).map_or(false, |(v,n)| v >= n),
            Some("Max") => value.as_f64().zip(rule["value"].as_f64()).map_or(false, |(v,n)| v <= n),
            _ => false,
        }
    }))
}
#[no_mangle] pub extern "C" fn rt_validate(value: i32, descriptor: i32) -> i32 {
    get(value).zip(get(descriptor)).map_or(false, |(v,d)| validate(v,d,0)) as i32
}
#[no_mangle] pub extern "C" fn rt_host_value(envelope: i32) -> i32 {
    let Some(object) = get(envelope).and_then(Value::as_object) else { return 0; };
    if object.len() != 2 || object.get("kind").and_then(Value::as_str) != Some("success") { return 0; }
    object.get("value").cloned().map(put).unwrap_or(0)
}
fn emit(value: Value, status: i32) -> i32 {
    let bytes = serde_json::to_vec(&value).unwrap_or_default();
    let oversized = bytes.len() > LIMIT;
    unsafe { OUTPUT = if oversized { br#"{"kind":"internal","operation":0}"#.to_vec() } else { bytes }; }
    if oversized { 3 } else { status }
}
#[no_mangle] pub extern "C" fn rt_complete(status: i32, value: i32, operation: i32) -> i32 {
    match status {
        0 => match get(value) { Some(v) => emit(json!({"kind":"success","value":v}),0), None => emit(json!({"kind":"internal","operation":operation}),3) },
        1 => match get(value).and_then(Value::as_str) { Some(v) => emit(json!({"kind":"domain","failure":v}),1), None => emit(json!({"kind":"internal","operation":operation}),3) },
        4 => emit(json!({"kind":"invalid"}),4),
        _ => emit(json!({"kind":"internal","operation":operation}),3),
    }
}
#[no_mangle] pub extern "C" fn rt_pending(capability: i32, args: i32, request: i32, operation: i32) -> i32 {
    match get(capability).and_then(Value::as_str).zip(get(args)) {
        Some((cap,a)) => emit(json!({"kind":"pending","requestId":request,"operationId":operation,"capability":cap,"args":a}),2),
        None => emit(json!({"kind":"internal","operation":0}),3),
    }
}
#[no_mangle] pub extern "C" fn result_ptr() -> i32 { unsafe { OUTPUT.as_ptr() as i32 } }
#[no_mangle] pub extern "C" fn result_len() -> i32 { unsafe { OUTPUT.len() as i32 } }
