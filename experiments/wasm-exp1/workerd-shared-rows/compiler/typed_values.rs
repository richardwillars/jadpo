// Immutable compiler-internal rows. No references into exported Wasm memory.
// Scalars and presence are stored in checked-schema order; sharing a row never
// duplicates its text. Generic inputs, updates and control messages remain JSON.
use std::rc::Rc;

#[derive(Clone)]
enum AppValue { Json(Value), Row(Rc<CheckedRow>) }
#[derive(Debug)]
enum Field { Missing, Null, Text(String), Shared(SharedText), Bool(bool), Int(i64) }
#[derive(Debug)]
struct SharedText {buffer:Rc<Vec<u8>>,start:usize,len:usize}
impl SharedText {
    fn as_str(&self)->&str {
        // Constructed only after checked_utf8 validated this exact immutable
        // range. The Rc owns the bytes; no reference points into a dropped frame.
        unsafe{std::str::from_utf8_unchecked(&self.buffer[self.start..self.start+self.len])}
    }
}
#[derive(Clone,Copy)]
struct RowOrigin { request:u32, operation:u32 }
struct CheckedRow {
    origin: Option<RowOrigin>,
    schema: &'static RowSchema,
    fields: Vec<Field>,
    // A canonical flat-row success envelope cannot exceed the original, accepted
    // JSON success envelope. Ownership and immutability preserve this proof.
    // The bound is obtained locally at resume, never accepted as host metadata.
    json_bound: Option<usize>,
}
impl Field {
    fn to_json(&self) -> Option<Value> { Some(match self {
        Self::Missing => return None, Self::Null => Value::Null,
        Self::Text(s) => Value::String(s.clone()), Self::Shared(s)=>Value::String(s.as_str().to_owned()), Self::Bool(b) => Value::Bool(*b),
        Self::Int(n) => Value::Number((*n).into()),
    }) }
    fn into_json(self) -> Option<Value> { Some(match self {
        Self::Missing => return None, Self::Null => Value::Null,
        Self::Text(s) => Value::String(s), Self::Shared(s)=>Value::String(s.as_str().to_owned()), Self::Bool(b) => Value::Bool(b),
        Self::Int(n) => Value::Number(n.into()),
    }) }
}
impl AppValue {
    // Called only after the generated entity validator succeeds. Unsupported
    // record shapes retain the reference representation and its size checks.
    fn checked_row(name: &str, row: Value, json_bound: Option<usize>) -> Self {
        Self::checked_row_origin(name,row,json_bound,None)
    }
    fn checked_row_origin(name:&str,row:Value,json_bound:Option<usize>,origin:Option<RowOrigin>)->Self {
        let schema=match row_schema(row_schema_for_name(name)) {Some(s)=>s,None=>return Self::Json(row)};
        let eligible=row.as_object().map_or(false,|o|
            o.keys().all(|k|schema.fields.contains(&k.as_str())) &&
            o.values().all(|v|v.is_null()||v.is_string()||v.is_boolean()||v.as_i64().is_some()));
        if !eligible || !row.as_object().unwrap().values().any(|v|v.as_str().map_or(false,|s|s.len()>=1024)) {return Self::Json(row);}
        let Value::Object(mut object)=row else {unreachable!()};
        let fields=schema.fields.iter().map(|key|match object.remove(*key) {
            None=>Field::Missing,Some(Value::Null)=>Field::Null,
            Some(Value::String(s))=>Field::Text(s),Some(Value::Bool(b))=>Field::Bool(b),
            Some(Value::Number(n))=>Field::Int(n.as_i64().unwrap()),_=>unreachable!(),
        }).collect();
        Self::Row(Rc::new(CheckedRow {schema,fields,json_bound,origin}))
    }
    fn into_json(self) -> Value {match self {
        Self::Json(v)=>v,
        Self::Row(row)=>{
            let object=match Rc::try_unwrap(row) {
                Ok(row)=>row.schema.fields.iter().zip(row.fields).filter_map(|(k,v)|v.into_json().map(|v|((*k).to_owned(),v))).collect(),
                Err(row)=>row.schema.fields.iter().zip(&row.fields).filter_map(|(k,v)|v.to_json().map(|v|((*k).to_owned(),v))).collect(),
            };
            Value::Object(object)
        },
    }}
}
impl CheckedRow {
    fn large(&self)->bool {self.fields.iter().any(|f|f.as_str().map_or(false,|s|s.len()>=1024))}
    fn binary(&self)->Option<Vec<u8>> {
        let capacity=40+self.fields.len()*9+self.fields.iter().map(|f|f.as_str().map_or(0,str::len)).sum::<usize>();
        let mut bytes=Vec::with_capacity(capacity);
        bytes.extend_from_slice(b"JRW1");bytes.extend_from_slice(&ROW_DIGEST);bytes.extend_from_slice(&self.schema.id.to_le_bytes());
        for field in &self.fields {match field {
            Field::Missing=>bytes.push(0),Field::Null=>bytes.push(1),
            Field::Text(s)=>{bytes.push(2);bytes.extend_from_slice(&(s.len() as u32).to_le_bytes());bytes.extend_from_slice(s.as_bytes());},
            Field::Shared(s)=>{let s=s.as_str();bytes.push(2);bytes.extend_from_slice(&(s.len() as u32).to_le_bytes());bytes.extend_from_slice(s.as_bytes());},
            Field::Bool(false)=>bytes.push(3),Field::Bool(true)=>bytes.push(4),
            Field::Int(n)=>{bytes.push(5);bytes.extend_from_slice(&n.to_le_bytes());},
        }}
        if bytes.len()>LIMIT {None} else {Some(bytes)}
    }
}
fn emit_app(value:AppValue)->i32 {
    if let AppValue::Row(row)=&value {
        if unsafe{ROW_FLAGS&4!=0 && RETURN_SCHEMA==row.schema.id} && row.large() && row.json_bound.map_or(false,|n|n<=LIMIT) {
            if let Some(origin)=row.origin {
                let mut bytes=Vec::with_capacity(48);bytes.extend_from_slice(b"JRR1");bytes.extend_from_slice(&ROW_DIGEST);
                bytes.extend_from_slice(&row.schema.id.to_le_bytes());bytes.extend_from_slice(&origin.request.to_le_bytes());bytes.extend_from_slice(&origin.operation.to_le_bytes());
                unsafe{OUTPUT=bytes;OUTPUT_FORMAT=2;}return 0;
            }
        }
        if unsafe{ROW_FLAGS&2!=0 && RETURN_SCHEMA==row.schema.id} && row.large() && row.json_bound.map_or(false,|bound|bound<=LIMIT) {
            if let Some(bytes)=row.binary() {unsafe{OUTPUT=bytes;OUTPUT_FORMAT=1;}return 0;}
        }
    }
    // Transformations, unknown provenance and oversized binary frames use the
    // reference path, which computes/enforces the exact outgoing JSON budget.
    {
        let mut envelope=serde_json::Map::new();
        envelope.insert("kind".to_owned(),Value::String("success".to_owned()));
        envelope.insert("value".to_owned(),value.into_json());
        emit(Value::Object(envelope),0)
    }
}

