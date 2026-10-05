import { afterAll, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { SQL } from "bun";
import { Database } from "bun:sqlite";

// Only the verifier-owned disposable database may select PostgreSQL.
const postgresUrl = Bun.env.JADPO_ENTITY_DOSSIER_DATABASE_URL;
delete Bun.env.DATABASE_URL;
delete Bun.env.SQLITE_PATH;
const root = mkdtempSync(join(tmpdir(), "jadpo-entity-dossier-"));
const databasePath = join(root, "entity.sqlite");
if (postgresUrl) Bun.env.DATABASE_URL = postgresUrl;
else Bun.env.SQLITE_PATH = databasePath;
const postgres = postgresUrl ? new SQL({ url: postgresUrl, prepare: false }) : null;

const { persistence } = await import(
  "../../tests/compile/pass/build/target/persistence.ts"
);

afterAll(() => {
  delete Bun.env.SQLITE_PATH;
  delete Bun.env.DATABASE_URL;
  rmSync(root, { recursive: true, force: true });
  return postgres?.close();
});

describe("entity dossier transaction and change-record runtime", () => {
  test("rolls back a handled nested action to its savepoint with its change record", async () => {
    const id = "018f57d0-bf42-7f25-9417-8b6f385f29c7";
    const operationTime = "2026-09-27T21:00:00.000Z";

    await persistence
      .withOperationTime(operationTime)
      .transaction(async (outer: typeof persistence) => {
        await outer.create_Customer({ id, email: "first@example.com" });

        try {
          await outer.transaction(async (nested: typeof persistence) => {
            await nested.update_required_Customer_by_id_set_email(
              id,
              "rolled-back@example.com",
            );
            throw new Error("handled nested failure");
          });
        } catch (error) {
          expect((error as Error).message).toBe("handled nested failure");
        }

        const afterSavepoint = (await outer.query_required_Customer_by_id(id)) as {
          email: string;
        };
        expect(afterSavepoint.email).toBe("first@example.com");

        await outer.update_required_Customer_by_id_set_email(
          id,
          "committed@example.com",
        );
      });

    let customerEmail: string;
    let changes: Array<{
      revision: number;
      operation: string;
      payload: string;
      created_at: string;
    }>;
    if (postgres) {
      const customers = await postgres`SELECT email FROM "customer" WHERE id = ${id}`;
      customerEmail = String(customers[0]?.email);
      const rows = await postgres`
        SELECT revision, operation, payload,
          to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at
        FROM "__jadpo_changes"
        WHERE entity = ${"Customer"} AND entity_id = ${id}
        ORDER BY revision
      `;
      changes = rows.map((change) => ({
        ...change,
        // PostgreSQL drivers represent BIGINT as a decimal string; these
        // deliberately small test revisions are safe to compare as numbers.
        revision: Number(change.revision),
      }));
    } else {
      const database = new Database(databasePath, { readonly: true, strict: true });
      try {
        customerEmail = (database
          .query("SELECT email FROM customer WHERE id = ?1")
          .get(id) as { email: string }).email;
        changes = database
          .query(
            'SELECT revision, operation, payload, created_at FROM "__jadpo_changes" WHERE entity = ?1 AND entity_id = ?2 ORDER BY revision',
          )
          .all("Customer", id) as typeof changes;
      } finally {
        database.close();
      }
    }

    expect(customerEmail).toBe("committed@example.com");
    expect(changes.map(({ revision, operation }) => ({ revision, operation }))).toEqual([
      { revision: 1, operation: "create" },
      { revision: 2, operation: "update" },
    ]);
    expect(changes.some((change) => change.payload.includes("rolled-back"))).toBeFalse();
    expect(changes[1].payload).toContain("committed@example.com");
    expect(changes.map((change) => change.created_at)).toEqual([
      operationTime,
      operationTime,
    ]);
  });

  test("isolates concurrent transaction contexts and appends ordered revisions", async () => {
    const id = "018f57d0-bf42-7f25-9417-8b6f385f29c8";
    const start = Date.parse("2026-09-27T22:00:00.000Z");
    const initialTime = new Date(start).toISOString();
    const updates = Array.from({ length: 24 }, (_, index) => ({
      email: `concurrent-${index}@example.com`,
      operationTime: new Date(start + (index + 1) * 1000).toISOString(),
    }));

    await persistence.withOperationTime(initialTime).transaction(async (transaction: typeof persistence) => {
      await transaction.create_Customer({ id, email: "initial@example.com" });
    });
    await Promise.all(updates.map(({ email, operationTime }) =>
      persistence.withOperationTime(operationTime).transaction(async (transaction: typeof persistence) => {
        await transaction.update_required_Customer_by_id_set_email(id, email);
      })
    ));

    let customerEmail: string;
    let changes: Array<{
      revision: number;
      operation: string;
      payload: string;
      created_at: string;
    }>;
    if (postgres) {
      const customers = await postgres`SELECT email FROM "customer" WHERE id = ${id}`;
      customerEmail = String(customers[0]?.email);
      const rows = await postgres`
        SELECT revision, operation, payload,
          to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at
        FROM "__jadpo_changes"
        WHERE entity = ${"Customer"} AND entity_id = ${id}
        ORDER BY revision
      `;
      changes = rows.map((change) => ({
        ...change,
        revision: Number(change.revision),
        payload: String(change.payload),
      }));
    } else {
      const database = new Database(databasePath, { readonly: true, strict: true });
      try {
        customerEmail = (database
          .query("SELECT email FROM customer WHERE id = ?1")
          .get(id) as { email: string }).email;
        changes = database
          .query(
            'SELECT revision, operation, payload, created_at FROM "__jadpo_changes" WHERE entity = ?1 AND entity_id = ?2 ORDER BY revision',
          )
          .all("Customer", id) as typeof changes;
      } finally {
        database.close();
      }
    }

    expect(changes.map(({ revision }) => revision)).toEqual(
      Array.from({ length: updates.length + 1 }, (_, index) => index + 1),
    );
    expect(changes.map(({ operation }) => operation)).toEqual([
      "create",
      ...updates.map(() => "update"),
    ]);
    const persistedUpdates = changes.slice(1).map((change) => {
      const value = JSON.parse(change.payload) as { email: string };
      return { email: value.email, created_at: change.created_at };
    });
    expect(persistedUpdates).toHaveLength(updates.length);
    for (const update of updates) {
      expect(persistedUpdates).toContainEqual({ email: update.email, created_at: update.operationTime });
    }
    expect(customerEmail).toBe(persistedUpdates.at(-1)?.email);
  });

  if (postgres) {
    test("serializes concurrent writers from separate PostgreSQL processes", async () => {
      const id = "018f57d0-bf42-7f25-9417-8b6f385f29c9";
      const start = Date.parse("2026-09-27T23:00:00.000Z");
      const initialTime = new Date(start).toISOString();
      const writerPath = new URL("./fixtures/entity-dossier-writer.ts", import.meta.url).pathname;
      await persistence.withOperationTime(initialTime).transaction(async (transaction: typeof persistence) => {
        await transaction.create_Customer({ id, email: "process-initial@example.com" });
      });

      const children = [0, 1].map((writer) => Bun.spawn(
        [process.execPath, "--no-install", "--no-env-file", writerPath, id, String(writer)],
        {
          env: {
            ...Bun.env,
            JADPO_ENTITY_DOSSIER_WRITER_START: String(start),
          },
          stdout: "pipe",
          stderr: "pipe",
        },
      ));
      const childResults = await Promise.all(children.map(async (child) => {
        const [exitCode, stdout, stderr] = await Promise.all([
          child.exited,
          new Response(child.stdout).text(),
          new Response(child.stderr).text(),
        ]);
        return { exitCode, stdout, stderr };
      }));
      for (const result of childResults) {
        if (result.exitCode !== 0) {
          throw new Error(`Entity-dossier writer exited ${result.exitCode}: ${result.stderr}${result.stdout}`);
        }
        expect(result.stderr).toBe("");
      }

      const customers = await postgres`SELECT email FROM "customer" WHERE id = ${id}`;
      const rows = await postgres`
        SELECT revision, operation, payload,
          to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at
        FROM "__jadpo_changes"
        WHERE entity = ${"Customer"} AND entity_id = ${id}
        ORDER BY revision
      `;
      const changes = rows.map((change) => ({
        ...change,
        revision: Number(change.revision),
        payload: String(change.payload),
      }));
      const updates = Array.from({ length: 24 }, (_, index) => {
        const writer = Math.floor(index / 12);
        const sequence = index % 12;
        return {
          email: `process-${writer}-${sequence}@example.com`,
          operationTime: new Date(start + (index + 1) * 1000).toISOString(),
        };
      });

      expect(changes.map(({ revision }) => revision)).toEqual(
        Array.from({ length: updates.length + 1 }, (_, index) => index + 1),
      );
      expect(changes.map(({ operation }) => operation)).toEqual([
        "create",
        ...updates.map(() => "update"),
      ]);
      const persistedUpdates = changes.slice(1).map((change) => {
        const value = JSON.parse(change.payload) as { email: string };
        return { email: value.email, created_at: change.created_at };
      });
      expect(persistedUpdates).toHaveLength(updates.length);
      for (const update of updates) {
        expect(persistedUpdates).toContainEqual({ email: update.email, created_at: update.operationTime });
      }
      expect(String(customers[0]?.email)).toBe(persistedUpdates.at(-1)?.email);
    });
  }

});
