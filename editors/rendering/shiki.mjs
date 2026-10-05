import grammar from "../vscode/syntaxes/jadpo.tmLanguage.json" with { type: "json" };

const jadpo = Object.freeze({ ...grammar, name: "jadpo", aliases: ["Jadpo"] });

export { jadpo as jadpoShikiLanguage };
export default jadpo;
