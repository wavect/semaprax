class Cart {
  constructor() {
    this.items = [];
  }
  add(sku, cents, qty = 1) {
    this.items.push({ sku, cents, qty });
  }
  subtotal() {
    return this.items.reduce((n, i) => n + i.cents * i.qty, 0);
  }
  total(rate = 0) {
    return Math.round(this.subtotal() * (1 + rate));
  }
}
module.exports = { Cart };
