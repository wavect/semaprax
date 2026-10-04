const { readDay, dayLabel } = require("../lib/when");
const { cashLabel } = require("../lib/cash");
function line(order) {
  return `${order.id}: ${cashLabel(order.cents)} on ${dayLabel(readDay(order.placed))}`;
}
module.exports = { line };
