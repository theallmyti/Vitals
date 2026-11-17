//! Values sent to Convex: the shapes of `agent:pushLatest`, `agent:pushSummaries` and
//! `agent:hello`. Key names and which keys are left out must match docs/Database.md.

use std::collections::BTreeMap;

use convex::Value;
use sysinfo::Components;

use crate::aggregator::Summary;
use crate::battery::BatteryStats;
use crate::collectors::{self, Drive, Proc, RamDetail};
use crate::dashboard::DockerSummary;
use crate::gpu::GpuStats;
use crate::hardware;
use crate::usage::UsageStats;

pub fn num(v: f64) -> Value {
    Value::Float64(v)
}

pub fn text(s: &str) -> Value {
    Value::String(s.to_string())
}

pub fn obj<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

pub fn numbers(values: &[f64]) -> Value {
    Value::Array(values.iter().map(|v| num(*v)).collect())
}

/// Convex `gpu` object (docs/Database.md `latest.gpu`). Optional fields are left out when absent.
pub fn gpu_value(g: &GpuStats) -> Value {
    let mut o = BTreeMap::new();
    o.insert("temp".to_string(), num(f64::from(g.temp_c)));
    o.insert("util".to_string(), num(f64::from(g.util_pct)));
    o.insert("vramUsed".to_string(), num(g.vram_used_mb as f64));
    o.insert("vramTotal".to_string(), num(g.vram_total_mb as f64));
    o.insert("powerW".to_string(), num(g.power_w));
    o.insert("clock".to_string(), num(f64::from(g.clock_mhz)));
    if let Some(fan) = g.fan_pct {
        o.insert("fanPct".to_string(), num(f64::from(fan)));
    }
    Value::Object(o)
}

pub fn ram_value(r: &RamDetail) -> Value {
    obj([
        ("used", num(r.used)),
        ("free", num(r.free)),
        ("total", num(r.total)),
        ("swapUsed", num(r.swap_used)),
        ("swapTotal", num(r.swap_total)),
    ])
}

/// Convex `usage` object: active time today, boot time and the recent boot/sleep/wake events.
pub fn usage_value(u: &UsageStats) -> Value {
    let mut o = BTreeMap::new();
    o.insert(
        "activeTimeSec".to_string(),
        num(u.active_time_today_sec as f64),
    );
    if let Some(bt) = u.boot_time {
        o.insert("bootTime".to_string(), num(bt as f64));
    }
    if !u.events.is_empty() {
        let evts = u
            .events
            .iter()
            .map(|e| obj([("event", text(&e.event)), ("ts", num(e.ts as f64))]))
            .collect();
        o.insert("events".to_string(), Value::Array(evts));
    }
    Value::Object(o)
}

pub fn docker_value(d: &DockerSummary) -> Value {
    obj([
        ("running", num(f64::from(d.running))),
        ("stopped", num(f64::from(d.stopped))),
        ("errored", num(f64::from(d.error))),
    ])
}

pub fn battery_value(b: &BatteryStats) -> Value {
    let mut o = BTreeMap::new();
    o.insert("pct".to_string(), num(b.pct));
    o.insert("charging".to_string(), Value::Boolean(b.charging));
    o.insert("healthPct".to_string(), num(b.health_pct));
    o.insert("rateW".to_string(), num(b.rate_w));
    if let Some(c) = b.cycles {
        o.insert("cycles".to_string(), num(f64::from(c)));
    }
    Value::Object(o)
}

pub fn drives_live(drives: &[Drive]) -> Value {
    Value::Array(
        drives
            .iter()
            .map(|d| {
                obj([
                    ("name", text(&d.name)),
                    ("totalGB", num(d.total_gb)),
                    ("freeGB", num(d.free_gb)),
                ])
            })
            .collect(),
    )
}

pub fn drives_summary(drives: &[Drive]) -> Value {
    Value::Array(
        drives
            .iter()
            .map(|d| {
                let pct = if d.total_gb > 0.0 {
                    d.free_gb / d.total_gb * 100.0
                } else {
                    0.0
                };
                obj([
                    ("name", text(&d.name)),
                    ("freeGB", num(d.free_gb)),
                    ("freePct", num(pct)),
                ])
            })
            .collect(),
    )
}

