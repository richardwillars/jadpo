"use strict";

function problemMessage(item) {
  return String((item.data && item.data.summary) || item.message || "Jadpo diagnostic");
}

function diagnosticDetailsHtml(data = {}, fallbackSummary = "Jadpo diagnostic") {
  const next = data.recommendedNextStep || {};
  const impact = data.impact || {};
  const context = Object.entries(data.context || {})
    .map(([key, value]) => `<li><code>${escapeHtml(key)}</code>: <code>${escapeHtml(value)}</code></li>`)
    .join("");
  const affected = (impact.affected || [])
    .map(value => `<li><code>${escapeHtml(value)}</code></li>`)
    .join("");
  const alternatives = (data.alternatives || [])
    .map(item => `<li><strong>${escapeHtml(item.title || "")}</strong><p>${escapeHtml(item.reason || "")}</p>${repairPreviewHtml(item)}</li>`)
    .join("");

  return `<!doctype html><meta charset="utf-8"><style>body{font-family:var(--vscode-font-family);padding:1.2rem;line-height:1.5;max-width:60rem}code{font-family:var(--vscode-editor-font-family)}li{margin:.6rem 0}h2{margin-top:1.4rem}.meta{opacity:.78}</style>`
    + `<h1>${escapeHtml(data.summary || fallbackSummary)}</h1>`
    + `<p>${escapeHtml(data.reason || "")}</p>`
    + `<h2>Recommended next step</h2><p><strong>${escapeHtml(next.title || "")}</strong></p><p>${escapeHtml(next.reason || "")}</p>`
    + `<p class="meta"><code>${escapeHtml(next.kind || "")}</code> · owner <code>${escapeHtml(next.decisionOwner || data.decisionOwner || "")}</code></p>`
    + repairPreviewHtml(next)
    + (alternatives ? `<h2>Alternatives</h2><ol>${alternatives}</ol>` : "")
    + `<h2>Impact</h2><p><strong>Behavior:</strong> ${escapeHtml(impact.behavioral || "")}</p><p><strong>Public contract:</strong> ${escapeHtml(impact.publicContract || "")}</p>`
    + (affected ? `<p><strong>Affected:</strong></p><ul>${affected}</ul>` : "")
    + (context ? `<h2>Context</h2><ul>${context}</ul>` : "")
    + `<p class="meta">Rule <code>${escapeHtml(data.ruleId || "")}</code> · <a href="${escapeHtml(diagnosticHelpUrl(data.helpId))}">View full guidance</a> · Revision <code>${escapeHtml(data.sourceRevision || "")}</code></p>`;
}

function diagnosticHelpUrl(helpId) {
  return `https://jadpo.dev/docs/${String(helpId || "")}`;
}

function repairPreviewHtml(repair = {}) {
  const preview = repair.preview || {};
  const edits = (repair.edits || [])
    .map(edit => `<li><code>${escapeHtml(edit.source || "")}:${escapeHtml(edit.range && edit.range.start)}..${escapeHtml(edit.range && edit.range.end)}</code> → <code>${escapeHtml(visibleReplacement(edit.replacement))}</code></li>`)
    .join("");
  return `<p><strong>Behavior:</strong> ${escapeHtml(preview.behavioral || "")}</p><p><strong>Public contract:</strong> ${escapeHtml(preview.publicContract || "")}</p>${edits ? `<ul>${edits}</ul>` : ""}`;
}

function visibleReplacement(value) {
  if (value === "") return "<delete>";
  return String(value == null ? "" : value).replace(/\n/g, "\\n").replace(/\r/g, "\\r").replace(/\t/g, "\\t");
}

function escapeHtml(value) {
  return String(value == null ? "" : value).replace(/[&<>"']/g, character => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;", "'": "&#39;" })[character]);
}

module.exports = { diagnosticDetailsHtml, problemMessage };
