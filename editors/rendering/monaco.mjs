import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { CONSTANTS, KEYWORDS, OPERATORS, PRIMITIVE_TYPES, languageId, wordsPattern } = require("./shared.cjs");

const STRING = /"(?:\\["\\nrt]|[^"\\])*"/;
const NUMBER = /\b[0-9]+(?:\.[0-9]+)?\b/;
const IDENTIFIER = /[A-Za-z_][A-Za-z0-9_]*/;
const PUNCTUATION = /[{}()[\],.:?]/;

function registerJadpoMonaco(monaco) {
  if (!monaco || !monaco.languages) throw new TypeError("A Monaco instance is required");
  const language = monaco.languages.register({
    id: languageId,
    extensions: [".jadpo"],
    aliases: ["Jadpo", "jadpo"],
  });
  const tokens = monaco.languages.setMonarchTokensProvider(languageId, {
    defaultToken: "",
    tokenPostfix: ".jadpo",
    keywords: [...KEYWORDS],
    typeKeywords: [...PRIMITIVE_TYPES],
    tokenizer: {
      root: [
        [/\/\/.*$/, "comment"],
        [STRING, "string"],
        [new RegExp(wordsPattern(CONSTANTS)), "constant"],
        [new RegExp(wordsPattern(KEYWORDS)), "keyword"],
        [new RegExp(wordsPattern(PRIMITIVE_TYPES)), "type"],
        [/\b[A-Z][A-Za-z0-9_]*\b/, "type.identifier"],
        [NUMBER, "number"],
        [new RegExp(OPERATORS), "operator"],
        [PUNCTUATION, "delimiter"],
        [IDENTIFIER, "identifier"],
      ],
    },
  });
  const configuration = monaco.languages.setLanguageConfiguration(languageId, {
    comments: { lineComment: "//" },
    brackets: [["{", "}"], ["[", "]"], ["(", ")"]],
    autoClosingPairs: [
      { open: "{", close: "}" },
      { open: "[", close: "]" },
      { open: "(", close: ")" },
      { open: '"', close: '"' },
    ],
  });
  return [language, tokens, configuration];
}

export { registerJadpoMonaco };
