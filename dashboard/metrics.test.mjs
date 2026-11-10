import test from "node:test";
import assert from "node:assert/strict";
import { SNAPSHOT_QUERY, appName, time, capturedValue } from "./metrics.mjs";
test("dashboard reads the latest BAML aggregate without interpolated input", () => {
  assert.match(SNAPSHOT_QUERY, /^SELECT /);
  assert.match(SNAPSHOT_QUERY, /smart_switch\.dashboard/);
  assert.match(SNAPSHOT_QUERY, /ORDER BY start_time DESC LIMIT 1$/);
  assert.ok(!SNAPSHOT_QUERY.includes("${"));
});
test("display names resolve Codex identity and observed app names", () => {
  assert.equal(appName("com.openai.codex"), "Codex");
  assert.equal(
    appName("custom.app", [{ id: "custom.app", name: "My App" }]),
    "My App",
  );
  assert.equal(appName(null), "—");
});
test("duration formatting handles missing and boundary values", () => {
  assert.equal(time(NaN), "—");
  assert.equal(time(-1), "0s");
  assert.equal(time(59999), "59s");
  assert.equal(time(60000), "1m 0s");
  assert.equal(time(3660000), "1h 1m");
});

test("Cloud shared class references resolve across all periods, including id zero", () => {
  const visit = {
    $id: 0,
    $class: "AppVisit",
    bundle_id: "codex",
    started: 100,
    active_ms: 1000,
  };
  const result = capturedValue({
    periods: { today: { visits: [visit] }, week: { visits: [{ $ref: 0 }] } },
  });
  assert.equal(result.periods.week.visits[0].bundle_id, "codex");
  assert.equal(result.periods.week.visits[0], result.periods.today.visits[0]);
  assert.ok(!("$class" in result.periods.today.visits[0]));
});
test("missing capture references surface an error instead of invented data", () => {
  assert.throws(
    () => capturedValue({ visit: { $ref: 99 } }),
    /unavailable reference/,
  );
});

test("latency formatting never turns missing historical data into zero", async () => {
  const { latency } = await import("./metrics.mjs");
  assert.equal(latency(null), "—");
  assert.equal(latency(144), "144 ms");
  assert.equal(latency(1680), "1.7 s");
});
test("model timing queries use BAML calendar boundaries and reject invalid dates", async () => {
  const { modelQuery } = await import("./metrics.mjs");
  assert.match(modelQuery(0), /1970-01-01T00:00:00.000Z/);
  assert.throws(
    () => modelQuery("'; DROP TABLE spans; --"),
    /Invalid time value/,
  );
});

test("spend formatting preserves tiny prices and unknown historical totals", async () => {
  const { money } = await import("./metrics.mjs");
  assert.equal(money(null), "—");
  assert.equal(money(0, { precise: true }), "$0.00");
  assert.equal(money(0.000042, { precise: true }), "$0.000042");
  assert.equal(money(0.00000042, { precise: true }), "<$0.000001");
  assert.equal(money(-0.25), "-$0.25");
});
test("model cost query uses recorded prices and exposes missing coverage", async () => {
  const { modelQuery } = await import("./metrics.mjs");
  const query = modelQuery(0);
  assert.match(query, /smart_switch\.model_cost/);
  assert.match(query, /SUM\(output_value\['cost_usd'\]\)/);
  assert.match(query, /unpriced_calls/);
  assert.ok(!query.includes("0.042"));
  assert.ok(!query.includes("COALESCE"));
});
