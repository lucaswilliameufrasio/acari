//! Low-disk alert evaluation. Pure: the caller provides the volume numbers,
//! the recent free-space samples and the thresholds.

use serde::{Deserialize, Serialize};

const GB: u64 = 1_000_000_000;
const DAY_SECS: u64 = 86_400;

/// User-tunable thresholds (`[alerts]` in config.toml).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// Warn when free space drops below this percentage of the disk.
    #[serde(default = "default_min_free_percent")]
    pub min_free_percent: f64,
    /// Warn when free space drops below this many gigabytes.
    #[serde(default = "default_min_free_gb")]
    pub min_free_gb: u64,
    /// Warn when the disk lost at least this many gigabytes in 24 hours.
    #[serde(default = "default_fast_fill_gb")]
    pub fast_fill_gb: u64,
}

fn default_min_free_percent() -> f64 {
    15.0
}
fn default_min_free_gb() -> u64 {
    50
}
fn default_fast_fill_gb() -> u64 {
    50
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            min_free_percent: default_min_free_percent(),
            min_free_gb: default_min_free_gb(),
            fast_fill_gb: default_fast_fill_gb(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertLevel {
    Ok,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlertReason {
    LowFreePercent { percent: f64, threshold: f64 },
    LowFreeBytes { free: u64, threshold: u64 },
    FastFill { lost: u64, hours: u64 },
}

/// One free-space measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    pub at: u64,
    pub free: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskStatus {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub free_percent: f64,
    pub level: AlertLevel,
    pub reasons: Vec<AlertReason>,
}

/// Evaluate the volume against the thresholds. `history` holds earlier
/// samples (any order); only the last 24 hours count for fast-fill.
pub fn evaluate(
    total: u64,
    used: u64,
    free: u64,
    history: &[Sample],
    now: u64,
    t: &AlertThresholds,
) -> DiskStatus {
    let free_percent = if total > 0 {
        free as f64 * 100.0 / total as f64
    } else {
        100.0
    };
    let mut reasons = Vec::new();
    let mut level = AlertLevel::Ok;
    let mut raise = |l: AlertLevel| level = level.max(l);

    if t.min_free_percent > 0.0 && free_percent < t.min_free_percent {
        reasons.push(AlertReason::LowFreePercent {
            percent: free_percent,
            threshold: t.min_free_percent,
        });
        raise(if free_percent < t.min_free_percent / 2.0 {
            AlertLevel::Critical
        } else {
            AlertLevel::Warning
        });
    }
    let min_free = t.min_free_gb.saturating_mul(GB);
    if min_free > 0 && free < min_free {
        reasons.push(AlertReason::LowFreeBytes {
            free,
            threshold: min_free,
        });
        raise(if free < min_free / 2 {
            AlertLevel::Critical
        } else {
            AlertLevel::Warning
        });
    }
    let fast = t.fast_fill_gb.saturating_mul(GB);
    if fast > 0
        && let Some(oldest) = history
            .iter()
            .filter(|s| s.at <= now && now - s.at <= DAY_SECS)
            .min_by_key(|s| s.at)
        && oldest.free > free
        && oldest.free - free >= fast
    {
        reasons.push(AlertReason::FastFill {
            lost: oldest.free - free,
            hours: ((now - oldest.at) / 3600).max(1),
        });
        raise(AlertLevel::Warning);
    }

    DiskStatus {
        total,
        used,
        free,
        free_percent,
        level,
        reasons,
    }
}

/// Keep the sample list bounded: drop anything older than two days.
pub fn push_sample(history: &mut Vec<Sample>, sample: Sample) {
    history.retain(|s| sample.at >= s.at && sample.at - s.at <= 2 * DAY_SECS);
    history.push(sample);
}

#[cfg(test)]
mod tests {
    use super::*;

    const TB: u64 = 1_000 * GB;

    #[test]
    fn healthy_disk_is_ok() {
        let s = evaluate(TB, 500 * GB, 500 * GB, &[], 0, &AlertThresholds::default());
        assert_eq!(s.level, AlertLevel::Ok);
        assert!(s.reasons.is_empty());
    }

    #[test]
    fn low_percent_warns_and_very_low_is_critical() {
        let t = AlertThresholds::default();
        let warn = evaluate(TB, 880 * GB, 120 * GB, &[], 0, &t);
        assert_eq!(warn.level, AlertLevel::Warning);
        let crit = evaluate(TB, 950 * GB, 50 * GB, &[], 0, &t);
        assert_eq!(crit.level, AlertLevel::Critical);
    }

    #[test]
    fn absolute_floor_applies_to_big_disks() {
        let t = AlertThresholds {
            min_free_percent: 0.0,
            ..AlertThresholds::default()
        };
        let s = evaluate(10 * TB, 0, 40 * GB, &[], 0, &t);
        assert!(matches!(s.reasons[0], AlertReason::LowFreeBytes { .. }));
    }

    #[test]
    fn fast_fill_uses_oldest_sample_within_a_day() {
        let t = AlertThresholds::default();
        let now = 10 * DAY_SECS;
        let history = [
            Sample {
                at: now - 3 * DAY_SECS,
                free: 900 * GB,
            },
            Sample {
                at: now - 20 * 3600,
                free: 600 * GB,
            },
            Sample {
                at: now - 3600,
                free: 560 * GB,
            },
        ];
        let s = evaluate(TB, 460 * GB, 540 * GB, &history, now, &t);
        assert_eq!(s.level, AlertLevel::Warning);
        assert_eq!(
            s.reasons,
            vec![AlertReason::FastFill {
                lost: 60 * GB,
                hours: 20
            }]
        );
    }

    #[test]
    fn samples_older_than_two_days_are_dropped() {
        let mut h = vec![
            Sample { at: 0, free: 1 },
            Sample {
                at: DAY_SECS,
                free: 1,
            },
        ];
        push_sample(
            &mut h,
            Sample {
                at: 3 * DAY_SECS,
                free: 1,
            },
        );
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].at, DAY_SECS);
    }
}
