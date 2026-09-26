use jadpo_core::{
    analyze_sources, discover_sources, format_source, index_recommendation_count,
    validate_schema_identities, AnalyzedProject, LanguageIndex, LanguageSymbol,
};
use jadpo_diagnostics::{Diagnostic, Severity};
use jadpo_syntax::{Declaration, SourceFile, TextRange, TokenKind, TypeReference};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

const KEYWORDS: &[&str] = &[
    "type", "enum", "entity", "value", "input", "output", "failure", "function", "action", "test",
    "route", "module", "import", "var", "mut", "return", "reject", "if", "else", "match", "assert",
    "some", "none", "true", "false", "and", "or", "not", "create", "query", "update", "delete",
];

pub fn run_stdio() -> Result<(), Diagnostic> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let mut server = Server::default();

    while let Some(message) = read_message(&mut reader)? {
        let should_exit = server.handle(message, &mut writer)?;
        if should_exit {
            break;
        }
    }
    Ok(())
}

#[derive(Default)]
struct Server {
    root: Option<PathBuf>,
    open_documents: BTreeMap<PathBuf, String>,
    shutdown: bool,
}

impl Server {
    fn handle(&mut self, message: Value, writer: &mut impl Write) -> Result<bool, Diagnostic> {
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        match method {
            Some("initialize") => {
                self.root = initialize_root(&params).or_else(|| env::current_dir().ok());
                respond(
                    writer,
                    id,
                    json!({
                        "capabilities": {
                            "textDocumentSync": { "openClose": true, "change": 1, "save": { "includeText": true } },
                            "documentSymbolProvider": true,
                            "workspaceSymbolProvider": true,
                            "definitionProvider": true,
                            "referencesProvider": true,
                            "hoverProvider": true,
                            "completionProvider": { "triggerCharacters": ["."] },
                            "signatureHelpProvider": { "triggerCharacters": ["(", ","] },
                            "renameProvider": { "prepareProvider": true },
                            "documentFormattingProvider": true,
                            "documentLinkProvider": { "resolveProvider": false },
                            "semanticTokensProvider": {
                                "legend": {
                                    "tokenTypes": ["namespace", "type", "enum", "enumMember", "property", "function", "variable", "parameter", "string", "number", "keyword", "operator", "comment"],
                                    "tokenModifiers": []
                                },
                                "full": true
                            }
                        },
                        "serverInfo": { "name": "jadpo", "version": env!("CARGO_PKG_VERSION") }
                    }),
                )?;
            }
            Some("initialized") => {}
            Some("shutdown") => {
                self.shutdown = true;
                respond(writer, id, Value::Null)?;
            }
            Some("exit") => return Ok(true),
            Some("textDocument/didOpen") => {
                if let (Some(uri), Some(text)) = (
                    pointer_str(&params, "/textDocument/uri"),
                    pointer_str(&params, "/textDocument/text"),
                ) {
                    self.open_documents
                        .insert(uri_to_path(uri), text.to_owned());
                    self.publish_diagnostics(writer)?;
                }
            }
            Some("textDocument/didChange") => {
                if let (Some(uri), Some(text)) = (
                    pointer_str(&params, "/textDocument/uri"),
                    params
                        .pointer("/contentChanges")
                        .and_then(Value::as_array)
                        .and_then(|changes| changes.last())
                        .and_then(|change| change.get("text"))
                        .and_then(Value::as_str),
                ) {
                    self.open_documents
                        .insert(uri_to_path(uri), text.to_owned());
                    self.publish_diagnostics(writer)?;
                }
            }
            Some("textDocument/didSave") => {
                if let Some(uri) = pointer_str(&params, "/textDocument/uri") {
                    if let Some(text) = params.get("text").and_then(Value::as_str) {
                        self.open_documents
                            .insert(uri_to_path(uri), text.to_owned());
                    }
                    self.publish_diagnostics(writer)?;
                }
            }
            Some("textDocument/didClose") => {
                if let Some(uri) = pointer_str(&params, "/textDocument/uri") {
                    self.open_documents.remove(&uri_to_path(uri));
                    self.publish_diagnostics(writer)?;
                }
            }
            Some("textDocument/documentSymbol") => {
                self.respond_analysis(writer, id, |project, index| {
                    let source = request_source(&params)?;
                    let source_name = source.to_string_lossy();
                    let symbols = index
                        .symbols
                        .iter()
                        .filter(|symbol| {
                            same_source(&symbol.source, &source_name)
                                && !symbol.key.starts_with("local:")
                        })
                        .map(|symbol| symbol_information(project, symbol))
                        .collect::<Vec<_>>();
                    Some(Value::Array(symbols))
                })?;
            }
            Some("workspace/symbol") => {
                self.respond_analysis(writer, id, |project, index| {
                    let query = params
                        .get("query")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    Some(Value::Array(
                        index
                            .symbols
                            .iter()
                            .filter(|symbol| {
                                !symbol.key.starts_with("local:")
                                    && symbol.name.to_ascii_lowercase().contains(&query)
                            })
                            .map(|symbol| symbol_information(project, symbol))
                            .collect(),
                    ))
                })?;
            }
            Some("textDocument/definition") => {
                self.respond_analysis(writer, id, |project, index| {
                    let (source, offset) = request_offset(project, &params)?;
                    let symbol = index.symbol_at(&source, offset)?;
                    Some(location(project, &symbol.source, symbol.range))
                })?;
            }
            Some("textDocument/references") => {
                self.respond_analysis(writer, id, |project, index| {
                    let (source, offset) = request_offset(project, &params)?;
                    let symbol = index.symbol_at(&source, offset)?;
                    let include_declaration = params
                        .pointer("/context/includeDeclaration")
                        .and_then(Value::as_bool)
                        .unwrap_or(true);
                    Some(Value::Array(
                        index
                            .occurrences_for(&symbol.key)
                            .filter(|occurrence| include_declaration || !occurrence.declaration)
                            .map(|occurrence| {
                                location(project, &occurrence.source, occurrence.range)
                            })
                            .collect(),
                    ))
                })?;
            }
            Some("textDocument/hover") => {
                self.respond_analysis(writer, id, |project, index| {
                    let (source, offset) = request_offset(project, &params)?;
                    let symbol = index.symbol_at(&source, offset);
                    let inferred = index.inferred_type_at(project, &source, offset);
                    if symbol.is_none() && inferred.is_none() {
                        return None;
                    }
                    let mut lines = Vec::new();
                    if let Some(symbol) = symbol {
                        lines.push(format!("```jadpo\n{}\n```", symbol.detail));
                    }
                    if let Some(inferred) = inferred {
                        lines.push(format!("Inferred type: `{inferred}`"));
                    }
                    Some(json!({
                        "contents": { "kind": "markdown", "value": lines.join("\n\n") }
                    }))
                })?;
            }
            Some("textDocument/completion") => {
                self.respond_analysis(writer, id, |project, index| {
                    let (source, offset) = request_offset(project, &params)?;
                    let mut labels = BTreeSet::new();
                    let mut items = Vec::new();
                    let member_container = completion_container(project, index, &source, offset);
                    let candidates = if let Some(container) = member_container.as_deref() {
                        index
                            .symbols
                            .iter()
                            .filter(|symbol| symbol.container.as_deref() == Some(container))
                            .collect::<Vec<_>>()
                    } else {
                        index
                            .locals_at(&source, offset)
                            .into_iter()
                            .chain(index.symbols.iter().filter(|symbol| {
                                !symbol.key.starts_with("local:")
                                    && !symbol.source.starts_with('<')
                                    && symbol.container.is_none()
                            }))
                            .collect::<Vec<_>>()
                    };
                    for symbol in candidates {
                        if labels.insert(symbol.name.clone()) {
                            items.push(json!({
                                "label": symbol.name,
                                "kind": completion_kind(&symbol.kind),
                                "detail": symbol.detail
                            }));
                        }
                    }
                    for keyword in KEYWORDS.iter().filter(|_| member_container.is_none()) {
                        if labels.insert((*keyword).to_owned()) {
                            items.push(
                                json!({ "label": keyword, "kind": 14, "detail": "Jadpo keyword" }),
                            );
                        }
                    }
                    Some(json!({ "isIncomplete": false, "items": items }))
                })?;
            }
            Some("textDocument/signatureHelp") => {
                self.respond_analysis(writer, id, |project, index| {
                    let (source, offset) = request_offset(project, &params)?;
                    signature_help(project, index, &source, offset)
                })?;
            }
            Some("textDocument/prepareRename") => {
                self.respond_analysis(writer, id, |project, index| {
                    let (source, offset) = request_offset(project, &params)?;
                    let symbol = index.symbol_at(&source, offset)?;
                    let occurrence = index.occurrences_for(&symbol.key).find(|occurrence| {
                        same_source(&occurrence.source, &source)
                            && occurrence.range.start <= offset
                            && offset < occurrence.range.end
                    })?;
                    Some(json!({
                        "range": lsp_range(project, &occurrence.source, occurrence.range),
                        "placeholder": symbol.name
                    }))
                })?;
            }
            Some("textDocument/rename") => {
                let new_name = params.get("newName").and_then(Value::as_str).unwrap_or("");
                if !valid_identifier(new_name) || KEYWORDS.contains(&new_name) {
                    error_response(
                        writer,
                        id,
                        -32602,
                        "new name is not a valid Jadpo identifier",
                    )?;
                } else {
                    self.respond_analysis(writer, id, |project, index| {
                        let (source, offset) = request_offset(project, &params)?;
                        let symbol = index.symbol_at(&source, offset)?;
                        let collision = symbol.container.as_ref().map_or_else(
                            || new_name.to_owned(),
                            |container| format!("{container}.{new_name}"),
                        );
                        if !symbol.key.starts_with("local:")
                            && index
                                .symbol(&collision)
                                .is_some_and(|candidate| candidate.key != symbol.key)
                        {
                            return None;
                        }
                        let mut changes = BTreeMap::<String, Vec<Value>>::new();
                        for occurrence in index.occurrences_for(&symbol.key) {
                            changes
                                .entry(path_to_uri(Path::new(&occurrence.source)))
                                .or_default()
                                .push(json!({
                                    "range": lsp_range(project, &occurrence.source, occurrence.range),
                                    "newText": new_name
                                }));
                        }
                        Some(json!({ "changes": changes }))
                    })?;
                }
            }
            Some("textDocument/formatting") => {
                let result = request_source(&params)
                    .and_then(|path| self.document_text(&path).map(|text| (path, text)))
                    .map(|(_path, text)| {
                        let formatted = format_source(&text);
                        let end = offset_to_position(&text, text.len());
                        Value::Array(vec![json!({
                            "range": { "start": { "line": 0, "character": 0 }, "end": end },
                            "newText": formatted
                        })])
                    })
                    .unwrap_or(Value::Null);
                respond(writer, id, result)?;
            }
            Some("textDocument/semanticTokens/full") => {
                self.respond_analysis(writer, id, |project, index| {
                    let source = request_source(&params)?;
                    let parsed = parsed_source(project, &source)?;
                    Some(json!({ "data": semantic_tokens(parsed, index) }))
                })?;
            }
            Some("textDocument/documentLink") => {
                let output_root = self
                    .root
                    .as_deref()
                    .map(|root| {
                        if root.is_file() {
                            root.parent().unwrap_or(root)
                        } else {
                            root
                        }
                    })
                    .unwrap_or_else(|| Path::new("."))
                    .join("build");
                self.respond_analysis(writer, id, |project, index| {
                    let source = request_source(&params)?;
                    let source_name = source.to_string_lossy();
                    let links = index
                        .symbols
                        .iter()
                        .filter(|symbol| {
                            same_source(&symbol.source, &source_name)
                                && !symbol.key.starts_with("local:")
                        })
                        .filter_map(|symbol| {
                            let (target, tooltip) = match symbol.kind.as_str() {
                                "route" => (
                                    output_root.join("openapi/openapi.json"),
                                    "Open generated route contract",
                                ),
                                "failure" => (
                                    output_root.join("audit/failures.json"),
                                    "Open generated failure audit",
                                ),
                                "function" | "action" => (
                                    output_root.join("inventory/callables.json"),
                                    "Open generated callable inventory",
                                ),
                                "type" | "enum" | "entity" | "value" | "input" | "output" => (
                                    output_root.join("validators/plan.json"),
                                    "Open generated validation plan",
                                ),
                                _ => return None,
                            };
                            Some(json!({
                                "range": lsp_range(project, &symbol.source, symbol.range),
                                "target": path_to_uri(&target),
                                "tooltip": tooltip
                            }))
                        })
                        .collect::<Vec<_>>();
                    Some(Value::Array(links))
                })?;
            }
            Some(_) if id.is_some() => respond(writer, id, Value::Null)?,
            _ => {}
        }
        Ok(self.shutdown && method == Some("exit"))
    }

