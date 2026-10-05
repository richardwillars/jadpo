"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const { registerJadpoPrism } = require("../../rendering/prism.cjs");
const jadpoHighlightJs = require("../../rendering/highlightjs.cjs");
const { CONSTANTS, KEYWORDS, PRIMITIVE_TYPES, scopeName } = require("../../rendering/shared.cjs");

test("Prism adapter registers the canonical Jadpo lexical categories", () => {
  const Prism = { languages: {} };
  const grammar = registerJadpoPrism(Prism);
  assert.equal(grammar, Prism.languages.jadpo);
  assert.ok(grammar.comment.pattern.test("// comment"));
  assert.ok(grammar.string.pattern.test('"a string"'));
  assert.ok(grammar.number.test("123"));
  assert.ok(grammar.keyword.test("action"));
  assert.ok(grammar.constant.test("none"));
  assert.ok(grammar["class-name"].test("Text"));
  assert.ok(grammar.operator.test("=>"));
  assert.ok(grammar.punctuation.test("{"));
});

test("Highlight.js adapter returns a language definition from TextMate vocabulary", () => {
  const grammar = jadpoHighlightJs({});
  assert.equal(grammar.name, "Jadpo");
  assert.ok(grammar.aliases.includes("jadpo"));
  assert.equal(grammar.keywords.keyword.split(" ").length, KEYWORDS.length);
  assert.equal(grammar.keywords.literal.split(" ").length, CONSTANTS.length);
  assert.equal(grammar.keywords.built_in.split(" ").length, PRIMITIVE_TYPES.length);
  assert.ok(grammar.contains.some(mode => mode.scope === "string"));
  assert.ok(grammar.contains.some(mode => mode.scope === "comment"));
  assert.ok(grammar.contains.some(mode => mode.scope === "operator"));
});

test("Monaco adapter registers the canonical language ID, tokenizer, and editor pairs", async () => {
  const { registerJadpoMonaco } = await import("../../rendering/monaco.mjs");
  const calls = [];
  const monaco = {
    languages: {
      register: value => (calls.push(["register", value]), value),
      setMonarchTokensProvider: (id, provider) => (calls.push(["tokens", id, provider]), provider),
      setLanguageConfiguration: (id, configuration) => (calls.push(["configuration", id, configuration]), configuration),
    },
  };
  registerJadpoMonaco(monaco);
  assert.equal(calls[0][1].id, "jadpo");
  assert.deepEqual(calls[0][1].extensions, [".jadpo"]);
  assert.equal(calls[1][1], "jadpo");
  assert.ok(calls[1][2].tokenizer.root.length >= 8);
  assert.equal(calls[2][1], "jadpo");
  assert.equal(calls[2][2].comments.lineComment, "//");
  assert.equal(calls[2][2].brackets.length, 3);
});

test("Shiki adapter registers the same TextMate grammar under the fenced identifier", async () => {
  const { default: grammar } = await import("../../rendering/shiki.mjs");
  assert.equal(grammar.name, "jadpo");
  assert.equal(grammar.scopeName, scopeName);
  assert.equal(grammar.aliases[0], "Jadpo");
  assert.ok(grammar.repository.strings);
  assert.ok(grammar.repository.comments);
});

test("renderer fixtures use the stable fence and cover the documented syntax families", () => {
  const fixture = fs.readFileSync(path.join(__dirname, "../../rendering/fixtures/jadpo-snippets.md"), "utf8");
  const blocks = [...fixture.matchAll(/^```([^\n]*)\n([\s\S]*?)^```\s*$/gm)];
  assert.equal(blocks.length, 6);
  assert.ok(blocks.every(([, language]) => language === "jadpo"));
  for (const category of ["Declarations", "Constraints", "Persistence", "Failures", "Routes", "Malformed source"]) {
    assert.ok(fixture.includes(`## ${category}`));
  }
  assert.ok(fixture.includes("// Keep the user-supplied display name unchanged."));
  assert.ok(fixture.includes('format: "email"'));
});
