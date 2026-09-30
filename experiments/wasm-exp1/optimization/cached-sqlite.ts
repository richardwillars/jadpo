import type {Database} from 'bun:sqlite';
import type {SqlAdapter} from '../storage/adapter.ts';
export function bunSqlite(database:Database):SqlAdapter {
  const statements=new Map<string,ReturnType<Database['prepare']>>();
  return {
    rows(sql,parameters){let statement=statements.get(sql);if(!statement){statement=database.prepare(sql);if(statements.size<128)statements.set(sql,statement);}return statement.all(...parameters) as any;},
    transactionSync:work=>database.transaction(work).immediate(),
    isUniqueConflict(error){const code=(error as {code?:string})?.code;return code==='SQLITE_CONSTRAINT_UNIQUE'||code==='SQLITE_CONSTRAINT_PRIMARYKEY';},
  };
}