    fn respond_analysis(
        &self,
        writer: &mut impl Write,
        id: Option<Value>,
        handler: impl FnOnce(&AnalyzedProject, &LanguageIndex) -> Option<Value>,
    ) -> Result<(), Diagnostic> {
        match self.analysis() {
            Ok((project, index)) => {
                respond(writer, id, handler(&project, &index).unwrap_or(Value::Null))
            }
            Err(diagnostic) => error_response(writer, id, -32603, &diagnostic.message),
        }
    }

    fn analysis(&self) -> Result<(AnalyzedProject, LanguageIndex), Diagnostic> {
        let root = self
            .root
            .clone()
            .or_else(|| env::current_dir().ok())
            .ok_or_else(|| {
                Diagnostic::error("LSP_ROOT_MISSING", "workspace root is unavailable")
            })?;
        let mut sources = match discover_sources(&root) {
            Ok(sources) => sources
                .into_iter()
                .map(|source| (normalise_path(&source.path), source))
                .collect::<BTreeMap<_, _>>(),
            Err(diagnostic) if diagnostic.code == "JADPO_NO_SOURCES" => BTreeMap::new(),
            Err(diagnostic) => return Err(diagnostic),
        };
        for (path, text) in &self.open_documents {
            sources.insert(
                normalise_path(path),
                SourceFile::new(normalise_path(path), text.clone()),
            );
        }
        let project = analyze_sources(sources.into_values().collect())?;
        let index = LanguageIndex::build(&project);
        Ok((project, index))
    }

