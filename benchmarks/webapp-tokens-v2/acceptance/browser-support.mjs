import assert from 'node:assert/strict';

export function signInLabel(field) {
  assert.ok(['email', 'password'].includes(field), 'known sign-in field');
  return new RegExp(`^\\s*${field}\\s*$`, 'i');
}

// Enum filters identify their field through an ordinary accessible control
// name or the legacy field-qualified clear-option caption.
export function enumFilterSelectors(field) {
  assert.ok(typeof field==='string'&&/^[A-Za-z][A-Za-z0-9_]*$/.test(field),'known enum field');
  const escaped=field.replace(/[.*+?^${}()|[\]\\]/g,'\\$&').replaceAll('_','[_ ]');
  return {name:new RegExp(`^\\s*${escaped}\\s*$`,'i'),
          legacyAll:new RegExp(`^\\s*${escaped}:\\s*all\\s*$`,'i')};
}

// Typed integer editors may use inputmode rather than the browser Number
// implementation. The physical create/readback checks remain the value oracle.
export function numericEditor(type, attributes) {
  const {type: inputType, inputmode, step} = attributes;
  const number = inputType === 'number';
  assert.ok(number || (type === 'int' && inputType === 'text' && inputmode === 'numeric'), 'a numeric editor is required');
  if(number && type === 'int')assert.ok(step === null || step === '1', 'integer stepping');
  if(number && type === 'float')assert.ok(step === null || step === 'any' || Number(step) < 1, 'fractional values supported');
}

// Hash-only page.goto can resolve against an earlier document's networkidle
// state before the SPA begins its asynchronous route. A new document exercises
// the public direct route and cannot leave a previous entity's DOM in place.
export async function directPage(page, url) {
  await page.goto('about:blank');
  await page.goto(url, {waitUntil: 'networkidle'});
}

// For actual SPA link clicks, observe the destination heading before checking
// fields. This does not depend on the candidate's framework or internal state.
export async function followEntity(page, link, entity, expect) {
  await link.click();
  const pattern=new RegExp('^'+entity+'(?:\\s|$)', 'i');
  await expect(page.getByRole('heading',{name:pattern}).first()).toBeVisible();
  await page.waitForLoadState('networkidle');
}
