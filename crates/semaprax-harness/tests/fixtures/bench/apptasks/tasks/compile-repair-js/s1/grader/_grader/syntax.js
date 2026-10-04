const { execFileSync } = require("node:child_process");
for (const f of ["src/parse.js", "src/stats.js", "src/utils.js", "src/index.js"]) {
  execFileSync(process.execPath, ["--check", f], { stdio: "pipe" });
}
console.log("syntax ok");
