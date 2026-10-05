"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const { diagnosticDetailsHtml, problemMessage } = require("../diagnostic-presentation");
const { GENERATED_ARTIFACTS } = require("../generated-artifacts");

function authDiagnostic() {
  const choice = (title, replacement, behavioral, publicContract) => ({
    kind: "human_decision",
    title,
    reason: `${title} because the security boundary must be explicit.`,
    decisionOwner: "human",
    preferred: false,
    edits: [{ source: "app.jadpo", range: { start: 10, end: 15 }, replacement }],
    preview: { behavioral, publicContract },
  });
  return {
    schemaVersion: 2,
    diagnosticId: "route.auth_value_invalid@app.jadpo:10:15",
    sourceRevision: "src_test",
    summary: "Route authentication value `nonke` is not valid",
    reason: "Routes require authentication by default; only `auth: none` opts out.",
    recommendedNextStep: {
      kind: "human_decision",
      title: "Choose the authentication boundary for `POST /registrations`",
      reason: "The compiler cannot choose a security policy.",
      decisionOwner: "human",
      preferred: false,
      edits: [],
      preview: { behavioral: "Blocked until chosen.", publicContract: "No change yet." },
    },
    alternatives: [
      choice("Keep authentication required for this route", "", "Authentication remains required.", "The route remains authenticated."),
      choice("Make this route explicitly unauthenticated", "none", "Authentication is not required.", "The route becomes publicly callable."),
    ],
    decisionOwner: "human",
    ruleId: "route.auth_value_invalid",
    helpId: "diagnostics/route.auth_value_invalid",
    context: { route: "POST /registrations", found: "nonke" },
    impact: {
      behavioral: "Compilation is blocked.",
      publicContract: "The security boundary is unresolved.",
      affected: ["POST /registrations"],
      queryId: null,
    },
  };
}

test("Problems uses only the canonical human summary", () => {
  const data = authDiagnostic();
  assert.equal(problemMessage({ message: "legacy message", data }), data.summary);
  assert.equal(problemMessage({ message: "fallback message" }), "fallback message");
});

test("Details preserves the complete shared repair protocol", () => {
  const data = authDiagnostic();
  const html = diagnosticDetailsHtml(data);
  for (const required of [
    data.summary,
    data.reason,
    data.recommendedNextStep.title,
    data.recommendedNextStep.preview.behavioral,
    data.alternatives[0].title,
    data.alternatives[0].preview.publicContract,
    data.alternatives[1].title,
    data.alternatives[1].preview.publicContract,
    data.impact.behavioral,
    data.impact.publicContract,
    data.context.route,
    data.ruleId,
    data.helpId,
    data.sourceRevision,
  ]) assert.ok(html.includes(required), `missing ${required}`);
  assert.ok(html.includes("&lt;delete&gt;"));
  assert.ok(html.includes(`href="https://jadpo.dev/docs/${data.helpId}"`));
  assert.ok(html.includes("View full guidance"));
});

test("Details escapes every compiler-supplied field", () => {
  const canary = `<img src=x onerror="credential-canary">`;
  const data = authDiagnostic();
  data.summary = canary;
  data.reason = canary;
  data.context = { [canary]: canary };
  data.alternatives[0].title = canary;
  data.impact.behavioral = canary;
  const html = diagnosticDetailsHtml(data);
  assert.ok(!html.includes("<img"));
  assert.ok(!html.includes("onerror=\"credential-canary\""));
  assert.ok(html.includes("&lt;img"));
});

test("Source underlines are reserved for compiler diagnostics", () => {
  const extension = fs.readFileSync(path.join(__dirname, "../extension.js"), "utf8");
  const manifest = JSON.parse(fs.readFileSync(path.join(__dirname, "../package.json"), "utf8"));

  assert.ok(!extension.includes("registerDocumentLinkProvider"));
  assert.ok(!extension.includes("textDocument/documentLink"));
  assert.ok(manifest.contributes.commands.some(command => command.command === "jadpo.openGeneratedOpenApi"));
  assert.ok(manifest.contributes.commands.some(command => command.command === "jadpo.openGeneratedValidators"));
});

test("Generated artifact picker exposes each stable artifact through a safe relative path", () => {
  const paths = GENERATED_ARTIFACTS.map(item => item.path);
  assert.equal(new Set(paths).size, paths.length, "artifact paths must be unique");
  assert.deepEqual(paths, [
    "app.meta.json",
    "inventory/routes.json",
    "inventory/callables.json",
    "audit/failures.json",
    "audit/entities.json",
    "audit/transactions.json",
    "audit/configuration.json",
    "audit/policy.json",
    "audit/authentication.json",
    "validators/plan.json",
    "compatibility/public-failure-codes.json",
    "openapi/openapi.json",
    "diagnostics/catalogue.json",
    "diagnostics/reference.md",
  ]);
  for (const { label, path: artifactPath } of GENERATED_ARTIFACTS) {
    assert.ok(label.trim().length > 0);
    assert.ok(!artifactPath.startsWith("/"));
    assert.ok(!artifactPath.split("/").includes(".."));
    assert.ok(!artifactPath.includes("\\"));
  }
});

test("Generated artifact picker is contributed and activated as a command", () => {
  const manifest = JSON.parse(fs.readFileSync(path.join(__dirname, "../package.json"), "utf8"));
  assert.ok(manifest.activationEvents.includes("onCommand:jadpo.openGeneratedArtifact"));
  assert.ok(manifest.contributes.commands.some(command => command.command === "jadpo.openGeneratedArtifact"));
});
