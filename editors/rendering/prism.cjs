"use strict";

const { CONSTANTS, KEYWORDS, OPERATORS, PRIMITIVE_TYPES, wordsPattern } = require("./shared.cjs");

function registerJadpoPrism(Prism) {
  if (!Prism || !Prism.languages) throw new TypeError("A Prism instance is required");
  Prism.languages.jadpo = {
    comment: { pattern: /\/\/[^\r\n]*/, greedy: true },
    string: { pattern: /"(?:\\["\\nrt]|[^"\\])*"/, greedy: true },
    constant: new RegExp(wordsPattern(CONSTANTS)),
    number: /\b[0-9]+(?:\.[0-9]+)?\b/,
    keyword: new RegExp(wordsPattern(KEYWORDS)),
    "class-name": new RegExp(wordsPattern(PRIMITIVE_TYPES)),
    operator: new RegExp(OPERATORS),
    punctuation: /[{}()[\],.:?]/,
  };
  return Prism.languages.jadpo;
}

module.exports = { registerJadpoPrism };
