"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const { diagnosticDetailsHtml, problemMessage } = require("../diagnostic-presentation");

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
