// Process-local bounded sign-in budget and session-bound synchronizer CSRF tokens.
import crypto from "node:crypto";
export function security({ now = Date.now, random = crypto.randomBytes } = {}) {
  const secret = random(32), buckets = new Map();
  const seedOf = (req) => (/(?:^|;\s*)csrfSeed=([0-9a-f]{64})(?:;|$)/.exec(req.headers.cookie || "") || [])[1];
  const sidOf = (req) => (/(?:^|;\s*)sid=([0-9a-f]{64})(?:;|$)/.exec(req.headers.cookie || "") || [])[1] || "";
  const token = (seed, sid) => crypto.createHmac("sha256", secret).update(seed + ":" + sid).digest("hex");
  function issue(req, sid = sidOf(req)) {
    const seed = seedOf(req) || random(32).toString("hex");
    return { token: token(seed, sid), cookie: "csrfSeed=" + seed + "; HttpOnly; SameSite=Strict; Path=/" };
  }
  function valid(req) {
    const seed = seedOf(req), supplied = req.headers["x-csrf-token"];
    if (!seed || typeof supplied !== "string" || !/^[0-9a-f]{64}$/.test(supplied)) return false;
    return crypto.timingSafeEqual(Buffer.from(supplied, "hex"), Buffer.from(token(seed, sidOf(req)), "hex"));
  }
  function attempt(address, login) {
    const time = now(), keys = [["ip:" + address, 20], ["login:" + crypto.createHash("sha256").update(String(login)).digest("hex"), 5]];
    for (const [key, value] of buckets) if (value.until <= time) buckets.delete(key);
    if (buckets.size + keys.filter(([key]) => !buckets.has(key)).length > 4096) return false;
    if (keys.some(([key, limit]) => (buckets.get(key)?.count || 0) >= limit)) return false;
    for (const [key] of keys) { const value = buckets.get(key) || { count: 0, until: time + 60000 }; value.count++; buckets.set(key, value); }
    return true;
  }
  return { issue, valid, attempt };
}
