const vscode = require("vscode");
const { spawn } = require("child_process");
const fs = require("fs");
const path = require("path");
const { diagnosticDetailsHtml, problemMessage } = require("./diagnostic-presentation");

const tokenTypes = ["namespace", "type", "enum", "enumMember", "property", "function", "variable", "parameter", "string", "number", "keyword", "operator", "comment"];

class JadpoLanguageClient {
  constructor(context) {
    this.context = context;
    this.nextId = 1;
    this.pending = new Map();
    this.buffer = Buffer.alloc(0);
    this.diagnostics = vscode.languages.createDiagnosticCollection("jadpo");
    this.output = vscode.window.createOutputChannel("Jadpo Language Server");
    this.changeTimers = new Map();
    this.diagnosticDetails = new Map();
    this.context.subscriptions.push(this.diagnostics, this.output);
    this.ready = this.start();
  }

  async start() {
    const configuredExecutable = vscode.workspace.getConfiguration("jadpo").get("executable", "jadpo");
    const extensionPath = fs.realpathSync(this.context.extensionPath);
    const workspaceExecutable = path.resolve(extensionPath, "../../jadpo/target/debug/jadpo");
    const executable = configuredExecutable === "jadpo" && fs.existsSync(workspaceExecutable)
      ? workspaceExecutable
      : configuredExecutable;
    const folder = vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders[0];
    const rootUri = folder ? folder.uri : vscode.Uri.file(process.cwd());
    this.process = spawn(executable, ["lsp"], { cwd: rootUri.fsPath, stdio: ["pipe", "pipe", "pipe"] });
    this.process.stdout.on("data", chunk => this.consume(chunk));
    this.process.stderr.on("data", chunk => this.output.append(chunk.toString()));
    this.process.on("error", error => {
      this.output.appendLine(`Could not start Jadpo language server: ${error.message}`);
      vscode.window.showErrorMessage(`Could not start Jadpo language server: ${error.message}`);
      this.rejectPending(error);
    });
    this.process.on("exit", (code, signal) => {
      if (code !== 0 && code !== null) this.output.appendLine(`Jadpo language server exited with code ${code}${signal ? ` (${signal})` : ""}.`);
      this.rejectPending(new Error("Jadpo language server stopped"));
    });
    await this.request("initialize", {
      processId: process.pid,
      rootUri: rootUri.toString(),
      workspaceFolders: [{ uri: rootUri.toString(), name: folder ? folder.name : "workspace" }],
      capabilities: {
        textDocument: {
          synchronization: { didSave: true },
          semanticTokens: { requests: { full: true }, tokenTypes, tokenModifiers: [], formats: ["relative"] }
        },
        workspace: { workspaceFolders: true }
      }
    }, true);
    this.notify("initialized", {});
    for (const document of vscode.workspace.textDocuments) {
      if (document.languageId === "jadpo") this.open(document);
    }
  }