#[cfg(test)]
mod typed_tests {
    use super::*;
    #[test]
    fn shared_row_has_owned_storage_and_materialisation_drops_proof() {
        let schema=ROW_SCHEMAS.iter().find(|s|s.fields.len()==1).unwrap();
        let row=json!({schema.fields[0]:"x".repeat(4096)});
        let value=AppValue::checked_row(schema.name,row.clone(),Some(4200));
        let clone=value.clone();
        match (&value,&clone) {(AppValue::Row(a),AppValue::Row(b))=>assert!(Rc::ptr_eq(a,b)),_=>panic!()};
        let mut changed=clone.into_json();changed[schema.fields[0]]=Value::String("y".repeat(LIMIT));
        assert_eq!(value.into_json(),row);
        unsafe{ROW_FLAGS=2;RETURN_SCHEMA=schema.id;}
        assert_eq!(emit_app(AppValue::Json(changed)),3);
        unsafe{ROW_FLAGS=0;RETURN_SCHEMA=0;}
    }
    #[test]
    fn unproven_rows_still_enforce_json_budget() {
        let schema=ROW_SCHEMAS.iter().find(|s|s.fields.len()==1).unwrap();
        let row=json!({schema.fields[0]:"\n".repeat(34000)});
        let value=AppValue::checked_row(schema.name,row,None);
        unsafe{ROW_FLAGS=2;RETURN_SCHEMA=schema.id;}
        assert_eq!(emit_app(value),3);
        unsafe{ROW_FLAGS=0;RETURN_SCHEMA=0;}
    }
}
