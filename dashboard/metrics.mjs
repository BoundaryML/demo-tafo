// Rendering and query transport only. All aggregates and pattern rules live in BAML.
export const SNAPSHOT_QUERY =
  "SELECT span_id, start_time, output_value AS snapshot FROM spans WHERE span_name = 'smart_switch.dashboard' ORDER BY start_time DESC LIMIT 1";
const common = {
  "com.openai.codex": "Codex",
  "com.anthropic.claudefordesktop": "Claude",
  "com.tinyspeck.slackmacgap": "Slack",
  "company.thebrowser.Browser": "Arc",
  "com.apple.MobileSMS": "Messages",
};
export function appName(id, apps = []) {
  return (
    common[id] ||
    apps.find((a) => a.id === id)?.name ||
    id?.split(".").at(-1) ||
    "—"
  );
}
export function time(ms) {
  if (!Number.isFinite(ms)) return "—";
  const s = Math.floor(Math.max(0, ms) / 1000);
  return s >= 3600
    ? `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`
    : s >= 60
      ? `${Math.floor(s / 60)}m ${s % 60}s`
      : `${s}s`;
}
export const clock = (seconds) =>
  new Date(seconds * 1000).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
// BAML captured classes may share identity across period outputs. Resolve the
// Cloud value graph at the transport boundary before React reads its fields.
export function capturedValue(value) {
  const ids = new Map(),
    seen = new WeakMap();
  const fields = (v) =>
    Object.entries(v).filter(([key]) => !key.startsWith("$"));
  function index(v) {
    if (!v || typeof v !== "object") return;
    if (Object.hasOwn(v, "$id")) ids.set(v.$id, v);
    for (const [, child] of fields(v)) index(child);
  }
  function decode(v) {
    if (!v || typeof v !== "object") return v;
    if (Object.hasOwn(v, "$ref")) {
      if (!ids.has(v.$ref))
        throw Error(
          "The captured summary has an unavailable reference. Refresh to try again.",
        );
      return decode(ids.get(v.$ref));
    }
    if (seen.has(v)) return seen.get(v);
    const out = Array.isArray(v) ? [] : {};
    seen.set(v, out);
    for (const [key, child] of fields(v))
      Object.defineProperty(out, key, {
        value: decode(child),
        enumerable: true,
        writable: true,
        configurable: true,
      });
    return out;
  }
  index(value);
  return decode(value);
}

export function latency(ms) {
  return ms == null || !Number.isFinite(ms)
    ? "—"
    : ms >= 1000
      ? `${(ms / 1000).toFixed(1)} s`
      : `${Math.round(ms)} ms`;
}
export function money(usd, { precise = false } = {}) {
  if (usd == null || !Number.isFinite(usd)) return "—";
  if (usd > 0 && usd < 0.000001) return "<$0.000001";
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: precise ? 6 : 2,
  }).format(usd);
}
export function preciseTime(ms) {
  if (ms == null || !Number.isFinite(ms)) return "—";
  const value = Math.abs(ms);
  return value < 1000
    ? `${Math.round(value)} ms`
    : value < 60000
      ? `${(value / 1000).toFixed(1)} s`
      : time(value);
}
export function modelQuery(since) {
  const iso = new Date(Number(since) * 1000).toISOString();
  return `WITH requests AS (
 SELECT COUNT(*) AS calls, AVG(duration)/1000000.0 AS mean_ms, MAX(duration)/1000000.0 AS max_ms
 FROM spans WHERE span_name='POST https://api.typesafe.ai/v1/systemone' AND start_time>='${iso}'
 ), metered AS (
 SELECT COUNT(*) AS receipts, COUNT(output_value['cost_usd']) AS priced_calls,
 SUM(output_value['cost_usd']) AS cost_usd, SUM(output_value['input_tokens']) AS input_tokens,
 AVG(output_value['cost_usd'])*1000.0 AS per_1000_usd,
 MIN(CASE WHEN output_value['cost_usd'] IS NOT NULL THEN start_time ELSE NULL END) AS metered_from
 FROM spans WHERE span_name='smart_switch.model_cost' AND start_time>='${iso}'
 ) SELECT requests.*,metered.*,CASE WHEN requests.calls>metered.priced_calls THEN requests.calls-metered.priced_calls ELSE 0 END AS unpriced_calls FROM requests CROSS JOIN metered`;
}
