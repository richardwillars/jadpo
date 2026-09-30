//! Shared credential verification, live authority, route boundary and SQL policy.
//! Hosts execute SQL/transactions and supply the clock/config; they do not select roles.
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
fn contract() -> &'static Value {
    static VALUE: OnceLock<Value> = OnceLock::new();
    VALUE.get_or_init(|| serde_json::from_str(include_str!("../../../capability-host/build/contract.json")).unwrap())
}
fn text(v: &Value) -> Result<&str, Fault> {
    v.as_str().ok_or(Fault::Internal(0))
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn quote(s: &str) -> String {
    assert!(!s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
    format!("\"{s}\"")
}
fn safe_integer(value: &Value) -> Option<i64> {
    let number = value.as_f64()?;
    (number.is_finite() && number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_991.0)
        .then_some(number as i64)
}
fn invalid() -> Fault {
    Fault::Http(401, "invalid_credentials")
}
fn unavailable() -> Fault {
    Fault::Http(503, "authentication_unavailable")
}
fn decode(s: &str) -> Result<Vec<u8>, Fault> {
    let bytes = B64.decode(s).map_err(|_| invalid())?;
    if B64.encode(&bytes) != s {
        return Err(invalid());
    }
    Ok(bytes)
}
async fn sql(
    ctx: InvocationContext,
    cap: &'static str,
    sql: String,
    params: Value,
    sid: u32,
) -> Result<Value, Fault> {
    let value = effect(ctx, cap, json!({"sql":sql,"params":params}), sid)
        .await?
        .value;
    if value.get("error").is_some() {
        return Err(Fault::Internal(sid));
    }
    Ok(value)
}
async fn session(ctx: InvocationContext, id: &str) -> Result<Value, Fault> {
    let rows = sql(
        ctx,
        "sql.query",
        "SELECT data, revoked FROM __jadpo_auth_sessions WHERE id = ?".into(),
        json!([id]),
        0,
    )
    .await
    .map_err(|_| unavailable())?;
    let rows = rows.as_array().ok_or_else(unavailable)?;
    if rows.is_empty() {
        return Err(invalid());
    }
    if rows.len() != 1 {
        return Err(unavailable());
    }
    let mut value: Value = serde_json::from_str(rows[0]["data"].as_str().ok_or_else(unavailable)?)
        .map_err(|_| unavailable())?;
    if !value.is_object() {
        return Err(unavailable());
    }
    value["revoked"] = json!(rows[0]["revoked"] != 0);
    let keys = [
        "id", "strategy", "subject", "userId", "expires", "verifier", "keyId", "revoked",
    ];
    if value.as_object().unwrap().len() != keys.len()
        || !keys.iter().all(|k| value.get(k).is_some())
        || !value["id"].as_str().map_or(false, identifier)
        || !value["strategy"].as_str().map_or(false, identifier)
        || !value["subject"]
            .as_str()
            .map_or(false, |s| !s.is_empty() && s.encode_utf16().count() <= 512)
        || !uuid(&value["userId"])
        || safe_integer(&value["expires"]).is_none()
        || !value["verifier"].is_string()
        || !value["keyId"].is_string()
    {
        return Err(unavailable());
    }
    Ok(value)
}
async fn authenticate(ctx: InvocationContext, request: &Value) -> Result<String, Fault> {
    let config = &contract()["auth"];
    let now = request["now"].as_i64().ok_or_else(unavailable)?;
    if now < 0 || now > 9_007_199_254_740_991 {
        return Err(unavailable());
    }
    let mut keys = Vec::new();
    for field in ["secretBinding", "previousBinding"] {
        let secret = &request["configuration"][text(&config[field])?];
        let bytes = decode(
            secret
                .as_str()
                .ok_or(Fault::Http(503, "authentication_misconfigured"))?,
        )
        .map_err(|_| Fault::Http(503, "authentication_misconfigured"))?;
        if bytes.len() != 32 {
            return Err(Fault::Http(503, "authentication_misconfigured"));
        }
        keys.push((B64.encode(Sha256::digest(&bytes))[..22].to_owned(), bytes));
    }
    if keys[0].0 == keys[1].0 {
        return Err(Fault::Http(503, "authentication_misconfigured"));
    }
    let header = request["authorization"].as_str().unwrap_or("");
    if header.encode_utf16().count() > 8192
        || request["cookie"]
            .as_str()
            .unwrap_or("")
            .encode_utf16()
            .count()
            > 16384
    {
        return Err(invalid());
    }
    if request["authorization"].is_null() {
        return Err(Fault::Http(401, "authentication_required"));
    }
    if header.split(',').count() != 1 {
        return Err(Fault::Http(401, "ambiguous_credentials"));
    }
    let header = header.trim();
    let split = header.find([' ', '\t']).ok_or_else(invalid)?;
    if !header[..split].eq_ignore_ascii_case("bearer") {
        return Err(invalid());
    }
    let credential = header[split..].trim_start_matches([' ', '\t']);
    if credential.is_empty()
        || credential.len() > 4096
        || credential
            .bytes()
            .any(|b| b == b' ' || b == b'\t' || b == b',')
    {
        return Err(invalid());
    }
    let parts: Vec<_> = credential.split('.').collect();
    if parts.len() != 3
        || parts[0] != "jdo1"
        || !identifier(parts[1])
        || decode(parts[2])?.len() != 32
    {
        return Err(invalid());
    }
    let record = session(ctx.clone(), parts[1]).await?;
    if record["id"] != parts[1]
        || record["revoked"] == true
        || safe_integer(&record["expires"]).unwrap() <= now
        || record["strategy"] != config["strategy"]
    {
        return Err(invalid());
    }
    let key = keys
        .iter()
        .find(|(id, _)| record["keyId"] == *id)
        .ok_or_else(invalid)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&key.1).map_err(|_| unavailable())?;
    mac.update(format!("opaque:{}:{credential}", text(&config["audience"])?).as_bytes());
    mac.verify_slice(&decode(text(&record["verifier"])?)?)
        .map_err(|_| invalid())?;
    // Match Bun's separate live-session and principal-resolution checks.
    let live = session(ctx.clone(), parts[1]).await?;
    if live["id"] != record["id"]
        || live["revoked"] == true
        || safe_integer(&live["expires"]).unwrap() <= now
        || live["strategy"] != record["strategy"]
        || live["subject"] != record["subject"]
        || live["userId"] != record["userId"]
    {
        return Err(invalid());
    }
    let identity = text(&config["identity"])?;
    let subject = text(&config["subject"])?;
    let active = text(&config["active"])?;
    let rows = sql(
        ctx,
        "sql.query",
        format!(
            "SELECT {}, {}, {} FROM {} WHERE {} = ? LIMIT 2",
            quote(identity),
            quote(subject),
            quote(active),
            quote(text(&config["table"])?),
            quote(subject)
        ),
        json!([record["subject"]]),
        0,
    )
    .await
    .map_err(|_| unavailable())?;
    let rows = rows.as_array().ok_or_else(unavailable)?;
    if rows.is_empty() {
        return Err(invalid());
    }
    if rows.len() != 1 {
        return Err(unavailable());
    }
    let mut user = rows[0].clone();
    if user[active] == 0 {
        user[active] = json!(false)
    } else if user[active] == 1 {
        user[active] = json!(true)
    }
    if !validate(text(&config["entity"])?, &user) {
        return Err(unavailable());
    }
    if user[active] != true {
        return Err(Fault::Domain(config["inactive"].as_str().unwrap()));
    }
    if user[identity] != record["userId"] || user[subject] != record["subject"] {
        return Err(unavailable());
    }
    Ok(text(&user[identity])?.to_owned())
}
fn group(fields: &Value, principal: &str, params: &mut Vec<Value>) -> Result<String, Fault> {
    let fields = fields.as_array().ok_or(Fault::Internal(0))?;
    if fields.is_empty() {
        return Err(Fault::Internal(0));
    }
    let mut terms = Vec::new();
    for field in fields {
        terms.push(format!("{} = ?", quote(text(field)?)));
        params.push(json!(principal));
    }
    Ok(format!("({})", terms.join(" OR ")))
}
// The trusted authority establishes the expected effect stream from the checked
// application. Only this side owns credentials, principal, SQL and transactions.
fn application_completion(result: &Outcome) -> Value {
    let value = match result {
        Ok(v) => json!({"kind":"success","value":v.clone().into_json()}),
        Err(Fault::Domain(name)) => json!({"kind":"domain","failure":name}),
        Err(Fault::Invalid) => json!({"kind":"invalid"}),
        _ => json!({"kind":"internal"}),
    };
    if within_limit(&value) {
        value
    } else {
        json!({"kind":"internal"})
    }
}
fn guest_row(ctx: &InvocationContext, sid: u32, value: &Value, write: bool) -> Value {
    let mut result = value.clone();
    let row = if write {
        result.get_mut("row")
    } else {
        Some(&mut result)
    };
    if let Some(row) = row.filter(|r| r.is_object()) {
        let principal = ctx.0.lock().unwrap().principal.clone().unwrap();
        let plan = contract()["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| {
                p["descriptor"]["semanticOperationId"] == sid
                    && p["effect"] == if write { "update" } else { "read" }
            })
            .unwrap();
        for (field, policy) in plan["fieldScopes"].as_object().unwrap() {
            let allowed = policy["read"]
                .as_array()
                .unwrap()
                .iter()
                .any(|binding| row[binding.as_str().unwrap()] == principal);
            if !allowed {
                row[field] = Value::Null;
            }
        }
    }
    result
}
pub(super) async fn scoped_storage(
    ctx: InvocationContext,
    args: Value,
    sid: u32,
    write: bool,
) -> Result<HostValue, Fault> {
    let mode = ctx.0.lock().unwrap().mode;
    let capability = if write {
        "entity.update"
    } else {
        "entity.read"
    };
    let mut scoped = json!({"plan":sid,"key":args["predicate"]["value"]});
    if write {
        scoped["changes"] = args["changes"].clone();
    }
    if mode == ExecutionMode::Guest {
        return effect(ctx, capability, scoped, sid).await;
    }
    if mode == ExecutionMode::Authority {
        effect(
            ctx.clone(),
            "guest.check",
            json!({"capability":capability,"args":scoped}),
            sid,
        )
        .await?;
    }
    let result = storage(ctx.clone(), args, sid, write).await;
    if mode == ExecutionMode::Authority {
        let reply = match &result {
            Ok(v) => json!({"kind":"success","value":guest_row(&ctx,sid,&v.value,write)}),
            Err(_) => json!({"kind":"internal"}),
        };
        effect(ctx, "guest.resume", reply, sid).await?;
    }
    result
}
pub(super) async fn storage(
    ctx: InvocationContext,
    args: Value,
    sid: u32,
    write: bool,
) -> Result<HostValue, Fault> {
    let plan = contract()["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| {
            p["descriptor"]["semanticOperationId"] == sid
                && p["effect"] == if write { "update" } else { "read" }
        })
        .ok_or(Fault::Internal(sid))?;
    let mut descriptor = args.clone();
    let changes = descriptor
        .as_object_mut()
        .ok_or(Fault::Internal(sid))?
        .remove("changes");
    let predicate = descriptor["predicate"]
        .as_object_mut()
        .ok_or(Fault::Internal(sid))?
        .remove("value")
        .ok_or(Fault::Internal(sid))?;
    if descriptor != plan["descriptor"] || (!write && changes.is_some()) {
        return Err(Fault::Internal(sid));
    }
    let entity = text(&descriptor["entity"])?;
    if !validate(
        &format!("{entity}.{}", text(&plan["identity"])?),
        &predicate,
    ) {
        return Err(Fault::Internal(sid));
    }
    let principal = ctx
        .0
        .lock()
        .unwrap()
        .principal
        .clone()
        .ok_or(Fault::Internal(sid))?;
    let fields = plan["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| quote(f.as_str().unwrap()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut params = Vec::new();
    let mut sets = Vec::new();
    let mut restrictions = Vec::new();
    if write {
        let changes = changes
            .as_ref()
            .and_then(Value::as_object)
            .ok_or(Fault::Internal(sid))?;
        if changes.is_empty() {
            return Err(Fault::Internal(sid));
        }
        for (field, value) in changes {
            if !plan["allowed"].as_array().unwrap().contains(&json!(field))
                || !validate(&format!("{entity}.{field}"), value)
            {
                return Err(Fault::Internal(sid));
            }
            sets.push(format!("{} = ?", quote(field)));
            params.push(value.clone());
            if let Some(policy) = plan["fieldScopes"].get(field) {
                restrictions.push(policy["update"].clone());
            }
        }
    }
    params.push(predicate);
    let mut predicates = vec![
        format!("{} = ?", quote(text(&plan["identity"])?)),
        group(&plan["scope"], &principal, &mut params)?,
    ];
    restrictions.extend(plan["readScopes"].as_array().unwrap().iter().cloned());
    for restriction in restrictions {
        predicates.push(group(&restriction, &principal, &mut params)?);
    }
    let where_sql = predicates.join(" AND ");
    let table = quote(text(&plan["table"])?);
    let statement = if write {
        format!(
            "UPDATE {table} SET {} WHERE {where_sql} RETURNING {fields}",
            sets.join(", ")
        )
    } else {
        format!("SELECT {fields} FROM {table} WHERE {where_sql} LIMIT 2")
    };
    let result = sql(
        ctx,
        if write { "sql.update" } else { "sql.query" },
        statement,
        json!(params),
        sid,
    )
    .await?;
    if write && result == json!({"constraint":true}) {
        return Ok(HostValue {
            value: json!({"status":"conflict"}),
        });
    }
    let rows = result.as_array().ok_or(Fault::Internal(sid))?;
    if rows.len() > 1 || rows.first().map_or(false, |r| !validate(entity, r)) {
        return Err(Fault::Internal(sid));
    }
    let value = if write {
        match rows.first() {
            Some(row) => json!({"status":"found","row":row}),
            None => json!({"status":"missing"}),
        }
    } else {
        rows.first().cloned().unwrap_or(Value::Null)
    };
    Ok(HostValue { value })
}
fn error(status: u16, code: &str, message: &str) -> Value {
    json!({"status":status,"body":{"error":{"code":code,"message":message}}})
}
fn failure(fault: Fault, authentication: bool) -> Value {
    match fault {
        Fault::Http(status, code) => error(status, code, "Authentication failed."),
        Fault::Invalid => error(400, "invalid_request", "Request validation failed."),
        Fault::Domain(name) => {
            let f = &contract()["failures"][name];
            error(
                f["status"].as_u64().unwrap() as u16,
                f["code"].as_str().unwrap(),
                f["message"].as_str().unwrap_or(if authentication {
                    "Authentication failed."
                } else {
                    "Request failed."
                }),
            )
        }
        Fault::Internal(_) => error(500, "internal_fault", "An internal error occurred."),
    }
}
pub(super) async fn request(ctx: InvocationContext, input: Value) -> Outcome {
    let route = contract()["routes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == input["path"] && r["method"] == input["method"]);
    let Some(route) = route else {
        return Ok(AppValue::Json(error(
            404,
            "route_not_found",
            "Route not found.",
        )));
    };
    let principal = match authenticate(ctx.clone(), &input).await {
        Ok(p) => p,
        Err(f) => return Ok(AppValue::Json(failure(f, true))),
    };
    ctx.0.lock().unwrap().principal = Some(principal);
    let body: Value = match input["body"]
        .as_str()
        .and_then(|s| serde_json::from_str(s).ok())
    {
        Some(v) => v,
        None => return Ok(AppValue::Json(failure(Fault::Invalid, false))),
    };
    let operation = route["operation"].as_i64().unwrap() as i32;
    let guest_input = body.clone();
    let authority = ctx.0.lock().unwrap().mode == ExecutionMode::Authority;
    let future = match make_entry(operation, body, ctx.clone()) {
        Ok(f) => f,
        Err(e) => return Ok(AppValue::Json(failure(e, false))),
    };
    let scope = ctx.0.lock().unwrap().request;
    if authority
        && effect(
            ctx.clone(),
            "guest.start",
            json!({"operation":operation,"scope":scope,"input":guest_input}),
            operation as u32,
        )
        .await
        .is_err()
    {
        return Ok(AppValue::Json(failure(
            Fault::Internal(operation as u32),
            false,
        )));
    }
    let atomic = route["atomic"] == true;
    if atomic
        && effect(
            ctx.clone(),
            "transaction.begin",
            Value::Null,
            operation as u32,
        )
        .await
        .is_err()
    {
        return Ok(AppValue::Json(failure(
            Fault::Internal(operation as u32),
            false,
        )));
    }
    let mut result = future.await;
    if let Ok(ref value) = result {
        let value = value.clone().into_json();
        if !validate(route["output"].as_str().unwrap(), &value)
            || !within_limit(&json!({"kind":"success","value":{"status":200,"body":value}}))
        {
            result = Err(Fault::Internal(operation as u32));
        }
    }
    if authority
        && effect(
            ctx.clone(),
            "guest.complete",
            application_completion(&result),
            operation as u32,
        )
        .await
        .is_err()
    {
        result = Err(Fault::Internal(operation as u32));
    }
    if atomic {
        let cap = if result.is_ok() {
            "transaction.commit"
        } else {
            "transaction.rollback"
        };
        if effect(ctx.clone(), cap, Value::Null, operation as u32)
            .await
            .is_err()
        {
            result = Err(Fault::Internal(operation as u32));
        }
    }
    Ok(AppValue::Json(match result {
        Ok(v) => json!({"status":200,"body":v.into_json()}),
        Err(e) => failure(e, false),
    }))
}

// The monitor accepts only the effect sequence declared for this checked
// fixture. Production lowering should emit this sequence beside the contract;
// keeping it explicit here makes the prototype's route scope reviewable.
fn monitor_sequence(operation: i32) -> &'static [(u32, bool)] {
    match operation {
        39 => &[(39, true)],
        43 => &[(43, false)],
        45 => &[(45, true)],
        46 => &[(46, false)],
        557 => &[(45, true), (45, true)],
        558 => &[(45, true)],
        _ => &[],
    }
}

