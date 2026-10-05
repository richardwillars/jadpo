"use strict";

const textMate = require("../vscode/syntaxes/jadpo.tmLanguage.json");

function matchPattern(repositoryKey) {
  const entry = textMate.repository[repositoryKey];
  if (!entry || !entry.patterns || !entry.patterns[0] || !entry.patterns[0].match) {
    throw new Error(`TextMate grammar is missing ${repositoryKey} match pattern`);
  }
  return entry.patterns[0].match;
}

function wordAlternatives(repositoryKey) {
  const pattern = matchPattern(repositoryKey);
  const match = /\\b\(([^()]*)\)\\b/.exec(pattern);
  if (!match) throw new Error(`TextMate grammar ${repositoryKey} pattern is not a word alternation`);
  return match[1].split("|");
}

function declarationAlternatives() {
  const pattern = matchPattern("declarations");
  const match = /\(([^()]*)\)\\s/.exec(pattern);
  if (!match) throw new Error("TextMate declaration pattern is not a keyword alternation");
  return match[1].split("|");
}

const KEYWORDS = Object.freeze([...new Set([
  ...declarationAlternatives(),
  ...wordAlternatives("keywords"),
])]);
const CONSTANTS = Object.freeze(wordAlternatives("constants"));
const PRIMITIVE_TYPES = Object.freeze(wordAlternatives("types"));
const OPERATORS = matchPattern("operators");

function wordsPattern(words) {
  return `\\b(?:${words.map(word => word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})\\b`;
}

module.exports = {
  languageId: "jadpo",
  scopeName: textMate.scopeName,
  KEYWORDS,
  CONSTANTS,
  PRIMITIVE_TYPES,
  OPERATORS,
  wordsPattern,
};
