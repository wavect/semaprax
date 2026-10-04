const { sum } = require("./utils");

function columnSum(table, idx) {
  return sum(table.map((r) => Number(r[idx])));
}
module.exports = { columnSum };
