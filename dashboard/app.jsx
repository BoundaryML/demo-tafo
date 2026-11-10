import React, { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  SNAPSHOT_QUERY,
  appName,
  time,
  clock,
  capturedValue,
  latency,
  modelQuery,
  money,
  preciseTime,
} from "./metrics.mjs";
import { icons } from "./icons.mjs";
function Arrow({ refresh = false }) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {refresh ? (
        <>
          <path d="M20 7v5h-5" />
          <path d="M19 12a7 7 0 1 0-2 5" />
        </>
      ) : (
        <path d="M5 12h14m-5-5 5 5-5 5" />
      )}
    </svg>
  );
}
function AppIcon({ id, apps = [], size = 30 }) {
  const name = appName(id, apps);
  return icons[id] ? (
    <img
      className="app-icon"
      src={icons[id]}
      width={size}
      height={size}
      alt={name}
    />
  ) : (
    <span
      className="app-icon fallback"
      style={{ width: size, height: size }}
      title={name}
    >
      {name.slice(0, 2)}
    </span>
  );
}
const positions = [
  [235, 210],
  [480, 210],
  [91, 76],
  [625, 76],
  [107, 335],
  [612, 335],
];
function AppMap({ view, selected, onSelect }) {
  const shown = view.apps.slice(0, 6);
  const pos = new Map(shown.map((a, i) => [a.id, positions[i]]));
  const strongest = view.loops[0];
  return (
    <div className="map-wrap">
      <div className="map-meta">
        <span>App map</span>
        <span>
          {view.app_count} apps · {view.total_moves} observed moves
        </span>
      </div>
      <svg
        className="app-map"
        viewBox="0 0 720 430"
        role="img"
        aria-label="Your observed app-to-app paths. Select an app to inspect its routes."
      >
        <defs>
          <linearGradient id="flow-line">
            <stop stopColor="var(--flow)" />
            <stop offset="1" stopColor="var(--flow-light)" />
          </linearGradient>
        </defs>
        {view.routes.map((r, i) => {
          const a = pos.get(r.from),
            b = pos.get(r.to);
          if (!a || !b) return null;
          const bend = r.from < r.to ? -40 : 40;
          const d = `M ${a[0]} ${a[1]} Q ${(a[0] + b[0]) / 2} ${(a[1] + b[1]) / 2 + bend} ${b[0]} ${b[1]}`;
          const active = selected
            ? r.from === selected || r.to === selected
            : strongest?.apps.includes(r.from) &&
              strongest?.apps.includes(r.to);
          return (
            <g
              key={`${r.from}-${r.to}`}
              className={active ? "edge active" : "edge"}
            >
              <path
                d={d}
                strokeWidth={Math.min(8, 1 + Math.log2(r.count + 1))}
              />
              <path className="edge-hit" d={d} onClick={() => onSelect(r.from)}>
                <title>
                  {appName(r.from, view.apps)} → {appName(r.to, view.apps)} ·{" "}
                  {r.count} moves · {time(r.average_stay_ms)} average stay
                </title>
              </path>
            </g>
          );
        })}
        {shown.map((a, i) => {
          const [x, y] = positions[i];
          return (
            <g
              className={`map-node ${selected === a.id ? "selected" : ""}`}
              key={a.id}
              transform={`translate(${x},${y})`}
              role="button"
              tabIndex="0"
              aria-label={`Inspect ${a.name} routes`}
              onClick={() => onSelect(selected === a.id ? null : a.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onSelect(selected === a.id ? null : a.id);
                }
              }}
            >
              <rect
                className="node-halo"
                x="-43"
                y="-43"
                width="86"
                height="86"
                rx="25"
              />
              {icons[a.id] ? (
                <image
                  href={icons[a.id]}
                  x="-27"
                  y="-27"
                  width="54"
                  height="54"
                />
              ) : (
                <text className="node-initial" textAnchor="middle" y="7">
                  {a.name.slice(0, 2)}
                </text>
              )}
              <text className="node-name" textAnchor="middle" y="66">
                {a.name}
              </text>
              <text className="node-time" textAnchor="middle" y="84">
                {time(a.active_ms)}
              </text>
            </g>
          );
        })}
        {!shown.length && (
          <text x="360" y="210" textAnchor="middle" className="map-empty">
            Your paths will appear as you use your apps.
          </text>
        )}
      </svg>
      <div className="map-foot">
        <span>Line weight shows frequency</span>
        {selected && (
          <button onClick={() => onSelect(null)}>Show all apps</button>
        )}
      </div>
    </div>
  );
}
function Loops({ view, selected, onSelect }) {
  const routes = selected
    ? view.routes.filter((r) => r.from === selected)
    : null;
  return (
    <aside className="loops">
      <div className="section-title">
        <h2>
          {selected
            ? `From ${appName(selected, view.apps)}`
            : "Your familiar loops"}
        </h2>
        {selected && (
          <button
            className="quiet-button"
            onClick={() => onSelect(null)}
            aria-label="Clear app selection"
          >
            ×
          </button>
        )}
      </div>
      {routes
        ? routes.map((r) => (
            <button
              className="route-row"
              key={r.to}
              onClick={() => onSelect(r.to)}
            >
              <div className="route-title">
                <AppIcon id={r.to} apps={view.apps} />
                <span>{appName(r.to, view.apps)}</span>
                <strong>{r.count}</strong>
              </div>
              <span className="route-detail">
                {time(r.average_stay_ms)} average stay
              </span>
            </button>
          ))
        : view.loops.slice(0, 5).map((loop, i) => (
            <button
              className="loop-row"
              key={i}
              onClick={() => onSelect(loop.apps[0])}
            >
              <div className="loop-icons">
                <AppIcon id={loop.apps[0]} apps={view.apps} size={35} />
                <span>↔</span>
                <AppIcon id={loop.apps[1]} apps={view.apps} size={35} />
              </div>
              <div className="loop-label">
                {appName(loop.apps[0], view.apps)} <span>↔</span>{" "}
                {appName(loop.apps[1], view.apps)}
              </div>
              <div className="loop-count">{loop.moves} moves</div>
            </button>
          ))}
      {(routes?.length === 0 || (!routes && !view.loops.length)) && (
        <div className="empty-inline">No repeated paths yet.</div>
      )}
    </aside>
  );
}
function SelectionHistory({ view }) {
  return (
    <section className="selections">
      <div className="section-title">
        <h2>Where you settled</h2>
        <div className="selection-counts">
          <span>{view.kept} kept</span>
          <span>{view.changed} changed</span>
        </div>
      </div>
      {view.switches.length ? (
        <div className="selection-list">
          {view.switches.slice(0, 8).map((e) => (
            <div className="selection-row" key={e.event_id}>
              <span className="when">{clock(e.ended_at)}</span>
              <div className="selection-path">
                <AppIcon id={e.initial_target} apps={view.apps} />
                <span
                  className={
                    e.initial_target !== e.final_target ? "passed" : ""
                  }
                >
                  {appName(e.initial_target, view.apps)}
                </span>
                {e.initial_target !== e.final_target && (
                  <>
                    <Arrow />
                    <AppIcon id={e.final_target} apps={view.apps} />
                    <span>{appName(e.final_target, view.apps)}</span>
                  </>
                )}
              </div>
              <span
                className={`outcome ${e.initial_target !== e.final_target ? "changed" : ""}`}
              >
                {e.initial_target !== e.final_target
                  ? "Changed on another tap"
                  : "Kept the first choice"}
              </span>
            </div>
          ))}
        </div>
      ) : (
        <div className="empty-inline">
          Tap-cycle selections will appear here.
        </div>
      )}
      <div
        className="selection-note"
        title="A changed selection is explicit tap-cycle feedback. Keeping the default does not prove it was the intended app."
      >
        First suggestion → final choice
      </div>
    </section>
  );
}
function RecentMoves({ view }) {
  return (
    <section className="moves">
      <div className="section-title">
        <h2>Recent moves</h2>
        <span className="quiet">Recorded on your Mac</span>
      </div>
      <div className="move-strip">
        {[...view.recent_visits]
          .reverse()
          .slice(-16)
          .map((v, i) => (
            <React.Fragment key={`${v.bundle_id}-${v.started}-${i}`}>
              {i > 0 && <span className="move-arrow">→</span>}
              <div
                className="move"
                title={`${appName(v.bundle_id, view.apps)} · ${clock(v.started)} · ${time(v.active_ms)}`}
              >
                <AppIcon id={v.bundle_id} apps={view.apps} size={32} />
                <span>{time(v.active_ms)}</span>
              </div>
            </React.Fragment>
          ))}
      </div>
    </section>
  );
}
function Sessions({ view }) {
  return (
    <section className="sessions">
      <div className="section-title">
        <h2>Workflows</h2>
        <span
          className="quiet"
          title="Labels are inferred from apps and timing, not window contents."
        >
          ≈ inferred
        </span>
      </div>
      {view.sessions.slice(0, 5).map((s, i) => (
        <div className="session" key={i}>
          <span className="when">{clock(s.started)}</span>
          <div className="session-name">
            {s.confidence
              ? s.title
              : s.apps
                  .slice(0, 2)
                  .map((id) => appName(id, view.apps))
                  .join(" + ")}
          </div>
          <div className="session-icons">
            {s.apps.slice(0, 3).map((id) => (
              <AppIcon id={id} key={id} apps={view.apps} size={22} />
            ))}
          </div>
          <span className="quiet">{time(s.active_ms)}</span>
        </div>
      ))}
    </section>
  );
}
function ValueSummary({ view, model, modelError }) {
  const [baseline, setBaseline] = useState(1000),
    [hourly, setHourly] = useState(75);
  const value = view.value;
  const scenario = value?.scenarios.find(
    (s) => s.baseline_ms === baseline && s.hourly_usd === hourly,
  );
  const baselines = [
    ...new Set(value?.scenarios.map((s) => s.baseline_ms) || []),
  ];
  const rates = [...new Set(value?.scenarios.map((s) => s.hourly_usd) || [])];
  return (
    <section className="value-summary" aria-label="Time cost and model spend">
      <div className="value-head">
        <span className="value-eyebrow">THE COST OF SWITCHING</span>
        <div className="assumptions" aria-label="Time value assumptions">
          <label>
            <span>Assume</span>
            <select
              aria-label="Usual time per app switch"
              value={baseline}
              onChange={(e) => setBaseline(Number(e.target.value))}
            >
              {baselines.map((ms) => (
                <option value={ms} key={ms}>
                  {ms / 1000} s / move
                </option>
              ))}
            </select>
          </label>
          <span className="assumption-dot">·</span>
          <label>
            <span>at</span>
            <select
              aria-label="Hourly time value in US dollars"
              value={hourly}
              onChange={(e) => setHourly(Number(e.target.value))}
            >
              {rates.map((rate) => (
                <option value={rate} key={rate}>
                  ${rate} / hour
                </option>
              ))}
            </select>
          </label>
        </div>
      </div>
      <div className="value-main">
        <div className="time-cost">
          <span className="value-label">
            Switching time budget <i>estimated</i>
          </span>
          <div className="value-number">
            {scenario ? preciseTime(scenario.switching_budget_ms) : "—"}
          </div>
          <div className="value-equivalent">
            {scenario ? money(scenario.switching_budget_usd) : "—"}{" "}
            <span>
              of your time · {value?.moves ?? view.total_moves} observed app
              moves
            </span>
          </div>
        </div>
        <div className="model-cost">
          <span className="value-label">
            Metered JEV spend <i>usage-based</i>
          </span>
          <div className="value-number money">
            {money(model?.cost_usd, { precise: true })}
          </div>
          <div className="value-equivalent">
            {model?.priced_calls
              ? `${model.priced_calls.toLocaleString()} priced requests`
              : modelError
                ? "Cost query unavailable"
                : model
                  ? "Waiting for token usage"
                  : "Loading spend…"}
            <span>
              {model?.priced_calls
                ? ` · ${money(model.per_1000_usd)} / 1k requests`
                : ""}
            </span>
          </div>
          <div
            className="cost-coverage"
            title="Spend uses provider-reported input tokens and the price recorded with each request. Older requests with no token receipt remain unpriced. This covers JEV inference, before credits or taxes."
          >
            {model?.unpriced_calls > 0 ? (
              <>
                <span className="coverage-dot" />
                {model.unpriced_calls.toLocaleString()} requests unpriced in
                this period
              </>
            ) : model?.priced_calls > 0 ? (
              <>
                <span className="coverage-dot complete" />
                All observed requests priced
              </>
            ) : null}
          </div>
        </div>
      </div>
      <div className="value-proof">
        <div>
          <span>Tafo choice time</span>
          <strong>
            {value?.measured_switches ? preciseTime(value.switch_ms) : "—"}
          </strong>
          <small>
            {value?.measured_switches ?? 0} timed choices
            {value?.unmeasured_switches
              ? ` · ${value.unmeasured_switches} older untimed`
              : ""}
          </small>
        </div>
        <div className={scenario?.measured_delta_ms > 0 ? "value-gain" : ""}>
          <span>Against your baseline</span>
          <strong>
            {scenario?.measured_delta_ms == null
              ? "—"
              : `${preciseTime(scenario.measured_delta_ms)} ${scenario.measured_delta_ms >= 0 ? "faster" : "slower"}`}
          </strong>
          <small>
            {scenario?.measured_delta_usd == null
              ? "Measured savings appear after timed switches"
              : `${money(scenario.measured_delta_usd)} estimated time value`}
          </small>
        </div>
        <div>
          <span>Correction overhead</span>
          <strong>
            {value?.measured_switches ? preciseTime(value.correction_ms) : "—"}
          </strong>
          <small>Extra choosing time after a changed suggestion</small>
        </div>
      </div>
      <div className="value-foot">
        <span>
          Time budget = observed moves × your assumed pace. Savings compare
          timed Tafo choices only.
        </span>
        <a
          href="https://help.typesafe.ai/articles/5187202169-how-much-does-jev-cost"
          target="_blank"
          rel="noreferrer"
          title="Rate verified November 10, 2026. The rate is saved with each request."
        >
          $0.042 / M input tokens · output free ↗
        </a>
      </div>
    </section>
  );
}
function Performance({ view, at, refresh }) {
  const [model, setModel] = useState(null),
    [modelError, setModelError] = useState(null);
  useEffect(() => {
    let active = true;
    setModel(null);
    setModelError(null);
    Promise.resolve()
      .then(() => window.boundary.query(modelQuery(view.since)))
      .then((result) => {
        if (result.outcome.status !== "complete")
          throw Error("Incomplete model-cost query");
        if (active) setModel(result.rows[0] || null);
      })
      .catch((error) => {
        if (active) setModelError(error.message);
      });
    return () => {
      active = false;
    };
  }, [view.since, at, refresh]);
  const p = view.performance;
  if (!p) return null;
  return (
    <>
      <ValueSummary view={view} model={model} modelError={modelError} />
      <section className="performance">
        <div className="response-strip">
          <div
            title={`Time from the first tap to macOS accepting an activation request. Includes preview waiting; excludes window rendering. ${p.measured} measured switches.`}
          >
            <span>Tap → switch</span>
            <strong>{latency(p.first_p50_ms)}</strong>
            <small>p50 · p95 {latency(p.first_p95_ms)}</small>
          </div>
          <div
            title={`First-tap immediate activations among switches with recorded path information. Final-choice p50: ${latency(p.settled_p50_ms)}.`}
          >
            <span>Fast path</span>
            <strong>
              {p.fast_percent == null ? "—" : `${p.fast_percent.toFixed(0)}%`}
            </strong>
            <small>
              {p.fast_switches} fast · {p.preview_switches} preview
            </small>
          </div>
          <div title="Final target differed from the initial suggestion. Behavioral corrections, not a calibrated model error rate.">
            <span>Changed choice</span>
            <strong>
              {p.changed_percent == null
                ? "—"
                : `${p.changed_percent.toFixed(0)}%`}
            </strong>
            <small>
              {view.changed} of {view.kept + view.changed} choices
            </small>
          </div>
          <div
            title={
              modelError ||
              `JEV HTTP duration from BAML query. Slowest: ${latency(model?.max_ms)}.`
            }
          >
            <span>JEV response</span>
            <strong>{latency(model?.mean_ms)}</strong>
            <small>
              {model
                ? `${model.calls} requests · mean`
                : modelError
                  ? "Unavailable"
                  : "Loading…"}
            </small>
          </div>
        </div>
        {p.learning.length > 0 && (
          <div className="learning-strip">
            <div className="section-title">
              <h2>Learning your switches</h2>
              <span>≈ behavioral evidence</span>
            </div>
            <div className="learning-pairs">
              {p.learning.map((pair) => (
                <div
                  key={`${pair.origin}-${pair.target}`}
                  className={`learning-pair ${pair.ready ? "ready" : ""}`}
                  title={`${pair.accepted} accepted, ${pair.corrected} corrected across ${pair.observations} recent observations. Recency-weighted acceptance estimate. Three consecutive accepted predictions and confidence ≥70% unlock the learned fast path.`}
                >
                  <AppIcon id={pair.origin} apps={view.apps} size={22} />
                  <span className="pair-name">
                    {appName(pair.origin, view.apps)}
                    <i>→</i>
                    {appName(pair.target, view.apps)}
                  </span>
                  <AppIcon id={pair.target} apps={view.apps} size={22} />
                  <strong>{(pair.confidence * 100).toFixed(0)}%</strong>
                  <span
                    className="pair-ready"
                    aria-label={
                      pair.ready ? "Fast path ready" : "Still learning"
                    }
                  />
                </div>
              ))}
            </div>
          </div>
        )}
      </section>
    </>
  );
}
function App() {
  const [range, setRange] = useState(
    () => new URLSearchParams(location.hash.slice(1)).get("range") || "today",
  );
  const [selected, setSelected] = useState(null),
    [record, setRecord] = useState(null),
    [error, setError] = useState(null),
    [busy, setBusy] = useState(true),
    [refresh, setRefresh] = useState(0);
  const serial = useRef(0);
  useEffect(() => {
    const i = ++serial.current;
    setBusy(true);
    setError(null);
    if (!window.boundary) {
      setError("Open this dashboard in Boundary Cloud.");
      setBusy(false);
      return;
    }
    window.boundary
      .query(SNAPSHOT_QUERY)
      .then((result) => {
        if (serial.current !== i) return;
        if (result.outcome.status !== "complete")
          throw Error(
            "The query returned incomplete data. Refresh to try again.",
          );
        const row = result.rows[0];
        setRecord(
          row ? { ...row, snapshot: capturedValue(row.snapshot) } : null,
        );
        setBusy(false);
      })
      .catch((e) => {
        if (serial.current === i) {
          setError(e.message);
          setBusy(false);
        }
      });
    return () => {
      serial.current++;
    };
  }, [refresh]);
  useEffect(() => {
    const onHash = () => {
      const next = new URLSearchParams(location.hash.slice(1)).get("range");
      if (["today", "7d", "30d"].includes(next)) setRange(next);
    };
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);
  const chooseRange = (key) => {
    setRange(key);
    setSelected(null);
    location.hash = `range=${key}`;
  };
  const snapshot = record?.snapshot;
  const view = snapshot?.periods?.[range] || snapshot?.periods?.today;
  return (
    <main>
      <header>
        <div className="brand">
          <div className="switch-mark" aria-hidden="true">
            <i />
            <i />
            <i />
          </div>
          <span>Tafo</span>
        </div>
        <div className="header-tools">
          <div className="range">
            {[
              ["today", "Today"],
              ["7d", "Week"],
              ["30d", "Month"],
            ].map(([key, title]) => (
              <button
                className={range === key ? "selected" : ""}
                key={key}
                onClick={() => chooseRange(key)}
              >
                {title}
              </button>
            ))}
          </div>
          <button
            className={`refresh ${busy ? "busy" : ""}`}
            title="Refresh"
            aria-label="Refresh patterns"
            disabled={busy}
            onClick={() => setRefresh((x) => x + 1)}
          >
            <Arrow refresh />
          </button>
        </div>
      </header>
      <div className="intro">
        <h1>Your switches</h1>
        {view && (
          <div className="intro-stat">
            <strong>{time(view.total_ms)}</strong>
            <span>recorded activity</span>
          </div>
        )}
      </div>
      {error ? (
        <div className="empty">
          <h2>Couldn’t load your patterns</h2>
          <p>{error}</p>
          <button onClick={() => setRefresh((x) => x + 1)}>Try again</button>
        </div>
      ) : busy && !record ? (
        <div className="skeleton" aria-label="Loading patterns" />
      ) : !view ? (
        <div className="empty">
          <h2>No activity in this environment</h2>
          <p>
            Use the environment menu above to switch to Personal, dev, or
            Production.
          </p>
          <p>
            Local builds publish to Personal. Release builds publish to
            Production.
          </p>
        </div>
      ) : (
        <>
          <Performance view={view} at={snapshot.at} refresh={refresh} />
          <div className="pattern-canvas">
            <AppMap view={view} selected={selected} onSelect={setSelected} />
            <Loops view={view} selected={selected} onSelect={setSelected} />
          </div>
          <RecentMoves view={view} />
          <div className="lower-grid">
            <SelectionHistory view={view} />
            <Sessions view={view} />
          </div>
          <footer>
            <span title="BAML calculates these patterns from Tafo's retained local activity. The app publishes a summary approximately once a minute while recording is enabled.">
              As of{" "}
              {new Date(snapshot.at * 1000).toLocaleDateString([], {
                month: "short",
                day: "numeric",
              })}
              , {clock(snapshot.at)}
            </span>
            <button onClick={() => window.boundary.openSpan(record.span_id)}>
              Inspect summary ↗
            </button>
          </footer>
        </>
      )}
    </main>
  );
}
createRoot(document.getElementById("root")).render(<App />);
