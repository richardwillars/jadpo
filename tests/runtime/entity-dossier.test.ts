import { afterAll, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Database } from "bun:sqlite";

const root = mkdtempSync(join(tmpdir(), "jadpo-entity-dossier-"));
const databasePath = join(root, "entity.sqlite");
Bun.env.SQLITE_PATH = databasePath;

const { persistence } = await import(
  "../../tests/compile/pass/build/target/persistence.ts"
);

afterAll(() => {
  delete Bun.env.SQLITE_PATH;
  rmSync(root, { recursive: true, force: true });
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

    const database = new Database(databasePath, { readonly: true, strict: true });
    try {
      const customer = database
        .query("SELECT email FROM customer WHERE id = ?1")
        .get(id) as { email: string };
      expect(customer.email).toBe("committed@example.com");

      const changes = database
        .query(
          'SELECT revision, operation, payload, created_at FROM "__jadpo_changes" WHERE entity = ?1 AND entity_id = ?2 ORDER BY revision',
        )
        .all("Customer", id) as Array<{
        revision: number;
        operation: string;
        payload: string;
        created_at: string;
      }>;

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
    } finally {
      database.close();
    }
  });
});
