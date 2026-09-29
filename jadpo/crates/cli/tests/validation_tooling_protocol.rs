//! Black-box stdio protocol checks; UTF-16 coordinates computed independently.
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Server {
    child: Child,
    input: ChildStdin,
    messages: Receiver<Value>,
    notifications: Vec<Value>,
    root: PathBuf,
    id: u64,
}
fn uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy().replace(' ', "%20"))
}
fn position(source: &str, byte: usize) -> Value {
    let prefix = &source[..byte];
    json!({"line":prefix.bytes().filter(|b| *b == b'\n').count(), "character":prefix.rsplit('\n').next().unwrap().encode_utf16().count()})
}
fn offset(source: &str, position: &Value) -> usize {
    let line = position["line"].as_u64().unwrap() as usize;
    let character = position["character"].as_u64().unwrap() as usize;
    let start = source
        .split_inclusive('\n')
        .take(line)
        .map(str::len)
        .sum::<usize>();
    let mut units = 0;
    for (byte, ch) in source[start..].char_indices() {
        if units == character {
            return start + byte;
        }
        units += ch.len_utf16();
        assert!(units <= character, "edit bisects a UTF-16 surrogate pair");
    }
    assert_eq!(units, character);
    source.len()
}
impl Server {
    fn start(files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "jadpo-tooling-protocol-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&root).unwrap();
        for (name, text) in files {
            fs::write(root.join(name), text).unwrap();
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_jadpo"))
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (sender, messages) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut length = None;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some(value) = line.strip_prefix("Content-Length:") {
                        length = Some(value.trim().parse::<usize>().unwrap());
                    }
                }
                let mut payload = vec![0; length.expect("framed output needs a content length")];
                if reader.read_exact(&mut payload).is_err() {
                    return;
                }
                if sender
                    .send(serde_json::from_slice(&payload).unwrap())
                    .is_err()
                {
                    return;
                }
            }
        });
        let mut server = Self {
            child,
            input,
            messages,
            notifications: vec![],
            root,
            id: 0,
        };
        let root_uri = uri(&server.root);
        let initialized =
            server.request("initialize", json!({"rootUri":root_uri,"capabilities":{}}));
        assert!(initialized["result"]["capabilities"]["definitionProvider"]
            .as_bool()
            .unwrap());
        server.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
        server
    }
    fn send(&mut self, value: Value) {
        let body = serde_json::to_vec(&value).unwrap();
        write!(self.input, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.input.write_all(&body).unwrap();
        self.input.flush().unwrap();
    }
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        self.send(json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params}));
        loop {
            let message = self
                .messages
                .recv_timeout(Duration::from_secs(5))
                .expect("LSP response deadline");
            if message["id"] == self.id {
                return message;
            }
            self.notifications.push(message);
        }
    }
    fn open(&mut self, name: &str, text: &str) -> String {
        let document = uri(&self.root.join(name));
        self.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":document,"languageId":"jadpo","version":7,"text":text}}}));
        document
    }
    fn diagnostics(&mut self, document: &str) -> Value {
        loop {
            if let Some(index) = self.notifications.iter().position(|m| {
                m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == document
            }) {
                return self.notifications.remove(index)["params"]["diagnostics"].clone();
            }
            let message = self
                .messages
                .recv_timeout(Duration::from_secs(5))
                .expect("diagnostic notification deadline");
            self.notifications.push(message);
        }
    }
}

#[test]
fn rename_cannot_capture_an_existing_local_binding() {
    for source in [
        "type Label = Text {}\nfunction choose(input: Label) -> Label { var other = Label(\"fixed\") return input }\n",
        "type Label = Text {}\ntype Flag = Bool {}\nfunction choose(input: Label, flag: Flag) -> Label { if flag == Flag(true) { var other = Label(\"fixed\") return input } return input }\n",
    ] {
        let mut server = Server::start(&[("app.jadpo", source)]);
        let document = server.open("app.jadpo", source);
        assert_eq!(server.diagnostics(&document), json!([]), "{source}");
        let response = server.request("textDocument/rename", json!({"textDocument":{"uri":document},"position":position(source,source.rfind("input").unwrap()),"newName":"other"}));
        assert!(response.get("error").is_some() || response["result"].is_null(), "semantic rename must reject capturing the existing `other` local: {response}");
    }
}