    fn document_text(&self, path: &Path) -> Option<String> {
        self.open_documents
            .get(&normalise_path(path))
            .cloned()
            .or_else(|| fs::read_to_string(path).ok())
    }

    fn publish_diagnostics(&self, writer: &mut impl Write) -> Result<(), Diagnostic> {
        let (project, _) = match self.analysis() {
            Ok(analysis) => analysis,
            Err(diagnostic) => {
                notify(
                    writer,
                    "window/showMessage",
                    json!({ "type": 1, "message": format!("{}: {}", diagnostic.code, diagnostic.message) }),
                )?;
                return Ok(());
            }
        };
        let mut by_source = project
            .syntax
            .sources
            .iter()
            .map(|source| (source.source_name.clone(), Vec::new()))
            .collect::<BTreeMap<_, Vec<Value>>>();
        let mut additional = Vec::new();
        let frontend_failed = project.syntax.diagnostics().next().is_some()
            || !project.semantics.diagnostics.is_empty()
            || !project.typing.diagnostics.is_empty()
            || !project.failures.diagnostics.is_empty();
        if !frontend_failed {
            if let Some(root) = &self.root {
                if let Err(diagnostic) = validate_schema_identities(root, &project) {
                    additional.push(diagnostic);
                }
            }
            let recommendations = index_recommendation_count(&project);
            if recommendations > 0 {
                additional.push(Diagnostic::warning(
                    "INDEX_RECOMMENDATION_AVAILABLE",
                    format!("{recommendations} query-backed index recommendation(s) are available"),
                ));
            }
        }
        let diagnostics = project
            .syntax
            .diagnostics()
            .chain(project.semantics.diagnostics.iter())
            .chain(project.typing.diagnostics.iter())
            .chain(project.failures.diagnostics.iter())
            .chain(additional.iter());
        for diagnostic in diagnostics {
            let located = diagnostic.primary.as_ref().and_then(|primary| {
                project
                    .syntax
                    .sources
                    .iter()
                    .find(|source| same_source(&source.source_name, &primary.source))
                    .map(|source| (source, TextRange::new(primary.start, primary.end)))
            });
            let Some((source, range)) = located.or_else(|| {
                project
                    .syntax
                    .sources
                    .first()
                    .map(|source| (source, TextRange::new(0, 0)))
            }) else {
                continue;
            };
            by_source
                .entry(source.source_name.clone())
                .or_default()
                .push(json!({
                    "range": byte_range(&source.source_text, range),
                    "severity": match diagnostic.severity { Severity::Error => 1, Severity::Warning => 2, Severity::Note => 3 },
                    "code": diagnostic.code,
                    "source": "jadpo",
                    "message": diagnostic.message,
                    "data": { "notes": diagnostic.notes }
                }));
        }
        for (source, diagnostics) in by_source {
            notify(
                writer,
                "textDocument/publishDiagnostics",
                json!({ "uri": path_to_uri(Path::new(&source)), "diagnostics": diagnostics }),
            )?;
        }
        Ok(())
    }
}

