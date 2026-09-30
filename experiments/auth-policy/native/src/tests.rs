use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    host: Host,
    seed: Value,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../build/auth-policy");
        let seed =
            serde_json::from_str(&fs::read_to_string(root.join("seed.json")).unwrap()).unwrap();
        let path = std::env::temp_dir().join(format!(
            "jadpo-auth-policy-{}-{}.sqlite",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::copy(root.join("seed.sqlite"), &path).unwrap();
        Self {
            host: Host {
                db: Connection::open(&path).unwrap(),
                trace: vec![],
            },
            seed,
            path,
        }
    }
    fn input(&self, user: &str, path: &str, body: Value) -> Value {
        json!({"method":"POST","path":path,"authorization":format!("Bearer {}",self.seed["credentials"][user].as_str().unwrap()),"cookie":null,"body":body.to_string(),"now":self.seed["now"],"configuration":self.seed["configuration"]})
    }
    fn start(&self, user: &str, path: &str, body: Value) -> Invocation {
        Invocation::new(1, -1, self.input(user, path, body))
    }
    fn advance(&mut self, call: &mut Invocation, step: Step) -> Step {
        match step {
            Step::Pending(p) => {
                let reply = self.host.reply(p.capability, &p.args);
                call.resume(p.request, p.operation, reply)
            }
            other => other,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.host.recover();
        let _ = fs::remove_file(&self.path);
    }
}
#[test]
fn direct_callable_entry_cannot_bypass_authentication() {
    let mut call = Invocation::new(1, 45, json!({"id":"00000000-0000-4000-8000-000000000011"}));
    assert!(matches!(call.poll(),Step::Complete(c) if c.value["kind"]=="invalid"));
}
#[test]
fn cancellation_after_first_real_write_rolls_back() {
    let mut f = Fixture::new();
    let id = f.seed["notes"][0]["id"].clone();
    let mut call=f.start("alice","/notes/pair",json!({"first":id,"second":f.seed["notes"][1]["id"],"title":"First changed","second_title":"Second changed"}));
    let mut step = call.poll();
    loop {
        let updated = matches!(&step,Step::Pending(p) if p.capability=="sql.update");
        assert!(matches!(step, Step::Pending(_)));
        step = f.advance(&mut call, step);
        if updated {
            break;
        }
    }
    assert_eq!(
        rows(
            &f.host.db,
            "SELECT title FROM note WHERE id=?",
            &json!([id])
        )
        .unwrap()[0]["title"],
        "First changed"
    );
    assert!(matches!(&step,Step::Pending(p) if p.capability=="sql.update"));
    call.cancel();
    f.host.recover();
    assert_eq!(
        rows(
            &f.host.db,
            "SELECT title FROM note WHERE id=?",
            &json!([id])
        )
        .unwrap()[0]["title"],
        "Alice first"
    );
    assert!(f.host.db.is_autocommit());
}
#[test]
fn revocation_between_verification_and_live_check_is_observed() {
    let mut f = Fixture::new();
    let mut call = f.start(
        "alice",
        "/notes/read",
        json!({"id":f.seed["notes"][0]["id"]}),
    );
    let first = call.poll();
    let step = f.advance(&mut call, first);
    f.host
        .db
        .execute("UPDATE __jadpo_auth_sessions SET revoked=1", [])
        .unwrap();
    let step = f.advance(&mut call, step);
    assert!(
        matches!(step,Step::Complete(c) if c.value["value"]["body"]["error"]["code"]=="invalid_credentials")
    );
    assert!(f.host.trace.iter().all(|t| !t["args"]["sql"]
        .as_str()
        .unwrap_or("")
        .contains("FROM \"note\"")));
}
#[test]
fn interleaved_credentials_keep_distinct_principals_and_reject_stale_resume() {
    let mut f = Fixture::new();
    let id = f.seed["notes"][0]["id"].clone();
    let mut alice = f.start("alice", "/notes/private", json!({"id":id}));
    let mut bob = f.start("bob", "/notes/private", json!({"id":id}));
    let mut a = alice.poll();
    let mut b = bob.poll();
    loop {
        if matches!(b, Step::Pending(_)) {
            b = f.advance(&mut bob, b)
        }
        if matches!(a, Step::Pending(_)) {
            a = f.advance(&mut alice, a)
        }
        if matches!((&a, &b), (Step::Complete(_), Step::Complete(_))) {
            break;
        }
    }
    assert!(matches!(a,Step::Complete(c) if c.value["value"]["status"]==200));
    assert!(matches!(b,Step::Complete(c) if c.value["value"]["status"]==404));
    assert!(
        matches!(alice.resume(1,1,json!({"kind":"success","value":null})),Step::Complete(c) if c.value["kind"]=="internal")
    );
}
#[test]
fn malformed_host_envelope_never_becomes_a_principal() {
    let f = Fixture::new();
    let mut call = f.start(
        "alice",
        "/notes/read",
        json!({"id":f.seed["notes"][0]["id"]}),
    );
    let Step::Pending(p) = call.poll() else {
        panic!("expected session lookup")
    };
    let result = call.resume(
        p.request,
        p.operation,
        json!({"kind":"domain","failure":"NoteMissing"}),
    );
    assert!(matches!(result,Step::Complete(c) if c.value["value"]["status"]==503));
}
