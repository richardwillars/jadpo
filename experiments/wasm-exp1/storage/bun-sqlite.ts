import type { Database } from "bun:sqlite";
import type { SqlAdapter } from "./adapter.ts";
export function bunSqlite(database:Database):SqlAdapter {
  return {
    rows:(sql,parameters)=>database.prepare(sql).all(...parameters) as any,
    transactionSync:work=>database.transaction(work).immediate(),
    isUniqueConflict(error){const code=(error as {code?:string})?.code;return code==="SQLITE_CONSTRAINT_UNIQUE"||code==="SQLITE_CONSTRAINT_PRIMARYKEY";},
  };
}