fn initialize_root(params: &Value) -> Option<PathBuf> {
    params
        .get("workspaceFolders")
        .and_then(Value::as_array)
        .and_then(|folders| folders.first())
        .and_then(|folder| folder.get("uri"))
        .and_then(Value::as_str)
        .or_else(|| params.get("rootUri").and_then(Value::as_str))
        .map(uri_to_path)
        .or_else(|| {
            params
                .get("rootPath")
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
}

fn request_source(params: &Value) -> Option<PathBuf> {
    pointer_str(params, "/textDocument/uri").map(uri_to_path)
}

fn request_offset(project: &AnalyzedProject, params: &Value) -> Option<(String, usize)> {
    let path = request_source(params)?;
    let source = parsed_source(project, &path)?;
    let line = params.pointer("/position/line")?.as_u64()? as usize;
    let character = params.pointer("/position/character")?.as_u64()? as usize;
    Some((
        source.source_name.clone(),
        position_to_offset(&source.source_text, line, character),
    ))
}

fn completion_container(
    project: &AnalyzedProject,
    index: &LanguageIndex,
    source_name: &str,
    offset: usize,
) -> Option<String> {
    let source = project
        .syntax
        .sources
        .iter()
        .find(|source| same_source(&source.source_name, source_name))?;
    let before = source.source_text.get(..offset)?;
    let dot = before
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_whitespace())?;
    if dot.1 != '.' {
        return None;
    }
    let probe = before[..dot.0]
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_whitespace())?
        .0;
    let symbol = index.symbol_at(source_name, probe);
    if let Some(type_name) = symbol.and_then(|symbol| symbol.type_name.as_deref()) {
        let base = type_name.trim_end_matches('?');
        if index.symbol(base).is_some() {
            return Some(base.to_owned());
        }
    }
    if let Some(inferred) = index.inferred_type_at(project, source_name, probe) {
        let base = inferred.trim_end_matches('?');
        if index.symbol(base).is_some() {
            return Some(base.to_owned());
        }
    }
    let symbol = symbol?;
    (!symbol.key.starts_with("local:")).then(|| symbol.key.clone())
}

