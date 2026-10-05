import { Database } from "bun:sqlite";
import { SQL } from "bun";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createConnection, createServer } from "node:net";
import type { AddressInfo, Socket } from "node:net";

// Diagnostic probe only. This does not enable retries in generated applications.
const root = mkdtempSync(join(tmpdir(), "jadpo-rm401-phase-"));
const sqlitePath = join(root, "phase.sqlite");
const reader = new Database(sqlitePath, { strict: true });
const writer = new Database(sqlitePath, { strict: true });
const result: Record<string, unknown> = {
  probe: "RM-401 adapter phase evidence v1",
  bun_version: Bun.version,
};

function errorFacts(error: unknown): Record<string, unknown> {
  const record = error as Record<string, unknown>;
  return {
    name: typeof record?.name === "string" ? record.name : null,
    code: typeof record?.code === "string" ? record.code : null,
    severity: typeof record?.severity === "string" ? record.severity : null,
    errno: typeof record?.errno === "string" ? record.errno : null,
    sqlstate: typeof record?.sqlstate === "string" ? record.sqlstate : null,
    sqlState: typeof record?.sqlState === "string" ? record.sqlState : null,
    constraint: typeof record?.constraint === "string" ? record.constraint : null,
    field_names: error && typeof error === "object" ? Object.getOwnPropertyNames(error).sort() : [],
  };
}

