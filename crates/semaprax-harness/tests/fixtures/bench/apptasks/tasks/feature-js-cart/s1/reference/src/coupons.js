const COUPONS = {
  SAVE10: (subtotal) => Math.floor(subtotal / 10),
  FIVE: (subtotal) => Math.min(500, subtotal),
};
function discount(code, subtotal) {
  const rule = COUPONS[code];
  if (!rule) throw new Error("unknown coupon");
  return rule(subtotal);
}
module.exports = { discount };
