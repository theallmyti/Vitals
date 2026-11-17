//! Every call from the agent to Convex (pushes, hello, the watch subscriptions) with its timeout
//! and retry rules, plus the version check on the hello answer.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use convex::{ConvexClient, FunctionResult, Value};
use futures::StreamExt;

use crate::backlog::Backlog;
use crate::clock::now_ms;
use crate::payload::{num, text};
use crate::update;

/// A push that takes longer than this is dropped so sampling keeps going.
pub const PUSH_TIMEOUT: Duration = Duration::from_secs(5);

pub const BATCH_MAX: usize = 60;

/// A count in a Convex answer (numbers arrive as Float64, or Int64 for bigints).
pub fn count(v: Option<&Value>) -> u64 {
    match v {
        Some(Value::Float64(f)) => *f as u64,
        Some(Value::Int64(i)) => (*i).max(0) as u64,
        _ => 0,
    }
}

/// Send the oldest waiting summaries. On failure they stay in the backlog for the next minute.
pub async fn send_backlog(
    client: &mut ConvexClient,
    secret_hash: &str,
    backlog: &mut Backlog<Value>,
) {
    let batch = backlog.batch(BATCH_MAX);
    let n = batch.len();
    let mut args = BTreeMap::new();
    args.insert("secretHash".to_string(), text(secret_hash));
    args.insert("sentAt".to_string(), num(now_ms() as f64));
    args.insert("summaries".to_string(), Value::Array(batch));
    let Some(Value::Object(answer)) = call(client, "agent:pushSummaries", args).await else {
        if backlog.len() > 1 {
            say!(
                "{} summaries kept until the connection is back",
                backlog.len()
            );
        }
        return;
    };
    // Sent within 50 s of the previous call (e.g. right after a restart): retry next minute.
    if answer.get("tooSoon") == Some(&Value::Boolean(true)) {
        return;
    }
    backlog.confirm(n);
    let skipped = count(answer.get("skipped"));
    if skipped > 0 {
        warn!("{skipped} summaries were refused (impossible values or too old)");
    }
    if n > 1 {
        say!(
            "sent {n} buffered summaries ({} stored, {} still waiting)",
            count(answer.get("stored")),
            backlog.len()
        );
    }
}

/// Run a mutation with a timeout; log failures. Returns the result on success.
pub async fn call(
    client: &mut ConvexClient,
    name: &str,
    args: BTreeMap<String, Value>,
) -> Option<Value> {
    match tokio::time::timeout(PUSH_TIMEOUT, client.mutation(name, args)).await {
        Ok(Ok(FunctionResult::Value(v))) => Some(v),
        Ok(Ok(FunctionResult::ErrorMessage(msg))) => {
            warn!("{name}: {msg}");
            None
        }
        Ok(Ok(FunctionResult::ConvexError(err))) => {
            warn!("{name}: {err:?}");
            None
        }
        Ok(Err(e)) => {
            warn!("{name} failed: {e}");
            None
        }
        Err(_) => {
            warn!(
                "{name}: no answer within {}s (offline?)",
                PUSH_TIMEOUT.as_secs()
            );
            None
        }
    }
}

/// "x.y.z" as numbers; missing or non-numeric parts count as 0.
fn version_parts(v: &str) -> [u64; 3] {
    let mut out = [0; 3];
    for (slot, part) in out.iter_mut().zip(v.split('.')) {
        *slot = part.trim().parse().unwrap_or(0);
    }
    out
}

/// The newer release named in `agent:hello`'s answer, if it is newer than this agent.
pub fn newer_version(hello: &Value, own: &str) -> Option<String> {
    let Value::Object(o) = hello else { return None };
    let Some(Value::String(latest)) = o.get("latestVersion") else {
        return None;
    };
    (version_parts(latest) > version_parts(own)).then(|| latest.clone())
}

/// First connection, retried with backoff 1 s → 60 s (docs/CHANGELOG.md decisions).
pub async fn connect(url: &str) -> ConvexClient {
    let mut delay = Duration::from_secs(1);
    loop {
        match ConvexClient::new(url).await {
            Ok(c) => return c,
            Err(e) => {
                warn!(
                    "cannot connect to Convex: {e}; retrying in {}s",
                    delay.as_secs()
                );
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(60));
            }
        }
    }
}

/// Keep `viewer_until` in sync with the device's viewer flag (subscription to agent:watchViewer).
pub fn watch_viewer(mut client: ConvexClient, secret_hash: String, viewer_until: Arc<AtomicU64>) {
    tokio::spawn(async move {
        loop {
            let mut args = BTreeMap::new();
            args.insert("secretHash".to_string(), text(&secret_hash));
            match client.subscribe("agent:watchViewer", args).await {
                Ok(mut sub) => {
                    while let Some(result) = sub.next().await {
                        match result {
                            FunctionResult::Value(Value::Float64(t)) => {
                                viewer_until.store(t as u64, Ordering::Relaxed)
                            }
                            FunctionResult::Value(Value::Int64(t)) => {
                                viewer_until.store(t.max(0) as u64, Ordering::Relaxed)
                            }
                            other => warn!("agent:watchViewer: {other:?}"),
                        }
                    }
                }
                Err(e) => warn!("agent:watchViewer failed: {e}"),
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

/// Push updates: subscribe to releases:latest and install each newer agent version the backend
/// announces (the release workflow publishes it once the GitHub release is live).
/// Older backends without releases:latest just make the subscription fail; the 6-hour poll stays.
pub fn watch_release(
    mut client: ConvexClient,
    secret_hash: &str,
    data_dir: PathBuf,
    current_exe: PathBuf,
) {
    let own = env!("CARGO_PKG_VERSION");
    // Per-device spread for announced_update_delay: the device key hash, stable across restarts.
    let spread = u64::from_str_radix(secret_hash.get(..16).unwrap_or("0"), 16).unwrap_or(0);
    tokio::spawn(async move {
        let mut handled: Option<String> = None;
        loop {
            match client.subscribe("releases:latest", BTreeMap::new()).await {
                Ok(mut sub) => {
                    while let Some(result) = sub.next().await {
                        let FunctionResult::Value(Value::Object(o)) = result else {
                            warn!("releases:latest: {result:?}");
                            continue;
                        };
                        let Some(Value::String(v)) = o.get("agentVersion") else {
                            continue;
                        };
                        if handled.as_deref() == Some(v.as_str()) || !update::is_newer(v, own) {
                            continue;
                        }
                        handled = Some(v.clone());
                        say!("version {v} published (this is {own}); installing it");
                        let (v, d, e) = (v.clone(), data_dir.clone(), current_exe.clone());
                        tokio::spawn(async move {
                            update::install_announced(&v, &d, own, &e, spread).await;
                        });
                    }
                }
                Err(e) => warn!("releases:latest failed: {e}"),
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello_answer(latest: &str) -> Value {
        let mut o = BTreeMap::new();
        o.insert("latestVersion".to_string(), text(latest));
        Value::Object(o)
    }

    #[test]
    fn newer_version_compares_numbers_not_text() {
        assert_eq!(
            newer_version(&hello_answer("0.10.0"), "0.9.0").as_deref(),
            Some("0.10.0")
        );
        assert_eq!(newer_version(&hello_answer("0.1.0"), "0.1.0"), None);
        assert_eq!(newer_version(&hello_answer("0.1.0"), "0.2.0"), None);
        // Older backends do not send latestVersion.
        assert_eq!(
            newer_version(&Value::Object(BTreeMap::new()), "0.1.0"),
            None
        );
    }
}
