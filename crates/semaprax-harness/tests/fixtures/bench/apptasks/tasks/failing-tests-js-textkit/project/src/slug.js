function slug(text) {
  return text.toLowerCase().replace(/[^a-z0-9]+/g, "-");
}
module.exports = { slug };
