const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const { renderInvoice } = require("../web/render");
const [withDue, without] = JSON.parse(fs.readFileSync("_cross.json", "utf8"));
test("python output renders the due date", () => {
  assert.strictEqual(renderInvoice(withDue),
    '<article class="invoice"><h2>Z-9</h2><span class="total">42.00</span><time class="due">2027-01-15</time></article>');
});
test("no due date renders as before", () => {
  assert.strictEqual(renderInvoice(without),
    '<article class="invoice"><h2>Z-10</h2><span class="total">1.00</span></article>');
});