fn signature_help(
    project: &AnalyzedProject,
    index: &LanguageIndex,
    source_name: &str,
    offset: usize,
) -> Option<Value> {
    let source = project
        .syntax
        .sources
        .iter()
        .find(|source| same_source(&source.source_name, source_name))?;
    let significant = source
        .tokens
        .iter()
        .filter(|token| !token.kind.is_trivia() && token.range.start < offset)
        .collect::<Vec<_>>();
    let mut depth = 0usize;
    let mut open_index = None;
    for (index, token) in significant.iter().enumerate().rev() {
        match token.kind {
            TokenKind::RightParen => depth += 1,
            TokenKind::LeftParen if depth == 0 => {
                open_index = Some(index);
                break;
            }
            TokenKind::LeftParen => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    let open_index = open_index?;
    let callee = significant.get(open_index.checked_sub(1)?)?;
    let symbol = index.symbol_at(source_name, callee.range.start)?;
    if !matches!(symbol.kind.as_str(), "function" | "action") {
        return None;
    }
    let callable = project.syntax.sources.iter().find_map(|source| {
        source.file.declarations.iter().find_map(|declaration| {
            let Declaration::Callable(callable) = declaration else {
                return None;
            };
            (callable.name.text == symbol.name).then_some(callable)
        })
    })?;
    let mut nested = 0usize;
    let mut active_parameter = 0usize;
    for token in significant.iter().skip(open_index + 1) {
        match token.kind {
            TokenKind::LeftParen | TokenKind::LeftBrace | TokenKind::LeftAngle => nested += 1,
            TokenKind::RightParen | TokenKind::RightBrace | TokenKind::RightAngle => {
                nested = nested.saturating_sub(1)
            }
            TokenKind::Comma if nested == 0 => active_parameter += 1,
            _ => {}
        }
    }
    let parameters = callable
        .parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name.text,
                display_type_reference(&parameter.parameter_type)
            )
        })
        .collect::<Vec<_>>();
    let label = format!(
        "{}({}) -> {}",
        callable.name.text,
        parameters.join(", "),
        display_type_reference(&callable.return_type)
    );
    Some(json!({
        "signatures": [{
            "label": label,
            "parameters": parameters.into_iter().map(|label| json!({ "label": label })).collect::<Vec<_>>()
        }],
        "activeSignature": 0,
        "activeParameter": active_parameter.min(callable.parameters.len().saturating_sub(1))
    }))
}

fn display_type_reference(reference: &TypeReference) -> String {
    let mut result = reference
        .path
        .iter()
        .map(|name| name.text.as_str())
        .collect::<Vec<_>>()
        .join(".");
    if !reference.arguments.is_empty() {
        result.push('<');
        result.push_str(
            &reference
                .arguments
                .iter()
                .map(display_type_reference)
                .collect::<Vec<_>>()
                .join(", "),
        );
        result.push('>');
    }
    if reference.nullable {
        result.push('?');
    }
    result
}

fn parsed_source<'project>(
    project: &'project AnalyzedProject,
    path: &Path,
) -> Option<&'project jadpo_syntax::ParsedSyntax> {
    let expected = normalise_path(path).to_string_lossy().into_owned();
    project
        .syntax
        .sources
        .iter()
        .find(|source| same_source(&source.source_name, &expected))
}

fn symbol_information(project: &AnalyzedProject, symbol: &LanguageSymbol) -> Value {
    json!({
        "name": symbol.name,
        "kind": symbol_kind(&symbol.kind),
        "location": location(project, &symbol.source, symbol.range),
        "containerName": symbol.container
    })
}

fn location(project: &AnalyzedProject, source: &str, range: TextRange) -> Value {
    json!({
        "uri": path_to_uri(Path::new(source)),
        "range": lsp_range(project, source, range)
    })
}

fn lsp_range(project: &AnalyzedProject, source: &str, range: TextRange) -> Value {
    project
        .syntax
        .sources
        .iter()
        .find(|candidate| same_source(&candidate.source_name, source))
        .map(|candidate| byte_range(&candidate.source_text, range))
        .unwrap_or_else(|| byte_range("", TextRange::new(0, 0)))
}

fn byte_range(text: &str, range: TextRange) -> Value {
    json!({
        "start": offset_to_position(text, range.start.min(text.len())),
        "end": offset_to_position(text, range.end.min(text.len()))
    })
}

fn offset_to_position(text: &str, offset: usize) -> Value {
    let mut line = 0usize;
    let mut line_start = 0usize;
    for (index, byte) in text.bytes().enumerate() {
        if index >= offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
            line_start = index + 1;
        }
    }
    let character = text[line_start..offset.min(text.len())]
        .encode_utf16()
        .count();
    json!({ "line": line, "character": character })
}

fn position_to_offset(text: &str, target_line: usize, target_character: usize) -> usize {
    let mut line = 0usize;
    let mut line_start = 0usize;
    for (index, byte) in text.bytes().enumerate() {
        if line == target_line {
            line_start = index;
            break;
        }
        if byte == b'\n' {
            line += 1;
            line_start = index + 1;
        }
    }
    if line < target_line {
        return text.len();
    }
    let tail = &text[line_start..];
    let mut utf16 = 0usize;
    for (relative, character) in tail.char_indices() {
        if character == '\n' || utf16 >= target_character {
            return line_start + relative;
        }
        utf16 += character.len_utf16();
        if utf16 > target_character {
            return line_start + relative;
        }
    }
    text.len()
}

