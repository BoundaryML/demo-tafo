// TypeSafe list price verified 2026-11-10. Keep the rate on every receipt so
// future price changes do not reprice historical requests.
const JEV_INPUT_USD_PER_MILLION = 0.042;
const JEV_PRICE_SOURCE = "https://help.typesafe.ai/articles/5187202169-how-much-does-jev-cost";
const PRICED_JEV_MODELS = ["jev-1.13.0", "jev-1.14.0"];

class JevTokenUsage {
    input_tokens: int,
    output_tokens: int?,
}

class JevUsageEnvelope {
    model: string?,
    usage: JevTokenUsage?,
}

class ModelCostReceipt {
    provider: string,
    model: string,
    input_tokens: int?,
    output_tokens: int?,
    cost_usd: float?,
    input_usd_per_million: float?,
    output_usd_per_million: float?,
    rate_source: string,
    rate_verified: string,
    basis: "provider_tokens" | "unpriced",
}

function valid_token_count(value: int?) -> int? throws never {
    if (value != null && value >= 0) {
        value
    } else {
        null
    }
}

function model_cost_receipt(
    body: string,
    requested_model: string,
) -> ModelCostReceipt throws never {
    let response = baml.json.decode<JevUsageEnvelope>(body) catch (error) {
        baml.json.ParseError | baml.json.DecodeError => {
            JevUsageEnvelope { model: null, usage: null }
        },
    };
    let model = response.model ?? requested_model;
    let input = valid_token_count(response.usage?.input_tokens);
    let receipt = ModelCostReceipt {
        provider: "typesafe",
        model: model,
        input_tokens: input,
        output_tokens: valid_token_count(response.usage?.output_tokens),
        cost_usd: null,
        input_usd_per_million: null,
        output_usd_per_million: null,
        rate_source: JEV_PRICE_SOURCE,
        rate_verified: "2026-11-10",
        basis: "unpriced",
    };
    if (PRICED_JEV_MODELS.includes(model) && input != null) {
        receipt.cost_usd = input * JEV_INPUT_USD_PER_MILLION / 1000000.0;
        receipt.input_usd_per_million = JEV_INPUT_USD_PER_MILLION;
        receipt.output_usd_per_million = 0.0;
        receipt.basis = "provider_tokens";
    }
    receipt
}

function record_model_cost(body: string, model: string) -> void throws trace.SpanError {
    if (!recording_allowed()) {
        return;
    }
    let receipt = model_cost_receipt(body, model);
    let span = trace.start_span("smart_switch.model_cost");
    span.finish(receipt);
}

class TimeValueScenario {
    baseline_ms: int,
    hourly_usd: int,
    switching_budget_ms: int,
    switching_budget_usd: float,
    measured_baseline_ms: int,
    measured_delta_ms: int?,
    measured_delta_usd: float?,
}

class SwitchValue {
    moves: int,
    measured_switches: int,
    unmeasured_switches: int,
    switch_ms: int,
    correction_ms: int,
    brief_visits: int,
    brief_active_ms: int,
    scenarios: TimeValueScenario[],

    function scenario(self, baseline_ms: int, hourly_usd: int) -> TimeValueScenario throws never {
        let comparable: int = self.measured_switches * baseline_ms;
        let delta: int = comparable - self.switch_ms;
        let value_usd: float = delta * hourly_usd / 3600000.0;
        let budget = self.moves * baseline_ms;
        TimeValueScenario {
            baseline_ms: baseline_ms,
            hourly_usd: hourly_usd,
            switching_budget_ms: budget,
            switching_budget_usd: budget * hourly_usd / 3600000.0,
            measured_baseline_ms: comparable,
            measured_delta_ms: if (self.measured_switches > 0) {
                delta
            } else {
                null
            },
            measured_delta_usd: if (self.measured_switches > 0) {
                value_usd
            } else {
                null
            },
        }
    }
}

