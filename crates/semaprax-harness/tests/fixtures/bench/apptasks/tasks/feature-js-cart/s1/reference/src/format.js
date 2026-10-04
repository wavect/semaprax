function money(cents) {
  return "$" + Math.floor(cents / 100) + "." + String(cents % 100).padStart(2, "0");
}
function receipt(cart) {
  const lines = cart.items.map((i) => `${i.sku} x${i.qty} ${money(i.cents * i.qty)}`);
  if (cart.coupon) lines.push(`Coupon ${cart.coupon} -${money(cart.discountCents())}`);
  lines.push(`Total ${money(cart.total())}`);
  return lines.join("\n");
}
module.exports = { money, receipt };
