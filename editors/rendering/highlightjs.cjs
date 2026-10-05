"use strict";

const { CONSTANTS, KEYWORDS, OPERATORS, PRIMITIVE_TYPES, wordsPattern } = require("./shared.cjs");

module.exports = function jadpoHighlightJs(hljs) {
  return {
    name: "Jadpo",
    aliases: ["jadpo"],
    keywords: {
      keyword: KEYWORDS.join(" "),
      literal: CONSTANTS.join(" "),
      built_in: PRIMITIVE_TYPES.join(" "),
    },
    contains: [
      { scope: "comment", begin: /\/\//, end: /$/ },
      {
        scope: "string",
        begin: /"/,
        end: /"/,
        contains: [{ scope: "char.escape", match: /\\["\\nrt]/ }],
      },
      { scope: "number", match: /\b[0-9]+(?:\.[0-9]+)?\b/ },
      { scope: "operator", match: new RegExp(OPERATORS) },
      { scope: "type", match: new RegExp(wordsPattern(PRIMITIVE_TYPES)) },
    ],
    illegal: /<\//,
    case_insensitive: false,
  };
};