function switch_value(
    visits: AppVisit[],
    events: SwitchEvent[],
    moves: int,
) -> SwitchValue throws never {
    let measured = events.filter((event) -> {
        event.settled_activation_ms != null
    });
    let spent: int = measured.reduce(
        (total, event) -> {
            total + (event.settled_activation_ms ?? 0)
        },
        0,
    );
    let corrections = measured
        .filter((event) -> {
            event.initial_target != event.final_target
        })
        .reduce(
            (total, event) -> {
                let first = event.first_activation_ms;
                let extra = if (first != null) {
                    (event.settled_activation_ms ?? 0) - first
                } else {
                    0
                };
                total
                    + if (extra > 0) {
                        extra
                    } else {
                        0
                    }
            },
            0,
        );
    let brief = visits.filter((visit) -> {
        visit.active_ms > 0 && visit.active_ms <= 15000
    });
    let result = SwitchValue {
        moves: moves,
        measured_switches: measured.length(),
        unmeasured_switches: events.length() - measured.length(),
        switch_ms: spent,
        correction_ms: corrections,
        brief_visits: brief.length(),
        brief_active_ms: brief.reduce(
            (total, visit) -> {
                total + visit.active_ms
            },
            0,
        ),
        scenarios: [],
    };
    result.scenarios
        = [500, 1000, 1500, 2000, 3000].flat_map((baseline) -> {
            [25, 50, 75, 100, 150, 200].map((rate) -> {
                result.scenario(baseline, rate)
            })
        });
    result
}

test "cost uses provider tokens and a versioned price with free output" {
    let receipt = model_cost_receipt(`{"model":"jev-1.14.0","usage":{"input_tokens":1000000,"output_tokens":900000}}`, "jev-latest");
    assert.equal(receipt.cost_usd, 0.042);
    assert.equal(receipt.output_usd_per_million, 0.0);
    assert.equal(receipt.basis, "provider_tokens");
    assert.equal(
        model_cost_receipt(`{"model":"jev-1.14.0","usage":{"input_tokens":0}}`, "jev-latest").cost_usd,
        0.0,
    )
}

test "missing invalid and unknown-model usage stays unpriced" {
    for (
        let body in [
            "{}",
            "not JSON",
            `{"model":"jev-1.14.0","usage":{"input_tokens":-1}}`,
            `{"model":"jev-99.0","usage":{"input_tokens":100}}`,
        ]
    ) {
        let receipt = model_cost_receipt(body, "jev-latest");
        assert.equal(receipt.cost_usd, null);
        assert.equal(receipt.basis, "unpriced");
    }
}

test "time value compares only measured switches and can show a negative return" {
    let fast = SwitchEvent { ...learned_event(1), first_activation_ms: 100, settled_activation_ms: 100 };
    let correction = SwitchEvent {
        ...learned_event(2, final = "slack", advances = 1),
        first_activation_ms: 100,
        settled_activation_ms: 1900,
    };
    let slow = SwitchEvent { ...learned_event(3), first_activation_ms: 2500, settled_activation_ms: 2500 };
    let result = switch_value([], [fast, correction, slow, learned_event(0)], 10);
    let scenario = result.scenarios.find((s) -> {
        s.baseline_ms == 1000 && s.hourly_usd == 75
    });
    assert.equal(result.measured_switches, 3);
    assert.equal(result.unmeasured_switches, 1);
    assert.equal(result.switch_ms, 4500);
    assert.equal(result.correction_ms, 1800);
    assert.equal(scenario?.switching_budget_ms, 10000);
    assert.equal(scenario?.measured_delta_ms, -1500);
    assert.equal((scenario?.measured_delta_usd ?? 0.0) < 0.0, true)
}

test "historical events never become measured savings or zero observed cost" {
    let result = switch_value([], [learned_event(1)], 5);
    assert.equal(result.measured_switches, 0);
    assert.equal(result.scenarios[0].measured_delta_ms, null);
    assert.equal(result.scenarios[0].measured_delta_usd, null)
}