#[test]
fn rename_allows_the_same_name_in_disjoint_functions_and_sibling_blocks() {
    for source in [
        "type Label = Text {}\nfunction alpha(input: Label) -> Label { return input }\nfunction beta(other: Label) -> Label { return other }\n",
        "type Label = Text {}\ntype Flag = Bool {}\nfunction choose(flag: Flag) -> Label { if flag == Flag(true) { var input = Label(\"left\") return input } else { var other = Label(\"right\") return other } }\n",
    ] {
        let mut server = Server::start(&[("app.jadpo", source)]);
        let document = server.open("app.jadpo", source);
        assert_eq!(server.diagnostics(&document), json!([]), "{source}");
        let response = server.request("textDocument/rename", json!({"textDocument":{"uri":document},"position":position(source,source.rfind("input").unwrap()),"newName":"other"}));
        let changes = response["result"]["changes"].as_object().expect("disjoint scopes permit rename");
        assert_eq!(changes.len(), 1);
        let edits = changes[&document].as_array().unwrap();
        assert_eq!(edits.len(), 2, "only the selected declaration and reference");
        let mut replacements = edits.iter().map(|edit| {
            let start = offset(source, &edit["range"]["start"]);
            let end = offset(source, &edit["range"]["end"]);
            assert_eq!(&source[start..end], "input");
            assert_eq!(edit["newText"], "other");
            (start, end)
        }).collect::<Vec<_>>();
        replacements.sort_by_key(|(start, _)| std::cmp::Reverse(*start));
        let mut repaired = source.to_owned();
        for (start, end) in replacements {
            repaired.replace_range(start..end, "other");
        }
        server.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":document,"version":8},"contentChanges":[{"text":repaired}]}}));
        assert_eq!(server.diagnostics(&document), json!([]));
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn utf16_navigation_and_rename_use_unsaved_semantics_without_touching_literal_lookalikes() {
    let domain = "module demo.domain\npublic type Label = Text {}\n";
    let disk = "module demo.api\nimport demo.domain { Label }\nfunction echo(input: Label) -> Label { return input }\n";
    let source = "module demo.api\nimport demo.domain { Label }\nfunction echo(input: Label) -> Label { var note = \"🦀 input é\" var output: Label = input return output }\n";
    let mut server = Server::start(&[("domain file.jadpo", domain), ("api.jadpo", disk)]);
    let document = server.open("api.jadpo", source);
    assert_eq!(server.diagnostics(&document), json!([]));
    let reference = source.rfind("Label").unwrap();
    let definition = server.request(
        "textDocument/definition",
        json!({"textDocument":{"uri":document},"position":position(source,reference)}),
    );
    assert_eq!(
        definition["result"]["uri"],
        uri(&server.root.join("domain file.jadpo"))
    );
    assert_eq!(
        definition["result"]["range"]["start"],
        position(domain, domain.find("Label").unwrap())
    );
    let reference = source.rfind("input").unwrap();
    let rename = server.request("textDocument/rename", json!({"textDocument":{"uri":document},"position":position(source,reference),"newName":"payload"}));
    let changes = rename["result"]["changes"].as_object().unwrap();
    assert_eq!(changes.len(), 1);
    let edits = changes[&document].as_array().unwrap();
    assert_eq!(
        edits.len(),
        2,
        "parameter declaration and its sole semantic use: {edits:?}"
    );
    let mut replacements = edits
        .iter()
        .map(|edit| {
            let start = offset(source, &edit["range"]["start"]);
            let end = offset(source, &edit["range"]["end"]);
            assert_eq!(&source[start..end], "input");
            (start, end, edit["newText"].as_str().unwrap())
        })
        .collect::<Vec<_>>();
    replacements.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    let mut repaired = source.to_owned();
    for (start, end, text) in replacements {
        repaired.replace_range(start..end, text);
    }
    assert!(repaired.contains("\"🦀 input é\""));
    assert!(repaired.contains("echo(payload: Label)"));
    assert!(repaired.contains("Label = payload return output"));
    server.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":document,"version":8},"contentChanges":[{"text":repaired}]}}));
    assert_eq!(server.diagnostics(&document), json!([]));
}

#[test]
fn code_actions_use_utf16_ranges_and_reject_a_real_previous_document_revision() {
    let source = "// 🦀é\ntype Label = Text { pattern: \"🦀é\" min_length 2 }\n";
    let mut server = Server::start(&[("app.jadpo", source)]);
    let document = server.open("app.jadpo", source);
    let diagnostics = server.diagnostics(&document);
    assert_eq!(diagnostics.as_array().unwrap().len(), 1);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["code"], "syntax.constraint_colon_required");
    let actions = server.request("textDocument/codeAction",json!({"textDocument":{"uri":document},"range":diagnostic["range"],"context":{"diagnostics":diagnostics}}));
    let action = &actions["result"][0];
    let edit = &action["edit"]["changes"][&document][0];
    let insertion = source.find("min_length").unwrap() + "min_length".len();
    assert_eq!(edit["range"]["start"], position(source, insertion));
    assert_eq!(edit["range"]["end"], position(source, insertion));
    assert_eq!(edit["newText"], ":");
    let changed = source.replace("min_length 2", "min_length 3");
    server.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":document,"version":8},"contentChanges":[{"text":changed}]}}));
    let current = server.diagnostics(&document);
    assert_ne!(
        current[0]["data"]["sourceRevision"],
        diagnostic["data"]["sourceRevision"]
    );
    let stale = server.request("textDocument/codeAction",json!({"textDocument":{"uri":document},"range":diagnostic["range"],"context":{"diagnostics":diagnostics}}));
    assert_eq!(stale["result"], json!([]));
    let fresh = server.request("textDocument/codeAction",json!({"textDocument":{"uri":document},"range":current[0]["range"],"context":{"diagnostics":current}}));
    assert_eq!(fresh["result"].as_array().unwrap().len(), 1);
}