fn semantic_tokens(source: &jadpo_syntax::ParsedSyntax, index: &LanguageIndex) -> Vec<u32> {
    let mut absolute = Vec::<(u32, u32, u32, u32)>::new();
    for token in &source.tokens {
        if token.kind == TokenKind::Eof || token.range.start == token.range.end {
            continue;
        }
        let token_type = match token.kind {
            TokenKind::LineComment => Some(12),
            TokenKind::StringLiteral => Some(8),
            TokenKind::IntegerLiteral | TokenKind::DecimalLiteral => Some(9),
            TokenKind::Identifier | TokenKind::Input | TokenKind::Output | TokenKind::Value => {
                index
                    .symbol_at(&source.source_name, token.range.start)
                    .map(|symbol| semantic_symbol_type(&symbol.kind))
            }
            kind if is_keyword(kind) => Some(10),
            kind if is_operator(kind) => Some(11),
            _ => None,
        };
        let Some(token_type) = token_type else {
            continue;
        };
        let start = offset_to_line_character(&source.source_text, token.range.start);
        let length = source.source_text[token.range.start..token.range.end]
            .encode_utf16()
            .count() as u32;
        absolute.push((start.0 as u32, start.1 as u32, length, token_type));
    }
    absolute.sort();
    let mut encoded = Vec::with_capacity(absolute.len() * 5);
    let mut previous_line = 0u32;
    let mut previous_start = 0u32;
    for (line, start, length, token_type) in absolute {
        let delta_line = line - previous_line;
        let delta_start = if delta_line == 0 {
            start - previous_start
        } else {
            start
        };
        encoded.extend([delta_line, delta_start, length, token_type, 0]);
        previous_line = line;
        previous_start = start;
    }
    encoded
}

fn offset_to_line_character(text: &str, offset: usize) -> (usize, usize) {
    let value = offset_to_position(text, offset);
    (
        value.get("line").and_then(Value::as_u64).unwrap_or(0) as usize,
        value.get("character").and_then(Value::as_u64).unwrap_or(0) as usize,
    )
}

fn semantic_symbol_type(kind: &str) -> u32 {
    match kind {
        "enum" => 2,
        "enum_variant" => 3,
        "field" | "relationship" | "persistence_constraint" => 4,
        "function" | "action" | "test" | "route" => 5,
        "parameter" => 7,
        "variable" => 6,
        _ => 1,
    }
}

fn symbol_kind(kind: &str) -> u32 {
    match kind {
        "enum" => 10,
        "enum_variant" => 22,
        "field" | "relationship" => 8,
        "function" | "action" => 12,
        "test" | "route" => 6,
        "variable" | "parameter" => 13,
        "persistence_constraint" => 14,
        _ => 23,
    }
}

fn completion_kind(kind: &str) -> u32 {
    match kind {
        "enum_variant" => 20,
        "field" | "relationship" => 5,
        "function" | "action" => 3,
        "variable" | "parameter" => 6,
        "enum" | "type" | "entity" | "value" | "input" | "output" => 7,
        _ => 1,
    }
}

fn is_keyword(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Type
            | TokenKind::Enum
            | TokenKind::Entity
            | TokenKind::Value
            | TokenKind::Input
            | TokenKind::Output
            | TokenKind::Failure
            | TokenKind::Function
            | TokenKind::Action
            | TokenKind::Test
            | TokenKind::Route
            | TokenKind::Module
            | TokenKind::Import
            | TokenKind::Public
            | TokenKind::Internal
            | TokenKind::Optional
            | TokenKind::Required
            | TokenKind::Many
            | TokenKind::Missing
            | TokenKind::Fails
            | TokenKind::Var
            | TokenKind::Mut
            | TokenKind::Return
            | TokenKind::Reject
            | TokenKind::If
            | TokenKind::Else
            | TokenKind::Match
            | TokenKind::Assert
            | TokenKind::Auth
            | TokenKind::Explicitly
            | TokenKind::Run
            | TokenKind::Create
            | TokenKind::Query
            | TokenKind::Where
            | TokenKind::Update
            | TokenKind::Delete
    )
}

fn is_operator(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::And
            | TokenKind::Or
            | TokenKind::Not
            | TokenKind::Equal
            | TokenKind::EqualEqual
            | TokenKind::BangEqual
            | TokenKind::LeftAngle
            | TokenKind::RightAngle
            | TokenKind::LessEqual
            | TokenKind::GreaterEqual
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Arrow
            | TokenKind::FatArrow
    )
}

fn valid_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn same_source(left: &str, right: &str) -> bool {
    normalise_path(Path::new(left)) == normalise_path(Path::new(right))
}

fn normalise_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        env::current_dir()
            .map(|directory| directory.join(path))
            .unwrap_or_else(|_| path.to_owned())
    }
}

fn uri_to_path(uri: &str) -> PathBuf {
    let encoded = uri.strip_prefix("file://").unwrap_or(uri);
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                decoded.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    PathBuf::from(String::from_utf8_lossy(&decoded).into_owned())
}

