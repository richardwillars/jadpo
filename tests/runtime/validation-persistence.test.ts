import { afterAll, describe, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { SQL } from "bun";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

// Only the dedicated verifier-owned disposable database may select PostgreSQL.
// Ambient application connection strings are never consumed by this suite.
const postgresUrl = Bun.env.JADPO_VALIDATION_PERSISTENCE_DATABASE_URL;
delete Bun.env.DATABASE_URL;
delete Bun.env.SQLITE_PATH;
const root = mkdtempSync(join(tmpdir(), "jadpo-validation-persistence-"));
const sqlitePath = join(root, "application.sqlite");
if (postgresUrl) Bun.env.DATABASE_URL = postgresUrl;
else Bun.env.SQLITE_PATH = sqlitePath;
const compiler = Bun.env.JADPO_BIN ?? new URL("../../jadpo/target/debug/jadpo", import.meta.url).pathname;
const source = `
type Template = Object { note: Email? }
type Linked = Object { note: Template.note }
entity Entry {
    id: Uuid
    label: Text
    note: Linked.note
    identity: id
    persistence { store: primary role: authority }
    cache hot { store: redis from: primary strategy: invalidate delivery: durable }
    action create_failing(input: CreateEntry)
        fails EntryConflict, DeliberateFailure
        -> Entry
    {
        var inserted = attempt create Entry {
            id: input.id label: input.label note: input.note
        } conflict: EntryConflict
        reject DeliberateFailure
    }
action insert_entry(input: CreateEntry) fails EntryConflict -> Entry {
    return attempt create Entry { id: input.id label: input.label note: input.note } conflict: EntryConflict
}
query find_entry(input: EntryLookup) freshness: authoritative fails EntryMissing -> Entry {
    return attempt query required Entry { where: id == input.id missing: EntryMissing }
}
action patch_entry(input: PatchRequest) fails EntryMissing, EntryConflict, PatchEmpty -> Entry {
    var changes: EntryPatch = input.changes
    return attempt update required Entry {
        where: id == input.id patch: changes
        empty: PatchEmpty missing: EntryMissing conflict: EntryConflict
    }
}
action update_label(input: BatchUpdate) fails EntryMissing, EntryConflict -> Entry {
    return attempt update required Entry {
        where: label == input.label set: { note: input.note }
        missing: EntryMissing conflict: EntryConflict
    }
}
action shadow_patch(id: Entry.id, changes: NotePatch, replacement: EntryPatch)
    fails EntryMissing, EntryConflict, PatchEmpty -> Entry
{
    if true {
        var changes: EntryPatch = replacement
        return attempt update required Entry {
            where: id == id patch: changes
            empty: PatchEmpty missing: EntryMissing conflict: EntryConflict
        }
    }
    return attempt update required Entry {
        where: id == id patch: changes
        empty: PatchEmpty missing: EntryMissing conflict: EntryConflict
    }
}
}
failure EntryConflict { kind: Conflict code: "entry_conflict" }
failure EntryMissing { kind: NotFound code: "entry_missing" }
failure PatchEmpty { kind: InvalidValue code: "patch_empty" }
failure DeliberateFailure { kind: Rejected code: "deliberate_failure" }
type CreateEntry = Object { id: Entry.id label: Entry.label note: Entry.note }
type EntryLookup = Object { id: Entry.id }
type EntryPatch = Object { label: Entry.label optional note: Entry.note optional }
type PatchRequest = Object { id: Entry.id changes: EntryPatch }
type NotePatch = Object { note: Entry.note optional }
type ShadowRequest = Object { id: Entry.id original: NotePatch replacement: EntryPatch }
type BatchUpdate = Object { label: Entry.label note: Entry.note }
type Pair = Object { first: CreateEntry second: CreateEntry }
action propagated_pair(input: Pair)
    consistency: atomic
    fails EntryConflict, DeliberateFailure -> Entry
{
    var first = attempt Entry.insert_entry(input.first)
    return attempt Entry.create_failing(input.second)
}
action recovered_pair(input: Pair)
    consistency: atomic
    fails EntryConflict -> Entry
{
    var first = attempt Entry.insert_entry(input.first)
    var recovered = match Entry.create_failing(input.second) {
        success(value) => value
        failure DeliberateFailure => first
        failure EntryConflict => propagate
    }
    return first
}
route POST /entries { auth: none input: CreateEntry output: Entry run: Entry.insert_entry(input) }
route POST /find { auth: none input: EntryLookup output: Entry run: Entry.find_entry(input) }
route POST /patch { auth: none input: PatchRequest output: Entry run: Entry.patch_entry(input) }
route POST /shadow { auth: none input: ShadowRequest output: Entry run: Entry.shadow_patch(input.id, input.original, input.replacement) }
route POST /batch { auth: none input: BatchUpdate output: Entry run: Entry.update_label(input) }
route POST /propagate { auth: none input: Pair output: Entry run: propagated_pair(input) }
route POST /recover { auth: none input: Pair output: Entry run: recovered_pair(input) }
`;
writeFileSync(join(root, "app.jadpo"), source);
const build = Bun.spawnSync([compiler, "build", root], { stdout: "pipe", stderr: "pipe" });
if (build.exitCode !== 0) throw new Error(`Persistence contract failed to build:\n${build.stdout}\n${build.stderr}`);
const app = await import(pathToFileURL(join(root, "build/target/app.ts")).href);
const sql = postgresUrl ? new SQL(postgresUrl, { prepare: false }) : null;
const sqlite = postgresUrl ? null : new Database(sqlitePath, { strict: true });
afterAll(async () => {
  sqlite?.close();
  await sql?.close();
  delete Bun.env.DATABASE_URL;
  delete Bun.env.SQLITE_PATH;
  rmSync(root, { recursive: true, force: true });
});
let sequence = 1;
function entry(label: string, note: string | null = "valid@example.com") {
  return { id: `018f57d0-bf42-4f25-9417-${String(sequence++).padStart(12, "0")}`, label, note };
}
async function request(path: string, body: unknown) {
  return app.handleRequest(new Request(`https://persistence.test${path}`, {
    method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body),
  }));
}
async function rows(query: string, values: unknown[] = []): Promise<any[]> {
  return sql ? Array.from(await sql.unsafe(query, values)) : sqlite!.query(query).all(...values);
}
async function stored(id: string) {
  return rows('SELECT id, label, note FROM "entry" WHERE id = $1', [id]);
}
async function insert(value: ReturnType<typeof entry>) {
  const response = await request("/entries", value);
  expect(response.status, await response.clone().text()).toBe(200);
  expect(await response.json()).toEqual(value);
  expect(await stored(value.id)).toEqual([value]);
}
async function expectOperationalFailure(response: Response) {
  expect(response.status).toBe(500);
  const body = await response.json();
  expect(body.error.code).toBe("internal_fault");
  expect(body.error).not.toHaveProperty("stack");
  expect(body.error).not.toHaveProperty("details");
  expect(JSON.stringify(body)).not.toContain(sqlitePath);
}

