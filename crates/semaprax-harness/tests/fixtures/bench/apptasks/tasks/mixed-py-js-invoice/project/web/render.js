function renderInvoice(obj) {
  const total = (obj.totalCents / 100).toFixed(2);
  return `<article class="invoice"><h2>${obj.number}</h2><span class="total">${total}</span></article>`;
}
module.exports = { renderInvoice };
