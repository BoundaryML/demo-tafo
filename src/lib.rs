pub use baml_sdk::{
    self as sdk, ActivitySummary, AppTime, AppVisit as Visit, CyclePlan, Decision,
    HistoryOrNameOrModelOrNo_match as SelectionSource, KnownApp, OpenApp, Selection, Snapshot,
    SwitchEvent, SwitchTiming, TrackingState, WorkflowGuess, WorkflowHint, WorkflowNote,
    WorkflowSession,
};
pub mod activity;
pub use activity::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};
pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct History {
    pub activations: Vec<String>,
}
impl History {
    pub fn record(&mut self, id: &str) -> Result<()> {
        self.activations = sdk::record_activation_sync(sdk::ActivationData::new(
            self.activations.clone(),
            id.to_owned(),
        ))
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        activity::save_private_json(path, self)
    }
}
pub fn ordered_bundle_ids(listing: &str) -> Result<Vec<String>> {
    sdk::process_order_sync(listing.to_owned()).map_err(|e| e.to_string())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressAction {
    None,
    Tap,
    Prompt,
    Commit,
    Cancel,
}
#[derive(Debug, Default)]
pub struct PressTracker {
    started: Option<Duration>,
    prompted: bool,
}
impl PressTracker {
    pub fn down(&mut self, now: Duration) -> bool {
        if self.started.is_some() {
            return false;
        }
        self.started = Some(now);
        self.prompted = false;
        true
    }
    pub fn held(&mut self, now: Duration) -> PressAction {
        if let Some(start) = self.started {
            if !self.prompted && now.saturating_sub(start) >= Duration::from_millis(600) {
                self.prompted = true;
                return PressAction::Prompt;
            }
        }
        PressAction::None
    }
    pub fn up(&mut self, now: Duration) -> PressAction {
        let Some(start) = self.started.take() else {
            return PressAction::None;
        };
        if std::mem::take(&mut self.prompted) {
            return PressAction::None;
        }
        if now.saturating_sub(start) >= Duration::from_millis(600) {
            PressAction::Prompt
        } else {
            PressAction::Tap
        }
    }
    pub fn cancel(&mut self) {
        self.started = None;
        self.prompted = false;
    }
    pub fn is_down(&self) -> bool {
        self.started.is_some()
    }
}

pub fn storage_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("SMART_SWITCH_STATE_DIR") {
        return path.into();
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join("Library/Application Support/Smart Switch")
}
pub fn config_path() -> PathBuf {
    std::env::var_os("SMART_SWITCH_ENV_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let installed = storage_dir().join(".env");
            if installed.exists() {
                installed
            } else {
                Path::new(env!("CARGO_MANIFEST_DIR")).join(".env")
            }
        })
}
// Only known values are read. No expansion, shell execution, or profile loading.
pub fn parse_environment(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .filter_map(|(key, value)| {
            let key = key.trim();
            if ["JEV_API_KEY", "TYPESAFE_API_KEY", "JEV_MODEL"].contains(&key) {
                Some((
                    key.to_owned(),
                    value.trim().trim_matches('"').trim_matches('\'').to_owned(),
                ))
            } else {
                None
            }
        })
        .collect()
}

// Startup only: load the restricted credential file before worker threads start.
pub fn initialize_runtime() -> Result<()> {
    static INIT: OnceLock<Result<()>> = OnceLock::new();
    INIT.get_or_init(|| {
        if let Ok(text) = fs::read_to_string(config_path()) {
            for (key, value) in parse_environment(&text) {
                if std::env::var_os(&key).is_none() {
                    std::env::set_var(key, value);
                }
            }
        }
        if !cloud_tracing_enabled() || std::env::var_os("SMART_SWITCH_STATE_DIR").is_some() {
            std::env::set_var("BAML_TELEMETRY", "off");
        }
        // Live builds use the generated bridge's regular recording mode.
        let library = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .ok_or("Executable parent missing")?
            .join("../Frameworks/libbridge_cffi.dylib");
        if library.exists() {
            baml_bridge::loader::set_shared_library_path(library);
        }
        sdk::init().map_err(|e| e.to_string())
    })
    .clone()
}

pub fn source_name(source: &SelectionSource) -> &'static str {
    match source {
        SelectionSource::History => "history",
        SelectionSource::Name => "name",
        SelectionSource::Model => "model",
        SelectionSource::No_match => "no_match",
    }
}

