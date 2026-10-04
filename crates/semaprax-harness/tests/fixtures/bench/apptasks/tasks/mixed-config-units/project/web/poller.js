const fs = require("node:fs");

function load(path = "config/schedule.json") {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}
// milliseconds until the next poll; rand01 in [0, 1)
function nextDelayMs(cfg, rand01) {
  return (cfg.interval + cfg.jitter * rand01) * 1000;
}
module.exports = { load, nextDelayMs };
