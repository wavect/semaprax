function pad(n) {
  return n < 10 ? "0" + n : String(n);
}
// month is 1-based
function iso(year, month, day) {
  return `${year}-${pad(month - 1)}-${pad(day)}`;
}
module.exports = { iso };
