//! Settings › Claude (add-claude-insights 3.4): the meter threshold, a context window per model,
//! and how long and how large the baselines may grow. Each lives in the plugin's settings; a
//! missing or bad value reads as its default.

use crate::baseline;
use crate::insights::{DEFAULT_THRESHOLD, DEFAULT_WINDOW};
use serde_json::{Map, Value, json};
use std::time::Duration;
use terminescu_plugin_api::baseline as baseline_store;
use terminescu_plugin_api::{PluginCx, PluginError};

pub const THRESHOLD: &str = "meterThreshold";
pub const WINDOWS: &str = "contextWindows";
pub const CAP_MB: &str = "baselineCapMb";
pub const KEEP_DAYS: &str = "baselineDays";
/// The largest file a baseline keeps, in MB (polish-claude-insights D3).
pub const FILE_MB: &str = "baselineFileMb";

const MB: u64 = 1024 * 1024;
const DAY: u64 = 24 * 3600;
const CAP_RANGE: (u64, u64) = (10, 100_000);
const DAYS_RANGE: (u64, u64) = (1, 365);
const FILE_RANGE: (u64, u64) = (1, baseline_store::MAX_SIZE_CAP / MB);
const THRESHOLD_RANGE: (u64, u64) = (1, 100);

/// Every number setting with its range, as the contract fixture records them.
pub const RANGES: &[(&str, (u64, u64))] = &[
    (THRESHOLD, THRESHOLD_RANGE),
    (CAP_MB, CAP_RANGE),
    (KEEP_DAYS, DAYS_RANGE),
    (FILE_MB, FILE_RANGE),
];
pub const WINDOW_RANGE: (u64, u64) = (1_000, 10_000_000);
/// Longest model id kept as a window's key.
const MODEL_CHARS: usize = 100;

fn within(v: Option<Value>, (lo, hi): (u64, u64)) -> Option<u64> {
    v.and_then(|v| v.as_u64()).filter(|n| (lo..=hi).contains(n))
}

/// The store's cap in bytes and how long an ended session's baseline stays.
pub fn limits(cx: &dyn PluginCx) -> (Duration, u64) {
    let days = within(cx.setting(KEEP_DAYS), DAYS_RANGE);
    let cap = within(cx.setting(CAP_MB), CAP_RANGE);
    (
        days.map_or(baseline::KEEP, |d| Duration::from_secs(d * DAY)),
        cap.map_or(baseline::CAP_BYTES, |c| c * MB),
    )
}

/// The per-file limit in bytes, or `None` for the default.
pub fn file_cap(cx: &dyn PluginCx) -> Option<u64> {
    within(cx.setting(FILE_MB), FILE_RANGE).map(|n| n * MB)
}

/// What the page shows: each value as it now reads, and the defaults beside them.
pub fn view(cx: &dyn PluginCx) -> Value {
    let (keep, cap) = limits(cx);
    let threshold = cx
        .setting(THRESHOLD)
        .and_then(|v| v.as_u64())
        .filter(|t| (1..=100).contains(t))
        .unwrap_or(DEFAULT_THRESHOLD);
    let windows = cx
        .setting(WINDOWS)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    json!({
        THRESHOLD: threshold,
        WINDOWS: windows,
        CAP_MB: cap / MB,
        KEEP_DAYS: keep.as_secs() / DAY,
        FILE_MB: file_cap(cx).unwrap_or(baseline_store::SIZE_CAP) / MB,
        "defaults": {
            THRESHOLD: DEFAULT_THRESHOLD,
            "window": DEFAULT_WINDOW,
            CAP_MB: baseline::CAP_BYTES / MB,
            KEEP_DAYS: baseline::KEEP.as_secs() / DAY,
            FILE_MB: baseline_store::SIZE_CAP / MB,
        },
    })
}

fn number(value: &Value, what: &str, (lo, hi): (u64, u64)) -> Result<Option<Value>, PluginError> {
    match value {
        Value::Null => Ok(None),
        v => v
            .as_u64()
            .filter(|n| (lo..=hi).contains(n))
            .map(|n| Some(json!(n)))
            .ok_or_else(|| {
                PluginError::BadArgs(format!("{what} is a whole number from {lo} to {hi}"))
            }),
    }
}

/// Saves one setting; null puts back its default.
pub fn set(cx: &dyn PluginCx, key: &str, value: &Value) -> Result<(), PluginError> {
    let kept = match key {
        THRESHOLD => number(value, "the threshold", THRESHOLD_RANGE)?,
        CAP_MB => number(value, "the cap", CAP_RANGE)?,
        KEEP_DAYS => number(value, "the days", DAYS_RANGE)?,
        FILE_MB => number(value, "the file limit", FILE_RANGE)?,
        WINDOWS => match value {
            Value::Null => None,
            Value::Object(m) => {
                let mut out = Map::new();
                for (model, tokens) in m {
                    let model = model.trim();
                    if model.is_empty() || model.chars().count() > MODEL_CHARS {
                        return Err(PluginError::BadArgs(
                            "a model id is 1 to 100 characters".into(),
                        ));
                    }
                    let n = number(tokens, "a window", WINDOW_RANGE)?
                        .ok_or_else(|| PluginError::BadArgs(format!("{model} has no window")))?;
                    out.insert(model.into(), n);
                }
                (!out.is_empty()).then_some(Value::Object(out))
            }
            _ => {
                return Err(PluginError::BadArgs(
                    "the windows are {model: tokens}".into(),
                ));
            }
        },
        other => return Err(PluginError::BadArgs(format!("no setting {other}"))),
    };
    cx.set_setting(key, kept)
}
