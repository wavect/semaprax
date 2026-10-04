const test = require("node:test");
const assert = require("node:assert");
const { renderInvoice } = require("./render");
test("render basic", () => {
  assert.strictEqual(renderInvoice({ number: "A-1", totalCents: 1250 }),
    '<article class="invoice"><h2>A-1</h2><span class="total">12.50</span></article>');
});
