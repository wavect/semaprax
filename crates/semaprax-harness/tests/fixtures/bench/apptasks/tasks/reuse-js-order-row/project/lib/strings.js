// kebab("Hello, World!") -> "hello-world"
function kebab(s) {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
function ellipsize(s, n) {
  return s.length <= n ? s : s.slice(0, n - 1) + "…";
}
function upperFirst(s) {
  return s.charAt(0).toUpperCase() + s.slice(1);
}
module.exports = { kebab, ellipsize, upperFirst };