pub fn procs_value(procs: &[Proc]) -> Value {
    Value::Array(
        procs
            .iter()
            .map(|p| obj([("name", text(&p.name)), ("mb", num(p.mb))]))
            .collect(),
    )
}

/// FR-15: what this device can report (docs/Database.md §2).
pub fn capabilities_value(
    has_gpu: bool,
    has_battery: bool,
    full_mode: bool,
    components: &Components,
    drives: &[Drive],
    cpu_model: Option<&str>,
) -> Value {
    let mut o = BTreeMap::new();
    // Fixed facts for the device page (read once; FR-15).
    if let Some(m) = cpu_model {
        o.insert("cpuModel".to_string(), text(m));
    }
    if let Some(mts) = hardware::ram_speed_mts() {
        o.insert("ramSpeedMTs".to_string(), num(f64::from(mts)));
    }
    o.insert("hasBattery".to_string(), Value::Boolean(has_battery));
    o.insert("hasNvidiaGpu".to_string(), Value::Boolean(has_gpu));
    o.insert(
        "perCoreTemps".to_string(),
        Value::Boolean(collectors::has_per_core_temps(components)),
    );
    if cfg!(windows) {
        let mode = if full_mode { "full" } else { "noDriver" };
        o.insert("windowsMode".to_string(), text(mode));
    }
    o.insert(
        "isPi".to_string(),
        Value::Boolean(collectors::is_raspberry_pi()),
    );
    o.insert(
        "drives".to_string(),
        Value::Array(
            drives
                .iter()
                .map(|d| obj([("name", text(&d.name)), ("totalGB", num(d.total_gb))]))
                .collect(),
        ),
    );
    Value::Object(o)
}

/// One minute's summary for `agent:pushSummaries`, stamped with this computer's clock (the
/// server corrects it). Optional readings are left out when absent.
pub fn summary_value(
    minute_ts: u64,
    s: &Summary,
    drives: &[Drive],
    collector_status: &BTreeMap<String, String>,
) -> Value {
    let mut a = BTreeMap::new();
    a.insert("minuteTs".to_string(), num(minute_ts as f64));
    a.insert("cpuUtilAvg".to_string(), num(s.cpu_util_avg));
    a.insert("cpuUtilMax".to_string(), num(s.cpu_util_max));
    a.insert("ramPctAvg".to_string(), num(s.ram_pct_avg));
    a.insert("ramPctMax".to_string(), num(s.ram_pct_max));
    let optional = [
        ("cpuTempAvg", s.cpu_temp_avg),
        ("cpuTempMax", s.cpu_temp_max),
        ("gpuTempMax", s.gpu_temp_max),
        ("gpuUtilAvg", s.gpu_util_avg),
        ("batteryPct", s.battery_pct),
    ];
    for (key, value) in optional {
        if let Some(v) = value {
            a.insert(key.to_string(), num(v));
        }
    }
    if !drives.is_empty() {
        a.insert("disks".to_string(), drives_summary(drives));
    }
    if !collector_status.is_empty() {
        let mut cs = BTreeMap::new();
        for (k, v) in collector_status {
            cs.insert(k.clone(), text(v));
        }
        a.insert("collectorStatus".to_string(), Value::Object(cs));
    }
    Value::Object(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_leaves_out_missing_readings() {
        let s = Summary {
            cpu_util_avg: 1.0,
            cpu_util_max: 2.0,
            ram_pct_avg: 3.0,
            ram_pct_max: 4.0,
            cpu_temp_avg: None,
            cpu_temp_max: None,
            gpu_temp_max: None,
            gpu_util_avg: None,
            battery_pct: None,
        };
        let Value::Object(o) = summary_value(60_000, &s, &[], &BTreeMap::new()) else {
            panic!("summary is not an object");
        };
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "cpuUtilAvg",
                "cpuUtilMax",
                "minuteTs",
                "ramPctAvg",
                "ramPctMax"
            ]
        );
    }

    #[test]
    fn drive_free_pct_is_zero_for_empty_drive() {
        let drives = [Drive {
            name: "X:".to_string(),
            total_gb: 0.0,
            free_gb: 0.0,
        }];
        let Value::Array(items) = drives_summary(&drives) else {
            panic!("drives are not an array");
        };
        let Value::Object(d) = &items[0] else {
            panic!("drive is not an object");
        };
        assert_eq!(d.get("freePct"), Some(&Value::Float64(0.0)));
    }
}
