// The entire WASM compiler output is included byte-for-byte. Never invoke its
// wasm32 pointer exports on a native address space. Only this synchronous bridge
// may access the inherited globals, on the server's sole application thread.
#![allow(dead_code, unused_parens)]
include!("../build/generated.rs");

pub fn valid(name: &str, value: &Value) -> bool {
    validate(name, value)
}
pub fn invoke(operation: i32, input: Value, mut host: impl FnMut(&str, &Value) -> Value) -> Value {
    assert_eq!(reset(), 1, "completed native invocation required");
    unsafe {
        CURRENT_SEMANTIC_OPERATION = operation as u32;
    }
    let result = (|| -> Outcome {
        if json_len(&input) > LIMIT {
            return Err(Fault::Invalid);
        }
        let mut frame = make_entry(operation, input)?;
        let waker = unsafe { Waker::from_raw(raw_waker()) };
        // No await or cross-thread movement: HostRead is synchronously serviced.
        for _ in 0..64 {
            match frame.as_mut().poll(&mut Context::from_waker(&waker)) {
                Poll::Ready(result) => return result,
                Poll::Pending => {
                    let pending: Value = unsafe { serde_json::from_slice(&OUTPUT) }
                        .map_err(|_| Fault::Internal(operation as u32))?;
                    let capability = pending["capability"]
                        .as_str()
                        .ok_or(Fault::Internal(operation as u32))?;
                    let value = host(capability, &pending["args"]);
                    let length = json_len(&value);
                    if length > LIMIT {
                        return Err(Fault::Internal(unsafe { CURRENT_SEMANTIC_OPERATION }));
                    }
                    unsafe {
                        HOST_RESULT = Some(value);
                        HOST_JSON_BOUND = Some(length);
                    }
                }
            }
        }
        Err(Fault::Internal(operation as u32))
    })();
    let envelope = match result {
        Ok(value) => json!({"kind":"success","value":value.into_json()}),
        Err(Fault::Domain(name)) => json!({"kind":"domain","failure":name}),
        Err(Fault::Invalid) => json!({"kind":"invalid"}),
        Err(Fault::Internal(id)) => {
            eprintln!("{{\"event\":\"native_internal\",\"operation\":{id}}}");
            json!({"kind":"internal"})
        }
    };
    // Cancellation/early failure must release state as well as completed calls.
    unsafe {
        PENDING = None;
        HOST_RESULT = None;
        HOST_JSON_BOUND = None;
        OUTPUT.clear();
    }
    if json_len(&envelope) > LIMIT {
        json!({"kind":"internal"})
    } else {
        envelope
    }
}

#[cfg(test)]
mod native_bridge_tests {
    use super::*;
    #[test]
    fn malformed_host_failures_and_budget_reset() {
        let config: Value = serde_json::from_str(include_str!("../build/storage.json")).unwrap();
        let entry = config["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"].as_str().unwrap().ends_with(".read"))
            .unwrap();
        let op = entry["semanticId"].as_i64().unwrap() as i32;
        let input = json!("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
        for host in [
            json!({"kind":"domain","failure":"ItemMissing"}),
            json!({"kind":"success"}),
            json!({"kind":"internal","detail":"SECRET_SENTINEL"}),
            json!({"kind":"success","value":"x".repeat(65536)}),
        ] {
            assert_eq!(
                invoke(op, input.clone(), |_, _| host.clone()),
                json!({"kind":"internal"})
            );
            assert_eq!(
                invoke(op, json!("bad"), |_, _| panic!(
                    "invalid must not reach host"
                )),
                json!({"kind":"invalid"})
            );
        }
    }
}