describe(`inherited nullable persistence and transaction contracts (${postgresUrl ? "postgres" : "sqlite"})`, () => {
  test("SQL and persistence metadata preserve inherited nullable contracts", async () => {
    const manifest = JSON.parse(readFileSync(join(root, "build/persistence/entities.json"), "utf8"));
    const fields = manifest.entities.find((entity: any) => entity.entity === "Entry").fields;
    expect(fields.find((field: any) => field.name === "note").nullable).toBe(true);
    expect(fields.find((field: any) => field.name === "label").nullable).toBe(false);
    for (const dialect of ["sqlite", "postgres"]) {
      const ddl = readFileSync(join(root, `build/sql/${dialect}/schema.sql`), "utf8");
      const noteColumn = ddl.split("\n").find((line) => line.trimStart().startsWith('"note" '));
      expect(noteColumn).toBeDefined();
      expect(noteColumn).not.toContain("NOT NULL");
    }
  });
  test("explicit none survives HTTP creation, actual storage and a later read", async () => {
    const value = entry("nullable-roundtrip", null);
    await insert(value);
    const response = await request("/find", { id: value.id });
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual(value);
  });
  test("a nullable field is still mandatory and required fields cannot become null", async () => {
    const value = entry("boundary-reject");
    for (const body of [{ id: value.id, label: value.label }, { ...value, label: null }]) {
      const response = await request("/entries", body);
      expect(response.status).toBe(400);
      expect(await stored(value.id)).toEqual([]);
    }
  });
  test("patch omission preserves a nullable value while supplied none clears it", async () => {
    const value = entry("patch-before");
    await insert(value);
    const omitted = await request("/patch", { id: value.id, changes: { label: "patch-after" } });
    expect(omitted.status).toBe(200);
    expect(await stored(value.id)).toEqual([{ ...value, label: "patch-after" }]);
    const cleared = await request("/patch", { id: value.id, changes: { note: null } });
    expect(cleared.status).toBe(200);
    expect(await cleared.json()).toEqual({ ...value, label: "patch-after", note: null });
    expect(await stored(value.id)).toEqual([{ ...value, label: "patch-after", note: null }]);
  });
  test("patch lowering uses the local binding at its lexical use, not a shadowed parameter", async () => {
    const value = entry("shadow-before");
    await insert(value);
    const response = await request("/shadow", {
      id: value.id, original: { note: "unused@example.com" },
      replacement: { label: "shadow-after", note: null },
    });
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ ...value, label: "shadow-after", note: null });
    expect(await stored(value.id)).toEqual([{ ...value, label: "shadow-after", note: null }]);
  });
  test("ambiguous required mutation rolls back without clearing either nullable field", async () => {
    const first = entry("ambiguous", "first@example.com");
    const second = entry("ambiguous", "second@example.com");
    await insert(first); await insert(second);
    await expectOperationalFailure(await request("/batch", { label: "ambiguous", note: null }));
    expect(await stored(first.id)).toEqual([first]);
    expect(await stored(second.id)).toEqual([second]);
  });
  test("propagated nested failure rolls back both entries including a nullable insert", async () => {
    const first = entry("propagated-first", null);
    const second = entry("propagated-second", null);
    const response = await request("/propagate", { first, second });
    expect(response.status).toBe(422);
    expect((await response.json()).error.code).toBe("deliberate_failure");
    expect(await stored(first.id)).toEqual([]);
    expect(await stored(second.id)).toEqual([]);
  });
  test("handled nested failure rolls back only its nullable insert and change record", async () => {
    const first = entry("recovered-first", null);
    const second = entry("recovered-second", null);
    const response = await request("/recover", { first, second });
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual(first);
    expect(await stored(first.id)).toEqual([first]);
    expect(await stored(second.id)).toEqual([]);
    const changes = await rows('SELECT entity_id FROM "__jadpo_changes" WHERE entity_id = $1 OR entity_id = $2 ORDER BY entity_id', [first.id, second.id]);
    expect(changes).toEqual([{ entity_id: first.id }]);
  });
  test("corrupt persisted semantic values are contained before entering trusted code", async () => {
    const value = entry("corrupt-read");
    await insert(value);
    await rows('UPDATE "entry" SET note = $1 WHERE id = $2 RETURNING id', ["invalid-stored-email", value.id]);
    await expectOperationalFailure(await request("/find", { id: value.id }));
    expect(await stored(value.id)).toEqual([{ ...value, note: "invalid-stored-email" }]);
  });
  test("parameterized text cannot change SQL shape", async () => {
    const value = entry(`a'); DELETE FROM entry; --`);
    await insert(value);
    const response = await request("/find", { id: value.id });
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual(value);
  });
});
