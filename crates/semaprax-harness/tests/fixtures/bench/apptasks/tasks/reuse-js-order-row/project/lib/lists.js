function chunk(xs, n) {
  const out = [];
  for (let i = 0; i < xs.length; i += n) out.push(xs.slice(i, i + n));
  return out;
}
function uniq(xs) {
  return [...new Set(xs)];
}
module.exports = { chunk, uniq };
