function renderInvoice(obj) {
  const total = (obj.totalCents / 100).toFixed(2);
  const due = obj.dueDate ? `<time class="due">${obj.dueDate}</time>` : "";
  return `<article class="invoice"><h2>${obj.number}</h2><span class="total">${total}</span>${due}</article>`;
}
module.exports = { renderInvoice };