  consume(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (true) {
      const headerEnd = this.buffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) return;
      const header = this.buffer.subarray(0, headerEnd).toString("ascii");
      const match = /^Content-Length:\s*(\d+)$/im.exec(header);
      if (!match) {
        this.output.appendLine("Language server sent a message without Content-Length.");
        this.buffer = this.buffer.subarray(headerEnd + 4);
        continue;
      }
      const length = Number(match[1]);
      const bodyStart = headerEnd + 4;
      if (this.buffer.length < bodyStart + length) return;
      const body = this.buffer.subarray(bodyStart, bodyStart + length).toString("utf8");
      this.buffer = this.buffer.subarray(bodyStart + length);
      try {
        this.handleMessage(JSON.parse(body));
      } catch (error) {
        this.output.appendLine(`Invalid language-server message: ${error.message}`);
      }
    }
  }

  handleMessage(message) {
    if (message.id !== undefined) {
      const pending = this.pending.get(message.id);
      if (!pending) return;
      this.pending.delete(message.id);
      if (message.error) pending.reject(new Error(message.error.message));
      else pending.resolve(message.result);
      return;
    }
    if (message.method === "textDocument/publishDiagnostics") {
      const uri = vscode.Uri.parse(message.params.uri);
      for (const key of [...this.diagnosticDetails.keys()]) {
        if (key.startsWith(`${uri.toString()}|`)) this.diagnosticDetails.delete(key);
      }
      const diagnostics = (message.params.diagnostics || []).map(item => {
        const diagnostic = new vscode.Diagnostic(fromRange(item.range), problemMessage(item), fromDiagnosticSeverity(item.severity));
        diagnostic.code = item.code;
        diagnostic.source = item.source || "jadpo";
        diagnostic.relatedInformation = (item.relatedInformation || []).map(related =>
          new vscode.DiagnosticRelatedInformation(
            fromLocation(related.location),
            related.message,
          ));
        this.diagnosticDetails.set(diagnosticKey(uri, diagnostic), item.data || {});
        return diagnostic;
      });
      this.diagnostics.set(uri, diagnostics);
    } else if (message.method === "window/showMessage") {
      const text = message.params.message;
      if (message.params.type === 1) vscode.window.showErrorMessage(text);
      else if (message.params.type === 2) vscode.window.showWarningMessage(text);
      else vscode.window.showInformationMessage(text);
    }
  }

  request(method, params, beforeReady = false) {
    const send = () => new Promise((resolve, reject) => {
      const id = this.nextId++;
      this.pending.set(id, { resolve, reject });
      this.send({ jsonrpc: "2.0", id, method, params });
    });
    return beforeReady ? send() : this.ready.then(send);
  }

  notify(method, params) {
    this.send({ jsonrpc: "2.0", method, params });
  }

  send(message) {
    if (!this.process || !this.process.stdin.writable) return;
    const body = Buffer.from(JSON.stringify(message), "utf8");
    this.process.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
    this.process.stdin.write(body);
  }

  rejectPending(error) {
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
  }

  open(document) {
    this.notify("textDocument/didOpen", { textDocument: documentItem(document) });
  }

  change(document) {
    const key = document.uri.toString();
    clearTimeout(this.changeTimers.get(key));
    this.changeTimers.set(key, setTimeout(() => {
      this.changeTimers.delete(key);
      this.notify("textDocument/didChange", {
        textDocument: { uri: key, version: document.version },
        contentChanges: [{ text: document.getText() }]
      });
    }, 120));
  }

  save(document) {
    this.notify("textDocument/didSave", { textDocument: { uri: document.uri.toString() }, text: document.getText() });
  }

  close(document) {
    const key = document.uri.toString();
    clearTimeout(this.changeTimers.get(key));
    this.changeTimers.delete(key);
    this.notify("textDocument/didClose", { textDocument: { uri: key } });
    this.diagnostics.delete(document.uri);
  }

  async stop() {
    for (const timer of this.changeTimers.values()) clearTimeout(timer);
    this.changeTimers.clear();
    if (!this.process || this.process.killed) return;
    try { await this.request("shutdown", null); } catch {}
    this.notify("exit", null);
  }
}