fn monitor_pending(value: &Value, request: u32, operation: u32) -> Result<(String, Value), Fault> {
    let object = value.as_object().ok_or(Fault::Internal(operation))?;
    if object.len() != 5
        || object.get("kind").and_then(Value::as_str) != Some("pending")
        || object.get("requestId").and_then(Value::as_u64) != Some(request as u64)
        || object.get("operationId").and_then(Value::as_u64) != Some(operation as u64)
        || !object.contains_key("capability")
        || !object.contains_key("args")
    {
        return Err(Fault::Internal(operation));
    }
    let capability = object["capability"]
        .as_str()
        .ok_or(Fault::Internal(operation))?
        .to_owned();
    Ok((capability, object["args"].clone()))
}

fn monitor_storage_args(plan_id: u32, key: &Value, changes: Option<&Value>) -> Result<(Value, bool), Fault> {
    let plan = contract()["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|plan| plan["descriptor"]["semanticOperationId"] == plan_id)
        .ok_or(Fault::Internal(plan_id))?;
    let write = plan["effect"] == "update";
    let mut args = plan["descriptor"].clone();
    args["predicate"]["value"] = key.clone();
    if write {
        args["changes"] = changes.cloned().ok_or(Fault::Internal(plan_id))?;
    } else if changes.is_some() {
        return Err(Fault::Internal(plan_id));
    }
    Ok((args, write))
}

fn monitor_projection(operation: i32, row: &Value) -> Value {
    if operation == 43 {
        json!({"id":row["id"],"private_note":row["private_note"]})
    } else {
        json!({"id":row["id"],"title":row["title"]})
    }
}

fn monitor_expected(
    operation: i32,
    stored: &Value,
    write: bool,
) -> (Option<Value>, Option<&'static str>) {
    if operation == 558 {
        return (None, Some("ChangeRejected"));
    }
    if write {
        match stored["status"].as_str() {
            Some("found") => (Some(monitor_projection(operation, &stored["row"])), None),
            Some("missing") => (None, Some("NoteMissing")),
            Some("conflict") => (None, Some("NoteConflict")),
            _ => (None, None),
        }
    } else if stored.is_null() {
        (None, Some("NoteMissing"))
    } else {
        (Some(monitor_projection(operation, stored)), None)
    }
}

fn monitor_completion(
    route: &Value,
    value: &Value,
    expected_output: Option<&Value>,
    expected_failure: Option<&'static str>,
    storage_fault: bool,
) -> Result<Value, Fault> {
    match value["kind"].as_str() {
        Some("success") => {
            let result = value.get("value").ok_or(Fault::Internal(0))?;
            if expected_failure.is_some()
                || expected_output != Some(result)
                || !validate(route["output"].as_str().unwrap(), result)
                || !within_limit(&json!({"kind":"success","value":{"status":200,"body":result}}))
            {
                return Err(Fault::Internal(route["operation"].as_u64().unwrap() as u32));
            }
            Ok(result.clone())
        }
        Some("domain") => match (expected_failure, value["failure"].as_str()) {
            (Some(expected), Some(actual)) if expected == actual => Err(Fault::Domain(expected)),
            _ => Err(Fault::Internal(route["operation"].as_u64().unwrap() as u32)),
        },
        Some("invalid") => Err(Fault::Invalid),
        Some("internal") if storage_fault => Err(Fault::Internal(route["operation"].as_u64().unwrap() as u32)),
        _ => Err(Fault::Internal(route["operation"].as_u64().unwrap() as u32)),
    }
}

pub(super) async fn monitor_request(ctx: InvocationContext, input: Value) -> Outcome {
    let route = contract()
        ["routes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == input["path"] && r["method"] == input["method"]);
    let Some(route) = route else {
        return Ok(AppValue::Json(error(404, "route_not_found", "Route not found.")));
    };
    let principal = match authenticate(ctx.clone(), &input).await {
        Ok(p) => p,
        Err(f) => return Ok(AppValue::Json(failure(f, true))),
    };
    ctx.0.lock().unwrap().principal = Some(principal);
    let body: Value = match input["body"]
        .as_str()
        .and_then(|s| serde_json::from_str(s).ok())
    {
        Some(v) => v,
        None => return Ok(AppValue::Json(failure(Fault::Invalid, false))),
    };
    let operation = route["operation"].as_i64().unwrap() as i32;
    if !checked_input(operation, &body) {
        return Ok(AppValue::Json(failure(Fault::Invalid, false)));
    }
    let expected = monitor_sequence(operation);
    if expected.is_empty() {
        return Ok(AppValue::Json(failure(Fault::Internal(operation as u32), false)));
    }
    let scope = ctx.0.lock().unwrap().request;
    let mut guest = effect(
        ctx.clone(),
        "guest.start",
        json!({"operation":operation,"scope":scope,"input":body}),
        operation as u32,
    )
    .await?
    .value;
    let atomic = route["atomic"] == true;
    if atomic {
        effect(ctx.clone(), "transaction.begin", Value::Null, operation as u32).await?;
    }
    let mut index = 0usize;
    let mut expected_output = None;
    let mut expected_failure = if operation == 558 {
        Some("ChangeRejected")
    } else {
        None
    };
    let mut storage_fault = false;
    let mut outcome: Result<Value, Fault> = loop {
        if guest["kind"] == "pending" {
            let guest_request = guest["requestId"].as_u64().ok_or(Fault::Internal(operation as u32))? as u32;
            let guest_operation = guest["operationId"].as_u64().ok_or(Fault::Internal(operation as u32))? as u32;
            if guest_request != scope || guest_operation != (index as u32 + 1) {
                break Err(Fault::Internal(operation as u32));
            }
            let (capability, args) = monitor_pending(&guest, scope, guest_operation)?;
            let (plan, write) = expected.get(index).copied().ok_or(Fault::Internal(operation as u32))?;
            let wanted = if write { "entity.update" } else { "entity.read" };
            if capability != wanted
                || args["plan"].as_u64() != Some(plan as u64)
                || !args.as_object().map_or(false, |object| {
                    object.len() == if write { 3 } else { 2 }
                        && object.contains_key("plan")
                        && object.contains_key("key")
                        && (write == object.contains_key("changes"))
                })
            {
                break Err(Fault::Internal(operation as u32));
            }
            let storage_args = match monitor_storage_args(plan, &args["key"], args.get("changes")) {
                Ok((value, actual_write)) if actual_write == write => value,
                _ => break Err(Fault::Internal(operation as u32)),
            };
            let reply = match storage(ctx.clone(), storage_args, plan, write).await {
                Ok(value) => {
                    let visible = guest_row(&ctx, plan, &value.value, write);
                    let (output, failure) = monitor_expected(operation, &visible, write);
                    expected_output = output;
                    expected_failure = failure.or(expected_failure);
                    json!({"kind":"success","value":visible})
                }
                Err(_) => {
                    storage_fault = true;
                    json!({"kind":"internal"})
                }
            };
            guest = effect(
                ctx.clone(),
                "guest.resume",
                json!({"requestId":guest_request,"operationId":guest_operation,"result":reply}),
                operation as u32,
            )
            .await?
            .value;
            index += 1;
            continue;
        }
        if index != expected.len() {
            break Err(Fault::Internal(operation as u32));
        }
        break monitor_completion(route, &guest, expected_output.as_ref(), expected_failure, storage_fault);
    };
    if atomic {
        let capability = if outcome.is_ok() {
            "transaction.commit"
        } else {
            "transaction.rollback"
        };
        if effect(ctx.clone(), capability, Value::Null, operation as u32)
            .await
            .is_err()
        {
            outcome = Err(Fault::Internal(operation as u32));
        }
    }
    Ok(AppValue::Json(match outcome {
        Ok(value) => json!({"status":200,"body":value}),
        Err(fault) => failure(fault, false),
    }))
}
