use crate::shared;
use rusqlite::{
    params_from_iter,
    types::{Value as SqlValue, ValueRef},
    Connection, TransactionBehavior,
};
use serde_json::{json, Value};

pub struct Storage {
    pub db: Connection,
    pub config: Value,
    pub trace: Vec<Value>,
    pub tracing: bool,
}
fn id(s: &str) -> String {
    assert!(!s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
    format!("\"{s}\"")
}
fn sql_value(v: &Value) -> Result<SqlValue, ()> {
    match v {
        Value::Null => Ok(SqlValue::Null),
        Value::String(s) => Ok(SqlValue::Text(s.clone())),
        _ => Err(()),
    }
}
fn rows(db: &Connection, sql: &str, params: &[SqlValue]) -> rusqlite::Result<Vec<Value>> {
    let mut statement = db.prepare_cached(sql)?;
    let names = statement
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut cursor = statement.query(params_from_iter(params))?;
    let mut result = Vec::new();
    while let Some(row) = cursor.next()? {
        let mut object = serde_json::Map::new();
        for (i, name) in names.iter().enumerate() {
            let v = match row.get_ref(i)? {
                ValueRef::Null => Value::Null,
                ValueRef::Text(b) => Value::String(
                    std::str::from_utf8(b)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?
                        .to_string(),
                ),
                ValueRef::Integer(n) => json!(n),
                ValueRef::Real(n) => json!(n),
                ValueRef::Blob(_) => return Err(rusqlite::Error::InvalidQuery),
            };
            object.insert(name.clone(), v);
        }
        result.push(Value::Object(object));
    }
    Ok(result)
}
impl Storage {
    pub fn open(path: &str, journal: &str) -> Self {
        assert!(journal == "WAL" || journal == "DELETE");
        let db = Connection::open(path).unwrap();
        db.set_prepared_statement_cache_capacity(128);
        db.execute_batch(&format!("PRAGMA journal_mode={journal}; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA fullfsync=OFF; PRAGMA wal_autocheckpoint=1000; PRAGMA busy_timeout=0;")).unwrap();
        let config: Value = serde_json::from_str(include_str!("../../build/storage.json")).unwrap();
        db.execute_batch(config["create"].as_str().unwrap())
            .unwrap();
        Self {
            db,
            config,
            trace: vec![],
            tracing: false,
        }
    }
    pub fn pragmas(&self) -> Value {
        let mut p = serde_json::Map::new();
        for key in [
            "journal_mode",
            "synchronous",
            "foreign_keys",
            "fullfsync",
            "wal_autocheckpoint",
            "busy_timeout",
            "page_size",
        ] {
            p.insert(
                key.into(),
                rows(&self.db, &format!("PRAGMA {key}"), &[]).unwrap()[0][if key == "busy_timeout" { "timeout" } else { key }].clone(),
            );
        }
        p.insert("sqlite_version".into(), json!(rusqlite::version()));
        p.insert(
            "compile_options".into(),
            json!(rows(&self.db, "PRAGMA compile_options", &[]).unwrap()),
        );
        Value::Object(p)
    }
    pub fn snapshot(&self) -> Value {
        json!(rows(
            &self.db,
            &format!(
                "SELECT * FROM {} ORDER BY {}",
                self.config["table"].as_str().unwrap(),
                id(self.config["identity"].as_str().unwrap())
            ),
            &[]
        )
        .unwrap())
    }
    pub fn reset(&self, seed: &Value) {
        let tx = self.db.unchecked_transaction().unwrap();
        tx.execute_batch(&format!(
            "DELETE FROM {}",
            self.config["table"].as_str().unwrap()
        ))
        .unwrap();
        let fields = self.config["fields"].as_array().unwrap();
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            self.config["table"].as_str().unwrap(),
            fields
                .iter()
                .map(|f| id(f.as_str().unwrap()))
                .collect::<Vec<_>>()
                .join(","),
            vec!["?"; fields.len()].join(",")
        );
        for row in seed.as_array().unwrap() {
            tx.execute(
                &sql,
                params_from_iter(
                    fields
                        .iter()
                        .map(|f| sql_value(&row[f.as_str().unwrap()]).unwrap()),
                ),
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    pub fn call(&mut self, name: &str, input: Value, principal: &Value) -> Value {
        let entry = self.config["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == name);
        let Some(entry) = entry else {
            return json!({"kind":"invalid"});
        };
        let op = entry["semanticId"].as_i64().unwrap() as i32;
        let atomic = entry["atomic"].as_bool().unwrap();
        if !shared::checked_input(op, &input) {
            return json!({"kind":"invalid"});
        }
        let config = &self.config;
        let trace = &mut self.trace;
        let tracing = self.tracing;
        if atomic {
            let tx = match self
                .db
                .transaction_with_behavior(TransactionBehavior::Immediate)
            {
                Ok(t) => t,
                Err(_) => return json!({"kind":"internal"}),
            };
            let result = shared::invoke(op, input, |cap, args| {
                execute(&tx, config, cap, args, principal, true, trace, tracing)
                    .unwrap_or_else(|_| json!({"kind":"internal"}))
            });
            let completion = if result["kind"] == "success" {
                tx.commit()
            } else {
                tx.rollback()
            };
            if completion.is_err() {
                return json!({"kind":"internal"});
            }
            result
        } else {
            shared::invoke(op, input, |cap, args| {
                execute(
                    &self.db, config, cap, args, principal, false, trace, tracing,
                )
                .unwrap_or_else(|_| json!({"kind":"internal"}))
            })
        }
    }
}
fn execute(
    db: &Connection,
    config: &Value,
    cap: &str,
    args: &Value,
    principal: &Value,
    atomic: bool,
    trace: &mut Vec<Value>,
    tracing: bool,
) -> Result<Value, ()> {
    let effect = match cap {
        "storage.read" => "read",
        "storage.update" if atomic => "update",
        _ => return Err(()),
    };
    let plan = config["plans"]
        .as_array()
        .ok_or(())?
        .iter()
        .find(|p| {
            p["effect"] == effect
                && p["descriptor"]["semanticOperationId"] == args["semanticOperationId"]
        })
        .ok_or(())?;
    let mut descriptor = args.clone();
    let changes = descriptor.as_object_mut().ok_or(())?.remove("changes");
    if effect == "read" && changes.is_some() {
        return Err(());
    }
    let predicate = descriptor["predicate"]
        .as_object_mut()
        .ok_or(())?
        .remove("value")
        .ok_or(())?;
    if descriptor != plan["descriptor"] {
        return Err(());
    }
    let entity = config["entity"].as_str().ok_or(())?;
    let identity = config["identity"].as_str().ok_or(())?;
    if !shared::valid(&format!("{entity}.{identity}"), &predicate) {
        return Err(());
    }
    let owner = &principal["id"];
    if principal["entity"] != plan["principal"]
        || !shared::valid(
            &format!("{entity}.{}", plan["scope"].as_str().ok_or(())?),
            owner,
        )
    {
        return Err(());
    }
    let where_sql = format!(
        "{} = ? AND ({} = ?)",
        id(identity),
        id(plan["scope"].as_str().ok_or(())?)
    );
    let fields = config["fields"]
        .as_array()
        .ok_or(())?
        .iter()
        .map(|f| id(f.as_str().unwrap()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut parameters = vec![];
    let sql = if effect == "read" {
        format!(
            "SELECT {fields} FROM {} WHERE {where_sql} LIMIT 2",
            config["table"].as_str().ok_or(())?
        )
    } else {
        let changes = changes.as_ref().and_then(Value::as_object).ok_or(())?;
        for (field, value) in changes {
            if !plan["allowed"]
                .as_array()
                .ok_or(())?
                .contains(&json!(field))
            {
                return Err(());
            }
            // Validate each supplied field using the compiler's exact effective type.
            // Nullability is checked by complete returned-row validation too.
            if !(value.is_null() && config["nullable"][field] == true)
                && !shared::valid(&format!("{entity}.{field}"), value)
            {
                return Err(());
            }
            parameters.push(sql_value(value)?);
        }
        if changes.is_empty() {
            return Ok(json!({"kind":"success","value":{"status":"empty"}}));
        }
        format!(
            "UPDATE {} SET {} WHERE {where_sql} RETURNING {fields}",
            config["table"].as_str().ok_or(())?,
            changes
                .keys()
                .map(|k| format!("{} = ?", id(k)))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    parameters.push(sql_value(&predicate)?);
    parameters.push(sql_value(owner)?);
    if tracing {
        trace.push(json!({"sql":sql,"parameters":parameters.iter().map(|v|match v{SqlValue::Null=>Value::Null,SqlValue::Text(s)=>json!(s),_=>Value::Null}).collect::<Vec<_>>(),"atomic":atomic}));
    }
    let result = match rows(db, &sql, &parameters) {
        Ok(r) => r,
        Err(rusqlite::Error::SqliteFailure(e, _))
            if effect == "update" && e.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            return Ok(json!({"kind":"success","value":{"status":"conflict"}}))
        }
        Err(_) => return Err(()),
    };
    if result.len() > 1 {
        return Err(());
    }
    if let Some(row) = result.first() {
        if !shared::valid(entity, row) {
            return Err(());
        }
    }
    Ok(
        json!({"kind":"success","value":if effect=="read" {result.into_iter().next().unwrap_or(Value::Null)} else {match result.into_iter().next(){Some(row)=>json!({"status":"found","row":row}),None=>json!({"status":"missing"})}}}),
    )
}

#[cfg(test)]
mod native_storage_tests {
    use super::*;
    #[test]
    fn renamed_schema_refinement_and_host_authority() {
        let mut s = Storage::open(":memory:", "DELETE");
        let config = s.config.clone();
        let read = config["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["effect"] == "read")
            .unwrap();
        let scope = read["scope"].as_str().unwrap();
        let entity = config["entity"].as_str().unwrap();
        let identity = config["identity"].as_str().unwrap();
        let idv = json!("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
        let owner = json!("11111111-1111-4111-8111-111111111111");
        let mut row = json!({"title":"alpha","note":null});
        row[identity] = idv.clone();
        row[scope] = owner.clone();
        s.reset(&json!([row.clone()]));
        let principal = json!({"entity":read["principal"],"id":owner});
        let name = read["descriptor"]["operation"].as_str().unwrap();
        assert_eq!(
            s.call(name, idv.clone(), &principal),
            json!({"kind":"success","value":row})
        );
        let mut args = read["descriptor"].clone();
        args["predicate"]["value"] = idv;
        let mut trace = vec![];
        for key in ["freshness", "semanticOperationId", "policy", "entity"] {
            let mut bad = args.clone();
            bad[key] = json!("forged");
            assert!(execute(
                &s.db,
                &config,
                "storage.read",
                &bad,
                &principal,
                false,
                &mut trace,
                false
            )
            .is_err());
        }
        assert!(execute(
            &s.db,
            &config,
            "storage.update",
            &args,
            &principal,
            false,
            &mut trace,
            false
        )
        .is_err());
        // The source mutation campaign sets min length five. Derive expected behavior
        // from the checked projection, never from an edited generated validator.
        let refined = std::env::var("NATIVE_EXPECT_REFINED").is_ok();
        assert_eq!(
            shared::valid(&format!("{entity}.title"), &json!("abcd")),
            !refined
        );
        assert!(shared::valid(&format!("{entity}.title"), &json!("abcde")));
    }
    #[test]
    fn commit_and_rollback_survive_reopen() {
        let path =
            std::env::temp_dir().join(format!("jadpo-native-reopen-{}.sqlite", std::process::id()));
        let mut s = Storage::open(path.to_str().unwrap(), "WAL");
        let cfg = s.config.clone();
        let rename = cfg["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"].as_str().unwrap().ends_with(".rename"))
            .unwrap()["name"]
            .as_str()
            .unwrap();
        let plan = cfg["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["effect"] == "read")
            .unwrap();
        let owner = "11111111-1111-4111-8111-111111111111";
        let a = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let b = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let make = |key, title| {
            let mut r = json!({"id":key,"title":title,"note":null});
            r[plan["scope"].as_str().unwrap()] = json!(owner);
            r
        };
        s.reset(&json!([make(a, "alpha"), make(b, "bravo")]));
        let principal = json!({"entity":plan["principal"],"id":owner});
        assert_eq!(
            s.call(rename, json!([a, "renamed"]), &principal)["kind"],
            "success"
        );
        let before = s.snapshot();
        assert_eq!(
            s.call(
                "update_pair",
                json!([a, "first-new", b, "first-new"]),
                &principal
            )["kind"],
            "domain"
        );
        assert_eq!(s.snapshot(), before);
        drop(s);
        let s = Storage::open(path.to_str().unwrap(), "WAL");
        assert_eq!(s.snapshot(), before);
        drop(s);
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    use jadpo_shared_core::{Invocation, Step};
    #[test]
    fn cancel_or_drop_after_first_write_rolls_back_driver_transaction() {
        let mut s = Storage::open(":memory:", "DELETE");
        let config = s.config.clone();
        let read = config["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["effect"] == "read")
            .unwrap();
        let scope = read["scope"].as_str().unwrap();
        let owner = "11111111-1111-4111-8111-111111111111";
        let a = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let b = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let make = |id, title| {
            let mut r = json!({"id":id,"title":title,"note":null});
            r[scope] = json!(owner);
            r
        };
        let seeds = json!([make(a, "alpha"), make(b, "bravo")]);
        s.reset(&seeds);
        let principal = json!({"entity":read["principal"],"id":owner});
        let op = config["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == "update_pair")
            .unwrap()["semanticId"]
            .as_i64()
            .unwrap() as i32;
        for cancel in [true, false] {
            let tx =
                s.db.transaction_with_behavior(TransactionBehavior::Immediate)
                    .unwrap();
            let mut invocation = Invocation::new(42, op, json!([a, "first-new", b, "second-new"]));
            let Step::Pending(first) = invocation.poll() else {
                panic!("first effect")
            };
            let reply = execute(
                &tx,
                &config,
                first.capability,
                &first.args,
                &principal,
                true,
                &mut vec![],
                false,
            )
            .unwrap();
            assert_eq!(reply["value"]["row"]["title"], "first-new");
            // Verify that the first write really happened before cancellation.
            let title: String = tx
                .query_row(
                    &format!(
                        "SELECT title FROM {} WHERE id=?",
                        config["table"].as_str().unwrap()
                    ),
                    [a],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(title, "first-new");
            let Step::Pending(second) = invocation.resume(first.request, first.operation, reply)
            else {
                panic!("second effect")
            };
            assert_eq!(second.operation, 2);
            if cancel {
                let Step::Complete(c) = invocation.cancel() else {
                    panic!()
                };
                assert_eq!(c.value["kind"], "internal");
            }
            drop(invocation);
            drop(tx); // rusqlite's existing rollback-on-drop behavior.
            assert_eq!(s.snapshot(), seeds);
        }
    }
}
