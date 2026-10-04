const { slug } = require("./slug");
const { iso } = require("./dates");

function permalink(title, y, m, d) {
  return `/${iso(y, m, d).replace(/-/g, "/")}/${slug(title)}`;
}
module.exports = { permalink };
