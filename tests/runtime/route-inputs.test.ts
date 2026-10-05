import { afterAll, beforeAll, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { connect } from "node:net";

const root = mkdtempSync(join(tmpdir(), "jadpo-route-inputs-"));
const compiler = Bun.env.JADPO_BIN ?? new URL("../../jadpo/target/debug/jadpo", import.meta.url).pathname;

writeFileSync(join(root, "app.jadpo"), `
type PageSize = Int { min: 1 max: 100 }
type TodoStatus = Enum { open completed }
type TodoCursor = Object {
    created_at: Instant
    id: Uuid
}
type ListTodos = Object {
    status: TodoStatus optional
    due_before: Instant optional
    page_size: PageSize default 25
    offset: Int optional
    after: TodoCursor optional
}
route GET /todos {
    auth: none
    query: ListTodos
    output: ListTodos
    action: { return query }
}
route GET /header {
    auth: none
    headers: { trace_id: Text from "X-Trace" }
    output: Text
    action: { return headers.trace_id }
}
`);

const build = Bun.spawnSync([compiler, "build", root], { stdout: "pipe", stderr: "pipe" });
if (build.exitCode !== 0) throw new Error(`Route input fixture failed to build:\n${build.stdout}\n${build.stderr}`);
const app = await import(pathToFileURL(join(root, "build/target/app.ts")).href);
let server: ReturnType<typeof app.createApplicationServer>;
let baseUrl: URL;
beforeAll(async () => {
  server = app.createApplicationServer();
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (address === null || typeof address === "string") throw new Error("route input server has no port");
  baseUrl = new URL(`http://127.0.0.1:${address.port}`);
});
afterAll(async () => {
  if (server !== undefined) await new Promise<void>((resolve) => server.close(() => resolve()));
  rmSync(root, { recursive: true, force: true });
});

async function get(rawQuery = "") {
  return fetch(new URL(`/todos${rawQuery.length === 0 ? "" : `?${rawQuery}`}`, baseUrl));
}

async function rawHeaderRequest(lines: string[]) {
  const port = Number(baseUrl.port);
  const response = await new Promise<string>((resolve, reject) => {
    let received = "";
    const socket = connect(port, "127.0.0.1", () => {
      socket.write(["GET /header HTTP/1.1", `Host: 127.0.0.1:${port}`, ...lines, "Connection: close", "", ""].join("\r\n"));
    });
    socket.on("data", (chunk) => { received += chunk.toString(); });
    socket.on("end", () => resolve(received));
    socket.on("error", reject);
  });
  const [head, body] = response.split("\r\n\r\n", 2);
  return { status: Number(head.split(" ")[1]), body: JSON.parse(body) };
}

test("query omission materializes defaults and preserves optional absence", async () => {
  const response = await get();
  expect(response.status).toBe(200);
  expect(await response.json()).toEqual({ page_size: 25 });
});

test("query values decode once and validate through the declared object", async () => {
  const cursor = { created_at: "2026-10-01T00:00:00Z", id: "00000000-0000-4000-8000-000000000001" };
  const response = await get(`status=open&due_before=2026-10-02T00%3A00%3A00Z&page_size=10&offset=9007199254740991&after=${encodeURIComponent(JSON.stringify(cursor))}`);
  expect(response.status).toBe(200);
  expect(await response.json()).toEqual({
    status: "open",
    due_before: "2026-10-02T00:00:00.000Z",
    page_size: 10,
    offset: 9007199254740991,
    after: { ...cursor, created_at: "2026-10-01T00:00:00.000Z" },
  });
});

test("malformed transport syntax is 400", async () => {
  for (const rawQuery of [
    "page_size=",
    "page_size=10&page_size=20",
    "limit=10",
    "after=%7Bbad%7D",
    "after=%257B%2522created_at%2522%253A%2522x%2522%257D",
    "after=%ZZ",
    "after=%E0%A4%A",
  ]) {
    const response = await get(rawQuery);
    expect(response.status, rawQuery).toBe(400);
    expect((await response.json()).error.code).toBe("invalid_request");
  }
});

test("well-formed values that fail declared types or constraints are 422", async () => {
  for (const rawQuery of [
    "page_size=0",
    "offset=9007199254740993",
    `after=${encodeURIComponent(JSON.stringify({ created_at: "2026-10-01T00:00:00Z" }))}`,
    `after=${encodeURIComponent(JSON.stringify({ created_at: "2026-10-01T00:00:00Z", id: "bad" }))}`,
    `after=${encodeURIComponent("null")}`,
  ]) {
    const response = await get(rawQuery);
    expect(response.status, rawQuery).toBe(422);
    expect((await response.json()).error.code).toBe("invalid_value");
  }
});

test("OpenAPI derives query requirements, schemas, and the source default", () => {
  const document = JSON.parse(readFileSync(join(root, "build/openapi/openapi.json"), "utf8"));
  const parameters = document.paths["/todos"].get.parameters;
  expect(parameters.map((parameter: { name: string }) => parameter.name)).toEqual(["status", "due_before", "page_size", "offset", "after"]);
  expect(parameters.find((parameter: { name: string }) => parameter.name === "page_size")).toMatchObject({
    in: "query", required: false, schema: { default: 25 },
  });
  expect(parameters.find((parameter: { name: string }) => parameter.name === "after")).toMatchObject({
    in: "query", required: false, content: { "application/json": { schema: { $ref: "#/components/schemas/TodoCursor" } } },
  });
  expect(document.paths["/header"].get.parameters).toEqual([
    { name: "X-Trace", in: "header", required: true, schema: { type: "string" } },
  ]);
});

test("raw-header adapter accepts one declared value and rejects duplicates", async () => {
  const accepted = await rawHeaderRequest(["X-Trace: one", "User-Agent: route-input-test"]);
  expect(accepted).toEqual({ status: 200, body: "one" });

  const duplicate = await rawHeaderRequest(["X-Trace: one", "x-trace: two"]);
  expect(duplicate.status).toBe(400);
  expect(duplicate.body.error.code).toBe("invalid_request");

  const missing = await rawHeaderRequest(["User-Agent: route-input-test"]);
  expect(missing.status).toBe(400);
  expect(missing.body.error.code).toBe("invalid_request");
});