function activate(context) {
  const client = new JadpoLanguageClient(context);
  context.subscriptions.push({ dispose: () => client.stop() });
  context.subscriptions.push(vscode.workspace.onDidOpenTextDocument(document => {
    if (document.languageId === "jadpo") client.ready.then(() => client.open(document));
  }));
  context.subscriptions.push(vscode.workspace.onDidChangeTextDocument(event => {
    if (event.document.languageId === "jadpo") client.change(event.document);
  }));
  context.subscriptions.push(vscode.workspace.onDidSaveTextDocument(document => {
    if (document.languageId === "jadpo") client.save(document);
  }));
  context.subscriptions.push(vscode.workspace.onDidCloseTextDocument(document => {
    if (document.languageId === "jadpo") client.close(document);
  }));
  context.subscriptions.push(vscode.commands.registerCommand("jadpo.check", async () => {
    const document = vscode.window.activeTextEditor && vscode.window.activeTextEditor.document;
    if (document && document.languageId === "jadpo") {
      await client.ready;
      client.save(document);
      vscode.window.setStatusBarMessage("Jadpo project checked", 2000);
    }
  }));
  context.subscriptions.push(vscode.commands.registerCommand("jadpo.showDiagnosticAlternative", argument => {
    vscode.window.showInformationMessage(`${argument.ruleId}: ${argument.reason}`);
  }));
  context.subscriptions.push(vscode.commands.registerCommand("jadpo.showDiagnosticDetails", (uri, diagnostic) => {
    const data = client.diagnosticDetails.get(diagnosticKey(uri, diagnostic));
    if (!data) return;
    const panel = vscode.window.createWebviewPanel("jadpoDiagnostic", diagnostic.message, vscode.ViewColumn.Beside, {});
    panel.webview.html = diagnosticDetailsHtml(data, diagnostic.message);
  }));

  context.subscriptions.push(vscode.languages.registerDocumentSymbolProvider("jadpo", {
    async provideDocumentSymbols(document) {
      return (await client.request("textDocument/documentSymbol", textDocumentParams(document))).map(fromSymbolInformation);
    }
  }));
  context.subscriptions.push(vscode.languages.registerWorkspaceSymbolProvider({
    async provideWorkspaceSymbols(query) {
      return (await client.request("workspace/symbol", { query })).map(fromSymbolInformation);
    }
  }));
  context.subscriptions.push(vscode.languages.registerDefinitionProvider("jadpo", {
    async provideDefinition(document, position) {
      return fromLocation(await client.request("textDocument/definition", positionParams(document, position)));
    }
  }));
  context.subscriptions.push(vscode.languages.registerReferenceProvider("jadpo", {
    async provideReferences(document, position, options) {
      const result = await client.request("textDocument/references", { ...positionParams(document, position), context: { includeDeclaration: options.includeDeclaration } });
      return (result || []).map(fromLocation);
    }
  }));
  context.subscriptions.push(vscode.languages.registerHoverProvider("jadpo", {
    async provideHover(document, position) {
      const result = await client.request("textDocument/hover", positionParams(document, position));
      if (!result) return undefined;
      const value = typeof result.contents === "string" ? result.contents : result.contents.value;
      return new vscode.Hover(new vscode.MarkdownString(value));
    }
  }));
  context.subscriptions.push(vscode.languages.registerCompletionItemProvider("jadpo", {
    async provideCompletionItems(document, position) {
      const result = await client.request("textDocument/completion", positionParams(document, position));
      return (result.items || []).map(item => {
        const completion = new vscode.CompletionItem(item.label, Math.max(0, (item.kind || 1) - 1));
        completion.detail = item.detail;
        return completion;
      });
    }
  }, "."));
  context.subscriptions.push(vscode.languages.registerSignatureHelpProvider("jadpo", {
    async provideSignatureHelp(document, position) {
      const result = await client.request("textDocument/signatureHelp", positionParams(document, position));
      if (!result) return undefined;
      const help = new vscode.SignatureHelp();
      help.activeSignature = result.activeSignature || 0;
      help.activeParameter = result.activeParameter || 0;
      help.signatures = (result.signatures || []).map(item => {
        const signature = new vscode.SignatureInformation(item.label);
        signature.parameters = (item.parameters || []).map(parameter => new vscode.ParameterInformation(parameter.label));
        return signature;
      });
      return help;
    }
  }, "(", ","));
  context.subscriptions.push(vscode.languages.registerRenameProvider("jadpo", {
    async prepareRename(document, position) {
      const result = await client.request("textDocument/prepareRename", positionParams(document, position));
      return result ? { range: fromRange(result.range), placeholder: result.placeholder } : undefined;
    },
    async provideRenameEdits(document, position, newName) {
      const result = await client.request("textDocument/rename", { ...positionParams(document, position), newName });
      if (!result) return undefined;
      const edit = new vscode.WorkspaceEdit();
      for (const [uri, edits] of Object.entries(result.changes || {})) {
        for (const item of edits) edit.replace(vscode.Uri.parse(uri), fromRange(item.range), item.newText);
      }
      return edit;
    }
  }));
  const legend = new vscode.SemanticTokensLegend(tokenTypes, []);
  context.subscriptions.push(vscode.languages.registerDocumentSemanticTokensProvider("jadpo", {
    async provideDocumentSemanticTokens(document) {
      const result = await client.request("textDocument/semanticTokens/full", textDocumentParams(document));
      return new vscode.SemanticTokens(new Uint32Array(result.data || []));
    }
  }, legend));
  context.subscriptions.push(vscode.languages.registerDocumentFormattingEditProvider("jadpo", {
    async provideDocumentFormattingEdits(document, options) {
      const result = await client.request("textDocument/formatting", { ...textDocumentParams(document), options });
      return (result || []).map(item => vscode.TextEdit.replace(fromRange(item.range), item.newText));
    }
  }));
  context.subscriptions.push(vscode.languages.registerCodeActionsProvider("jadpo", {
    async provideCodeActions(document, range, context) {
      const diagnostics = context.diagnostics.map(diagnostic => ({
        range: toRange(diagnostic.range),
        message: diagnostic.message,
        code: diagnostic.code,
        source: diagnostic.source,
        data: client.diagnosticDetails.get(diagnosticKey(document.uri, diagnostic)) || null
      }));
      const result = await client.request("textDocument/codeAction", {
        ...textDocumentParams(document),
        range: toRange(range),
        context: { diagnostics }
      });
      const actions = (result || []).map(fromCodeAction);
      for (const diagnostic of context.diagnostics) {
        actions.push(new vscode.CodeAction(`Show details for ${diagnostic.code}`, vscode.CodeActionKind.QuickFix));
        actions[actions.length - 1].command = { command: "jadpo.showDiagnosticDetails", title: "Show details", arguments: [document.uri, diagnostic] };
      }
      return actions;
    }
  }, { providedCodeActionKinds: [vscode.CodeActionKind.QuickFix] }));
  context.subscriptions.push(vscode.languages.registerDocumentLinkProvider("jadpo", {
    async provideDocumentLinks(document) {
      const result = await client.request("textDocument/documentLink", textDocumentParams(document));
      return (result || []).map(item => {
        const link = new vscode.DocumentLink(fromRange(item.range), vscode.Uri.parse(item.target));
        link.tooltip = item.tooltip;
        return link;
      });
    }
  }));
}

