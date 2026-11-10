//! Native persistence and capture buffers. All shared data types are generated.
use crate::{sdk, Decision, Result, Selection, Snapshot, TrackingState, Visit};
use serde::{Deserialize, Serialize};
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Activity {
    pub enabled: bool,
    #[serde(default)]
    pub paused_until: Option<i64>,
    pub visits: Vec<Visit>,
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub workflow_notes: Vec<crate::WorkflowNote>,
    #[serde(default)]
    pub switches: Vec<crate::SwitchEvent>,
    #[serde(skip)]
    current: Option<usize>,
}
impl Default for Activity {
    fn default() -> Self {
        Self {
            enabled: true,
            paused_until: None,
            visits: vec![],
            decisions: vec![],
            workflow_notes: vec![],
            switches: vec![],
            current: None,
        }
    }
}
impl Activity {
    pub fn as_data(&self) -> sdk::ActivityData {
        sdk::ActivityData::new(
            self.enabled,
            self.visits.clone(),
            self.workflow_notes.clone(),
            self.switches.clone(),
        )
    }
    pub fn set_tracking(&mut self, state: &TrackingState) {
        self.enabled = state.enabled;
        self.paused_until = state.paused_until;
        self.current = None;
    }
    pub fn record_workflow(
        &mut self,
        visit_started: i64,
        guess: &crate::WorkflowGuess,
    ) -> Result<()> {
        self.workflow_notes = sdk::record_workflow_note_sync(sdk::WorkflowUpdate::new(
            self.enabled,
            self.workflow_notes.clone(),
            visit_started,
            guess.clone(),
        ))
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Capture only OS measurements here; BAML decides what time is chargeable.
    pub fn observe(
        &mut self,
        app: Option<(&str, &str)>,
        at: i64,
        delta_ms: i64,
        idle_seconds: f64,
    ) -> Result<()> {
        let previous = self.current.and_then(|index| self.visits.get(index));
        let data = sdk::CaptureData {
            previous: previous.cloned(),
            previous_day: previous.map(|visit| local_day_start(visit.started)),
            app: app.map(|(id, name)| sdk::CapturedApp::new(id.to_owned(), name.to_owned())),
            idle_seconds: idle_seconds.is_finite().then_some(idle_seconds),
            ..sdk::CaptureData::new(self.enabled, local_day_start(at), at, delta_ms)
        };
        let result = sdk::capture_observation_sync_with_options(
            data,
            sdk::BamlOptions::new().timeout_ms(50),
        )
        .map_err(|e| e.to_string());
        let delta = match result {
            Ok(delta) => delta,
            Err(error) => {
                self.current = None;
                return Err(error);
            }
        };
        if let (Some(index), Some(visit)) = (self.current, delta.previous) {
            self.visits[index] = visit;
        }
        if let Some(visit) = delta.append {
            self.visits.push(visit);
            self.current = Some(self.visits.len() - 1);
        } else if !delta.keep_current {
            self.current = None;
        }
        Ok(())
    }
    pub fn record_decision(
        &mut self,
        at: i64,
        context: &Snapshot,
        choice: &Selection,
        reason: String,
    ) -> Result<()> {
        let decision = sdk::decision_record_sync(sdk::DecisionData::new(
            self.enabled,
            at,
            context.clone(),
            choice.clone(),
            reason,
        ))
        .map_err(|e| e.to_string())?;
        if let Some(decision) = decision {
            self.decisions.push(decision);
        }
        Ok(())
    }
    pub fn prune(&mut self, now: i64) -> Result<()> {
        let plan = sdk::retention_plan_sync(sdk::RetentionData::new(
            now,
            self.visits.iter().map(|v| v.ended).collect(),
            self.decisions.iter().map(|v| v.at).collect(),
            self.workflow_notes
                .iter()
                .map(|v| v.visit_started)
                .collect(),
            self.switches.iter().map(|v| v.ended_at).collect(),
        ))
        .map_err(|e| e.to_string())?;
        self.visits.drain(..plan.visits_drop as usize);
        self.decisions.drain(..plan.decisions_drop as usize);
        self.current = self
            .current
            .and_then(|index| index.checked_sub(plan.visits_drop as usize));
        self.workflow_notes = plan
            .workflow_notes_keep
            .into_iter()
            .map(|index| self.workflow_notes[index as usize].clone())
            .collect();
        self.switches = plan
            .switches_keep
            .into_iter()
            .map(|index| self.switches[index as usize].clone())
            .collect();
        Ok(())
    }
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        save_private_json(path, self)
    }
}
// Unix permissions and atomic replacement are host filesystem capabilities.
pub fn save_private_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    let temporary = path.with_extension("tmp");
    // Create private before writing any activity data.
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())
}

pub fn display_name(id: &str, name: &str) -> String {
    if id == "com.openai.codex" {
        "Codex".into()
    } else {
        name.into()
    }
}
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn duration(ms: i64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}h {}m", s / 3600, s % 3600 / 60)
    } else if s >= 60 {
        format!("{}m {}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}
pub fn local_time(at: i64) -> String {
    let timestamp = at as libc::time_t;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    let mut buffer = [0u8; 48];
    unsafe {
        if libc::localtime_r(&timestamp, tm.as_mut_ptr()).is_null() {
            return at.to_string();
        }
        let count = libc::strftime(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            c"%b %d %H:%M:%S".as_ptr(),
            tm.as_ptr(),
        );
        String::from_utf8_lossy(&buffer[..count]).into_owned()
    }
}
pub fn local_day_start(at: i64) -> i64 {
    let timestamp = at as libc::time_t;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    unsafe {
        if libc::localtime_r(&timestamp, tm.as_mut_ptr()).is_null() {
            return at;
        }
        let mut tm = tm.assume_init();
        tm.tm_hour = 0;
        tm.tm_min = 0;
        tm.tm_sec = 0;
        tm.tm_isdst = -1;
        libc::mktime(&mut tm).max(0) as i64
    }
}
/// A cached prediction only applies to exactly the app sequence and inventory it saw.
pub fn prediction_key(snapshot: &Snapshot) -> String {
    serde_json::to_string(&serde_json::json!({"current_app_id":snapshot.current_app_id,"apps":snapshot.apps,"recent_history":snapshot.recent_history,"recent_switches":snapshot.recent_switches})).unwrap_or_default()
}

pub fn short_time(at: i64) -> String {
    let timestamp = at as libc::time_t;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    let mut buffer = [0u8; 32];
    unsafe {
        if libc::localtime_r(&timestamp, tm.as_mut_ptr()).is_null() {
            return String::new();
        }
        let count = libc::strftime(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            c"%H:%M".as_ptr(),
            tm.as_ptr(),
        );
        String::from_utf8_lossy(&buffer[..count]).into_owned()
    }
}

pub fn local_day_ago(at: i64, days: i32) -> i64 {
    let timestamp = at as libc::time_t;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    unsafe {
        if libc::localtime_r(&timestamp, tm.as_mut_ptr()).is_null() {
            return at;
        }
        let mut tm = tm.assume_init();
        tm.tm_hour = 0;
        tm.tm_min = 0;
        tm.tm_sec = 0;
        tm.tm_mday -= days;
        tm.tm_isdst = -1;
        libc::mktime(&mut tm).max(0) as i64
    }
}