fn path_to_uri(path: &Path) -> String {
    let path = normalise_path(path);
    let mut encoded = String::from("file://");
    for byte in path.to_string_lossy().as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'/' | b':' | b'-' | b'.' | b'_' | b'~')
        {
            encoded.push(*byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn pointer_str<'value>(value: &'value Value, pointer: &str) -> Option<&'value str> {
    value.pointer(pointer).and_then(Value::as_str)
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, Diagnostic> {
    let mut content_length = None;
    loop {
        let mut header = String::new();
        let read = reader.read_line(&mut header).map_err(|error| {
            io_diagnostic("LSP_READ_FAILED", "could not read LSP header", error)
        })?;
        if read == 0 {
            return Ok(None);
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some(value) = header
            .strip_prefix("Content-Length:")
            .or_else(|| header.strip_prefix("content-length:"))
        {
            content_length = value.trim().parse::<usize>().ok();
        }
    }
    let length = content_length.ok_or_else(|| {
        Diagnostic::error(
            "LSP_CONTENT_LENGTH_MISSING",
            "LSP message has no Content-Length header",
        )
    })?;
    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|error| io_diagnostic("LSP_READ_FAILED", "could not read LSP message", error))?;
    serde_json::from_slice(&body).map(Some).map_err(|error| {
        Diagnostic::error("LSP_JSON_INVALID", format!("invalid LSP JSON: {error}"))
    })
}

fn respond(writer: &mut impl Write, id: Option<Value>, result: Value) -> Result<(), Diagnostic> {
    write_message(
        writer,
        &json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result }),
    )
}

fn error_response(
    writer: &mut impl Write,
    id: Option<Value>,
    code: i32,
    message: &str,
) -> Result<(), Diagnostic> {
    write_message(
        writer,
        &json!({
            "jsonrpc": "2.0",
            "id": id.unwrap_or(Value::Null),
            "error": { "code": code, "message": message }
        }),
    )
}

fn notify(writer: &mut impl Write, method: &str, params: Value) -> Result<(), Diagnostic> {
    write_message(
        writer,
        &json!({ "jsonrpc": "2.0", "method": method, "params": params }),
    )
}

fn write_message(writer: &mut impl Write, message: &Value) -> Result<(), Diagnostic> {
    let body = serde_json::to_vec(message).map_err(|error| {
        Diagnostic::error(
            "LSP_JSON_WRITE_FAILED",
            format!("could not encode LSP JSON: {error}"),
        )
    })?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())
        .and_then(|_| writer.write_all(&body))
        .and_then(|_| writer.flush())
        .map_err(|error| io_diagnostic("LSP_WRITE_FAILED", "could not write LSP message", error))
}

