mod shared;
mod storage;
use hyper::{
    body::HttpBody,
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server, StatusCode,
};
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
    time::Instant,
};
use storage::Storage;
fn usage() -> Value {
    unsafe {
        let mut r: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut r);
        json!({"user":r.ru_utime.tv_sec as i64*1_000_000+r.ru_utime.tv_usec as i64,"system":r.ru_stime.tv_sec as i64*1_000_000+r.ru_stime.tv_usec as i64,"maxRssBytes":r.ru_maxrss})
    }
}
struct State {
    storage: Storage,
    seed: Value,
    noop_row: Value,
    principals: Value,
    token: String,
    count: u64,
    start: Instant,
    cpu: Value,
}
impl State {
    fn handle(&mut self, path: &str, body: Value) -> Value {
        match path {
            "/begin" => {
                self.count = 0;
                self.start = Instant::now();
                self.cpu = usage();
                json!({"ok":true})
            }
            "/stats" => {
                let u = usage();
                json!({"count":self.count,"cpu":{"user":u["user"].as_i64().unwrap()-self.cpu["user"].as_i64().unwrap(),"system":u["system"].as_i64().unwrap()-self.cpu["system"].as_i64().unwrap()},"maxRssBytes":u["maxRssBytes"],"elapsedMs":self.start.elapsed().as_secs_f64()*1000.,"snapshot":self.storage.snapshot(),"trace":self.storage.trace})
            }
            "/reset" => {
                let mut rows = self.seed.clone();
                if let Some(note) = body.get("note") {
                    rows[0]["note"] = note.clone();
                }
                self.storage.reset(&rows);
                self.noop_row = rows[0].clone();
                self.storage.trace.clear();
                json!({"rows":rows})
            }
            "/snapshot" => self.storage.snapshot(),
            "/mutate" => {
                self.storage
                    .db
                    .execute_batch(body["sql"].as_str().unwrap())
                    .unwrap();
                json!({"ok":true})
            }
            "/trace" => {
                self.storage.tracing = body["enabled"] == true;
                self.storage.trace.clear();
                json!({"ok":true})
            }
            "/call" => {
                self.count += 1;
                let principal = body["principal"].as_str().unwrap_or("owner");
                let Some(p) = self.principals.get(principal) else {
                    return json!({"kind":"invalid"});
                };
                self.storage.call(
                    body["operation"].as_str().unwrap_or(""),
                    body["input"].clone(),
                    p,
                )
            }
            "/noop" => {
                self.count += 1;
                json!({"kind":"success","value":if body["operation"]=="probe" {json!("alpha")} else {self.noop_row.clone()}})
            }
            "/micro" => {
                let n = body["count"].as_u64().unwrap().min(100_000);
                let mut times = Vec::with_capacity(n as usize);
                let p = &self.principals["owner"];
                let cpu = usage();
                let started = Instant::now();
                for _ in 0..n {
                    let now = Instant::now();
                    let result = self.storage.call(
                        body["operation"].as_str().unwrap(),
                        body["input"].clone(),
                        p,
                    );
                    assert_eq!(result, body["expected"]);
                    times.push(now.elapsed().as_secs_f64() * 1e6);
                }
                let elapsed = started.elapsed().as_secs_f64();
                let u = usage();
                times.sort_by(f64::total_cmp);
                json!({"count":n,"elapsedMs":elapsed*1000.,"throughput":n as f64/elapsed,"p50Us":times[(n as usize-1)/2],"p95Us":times[((n-1) as f64*0.95) as usize],"p99Us":times[((n-1) as f64*0.99) as usize],"maxUs":times.last(),"cpuUs":u["user"].as_i64().unwrap()+u["system"].as_i64().unwrap()-cpu["user"].as_i64().unwrap()-cpu["system"].as_i64().unwrap()})
            }
            _ => json!({"kind":"invalid"}),
        }
    }
}
async fn serve(
    mut req: Request<Body>,
    state: Arc<Mutex<State>>,
) -> Result<Response<Body>, Infallible> {
    if req
        .headers()
        .get("x-experiment-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        != state.lock().unwrap().token
    {
        return Ok(Response::builder().status(404).body(Body::empty()).unwrap());
    }
    let path = req.uri().path().to_string();
    let mut bytes = Vec::new();
    let mut invalid = false;
    while let Some(chunk) = req.body_mut().data().await {
        match chunk {
            Ok(c) if bytes.len() + c.len() <= 65536 => bytes.extend_from_slice(&c),
            _ => {
                invalid = true;
                break;
            }
        }
    }
    let value = if invalid {
        json!({"kind":"invalid"})
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(body) => state.lock().unwrap().handle(&path, body),
            Err(_) => json!({"kind":"invalid"}),
        }
    };
    let status = match value["kind"].as_str() {
        Some("invalid") => StatusCode::BAD_REQUEST,
        Some("domain") => StatusCode::UNPROCESSABLE_ENTITY,
        Some("internal") => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::OK,
    };
    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&value).unwrap()))
        .unwrap())
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(args.len(), 4, "db path, token, journal");
    let spec: Value =
        serde_json::from_str(include_str!("../../../wasm-exp1/acceptance.json")).unwrap();
    let storage = Storage::open(&args[1], &args[3]);
    storage.reset(&spec["seeds"]["Item"]);
    let pragmas = storage.pragmas();
    let state = Arc::new(Mutex::new(State {
        storage,
        seed: spec["seeds"]["Item"].clone(),
        noop_row: spec["seeds"]["Item"][0].clone(),
        principals: spec["seeds"]["principals"].clone(),
        token: args[2].clone(),
        count: 0,
        start: Instant::now(),
        cpu: usage(),
    }));
    let server = Server::bind(&([127, 0, 0, 1], 0).into())
        .http1_keepalive(true)
        .serve(make_service_fn(move |_| {
            let state = state.clone();
            async move { Ok::<_, Infallible>(service_fn(move |req| serve(req, state.clone()))) }
        }));
    println!(
        "{}",
        json!({"url":format!("http://{}/",server.local_addr()),"pragmas":pragmas,"pid":std::process::id()})
    );
    server.await.unwrap();
}
