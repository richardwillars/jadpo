//! Shared credential verification, live authority, route boundary and SQL policy.
//! Hosts execute SQL/transactions and supply the clock/config; they do not select roles.
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
fn contract() -> &'static Value {
    static VALUE: OnceLock<Value> = OnceLock::new();
    VALUE.get_or_init(|| serde_json::from_str(include_str!("../../build/contract.json")).unwrap())
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
    let future = match make_entry(operation, body, ctx.clone()) {
        Ok(f) => f,
        Err(e) => return Ok(AppValue::Json(failure(e, false))),
    };
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
