import assert from 'node:assert/strict';
import test from 'node:test';
import { canWrite,choices,entities,type Row } from '../shared/schema.ts';

const actor=(role:string):Row=>({id:7n,role,active:true});
const state=entities.Expense.fields.find(field=>field.name==='state')!;
const leave=entities.Leave.fields.find(field=>field.name==='state')!;
test('Agent keeps own draft submission without seeing expense approval actions',()=>{
  const me=actor('Agent');
  assert.deepEqual(choices('Expense',state,'Draft',me,{member_id:7n,state:'Draft'}),['Draft','Submitted']);
  assert.deepEqual(choices('Expense',state,'Submitted',me,{member_id:7n,state:'Submitted'}),['Submitted']);
  assert.deepEqual(choices('Expense',state,'Submitted',me,{member_id:8n,state:'Submitted'}),[]);
  assert.equal(canWrite(me,'Expense',{member_id:7n,state:'Approved'}),false);
});
test('Agent leave approval is hidden while Admin and Manager retain all valid edges',()=>{
  const row={member_id:7n,state:'Requested'};
  assert.deepEqual(choices('Leave',leave,'Requested',actor('Agent'),row),['Requested']);
  for(const role of ['Admin','Manager']) {
    assert.deepEqual(choices('Leave',leave,'Requested',actor(role),row),['Requested','Approved','Rejected']);
    assert.deepEqual(choices('Expense',state,'Submitted',actor(role),{member_id:7n,state:'Submitted'}),['Submitted','Approved','Rejected']);
  }
});
test('Viewer cannot write any entity and ordinary enumeration choices are unaffected',()=>{
  for(const name of Object.keys(entities) as (keyof typeof entities)[])assert.equal(canWrite(actor('Viewer'),name,{member_id:7n,state:'Draft'}),false);
  const field=entities.Member.fields.find(field=>field.name==='role')!;
  assert.deepEqual(choices('Member',field,'Admin',actor('Admin'),{role:'Admin'}),['Admin','Manager','Agent','Viewer']);
});
