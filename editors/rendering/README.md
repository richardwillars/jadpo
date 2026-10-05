# Jadpo syntax-renderer adapters

The stable language and fence identifier is `jadpo`; the TextMate scope is
`source.jadpo`. VS Code uses the checked-in TextMate grammar directly. The
Shiki adapter also consumes that grammar. Prism, Highlight.js, and Monaco use
small lexical adapters that derive their keyword, literal, primitive-type, and
operator vocabulary from the same TextMate grammar file. These adapters colour
source for display only; the Jadpo compiler remains the syntax and semantic
authority.

## Shiki

Register the TextMate grammar from [`shiki.mjs`](shiki.mjs) with Shiki's custom
language support:

```js
import { createHighlighter } from "shiki";
import jadpo from "./shiki.mjs";

const highlighter = await createHighlighter({
  langs: [jadpo],
  themes: ["nord"],
});
const html = highlighter.codeToHtml(source, { lang: "jadpo", theme: "nord" });
```

Shiki accepts TextMate grammar objects in its `langs` list. See its [custom
language guide](https://shiki.style/guide/load-lang).

## Prism

Pass the Prism instance to [`registerJadpoPrism`](prism.cjs):

```js
const { registerJadpoPrism } = require("./prism.cjs");
registerJadpoPrism(Prism);
```

This follows Prism's [new language definition
workflow](https://prismjs.com/extending.html).

## Highlight.js

Register [`highlightjs.cjs`](highlightjs.cjs) with the Highlight.js instance:

```js
hljs.registerLanguage("jadpo", require("./highlightjs.cjs"));
const html = hljs.highlight(source, { language: "jadpo" }).value;
```

Highlight.js documents this extension point as [`registerLanguage`](https://highlightjs.readthedocs.io/en/latest/api.html#registerlanguage).

## Monaco

Register [`registerJadpoMonaco`](monaco.mjs) once for the Monaco instance:

```js
import { registerJadpoMonaco } from "./monaco.mjs";
const registrations = registerJadpoMonaco(monaco);
```

It registers the `.jadpo` language, a Monarch tokenizer, comments, brackets,
and quote auto-closing. Monaco exposes custom Monarch tokenizers through
[`setMonarchTokensProvider`](https://microsoft.github.io/monaco-editor/typedoc/functions/editor_editor_api.languages.setMonarchTokensProvider.html).

## Fixtures and fallback

[`fixtures/jadpo-snippets.md`](fixtures/jadpo-snippets.md) contains fenced
examples for declarations, constraints, persistence, failures, routes,
comments, strings, and malformed source. Hosts that do not register the
`jadpo` language should show the original fenced content as plain text; they
must not relabel it as another language to force highlighting.

The adapters are covered by registration-shape tests in the VS Code test
suite. Prism 1.30.0 and Shiki 4.4.3 have also rendered all six fixtures in an
isolated local engine smoke check; the check confirmed Prism token categories
and Shiki's Jadpo TextMate scopes. Highlight.js and Monaco still have only
registration-shape coverage. Visual comparison in each renderer host, plus
plain-text fallback behavior, remains part of TOOL-003's integration evidence.