// Native convenience calls supply disk state and host calendar boundaries. Types,
// invocation, errors, cancellation and runtime ownership come from the generated SDK.
pub struct Engine {
    record: bool,
}
impl Engine {
    pub fn live() -> Self {
        Self {
            record: cloud_tracing_enabled() && std::env::var_os("SMART_SWITCH_STATE_DIR").is_none(),
        }
    }
    pub fn offline() -> Self {
        Self { record: false }
    }
    pub fn choose(&mut self, snapshot: &Snapshot, request: &str) -> Result<Selection> {
        self.request(snapshot, request, false)
    }
    pub fn predict(&mut self, snapshot: &Snapshot) -> Result<Selection> {
        self.request(snapshot, "", true)
    }
    pub fn request(
        &self,
        snapshot: &Snapshot,
        request: &str,
        prediction: bool,
    ) -> Result<Selection> {
        #[cfg(feature = "native-smoke")]
        if let Ok(mode) = std::env::var("SMART_SWITCH_SMOKE_MODE") {
            return sdk::native_smoke_choice_sync(snapshot, mode == "fast")
                .map_err(|e| e.to_string());
        }
        let activity = Activity::load(&storage_dir().join("activity.json"));
        sdk::decide_sync_with_options(
            sdk::SwitchRequest::new(
                snapshot.clone(),
                request.to_owned(),
                prediction,
                activity.as_data(),
                self.record,
            ),
            sdk::BamlOptions::new().timeout_ms(8000),
        )
        .map_err(|e| e.to_string())
    }
    pub fn tracking(
        &mut self,
        state: &TrackingState,
        action: &str,
        at: i64,
    ) -> Result<TrackingState> {
        let action = sdk::tracking_action_sync(action.to_owned()).map_err(|e| e.to_string())?;
        sdk::update_tracking_sync(sdk::TrackingData::new(
            state.clone(),
            action,
            at,
            local_day_ago(at, -1),
        ))
        .map_err(|e| e.to_string())
    }
    pub fn dashboard(&mut self, activity: &Activity) -> Result<sdk::DashboardSnapshot> {
        let at = unix_now();
        sdk::record_dashboard_sync(
            sdk::DashboardData::new(
                activity.as_data(),
                vec![
                    sdk::PeriodBoundary::new("today".into(), local_day_ago(at, 0)),
                    sdk::PeriodBoundary::new("7d".into(), local_day_ago(at, 6)),
                    sdk::PeriodBoundary::new("30d".into(), local_day_ago(at, 29)),
                ],
                at,
            ),
            self.record && Activity::load(&storage_dir().join("activity.json")).enabled,
        )
        .map(|reply| reply.dashboard)
        .map_err(|e| e.to_string())
    }
    pub fn summarize(&mut self, activity: &Activity, since: i64) -> Result<ActivitySummary> {
        sdk::summarize_activity_sync(sdk::SummarizeData::new(activity.as_data(), since))
            .map_err(|e| e.to_string())
    }
    pub fn cycle_start(
        &mut self,
        context: &Snapshot,
        source: &str,
        event_id: &str,
        at: i64,
        ranking: &[String],
        fast: bool,
    ) -> Result<Option<CyclePlan>> {
        sdk::cycle_start_sync(sdk::CycleStartData::new(
            ranking.to_vec(),
            context.clone(),
            fast,
            source.to_owned(),
            event_id.to_owned(),
            at,
        ))
        .map(|reply| reply.cycle)
        .map_err(|e| e.to_string())
    }
    pub fn cycle_next(
        &mut self,
        open: &[String],
        pick: Option<usize>,
    ) -> Result<Option<CyclePlan>> {
        sdk::cycle_next_sync(sdk::CycleNextData {
            pick: pick.map(|index| index as i64),
            ..sdk::CycleNextData::new(open.to_vec())
        })
        .map(|reply| reply.cycle)
        .map_err(|e| e.to_string())
    }
    pub fn cycle_cancel(&mut self) -> Result<Option<SwitchEvent>> {
        sdk::cycle_cancel_sync()
            .map(|reply| reply.ended)
            .map_err(|e| e.to_string())
    }
    pub fn cycle_end(&mut self, at: i64) -> Result<Option<SwitchEvent>> {
        self.cycle_end_timed(at, None)
    }
    pub fn cycle_end_timed(
        &mut self,
        at: i64,
        timing: Option<SwitchTiming>,
    ) -> Result<Option<SwitchEvent>> {
        let record = self.record && Activity::load(&storage_dir().join("activity.json")).enabled;
        sdk::cycle_end_sync(at, timing, record)
            .map(|reply| reply.ended)
            .map_err(|e| e.to_string())
    }
}
#[derive(Debug, Clone, Copy)]
pub struct ToastMotion {
    pub opacity: f64,
    pub offset_y: f64,
    pub remaining: f64,
    pub finished: bool,
}
impl ToastMotion {
    pub fn at(elapsed: Duration, reduce_motion: bool) -> Self {
        let t = elapsed.as_secs_f64();
        let finished = t >= 3.2;
        if reduce_motion {
            return Self {
                opacity: if finished { 0.0 } else { 1.0 },
                offset_y: 0.0,
                remaining: 1.0,
                finished,
            };
        }
        let enter = (t / 0.18).clamp(0.0, 1.0);
        let exit = ((t - 2.85) / 0.35).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - enter).powi(3);
        Self {
            opacity: eased * (1.0 - exit),
            offset_y: -10.0 * (1.0 - eased) - 6.0 * exit,
            remaining: (1.0 - t / 3.2).clamp(0.0, 1.0),
            finished,
        }
    }
}

pub fn cloud_tracing_enabled() -> bool {
    option_env!("SMART_SWITCH_CLOUD") == Some("1")
}
