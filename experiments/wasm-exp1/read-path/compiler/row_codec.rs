// Experimental schema-bound row frames. Control/input/error frames remain JSON.
// Typed field order is generated from the checked projection, not fixture names.
struct RowSchema { id: u32, name: &'static str, fields: &'static [&'static str] }
static mut ROW_FLAGS: u32 = 0;
static mut OUTPUT_FORMAT: u32 = 0;
static mut RETURN_SCHEMA: u32 = 0;
static mut PENDING_ROW_SCHEMA: u32 = 0;

#[no_mangle]
pub extern "C" fn set_row_transport(flags: u32) -> i32 {
    unsafe {
        if STARTED || flags > 15 { return 0; }
        ROW_FLAGS = flags; OUTPUT_FORMAT = 0; 1
    }
}
#[no_mangle]
pub extern "C" fn result_format() -> u32 { unsafe { OUTPUT_FORMAT } }

// Match the existing JSON byte bound without constructing an intermediate JSON
// buffer. Number serialization uses serde's spelling; strings retain JSON escapes.
fn json_text_len(s: &str) -> usize {
        fn extra(bytes:&[u8])->usize {bytes.iter().map(|b|match b {b'"'|b'\\'|8|9|10|12|13=>1,0..=31=>5,_=>0}).sum()}
        fn has_zero(n:u64)->bool {n.wrapping_sub(0x0101010101010101)&!n&0x8080808080808080!=0}
        let mut total=2+s.len();let mut chunks=s.as_bytes().chunks_exact(8);
        for chunk in &mut chunks {
            let n=u64::from_le_bytes(chunk.try_into().unwrap());
            // A zero byte after masking the top three bits means a control byte.
            // Borrow propagation can give false positives, never false negatives.
            if has_zero(n&0xe0e0e0e0e0e0e0e0)||has_zero(n^0x2222222222222222)||has_zero(n^0x5c5c5c5c5c5c5c5c){total+=extra(chunk);}
        }
        total+extra(chunks.remainder())
    }

fn json_len(value: &Value) -> usize {
    match value {
        Value::Null => 4, Value::Bool(true) => 4, Value::Bool(false) => 5,
        Value::Number(n) => n.to_string().len(), Value::String(s) => json_text_len(s),
        Value::Array(a) => 2 + a.len().saturating_sub(1) + a.iter().map(json_len).sum::<usize>(),
        Value::Object(o) => 2 + o.len().saturating_sub(1) + o.iter().map(|(k,v)| json_text_len(k)+1+json_len(v)).sum::<usize>(),
    }
}
fn row_schema(id: u32) -> Option<&'static RowSchema> { ROW_SCHEMAS.iter().find(|s| s.id == id) }
fn encode_row(schema: &RowSchema, row: &Value) -> Option<Vec<u8>> {
    let object = row.as_object()?;
    if object.keys().any(|k| !schema.fields.contains(&k.as_str())) { return None; }
    let capacity=40+schema.fields.len()*9+object.values().filter_map(Value::as_str).map(str::len).sum::<usize>();
    let mut bytes=Vec::with_capacity(capacity);
    bytes.extend_from_slice(b"JRW1");bytes.extend_from_slice(&ROW_DIGEST);
    bytes.extend_from_slice(&schema.id.to_le_bytes());
    for key in schema.fields {
        match object.get(*key) {
            None => bytes.push(0), Some(Value::Null) => bytes.push(1),
            Some(Value::String(s)) => { bytes.push(2);bytes.extend_from_slice(&(s.len() as u32).to_le_bytes());bytes.extend_from_slice(s.as_bytes()); },
            Some(Value::Bool(false)) => bytes.push(3), Some(Value::Bool(true)) => bytes.push(4),
            Some(Value::Number(n)) => {bytes.push(5);bytes.extend_from_slice(&n.as_i64()?.to_le_bytes());},
            _ => return None,
        }
    }
    if bytes.len()>LIMIT {None} else {Some(bytes)}
}
fn decode_row(bytes: &[u8], expected: u32) -> Option<Value> {
    if bytes.len()<40 || bytes.len()>LIMIT || &bytes[..4]!=b"JRW1" || bytes[4..36]!=ROW_DIGEST { return None; }
    let id=u32::from_le_bytes(bytes[36..40].try_into().ok()?);
    if id!=expected {return None;}
    let schema=row_schema(id)?; let mut pos=40;
    let mut object=serde_json::Map::new();
    for key in schema.fields {
        let tag=*bytes.get(pos)?;pos+=1;
        let value=match tag {
            0 => continue, 1 => Value::Null, 3 => Value::Bool(false), 4 => Value::Bool(true),
            2 => {
                let end=pos.checked_add(4)?;let len=u32::from_le_bytes(bytes.get(pos..end)?.try_into().ok()?) as usize;pos=end;
                let end=pos.checked_add(len)?;let s=std::str::from_utf8(bytes.get(pos..end)?).ok()?;pos=end;Value::String(s.to_owned())
            },
            5 => {let end=pos.checked_add(8)?;let n=i64::from_le_bytes(bytes.get(pos..end)?.try_into().ok()?);pos=end;Value::Number(n.into())},
            _ => return None,
        };
        object.insert((*key).to_owned(),value);
    }
    if pos!=bytes.len() {return None;}
    let row=Value::Object(object);
    // Generated application validates the returned row after this decoding step.
    // Enforce the original host success-envelope JSON size as well as binary size.
    if json_len(&row)+27>LIMIT {None} else {Some(row)}
}
#[no_mangle]
pub extern "C" fn resume_row(request:i32,operation:i32,pointer:i32,length:i32)->i32 {
    resume_row_impl(request,operation,pointer,length,None)
}
#[no_mangle]
pub extern "C" fn resume_row_ref(request:i32,operation:i32,pointer:i32,length:i32)->i32 {
    if unsafe{ROW_FLAGS&4==0} {return fault(Fault::Internal(unsafe{CURRENT_SEMANTIC_OPERATION}));}
    resume_row_impl(request,operation,pointer,length,Some(RowOrigin{request:request as u32,operation:operation as u32}))
}
fn resume_row_impl(request:i32,operation:i32,pointer:i32,length:i32,origin:Option<RowOrigin>)->i32 {
    let invalid=||unsafe {FRAME=None;PENDING=None;HOST_RESULT=None;fault(Fault::Internal(CURRENT_SEMANTIC_OPERATION))};
    unsafe {
        if ROW_FLAGS&1==0 || PENDING_ROW_SCHEMA==0 || PENDING!=Some((request as u32,operation as u32)) || FRAME.is_none() {return invalid();}
    }
    let bytes=match allocated_bytes(pointer,length) {Ok(b)=>b,Err(_)=>return invalid()};
    let row=match decode_typed_row(bytes,unsafe{PENDING_ROW_SCHEMA}) {Some(v)=>v,None=>return invalid()};
    unsafe {HOST_TYPED_ROW=Some(row);HOST_RESULT=None;HOST_JSON_BOUND=None;
        HOST_ROW_ORIGIN=origin;
    }
    drive()
}

#[cfg(test)]
mod row_tests {
    use super::*;
    #[test]
    fn larger_binary_frame_falls_back_to_valid_json() {
        let schema=ROW_SCHEMAS.iter().find(|s|s.fields.len()==1).unwrap();
        let mut object=serde_json::Map::new();object.insert(schema.fields[0].to_owned(),Value::String(String::new()));
        let mut value=json!({"kind":"success","value":object});
        let length=LIMIT-json_len(&value);
        value["value"][schema.fields[0]]=Value::String("x".repeat(length));
        assert_eq!(json_len(&value),LIMIT);
        assert!(encode_row(schema,&value["value"]).is_none());
        unsafe{ROW_FLAGS=2;RETURN_SCHEMA=schema.id;}
        assert_eq!(emit(value,0),0);assert_eq!(result_format(),0);
        unsafe{ROW_FLAGS=0;RETURN_SCHEMA=0;}
    }
    #[test]
    fn json_budget_matches_serializer() {
        for s in ["", "ascii", "é中😀", "\"\\\n\r\t\0\u{0008}\u{000c}", "\u{0001}\u{001f}"] {
            let v=json!({"kind":"success","value":{"a":s,"b":[null,true,false,-42]}});
            assert_eq!(json_len(&v),serde_json::to_vec(&v).unwrap().len());
        }
        assert_eq!(serde_json::to_vec(&json!({"kind":"success","value":null})).unwrap().len(),27+4);
        // Exhaustive byte values at every chunk position, plus Unicode and tails.
        for byte in 0..=127u8 {for pos in 0..24 {
            let mut bytes=vec![b'x';31];bytes[pos]=byte;
            let value=Value::String(String::from_utf8(bytes).unwrap());
            assert_eq!(json_len(&value),serde_json::to_vec(&value).unwrap().len());
        }}
    }
}
