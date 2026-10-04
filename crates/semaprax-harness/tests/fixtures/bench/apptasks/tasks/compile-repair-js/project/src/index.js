const { parseTable } = require("./parse");
const { columnSum } = require("./stats");

function totalOf(text, idx) {
  const rows = parseTable(text);
  return columnSum(rows.slice(1), idx);
}
module.exports = { totalOf, parseRow: require("./parse").parseRow };
