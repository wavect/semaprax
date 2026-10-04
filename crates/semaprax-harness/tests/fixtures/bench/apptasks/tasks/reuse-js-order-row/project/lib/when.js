const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
// readDay("2024-03-05") -> {y, m, d}
function readDay(text) {
  const [y, m, d] = text.split("-").map(Number);
  return { y, m, d };
}
// dayLabel({y,m,d}) -> "05 Mar 2024"
function dayLabel({ y, m, d }) {
  return `${String(d).padStart(2, "0")} ${MONTHS[m - 1]} ${y}`;
}
module.exports = { readDay, dayLabel };
