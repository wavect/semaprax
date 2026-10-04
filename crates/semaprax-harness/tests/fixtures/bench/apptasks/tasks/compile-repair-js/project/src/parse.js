function parseRow(line) {
  return line.split(",").map((c) => c.trim();
}
function parseTable(text) {
  return text.trim().split("\n").map(parseRow);
}
module.exports = { parseRow, parseTable };
