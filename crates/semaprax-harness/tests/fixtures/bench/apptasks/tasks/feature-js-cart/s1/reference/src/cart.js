const { discount } = require("./coupons");

class Cart {
  constructor() {
    this.items = [];
    this.coupon = null;
  }
  add(sku, cents, qty = 1) {
    this.items.push({ sku, cents, qty });
  }
  applyCoupon(code) {
    discount(code, 0);
    this.coupon = code;
  }
  subtotal() {
    return this.items.reduce((n, i) => n + i.cents * i.qty, 0);
  }
  discountCents() {
    return this.coupon ? discount(this.coupon, this.subtotal()) : 0;
  }
  total(rate = 0) {
    return Math.round((this.subtotal() - this.discountCents()) * (1 + rate));
  }
}
module.exports = { Cart };
