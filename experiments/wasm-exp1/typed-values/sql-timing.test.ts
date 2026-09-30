import {test,expect} from 'bun:test';
import {Database} from 'bun:sqlite';
import {timedSqlite} from './sql-timing.ts';
test('SQL diagnostics preserve commit and rollback and account for every statement',()=>{
 const db=new Database(':memory:');const t=timedSqlite(db),a=t.adapter;
 try{
  a.rows('CREATE TABLE data (value TEXT)',[]);
  a.transactionSync(()=>a.rows('INSERT INTO data VALUES (?)',['first']));
  expect(()=>a.transactionSync(()=>{a.rows('UPDATE data SET value = ?',['second']);throw Error('rollback');})).toThrow('rollback');
  expect(a.rows('SELECT * FROM data',[])).toEqual([{value:'first'}]);
  const samples=t.snapshot();expect(samples.length).toBe(2);
  expect(samples.map(s=>s.rollback)).toEqual([false,true]);
  expect(samples.map(s=>s.calls.map(c=>c.verb))).toEqual([['INSERT'],['UPDATE']]);
  for(const s of samples){expect(s.beginMs+s.bodyMs+s.finishMs).toBeCloseTo(s.totalMs,8);expect(s.sqlMs).toBeLessThanOrEqual(s.bodyMs);}
  expect(t.summary().count).toBe(2);t.reset();expect(t.summary().count).toBe(0);
 }finally{db.close();}
});
