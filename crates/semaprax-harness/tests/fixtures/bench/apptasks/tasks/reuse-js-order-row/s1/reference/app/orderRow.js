const { kebab } = require("../lib/strings");
const { readDay, dayLabel } = require("../lib/when");
const { cashLabel } = require("../lib/cash");

function orderRow(order) {
  return [kebab(order.title), dayLabel(readDay(order.placed)), cashLabel(order.cents)].join("|");
}
module.exports = { orderRow };
