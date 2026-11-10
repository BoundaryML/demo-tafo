# Tafo dashboard

A self-contained React dashboard for Boundary Cloud. Queries run with the viewer's
selected environment and access; no credentials or activity data are embedded.

```bash
npm ci --ignore-scripts
npm run format:check
npm test
npm run build
```

Upload `dist/activity.html` as a custom dashboard or a new version of an existing
one. React, CSS, and static app icons are bundled. No external fonts or scripts
are loaded. The native app publishes `smart_switch.dashboard` summaries while
tracking is enabled; `smart_switch.model_cost` spans carry metered JEV usage.
These event names are retained for compatibility with existing recordings.

BAML computes routes, inferred workflows, learning evidence, timing summaries,
retention, and time-value scenarios. React selects precomputed scenarios, formats
values, and runs read-only BAML queries. Today/Week/Month boundaries come from the
Mac's calendar. Shared captured values are resolved at the query boundary.

Model spend uses reported tokens and the price stored with each request. Unpriced
history remains explicit. The time budget uses visible switching-pace and hourly
assumptions; measured savings use only timed choices and can be negative. A
brief app visit is not automatically classified as wasted time. Costs cover JEV
inference before credits/taxes, not Cloud hosting or the account invoice.

The README screenshot uses illustrative synthetic data.
