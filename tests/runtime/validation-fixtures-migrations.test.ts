import { afterAll, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const root = mkdtempSync(join(tmpdir(), "jadpo-fixtures-migrations-"));
afterAll(() => rmSync(root, { recursive: true, force: true }));
const compiler = Bun.env.JADPO_BIN ?? new URL("../../jadpo/target/debug/jadpo", import.meta.url).pathname;
function command(...args: string[]) {
  const result = Bun.spawnSync([compiler, ...args], { stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0) throw new Error(`Compiler command failed: ${args.join(" ")}\n${result.stdout}\n${result.stderr}`);
  return result;
}
function project(name: string, source: string) { const path = join(root, name); mkdirSync(path); writeFileSync(join(path, "app.jadpo"), source); return path; }
const fixedId = "00000000-0000-4000-8000-000000000001";
const fixtureSource = `
entity Item { id: Text identity label: Text }
failure ItemConflict { kind: Conflict code: "item_conflict" }
input NewItem { id: Item.id label: Item.label }
action insert_item(input: NewItem) fails ItemConflict -> Item {
    return attempt create Item { id: input.id label: input.label } conflict: ItemConflict
}
action find_item(id: Item.id) -> Item? { return attempt query optional Item { where: id == id } }
fixture controlled { clock: fixed Instant("2026-01-15T12:00:00Z") }
`;
const authored = [
  `test "write then fail" using controlled {
    var row = call insert_item(NewItem { id: Item.id("${fixedId}") label: Item.label("private-test-state") })
    var found = call find_item(row.id)
    match found { none => { assert false } some(value) => { assert value.id == row.id } }
    assert false
  }`,
  `test "next test starts empty" using controlled {
    var absent = call find_item(Item.id("${fixedId}"))
    match absent { none => { assert true } some(value) => { assert false } }
    var row = call insert_item(NewItem { id: Item.id("${fixedId}") label: Item.label("fresh") })
    assert row.label == Item.label("fresh")
  }`,
  `test "state persists within one test" using controlled {
    var first = call find_item(Item.id("${fixedId}"))
    match first { none => { assert true } some(value) => { assert false } }
    var row = call insert_item(NewItem { id: Item.id("${fixedId}") label: Item.label("same-test") })
    var later = call find_item(row.id)
    match later { none => { assert false } some(value) => { assert value.id == row.id } }
  }`,
];
const fixtureProject = project("fixture-state", fixtureSource + authored.join("\n"));
const reorderedProject = project("fixture-reordered", fixtureSource + [authored[2], authored[0], authored[1]].join("\n"));
for (const path of [fixtureProject, reorderedProject]) command("build", path);
delete Bun.env.DATABASE_URL;
Bun.env.SQLITE_PATH = join(root, "production-sentinel.sqlite");
const fixtureApp = await import(pathToFileURL(join(fixtureProject, "build/target/app.ts")).href);
const reorderedApp = await import(pathToFileURL(join(reorderedProject, "build/target/app.ts")).href);
const sentinel = new Database(Bun.env.SQLITE_PATH, { strict: true });
sentinel.prepare('INSERT INTO item (id, label) VALUES (?, ?)').run(fixedId, "production-sentinel");
afterAll(() => { sentinel.close(); delete Bun.env.SQLITE_PATH; });
function outcomes(report: any) { return Object.fromEntries(report.results.map((result: any) => [result.name, result.status])); }

test("authored database fixtures isolate failed tests, reordered tests and repeated runs", async () => {
  for (const app of [fixtureApp, reorderedApp, fixtureApp]) {
    const report = await app.runTests();
    expect(report.summary).toEqual({ total: 3, passed: 2, failed: 1 });
    expect(outcomes(report)).toEqual({ "write then fail": "failed", "next test starts empty": "passed", "state persists within one test": "passed" });
    expect(report.results.find((item: any) => item.name === "write then fail").diagnostic.message).toContain("assertion failed at source bytes");
    expect(sentinel.prepare('SELECT id, label FROM item').all()).toEqual([{ id: fixedId, label: "production-sentinel" }]);
  }
});

test("concurrent authored-suite invocations retain independent database fixtures", async () => {
  const reports = await Promise.all([fixtureApp.runTests(), fixtureApp.runTests()]);
  for (const report of reports) {
    expect(report.summary).toEqual({ total: 3, passed: 2, failed: 1 });
    expect(outcomes(report)).toEqual({ "write then fail": "failed", "next test starts empty": "passed", "state persists within one test": "passed" });
  }
  expect(sentinel.prepare('SELECT id, label FROM item').all()).toEqual([{ id: fixedId, label: "production-sentinel" }]);
});

test("authored fixture database isolation survives a fresh process", () => {
  const runner = join(root, "fixture-runner.ts");
  writeFileSync(runner, `import { runTests } from ${JSON.stringify(join(fixtureProject, "build/target/app.ts"))}; console.log(JSON.stringify(await runTests()));`);
  const result = Bun.spawnSync([process.execPath, "--no-install", "--no-env-file", runner], { env: { PATH: Bun.env.PATH ?? "", SQLITE_PATH: join(root, "production-sentinel.sqlite") }, stdout: "pipe", stderr: "pipe" });
  expect(result.exitCode).toBe(0);
  const report = JSON.parse(result.stdout.toString());
  expect(report.summary).toEqual({ total: 3, passed: 2, failed: 1 });
  expect(sentinel.prepare('SELECT label FROM item').get()).toEqual({ label: "production-sentinel" });
});

const schema = `entity Parent { id: Uuid identity }
entity Entry {
 id: Uuid identity
 parent_id: Parent.id references Parent.id on_delete cascade
 label: Text index
 constraint parent_label: unique(parent_id, label)
}
`;
function migration(name: string, before: string, after: string, decision?: { strategy: string; evidence: { kind: string; value: string }[] }) {
  const path = project(name, before);
  command("build", path);
  const baselineSql = readFileSync(join(path, "build/sql/sqlite/schema.sql"), "utf8");
  command("schema", "init", path);
  const snapshot = join(path, "baseline.json");
  command("schema", "snapshot", path, snapshot);
  writeFileSync(join(path, "app.jadpo"), after);
  command("schema", "add", path);
  const decisions = join(path, "decisions.json");
  command("schema", "decision-template", path, "--against", snapshot, decisions);
  if (decision) {
    const artifact = JSON.parse(readFileSync(decisions, "utf8"));
    for (const item of artifact.decisions) Object.assign(item, decision);
    writeFileSync(decisions, JSON.stringify(artifact));
  }
  const output = join(path, "review.json");
  command("schema", "sql", path, "--against", snapshot, "--decisions", decisions, "--adapter", "sqlite", output);
  const review = JSON.parse(readFileSync(output, "utf8"));
  expect(review.executable).toBe(false);
  expect(review.review_required).toBe(true);
  expect(review.adapter).toBe("sqlite");
  return { path, baselineSql, review };
}
// This harness executes reviewed statements only on disposable test databases.
// Rollback on a failed statement belongs to the test executor; a review artifact
// remains explicitly non-executable and is not a deployment/migration runner.
function executeReview(db: Database, statements: string[]) {
  try {
    for (const statement of statements) {
      if (/^PRAGMA foreign_key_check/iu.test(statement.trim())) expect(db.query(statement).all()).toEqual([]);
      else db.exec(statement);
    }
  } catch (error) {
    if (db.inTransaction) db.exec("ROLLBACK");
    db.exec("PRAGMA foreign_keys = ON");
    throw error;
  }
}
function tables(db: Database) { return db.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name").all(); }

test("reviewed additive SQLite rebuild preserves data, identities, foreign keys and unique indexes", () => {
  const value = "O'Reilly'); DROP TABLE parent; --";
  const { baselineSql, review } = migration("addition", schema, schema.replace(" label: Text index", " label: Text index\n title: Text\n comment: Text?"), { strategy: "backfill", evidence: [{ kind: "typed_expression", value: `literal(${value})` }] });
  const db = new Database(":memory:");
  try {
    db.exec("PRAGMA foreign_keys = ON"); db.exec(baselineSql);
    db.prepare('INSERT INTO parent (id) VALUES (?)').run(fixedId);
    db.prepare('INSERT INTO entry (id, parent_id, label) VALUES (?, ?, ?)').run(fixedId, fixedId, "kept");
    executeReview(db, review.forward_sql);
    expect(db.prepare('SELECT id, parent_id, label, title, comment FROM entry').all()).toEqual([{ id: fixedId, parent_id: fixedId, label: "kept", title: value, comment: null }]);
    expect(() => db.prepare('INSERT INTO entry (id, parent_id, label, title) VALUES (?, ?, ?, ?)').run("duplicate", fixedId, "kept", "title")).toThrow();
    expect(() => db.prepare('INSERT INTO entry (id, parent_id, label, title) VALUES (?, ?, ?, ?)').run("orphan", "missing", "other", "title")).toThrow();
    const indexes = db.query("PRAGMA index_list('entry')").all() as { name: string }[];
    expect(indexes.some(index => index.name === "entry_label_idx")).toBe(true);
    expect(indexes.some(index => index.name === "entry_parent_id_idx")).toBe(true);
    executeReview(db, review.rollback_sql);
    expect(db.prepare('SELECT id, parent_id, label FROM entry').all()).toEqual([{ id: fixedId, parent_id: fixedId, label: "kept" }]);
    expect(db.query("PRAGMA table_info('entry')").all().map((field: any) => field.name)).toEqual(["id", "parent_id", "label"]);
    expect(tables(db)).toEqual([{ name: "entry" }, { name: "parent" }]);
    db.prepare('DELETE FROM parent WHERE id = ?').run(fixedId);
    expect(db.query("SELECT COUNT(*) AS count FROM entry").get()).toEqual({ count: 0 });
  } finally { db.close(); }
});

test("failed required-column narrowing can roll back without changing rows or leaving shadow tables", () => {
  const before = "entity Entry { id: Uuid identity label: Text? }";
  const { baselineSql, review } = migration("narrow", before, before.replace("Text?", "Text"), { strategy: "validate_existing", evidence: [{ kind: "typed_predicate", value: "not_null" }] });
  const db = new Database(":memory:");
  try {
    db.exec("PRAGMA foreign_keys = ON"); db.exec(baselineSql);
    db.prepare('INSERT INTO entry (id, label) VALUES (?, NULL)').run(fixedId);
    const originalSchema = db.query("SELECT sql FROM sqlite_master WHERE name = 'entry'").get();
    expect(() => executeReview(db, review.forward_sql)).toThrow();
    expect(db.inTransaction).toBe(false);
    expect(db.prepare('SELECT id, label FROM entry').all()).toEqual([{ id: fixedId, label: null }]);
    expect(db.query("SELECT sql FROM sqlite_master WHERE name = 'entry'").get()).toEqual(originalSchema);
    expect(tables(db)).toEqual([{ name: "entry" }]);
    db.prepare('UPDATE entry SET label = ? WHERE id = ?').run("validated", fixedId);
    executeReview(db, review.forward_sql);
    expect(() => db.prepare('UPDATE entry SET label = NULL WHERE id = ?').run(fixedId)).toThrow();
    executeReview(db, review.rollback_sql);
    db.prepare('UPDATE entry SET label = NULL WHERE id = ?').run(fixedId);
    expect(db.prepare('SELECT label FROM entry').get()).toEqual({ label: null });
  } finally { db.close(); }
});