fn io_diagnostic(code: &'static str, message: &str, error: io::Error) -> Diagnostic {
    Diagnostic::error(code, format!("{message}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        completion_container, offset_to_line_character, path_to_uri, position_to_offset,
        read_message, uri_to_path, Server,
    };
    use jadpo_core::{analyze_project, LanguageIndex};
    use serde_json::{json, Value};
    use std::io::Cursor;
    use std::path::Path;

    #[test]
    fn converts_utf8_bytes_and_lsp_utf16_positions() {
        let source = "first\nemoji 😀 value\n";
        let byte = source.find("value").expect("value should exist");
        let position = offset_to_line_character(source, byte);
        assert_eq!(position, (1, 9));
        assert_eq!(position_to_offset(source, 1, 9), byte);
    }

    #[test]
    fn round_trips_file_uris_with_spaces() {
        let path = Path::new("/tmp/Jadpo project/app.jadpo");
        assert_eq!(uri_to_path(&path_to_uri(path)), path);
    }

    #[test]
    fn serves_symbols_definition_hover_completion_rename_and_semantic_tokens() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("CLI crate should be inside the repository");
        let fixture = repository.join("tests/compile/pass/56_tagged_sum_match.jadpo");
        let source = std::fs::read_to_string(&fixture).expect("fixture should be readable");
        let byte = source
            .find("PaymentOutcome.paid {")
            .expect("variant construction should exist")
            + "PaymentOutcome.".len();
        let (line, character) = offset_to_line_character(&source, byte);
        let uri = path_to_uri(&fixture);
        let mut server = Server::default();

        let initialized = request(
            &mut server,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": { "rootUri": uri }
            }),
        );
        assert_eq!(initialized["id"], 1);
        assert_eq!(
            initialized["result"]["capabilities"]["definitionProvider"],
            true
        );

        let symbols = request(
            &mut server,
            json!({
                "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentSymbol",
                "params": { "textDocument": { "uri": uri } }
            }),
        );
        assert!(symbols["result"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["name"] == "PaymentOutcome")));

        for (id, method) in [
            (3, "textDocument/definition"),
            (4, "textDocument/hover"),
            (5, "textDocument/completion"),
            (6, "textDocument/prepareRename"),
            (7, "textDocument/semanticTokens/full"),
            (9, "textDocument/documentLink"),
        ] {
            let response = request(
                &mut server,
                json!({
                    "jsonrpc": "2.0", "id": id, "method": method,
                    "params": {
                        "textDocument": { "uri": uri },
                        "position": { "line": line, "character": character }
                    }
                }),
            );
            assert_ne!(
                response["result"],
                Value::Null,
                "{method} should return a result"
            );
        }

        let renamed = request(
            &mut server,
            json!({
                "jsonrpc": "2.0", "id": 8, "method": "textDocument/rename",
                "params": {
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                    "newName": "settled"
                }
            }),
        );
        assert!(renamed["result"]["changes"][&uri]
            .as_array()
            .is_some_and(|edits| edits.len() >= 3));
    }

    #[test]
    fn completion_is_contextual_for_enums_and_typed_parameters() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("CLI crate should be inside the repository");
        let fixture = repository.join("tests/compile/pass/56_tagged_sum_match.jadpo");
        let project = analyze_project(&fixture).expect("fixture should analyze");
        let index = LanguageIndex::build(&project);
        let source = &project.syntax.sources[0];

        let enum_offset = source
            .source_text
            .find("PaymentOutcome.paid")
            .expect("enum use should exist")
            + "PaymentOutcome.".len();
        assert_eq!(
            completion_container(&project, &index, &source.source_name, enum_offset),
            Some("PaymentOutcome".to_owned())
        );

        let parameter_offset = source
            .source_text
            .find("input.outcome")
            .expect("parameter field selection should exist")
            + "input.".len();
        assert_eq!(
            completion_container(&project, &index, &source.source_name, parameter_offset),
            Some("OutcomeRequest".to_owned())
        );
    }

    #[test]
    fn publishes_live_diagnostics_for_unsaved_document_text() {
        let temporary =
            std::env::temp_dir().join(format!("jadpo-lsp-live-diagnostic-{}", std::process::id()));
        std::fs::create_dir_all(&temporary).expect("temporary project should be created");
        let source = temporary.join("app.jadpo");
        std::fs::write(&source, "type Valid = Text {}\n")
            .expect("valid disk source should be written");
        let uri = path_to_uri(&source);
        let mut server = Server::default();
        request(
            &mut server,
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "rootUri": path_to_uri(&temporary) }
            }),
        );

        let mut output = Vec::new();
        server
            .handle(
                json!({
                    "jsonrpc": "2.0", "method": "textDocument/didOpen",
                    "params": { "textDocument": {
                        "uri": uri, "languageId": "jadpo", "version": 1,
                        "text": "type Broken = {\n"
                    }}
                }),
                &mut output,
            )
            .expect("open notification should be handled");
        let published = read_message(&mut Cursor::new(output))
            .expect("diagnostic notification should decode")
            .expect("diagnostic notification should exist");
        assert_eq!(published["method"], "textDocument/publishDiagnostics");
        assert!(published["params"]["diagnostics"]
            .as_array()
            .is_some_and(|diagnostics| !diagnostics.is_empty()));
        std::fs::remove_dir_all(temporary).expect("temporary project should be removable");
    }

    #[test]
    fn navigates_imported_symbols_across_files() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("CLI crate should be inside the repository");
        let project = repository.join("examples/module-seed");
        let api = project.join("api.jadpo");
        let domain = project.join("domain.jadpo");
        let source = std::fs::read_to_string(&api).expect("API source should be readable");
        let byte = source
            .rfind("TodoTitle")
            .expect("imported type use should exist");
        let (line, character) = offset_to_line_character(&source, byte);
        let uri = path_to_uri(&api);
        let mut server = Server::default();
        request(
            &mut server,
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "rootUri": path_to_uri(&project) }
            }),
        );
        let definition = request(
            &mut server,
            json!({
                "jsonrpc": "2.0", "id": 2, "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                }
            }),
        );
        assert_eq!(definition["result"]["uri"], path_to_uri(&domain));
    }

    #[test]
    fn provides_callable_signature_help() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("CLI crate should be inside the repository");
        let fixture = repository.join("tests/compile/pass/58_authored_tests.jadpo");
        let source = std::fs::read_to_string(&fixture).expect("fixture should be readable");
        let byte = source.rfind("add(").expect("test call should exist") + "add(".len();
        let (line, character) = offset_to_line_character(&source, byte);
        let uri = path_to_uri(&fixture);
        let mut server = Server::default();
        request(
            &mut server,
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "rootUri": uri }
            }),
        );
        let help = request(
            &mut server,
            json!({
                "jsonrpc": "2.0", "id": 2, "method": "textDocument/signatureHelp",
                "params": {
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                }
            }),
        );
        assert_eq!(
            help["result"]["signatures"][0]["label"],
            "add(left: Number, right: Number) -> Number"
        );
    }

    #[test]
    fn exposes_the_canonical_seed_over_the_protocol() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("CLI crate should be inside the repository");
        let project = repository.join("examples/jadpo-seed");
        let source = project.join("app.jadpo");
        let uri = path_to_uri(&source);
        let mut server = Server::default();
        request(
            &mut server,
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "rootUri": path_to_uri(&project) }
            }),
        );
        let symbols = request(
            &mut server,
            json!({
                "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentSymbol",
                "params": { "textDocument": { "uri": uri } }
            }),
        );
        assert!(symbols["result"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["name"] == "RegisterCustomer")));
    }

    fn request(server: &mut Server, message: Value) -> Value {
        let mut output = Vec::new();
        server
            .handle(message, &mut output)
            .expect("request should be handled");
        read_message(&mut Cursor::new(output))
            .expect("response should decode")
            .expect("response should exist")
    }
}
