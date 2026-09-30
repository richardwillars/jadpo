import type { SqlAdapter } from "./adapter.ts";
// Cloudflare globals come from `wrangler types` at the consuming Worker boundary.
export function cloudflareSqlite(storage:Pick<DurableObjectStorage,"sql"|"transactionSync">):SqlAdapter {
  return {
    rows:(sql,parameters)=>storage.sql.exec(sql,...parameters).toArray(),
    transactionSync:work=>storage.transactionSync(work),
    // workerd reports SQLite failures as Error messages; constrain classification to
    // the SQLite unique violation signature. Raw detail never becomes a wire value.
    isUniqueConflict:error=>error instanceof Error&&/^UNIQUE constraint failed: [A-Za-z_][A-Za-z0-9_. ,]*(?:: SQLITE_CONSTRAINT(?: \(extended: SQLITE_CONSTRAINT_(?:UNIQUE|PRIMARYKEY)\))?)?$/.test(error.message),
  };
}
