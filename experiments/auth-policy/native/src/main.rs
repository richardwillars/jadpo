//! Local HTTP/SQLite adapter. Authentication and policy decisions are in the shared crate.
use hyper::{
    body::HttpBody,
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use jadpo_auth_policy_core::{Invocation, Step};
use rusqlite::{
    params_from_iter,
    types::{Value as SqlValue, ValueRef},
    Connection,
};
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
struct Host {
    db: Connection,
    trace: Vec<Value>,
}
fn rows(db: &Connection, sql: &str, params: &Value) -> rusqlite::Result<Value> {
    let values = params
        .as_array()
        .ok_or(rusqlite::Error::InvalidQuery)?
        .iter()
        .map(|v| match v {
            Value::Null => Ok(SqlValue::Null),
            Value::String(s) => Ok(SqlValue::Text(s.clone())),
            Value::Bool(b) => Ok(SqlValue::Integer(*b as i64)),
            Value::Number(n) => n
                .as_i64()
                .map(SqlValue::Integer)
                .ok_or(rusqlite::Error::InvalidQuery),
            _ => Err(rusqlite::Error::InvalidQuery),
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut s = db.prepare_cached(sql)?;
    let names = s
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut cursor = s.query(params_from_iter(values))?;
    let mut result = Vec::new();
    while let Some(row) = cursor.next()? {
        let mut value = serde_json::Map::new();
        for (i, name) in names.iter().enumerate() {
            value.insert(
                name.clone(),
                match row.get_ref(i)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Text(b) => {
                        json!(std::str::from_utf8(b).map_err(|_| rusqlite::Error::InvalidQuery)?)
                    }
                    ValueRef::Integer(n) => json!(n),
                    ValueRef::Real(n) => json!(n),
                    _ => return Err(rusqlite::Error::InvalidQuery),
                },
            );
        }
        result.push(Value::Object(value));
    }
    Ok(json!(result))
}
impl Host {
    fn recover(&self) {
        if !self.db.is_autocommit() {
            self.db
                .execute_batch("ROLLBACK")
                .expect("failed transaction recovery");
        }
    }
    fn reply(&mut self, cap: &str, args: &Value) -> Value {
        self.trace.push(json!({"capability":cap,"args":args}));
        let value = match cap {
            "sql.query" | "sql.update" => {
                if cap == "sql.update" && self.db.is_autocommit() {
                    return json!({"kind":"internal"});
                }
                match rows(
                    &self.db,
                    args["sql"].as_str().unwrap_or(""),
                    &args["params"],
                ) {
                    Ok(v) => v,
                    Err(rusqlite::Error::SqliteFailure(e, _))
                        if cap == "sql.update"
                            && e.code == rusqlite::ErrorCode::ConstraintViolation =>
                    {
                        json!({"constraint":true})
                    }
                    Err(_) => json!({"error":"database"}),
                }
            }
            "transaction.begin" | "transaction.commit" | "transaction.rollback" => {
                let sql = match cap {
                    "transaction.begin" => "BEGIN IMMEDIATE",
                    "transaction.commit" => "COMMIT",
                    _ => "ROLLBACK",
                };
                if self.db.execute_batch(sql).is_err() {
                    return json!({"kind":"internal"});
                }
                Value::Null
            }
            _ => return json!({"kind":"internal"}),
        };
        json!({"kind":"success","value":value})
    }
    fn invoke(&mut self, input: Value) -> Value {
        let mut call = Invocation::new(1, -1, input);
        let mut step = call.poll();
        let result = loop {
            match step {
                Step::Pending(p) => {
                    let reply = self.reply(p.capability, &p.args);
                    step = call.resume(p.request, p.operation, reply)
                }
                Step::Complete(c) => break c.value,
            }
        };
        // Cancellation, malformed frames, and commit errors cannot leave an open transaction.
        self.recover();
        if result["kind"] == "success" {
            result["value"].clone()
        } else {
            json!({"status":500,"body":{"error":{"code":"internal_fault","message":"An internal error occurred."}}})
        }
    }
}
fn response(status: u16, mut body: Value) -> Response<Body> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let request_id = format!(
        "req_{}_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    if let Some(error) = body.get_mut("error").and_then(Value::as_object_mut) {
        error.insert("request_id".into(), json!(request_id));
    }
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("cache-control", "no-store")
        .header("x-request-id", request_id)
        .body(Body::from(body.to_string()))
        .unwrap()
}
async fn handle(
    mut request: Request<Body>,
    host: Arc<Mutex<Host>>,
    configuration: Arc<Value>,
    control: Arc<String>,
) -> Result<Response<Body>, Infallible> {
    let path = request.uri().path().to_owned();
    let method = request.method().as_str().to_owned();
    let auth = request
        .headers()
        .get_all("authorization")
        .iter()
        .map(|v| v.to_str().unwrap_or("invalid"))
        .collect::<Vec<_>>();
    let authorization = if auth.is_empty() {
        Value::Null
    } else {
        json!(auth.join(", "))
    };
    let cookie = request
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let is_control = request
        .headers()
        .get("x-experiment-control")
        .and_then(|v| v.to_str().ok())
        == Some(control.as_str());
    let mut bytes = Vec::new();
    while let Some(chunk) = request.body_mut().data().await {
        let Ok(chunk) = chunk else {
            return Ok(response(400, json!({"error":{"code":"invalid_request"}})));
        };
        if bytes.len() + chunk.len() > 32768 {
            return Ok(response(413, json!({"error":{"code":"request_too_large"}})));
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut host = host.lock().unwrap();
    if path == "/__control" && is_control {
        let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        let result = if body["trace"] == true {
            json!(std::mem::take(&mut host.trace))
        } else {
            match rows(
                &host.db,
                body["sql"].as_str().unwrap_or(""),
                &body["params"],
            ) {
                Ok(v) => v,
                Err(_) => json!({"error":"control_sql"}),
            }
        };
        return Ok(response(200, result));
    }
    if path == "/health" {
        return Ok(response(200, json!({"ready":true})));
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let result=host.invoke(json!({"path":path,"method":method,"authorization":authorization,"cookie":cookie,"body":String::from_utf8_lossy(&bytes),"now":now,"configuration":*configuration}));
    Ok(response(
        result["status"].as_u64().unwrap_or(500) as u16,
        result["body"].clone(),
    ))
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let path = std::env::var("SQLITE_PATH").expect("SQLITE_PATH required");
    let db = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
        .expect("existing fixture database required");
    let journal = std::env::var("JOURNAL").unwrap_or("WAL".into());
    assert!(journal == "WAL" || journal == "DELETE");
    db.execute_batch(&format!("PRAGMA journal_mode={journal};"))
        .unwrap();
    db.execute_batch("PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=0;")
        .unwrap();
    db.set_prepared_statement_cache_capacity(128);
    let host = Arc::new(Mutex::new(Host { db, trace: vec![] }));
    let contract: Value = serde_json::from_str(include_str!("../../build/contract.json")).unwrap();
    let mut config = serde_json::Map::new();
    for k in ["secretBinding", "previousBinding"] {
        let binding = contract["auth"][k].as_str().unwrap();
        config.insert(
            binding.into(),
            json!(std::env::var(binding).unwrap_or_default()),
        );
    }
    let config = Arc::new(Value::Object(config));
    let control = Arc::new(
        std::env::var("TEST_CONTROL_TOKEN").expect("local harness control token required"),
    );
    let (pragmas, sqlite_version) = {
        let state = host.lock().unwrap();
        let mut values = serde_json::Map::new();
        for key in [
            "journal_mode",
            "synchronous",
            "foreign_keys",
            "busy_timeout",
            "fullfsync",
            "wal_autocheckpoint",
        ] {
            let result = rows(&state.db, &format!("PRAGMA {key}"), &json!([])).unwrap();
            values.insert(
                key.into(),
                result[0]
                    .as_object()
                    .unwrap()
                    .values()
                    .next()
                    .unwrap()
                    .clone(),
            );
        }
        (json!([values]), rusqlite::version())
    };
    let service = make_service_fn(move |_| {
        let h = host.clone();
        let c = config.clone();
        let t = control.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |r| {
                handle(r, h.clone(), c.clone(), t.clone())
            }))
        }
    });
    let port = std::env::var("PORT")
        .unwrap_or("3000".into())
        .parse::<u16>()
        .unwrap();
    let server = Server::bind(&([127, 0, 0, 1], port).into()).serve(service);
    println!(
        "{}",
        json!({"url":format!("http://{}/",server.local_addr()),"pid":std::process::id(),"target":"native","pragmas":pragmas,"sqliteVersion":sqlite_version})
    );
    server.await.unwrap();
}

#[cfg(test)]
mod tests;