function documentItem(document) {
  return { uri: document.uri.toString(), languageId: "jadpo", version: document.version, text: document.getText() };
}

function textDocumentParams(document) {
  return { textDocument: { uri: document.uri.toString() } };
}

function positionParams(document, position) {
  return { ...textDocumentParams(document), position: { line: position.line, character: position.character } };
}

function fromRange(range) {
  return new vscode.Range(range.start.line, range.start.character, range.end.line, range.end.character);
}

function fromLocation(location) {
  if (!location) return undefined;
  return new vscode.Location(vscode.Uri.parse(location.uri), fromRange(location.range));
}

function fromSymbolInformation(item) {
  return new vscode.SymbolInformation(item.name, Math.max(0, item.kind - 1), item.containerName || "", fromLocation(item.location));
}

function fromDiagnosticSeverity(severity) {
  if (severity === 2) return vscode.DiagnosticSeverity.Warning;
  if (severity === 3 || severity === 4) return vscode.DiagnosticSeverity.Information;
  return vscode.DiagnosticSeverity.Error;
}

function diagnosticKey(uri, diagnostic) {
  return `${uri.toString()}|${diagnostic.range.start.line}:${diagnostic.range.start.character}-${diagnostic.range.end.line}:${diagnostic.range.end.character}|${diagnostic.code}`;
}

function toRange(range) {
  return { start: { line: range.start.line, character: range.start.character }, end: { line: range.end.line, character: range.end.character } };
}

function fromCodeAction(item) {
  const action = new vscode.CodeAction(item.title, new vscode.CodeActionKind(item.kind || "quickfix"));
  action.isPreferred = Boolean(item.isPreferred);
  if (item.edit && item.edit.changes) {
    action.edit = new vscode.WorkspaceEdit();
    for (const [uri, edits] of Object.entries(item.edit.changes)) {
      for (const edit of edits) action.edit.replace(vscode.Uri.parse(uri), fromRange(edit.range), edit.newText);
    }
  }
  if (item.command) action.command = item.command;
  return action;
}

async function deactivate() {}

module.exports = { activate, deactivate };
