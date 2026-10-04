// cashLabel(-5) -> "-$0.05"
function cashLabel(cents, symbol = "$") {
  const abs = Math.abs(cents);
  return `${cents < 0 ? "-" : ""}${symbol}${Math.floor(abs / 100)}.${String(abs % 100).padStart(2, "0")}`;
}
module.exports = { cashLabel };