try {
  reader.exec("PRAGMA journal_mode = DELETE");
  reader.exec("PRAGMA busy_timeout = 0");
  writer.exec("PRAGMA busy_timeout = 0");
  reader.exec("CREATE TABLE item (id INTEGER PRIMARY KEY, value INTEGER NOT NULL)");
  reader.exec("INSERT INTO item VALUES (1, 0)");

  reader.exec("BEGIN IMMEDIATE");
  let beginError: unknown = null;
  try {
    writer.exec("BEGIN IMMEDIATE");
  } catch (error) {
    beginError = errorFacts(error);
  }
  reader.exec("ROLLBACK");
  result.sqlite_begin_busy = {
    error: beginError,
    rollback_after_failed_begin: (() => {
      try { writer.exec("ROLLBACK"); return "succeeded"; }
      catch { return "no_active_transaction"; }
    })(),
  };
  if ((beginError as { code?: string } | null)?.code !== "SQLITE_BUSY") {
    throw new Error("expected SQLite begin to report SQLITE_BUSY");
  }

  reader.exec("BEGIN");
  reader.query("SELECT value FROM item WHERE id = 1").all();
  writer.exec("BEGIN IMMEDIATE");
  writer.exec("UPDATE item SET value = 1 WHERE id = 1");
  let commitError: unknown = null;
  try {
    writer.exec("COMMIT");
  } catch (error) {
    commitError = errorFacts(error);
  }
  let rollbackAfterCommit: string;
  try { writer.exec("ROLLBACK"); rollbackAfterCommit = "succeeded"; }
  catch { rollbackAfterCommit = "failed"; }
  reader.exec("ROLLBACK");
  result.sqlite_commit_busy = {
    error: commitError,
    rollback_after_commit: rollbackAfterCommit,
    stored_value: (reader.query("SELECT value FROM item WHERE id = 1").get() as { value: number }).value,
  };
  if ((commitError as { code?: string } | null)?.code !== "SQLITE_BUSY" ||
      rollbackAfterCommit !== "succeeded" ||
      (result.sqlite_commit_busy as { stored_value: number }).stored_value !== 0) {
    throw new Error("expected SQLite commit busy with proved rollback");
  }

  const postgresUrl = Bun.env.RM401_PROBE_PG_URL;
  if (postgresUrl) {
    const sql = new SQL({ url: postgresUrl, prepare: false });
    try {
      const version = await sql.unsafe("SHOW server_version");
      result.postgres_version = String(version[0]?.server_version);
      await sql.unsafe("CREATE TABLE rm401_parent (id INTEGER PRIMARY KEY)");
      await sql.unsafe("CREATE TABLE rm401_child (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES rm401_parent(id) DEFERRABLE INITIALLY DEFERRED)");
      let callbackReturned = false;
      let commitError: unknown = null;
      try {
        await sql.begin(async (tx) => {
          await tx.unsafe("INSERT INTO rm401_child VALUES (1, 999)");
          callbackReturned = true;
        });
      } catch (error) {
        commitError = errorFacts(error);
      }
      const rows = await sql.unsafe("SELECT COUNT(*)::INTEGER AS count FROM rm401_child");
      result.postgres_commit_server_abort = {
        callback_returned: callbackReturned,
        error: commitError,
        stored_rows: Number(rows[0]?.count),
      };
      if (!callbackReturned ||
          (commitError as { errno?: string } | null)?.errno !== "23503" ||
          Number(rows[0]?.count) !== 0) {
        throw new Error("expected PostgreSQL commit-time constraint abort");
      }

      await sql.unsafe("CREATE TABLE rm401_pair (id INTEGER PRIMARY KEY, value INTEGER NOT NULL)");
      await sql.unsafe("INSERT INTO rm401_pair VALUES (1, 0), (2, 0)");
      const peer = new SQL({ url: postgresUrl, prepare: false });
      try {
        let ready = 0;
        let release!: () => void;
        const gate = new Promise<void>((resolve) => { release = resolve; });
        const callbacks: Record<string, boolean> = { first: false, second: false };
        async function serializableWrite(client: SQL, label: "first" | "second", id: number): Promise<void> {
          await client.begin(async (tx) => {
            await tx.unsafe("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE");
            await tx.unsafe("SELECT SUM(value) FROM rm401_pair");
            ready += 1;
            if (ready === 2) release();
            await gate;
            await tx.unsafe(`UPDATE rm401_pair SET value = 1 WHERE id = ${id}`);
            callbacks[label] = true;
          });
        }
        const outcomes = await Promise.allSettled([
          serializableWrite(sql, "first", 1),
          serializableWrite(peer, "second", 2),
        ]);
        const stored = await sql.unsafe("SELECT id, value FROM rm401_pair ORDER BY id");
        result.postgres_serializable_commit = {
          callbacks,
          outcomes: outcomes.map((outcome) => outcome.status === "fulfilled"
            ? { status: "committed" }
            : { status: "rejected", error: errorFacts(outcome.reason) }),
          stored: stored.map((row) => ({ id: Number(row.id), value: Number(row.value) })),
        };
        const rejected = outcomes.filter((outcome) => outcome.status === "rejected");
        if (!callbacks.first || !callbacks.second || rejected.length !== 1 ||
            (rejected[0] as PromiseRejectedResult).reason?.errno !== "40001" ||
            stored.reduce((sum, row) => sum + Number(row.value), 0) !== 1) {
          throw new Error("expected one PostgreSQL commit-time serialization abort");
        }

        await sql.unsafe("CREATE TABLE rm401_deadlock (id INTEGER PRIMARY KEY, value INTEGER NOT NULL)");
        await sql.unsafe("INSERT INTO rm401_deadlock VALUES (1, 0), (2, 0)");
        let deadlockReady = 0;
        let releaseDeadlock!: () => void;
        const deadlockGate = new Promise<void>((resolve) => { releaseDeadlock = resolve; });
        async function crossWrite(client: SQL, first: number, second: number): Promise<void> {
          await client.begin(async (tx) => {
            await tx.unsafe("SET LOCAL deadlock_timeout = '100ms'");
            await tx.unsafe(`UPDATE rm401_deadlock SET value = value + 1 WHERE id = ${first}`);
            deadlockReady += 1;
            if (deadlockReady === 2) releaseDeadlock();
            await deadlockGate;
            await tx.unsafe(`UPDATE rm401_deadlock SET value = value + 1 WHERE id = ${second}`);
          });
        }
        const deadlockOutcomes = await Promise.allSettled([
          crossWrite(sql, 1, 2),
          crossWrite(peer, 2, 1),
        ]);
        const deadlockRows = await sql.unsafe("SELECT id, value FROM rm401_deadlock ORDER BY id");
        result.postgres_deadlock = {
          outcomes: deadlockOutcomes.map((outcome) => outcome.status === "fulfilled"
            ? { status: "committed" }
            : { status: "rejected", error: errorFacts(outcome.reason) }),
          stored: deadlockRows.map((row) => ({ id: Number(row.id), value: Number(row.value) })),
        };
        const deadlockVictims = deadlockOutcomes.filter((outcome) => outcome.status === "rejected");
        if (deadlockVictims.length !== 1 ||
            (deadlockVictims[0] as PromiseRejectedResult).reason?.errno !== "40P01" ||
            deadlockRows.some((row) => Number(row.value) !== 1)) {
          throw new Error("expected one PostgreSQL deadlock victim with confirmed abort");
        }
      } finally {
        await peer.close();
      }

      await sql.unsafe("CREATE TABLE rm401_ack (id INTEGER PRIMARY KEY)");
      let commitForwarded = false;
      let responseDropped = false;
      const sockets: Socket[] = [];
      const direct = new URL(postgresUrl);
      const proxy = createServer((downstream) => {
        const upstream = createConnection({
          host: direct.hostname,
          port: Number(direct.port || 5432),
        });
        sockets.push(downstream, upstream);
        downstream.on("data", (chunk: Buffer) => {
          if (chunk.toString("utf8").toUpperCase().includes("COMMIT")) commitForwarded = true;
          upstream.write(chunk);
        });
        upstream.on("data", (chunk: Buffer) => {
          if (commitForwarded) {
            responseDropped = true;
            downstream.destroy();
            upstream.destroy();
          } else {
            downstream.write(chunk);
          }
        });
        downstream.on("error", () => {});
        upstream.on("error", () => {});
        downstream.on("close", () => upstream.destroy());
        upstream.on("close", () => downstream.destroy());
      });
      await new Promise<void>((resolve) => proxy.listen(0, "127.0.0.1", resolve));
      const proxyUrl = new URL(postgresUrl);
      proxyUrl.hostname = "127.0.0.1";
      proxyUrl.port = String((proxy.address() as AddressInfo).port);
      const throughProxy = new SQL({ url: proxyUrl.toString(), prepare: false, max: 1 });
      try {
        let callbackReturned = false;
        let clientError: unknown = null;
        try {
          await throughProxy.begin(async (tx) => {
            await tx.unsafe("INSERT INTO rm401_ack VALUES (1)");
            callbackReturned = true;
          });
        } catch (error) {
          clientError = errorFacts(error);
        }
        const stored = await sql.unsafe("SELECT COUNT(*)::INTEGER AS count FROM rm401_ack");
        result.postgres_lost_commit_ack = {
          callback_returned: callbackReturned,
          commit_forwarded: commitForwarded,
          response_dropped: responseDropped,
          client_error: clientError,
          stored_rows: Number(stored[0]?.count),
        };
        if (!callbackReturned || !commitForwarded || !responseDropped ||
            clientError === null || Number(stored[0]?.count) !== 1) {
          throw new Error("expected committed PostgreSQL write with lost acknowledgement");
        }
      } finally {
        for (const socket of sockets) socket.destroy();
        await throughProxy.close();
        await new Promise<void>((resolve) => proxy.close(() => resolve()));
      }
    } finally {
      await sql.close();
    }
  }

  console.log(JSON.stringify(result, null, 2));
} finally {
  reader.close();
  writer.close();
  rmSync(root, { recursive: true, force: true });
}
