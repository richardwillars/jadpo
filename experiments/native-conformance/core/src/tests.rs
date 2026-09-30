use super::*;
fn entry(suffix: &str) -> i32 {
    let config: Value = serde_json::from_str(include_str!("../../build/manifest.json")).unwrap();
    config["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"].as_str().unwrap().ends_with(suffix))
        .unwrap()["semanticId"]
        .as_i64()
        .unwrap() as i32
}
fn id() -> Value {
    json!("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
}
fn row(note: &str) -> Value {
    let c: Value = serde_json::from_str(include_str!("../../build/storage.json")).unwrap();
    let mut r = json!({"id":id(),"title":"alpha","note":note});
    let p = &c["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["effect"] == "read")
        .unwrap();
    r[p["scope"].as_str().unwrap()] = json!("11111111-1111-4111-8111-111111111111");
    r
}
fn pending(i: &mut Invocation) -> Pending {
    match i.poll() {
        Step::Pending(p) => p,
        _ => panic!("expected pending"),
    }
}
fn complete(step: Step) -> Completion {
    match step {
        Step::Complete(c) => c,
        _ => panic!("expected completion"),
    }
}
#[test]
fn reversed_resumes_are_owned_and_isolated() {
    let mut a = Invocation::new(1, entry(".read"), id());
    let mut b = Invocation::new(2, entry(".read"), id());
    let pa = pending(&mut a);
    let pb = pending(&mut b);
    let value = row(&"é😀".repeat(2000));
    assert_eq!(
        complete(b.resume(
            pb.request,
            pb.operation,
            json!({"kind":"success","value":value})
        ))
        .value,
        json!({"kind":"success","value":value})
    );
    assert_eq!(
        complete(a.resume(
            pa.request,
            pa.operation,
            json!({"kind":"success","value":null})
        ))
        .value["kind"],
        "domain"
    );
}
#[test]
fn suspended_invocation_can_move_to_a_thread() {
    let mut i = Invocation::new(7, entry(".read"), id());
    let p = pending(&mut i);
    let expected = row("thread");
    let result = std::thread::spawn(move || {
        complete(i.resume(
            p.request,
            p.operation,
            json!({"kind":"success","value":expected}),
        ))
    })
    .join()
    .unwrap();
    assert_eq!(result.value["value"]["note"], "thread");
}
#[test]
fn parallel_invocations_have_no_request_globals() {
    let threads = (0..8)
        .map(|n| {
            std::thread::spawn(move || {
                for j in 0..100 {
                    let note = format!("{n}-{j}");
                    let expected = row(&note);
                    assert_eq!(
                        run(
                            entry(".read"),
                            id(),
                            |_, _| json!({"kind":"success","value":expected})
                        )
                        .value,
                        json!({"kind":"success","value":expected})
                    );
                }
            })
        })
        .collect::<Vec<_>>();
    for t in threads {
        t.join().unwrap();
    }
}
#[test]
fn stale_duplicate_cancel_and_drop_do_not_poison_another_call() {
    for mode in 0..4 {
        let mut i = Invocation::new(10, entry(".read"), id());
        let p = pending(&mut i);
        assert_eq!(pending(&mut i), p);
        let result = match mode {
            0 => i.resume(9, p.operation, json!({"kind":"success","value":row("bad")})),
            1 => i.resume(p.request, p.operation + 1, Value::Null),
            2 => i.cancel(),
            _ => {
                i.resume(
                    p.request,
                    p.operation,
                    json!({"kind":"success","value":row("ok")}),
                );
                i.resume(p.request, p.operation, Value::Null)
            }
        };
        assert_eq!(complete(result).value, json!({"kind":"internal"}));
    }
    let mut dropped = Invocation::new(11, entry(".read"), id());
    pending(&mut dropped);
    drop(dropped);
    assert_eq!(
        run(
            entry(".read"),
            id(),
            |_, _| json!({"kind":"success","value":row("fresh")})
        )
        .value["value"]["note"],
        "fresh"
    );
}
#[test]
fn malformed_host_and_invalid_input_keep_failure_classes() {
    for reply in [
        json!({"kind":"domain","failure":"ItemMissing"}),
        json!({"kind":"success"}),
        json!({"kind":"internal","detail":"SECRET_SENTINEL"}),
    ] {
        let result = run(entry(".read"), id(), |_, _| reply.clone());
        assert_eq!(result.value, json!({"kind":"internal"}));
        assert_eq!(result.fault_operation, Some(entry(".read") as u32));
    }
    assert_eq!(
        run(entry(".read"), json!("bad"), |_, _| panic!("no host call")).value,
        json!({"kind":"invalid"})
    );
}
#[test]
fn exact_budget_unicode_escapes_and_row_ownership() {
    let empty = json!({"kind":"success","value":row("")});
    let overhead = serde_json::to_vec(&empty).unwrap().len();
    for (extra, kind) in [(0, "success"), (1, "internal")] {
        let value = row(&"x".repeat(LIMIT - overhead + extra));
        let r = run(
            entry(".read"),
            id(),
            |_, _| json!({"kind":"success","value":value}),
        );
        assert_eq!(r.value["kind"], kind);
    }
    for text in ["é😀", "\"\\\n\0", "\u{feff}"] {
        let v = json!({"x":text.repeat(1000)});
        assert_eq!(
            within_limit(&v),
            serde_json::to_vec(&v).unwrap().len() <= LIMIT
        );
    }
    let value = AppValue::checked_row(row(&"x".repeat(4096)));
    let mut changed = value.clone().into_json();
    changed["note"] = json!("mutated");
    assert_ne!(value.into_json(), changed);
}
#[test]
fn sequential_effects_and_cancellation_skip_second_mutation() {
    let cfg: Value = serde_json::from_str(include_str!("../../build/storage.json")).unwrap();
    let rename = cfg["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"].as_str().unwrap().ends_with(".rename"))
        .unwrap()["semanticId"]
        .as_i64()
        .unwrap();
    let mut i = Invocation::new(
        8,
        entry("update_pair"),
        json!([
            id(),
            "first-new",
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            "second-new"
        ]),
    );
    let p = pending(&mut i);
    assert_eq!(p.semantic, rename as u32);
    assert_eq!(p.operation, 1);
    i.cancel();
    assert_eq!(
        complete(i.resume(
            8,
            1,
            json!({"kind":"success","value":{"status":"found","row":row("ignored")}})
        ))
        .value["kind"],
        "internal"
    );
}
