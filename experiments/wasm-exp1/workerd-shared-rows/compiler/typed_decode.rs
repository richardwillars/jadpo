// Decode owned scalars directly, avoiding a generic JSON/map round trip.
impl Field {
    fn is_null(&self)->bool {matches!(self,Self::Null)}
    fn is_string(&self)->bool {matches!(self,Self::Text(_)|Self::Shared(_))}
    fn is_boolean(&self)->bool {matches!(self,Self::Bool(_))}
    fn as_str(&self)->Option<&str> {match self {Self::Text(s)=>Some(s),Self::Shared(s)=>Some(s.as_str()),_=>None}}
    fn as_i64(&self)->Option<i64> {if let Self::Int(n)=self {Some(*n)} else {None}}
    fn json_len(&self)->usize {match self {
        Self::Missing=>0,Self::Null=>4,Self::Bool(true)=>4,Self::Bool(false)=>5,
        Self::Text(s)=>json_text_len(s),Self::Shared(s)=>json_text_len(s.as_str()),Self::Int(n)=>n.to_string().len(),
    }}
}
fn typed_uuid(value:&Field)->bool {
    let Some(text)=value.as_str() else{return false;};let b=text.as_bytes();
    b.len()==36 && b.iter().enumerate().all(|(i,c)|if [8,13,18,23].contains(&i){*c==b'-'}else{c.is_ascii_hexdigit()})
        && matches!(b[14],b'1'..=b'5') && matches!(b[19],b'8'|b'9'|b'a'|b'b'|b'A'|b'B')
}
fn checked_utf8(bytes:&[u8])->Option<&str> {
    #[cfg(feature="vector")]
    {simdutf8::basic::from_utf8(bytes).ok()}
    #[cfg(not(feature="vector"))]
    {std::str::from_utf8(bytes).ok()}
}
// Explicit host/guest agreement: typed ingress enforces the original logical
// JSON envelope bound itself, including all escapes and independently of host.
static mut ROW_BUDGET_EXCEEDED:bool=false;
#[no_mangle]
pub extern "C" fn row_budget_exceeded()->i32 {unsafe{ROW_BUDGET_EXCEEDED as i32}}
#[no_mangle]
pub extern "C" fn row_budget_version()->i32 {1}
fn decode_typed_row(bytes:&[u8],expected:u32)->Option<CheckedRow> {decode_shared_row(bytes,expected,None)}
fn decode_shared_row(bytes:&[u8],expected:u32,owner:Option<(&Rc<Vec<u8>>,usize)>)->Option<CheckedRow> {
    if bytes.len()<40 || bytes.len()>LIMIT || &bytes[..4]!=b"JRW1" || bytes[4..36]!=ROW_DIGEST {return None;}
    let id=u32::from_le_bytes(bytes[36..40].try_into().ok()?);if id!=expected{return None;}
    let schema=row_schema(id)?;let mut pos=40;let mut fields=Vec::with_capacity(schema.fields.len());
    // Canonical JSON envelope size, independently recomputed inside the guest.
    // The host's shorter binary encoding cannot relax the existing JSON budget.
    let mut json_bound=29;let mut present=0;
    for key in schema.fields {
        let tag=*bytes.get(pos)?;pos+=1;
        let field=match tag {
            0=>Field::Missing,1=>Field::Null,3=>Field::Bool(false),4=>Field::Bool(true),
            2=>{
                let end=pos.checked_add(4)?;let len=u32::from_le_bytes(bytes.get(pos..end)?.try_into().ok()?) as usize;pos=end;
                let end=pos.checked_add(len)?;let s=checked_utf8(bytes.get(pos..end)?)?;
                let field=if let Some((buffer,offset))=owner {Field::Shared(SharedText{buffer:buffer.clone(),start:offset+pos,len})}else{Field::Text(s.to_owned())};pos=end;field
            },
            5=>{let end=pos.checked_add(8)?;let n=i64::from_le_bytes(bytes.get(pos..end)?.try_into().ok()?);pos=end;Field::Int(n)},
            _=>return None,
        };
        if !matches!(field,Field::Missing) {
            json_bound+=json_text_len(key)+1+field.json_len()+usize::from(present>0);present+=1;
        }
        fields.push(field);
    }
    if pos!=bytes.len(){return None;}
    if json_bound>LIMIT{unsafe{ROW_BUDGET_EXCEEDED=true;}return None;}
    // The generated query validates every field/refinement before promotion to
    // AppValue::Row. Neither decoding nor host validation supplies that proof.
    Some(CheckedRow{schema,fields,json_bound:Some(json_bound),origin:None})
}

#[cfg(test)]
mod typed_decode_tests {
    use super::*;
    #[test]
    fn direct_decode_matches_reference_json_budget_and_presence() {
        let schema=ROW_SCHEMAS.iter().find(|s|s.fields.len()>=4).unwrap();
        for text in ["", "x", "é中😀", "\"\\\n\r\t\0", "\u{feff}"] {
            let mut object=serde_json::Map::new();
            object.insert(schema.fields[0].to_owned(),Value::String(text.repeat(1024)));
            object.insert(schema.fields[1].to_owned(),Value::Null);
            object.insert(schema.fields[2].to_owned(),Value::Bool(false));
            let value=Value::Object(object);
            let bytes=encode_row(schema,&value).unwrap();
            let decoded=decode_typed_row(&bytes,schema.id).unwrap();
            assert_eq!(decoded.json_bound,Some(27+json_len(&value)));
            assert_eq!(AppValue::Row(Rc::new(decoded)).into_json(),value);
        }
    }
    #[test]
    fn provenance_does_not_survive_materialisation_or_an_unvalidated_result() {
        let schema=ROW_SCHEMAS.iter().find(|s|s.fields.len()==1).unwrap();
        let row=json!({schema.fields[0]:"x".repeat(2000)});
        let typed=AppValue::checked_row_origin(schema.name,row,Some(2100),Some(RowOrigin{request:7,operation:8}));
        let plain=typed.into_json();
        unsafe{ROW_FLAGS=6;RETURN_SCHEMA=schema.id;}
        assert_eq!(emit_app(AppValue::Json(plain)),0);assert_ne!(result_format(),2);
        unsafe{ROW_FLAGS=0;RETURN_SCHEMA=0;}
    }
}

#[cfg(test)]
mod shared_tests {
    use super::*;
    #[test]
    fn shared_strings_own_their_frame_and_materialization_is_independent() {
        let schema=ROW_SCHEMAS.iter().find(|s|s.fields.len()>=4).unwrap();
        let value=json!({schema.fields[0]:"a".repeat(2048),schema.fields[1]:"é中😀".repeat(512)});
        let encoded=encode_row(schema,&value).unwrap();
        let mut padded=vec![77;13];padded.extend_from_slice(&encoded);padded.extend_from_slice(&[99;17]);
        let buffer=Rc::new(padded);let weak=Rc::downgrade(&buffer);
        let decoded=decode_shared_row(&buffer[13..13+encoded.len()],schema.id,Some((&buffer,13))).unwrap();
        assert!(matches!(&decoded.fields[0],Field::Shared(s) if Rc::ptr_eq(&s.buffer,&buffer)));
        let app=AppValue::Row(Rc::new(decoded));drop(buffer);
        assert!(weak.upgrade().is_some());
        let mut changed=app.clone().into_json();changed[schema.fields[0]]=Value::String("changed".into());
        assert_eq!(app.into_json(),value);
        assert!(weak.upgrade().is_none());
        assert_eq!(changed[schema.fields[0]],"changed");
    }
}
