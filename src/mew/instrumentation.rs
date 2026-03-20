use std::{
    collections::{HashMap, HashSet},
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, MutexGuard},
};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{json, Value};

use crate::mew::{
    config::{self, Config, SnipeConfig, StratsConfig, TxSettings},
    deagle::deagle::{AlgoConfig, CandidateSelectionReport},
};

const FIXED_WINDOWS_ROOT: &str = r"C:\Users\bot\quant\Vexter";
const PREFERRED_WINDOWS_ROOT: &str = r"D:\Quant\Vexter";

fn isoformat_utc(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn mask_secret(value: Option<String>) -> Option<String> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    if value.len() <= 8 {
        return Some("*".repeat(value.len()));
    }
    Some(format!("{}...{}", &value[..4], &value[value.len() - 4..]))
}

fn as_f64(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

fn pct_change(entry: Option<f64>, value: Option<f64>) -> f64 {
    let entry = entry.unwrap_or_default();
    let value = value.unwrap_or_default();
    if entry <= 0.0 {
        return 0.0;
    }
    ((value - entry) / entry) * 100.0
}

fn host_name() -> String {
    env::var("HOSTNAME")
        .or_else(|_| env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string())
        .to_ascii_lowercase()
}

fn resolve_mode() -> String {
    match config::get_mode().as_str() {
        "sim" => "sim_live".to_string(),
        "trade" => "trade_live".to_string(),
        other => other.to_string(),
    }
}

fn resolve_transport_mode(tx_settings: &TxSettings) -> String {
    let intake = if config::get_use_grpc() { "grpc" } else { "ws" };
    let tx = tx_settings.tx_strat.trim();
    if tx.is_empty() || tx == intake {
        return intake.to_string();
    }
    match tx {
        "rpc" | "swqos" => "mixed".to_string(),
        other => other.to_string(),
    }
}

fn write_json(path: &Path, payload: &impl Serialize) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(serialized) = serde_json::to_string_pretty(payload) {
        if let Ok(mut file) = fs::File::create(path) {
            let _ = file.write_all(serialized.as_bytes());
            let _ = file.write_all(b"\n");
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeLayout {
    pub output_root: PathBuf,
    pub runtime_root: String,
    pub config_dir: PathBuf,
    pub export_dir: PathBuf,
    pub raw_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub replays_dir: PathBuf,
    pub db_dir: PathBuf,
}

impl RuntimeLayout {
    pub fn new() -> Self {
        let runtime_root = env::var("VEXTER_RUNTIME_ROOT").unwrap_or_else(|_| FIXED_WINDOWS_ROOT.to_string());
        let output_root = if let Ok(root) = env::var("VEXTER_OUTPUT_ROOT") {
            PathBuf::from(root)
        } else if cfg!(target_os = "windows") {
            let preferred = PathBuf::from(PREFERRED_WINDOWS_ROOT);
            if preferred.exists() {
                preferred
            } else {
                PathBuf::from(FIXED_WINDOWS_ROOT)
            }
        } else {
            PathBuf::from("dev").join("vexter_runtime")
        };

        Self {
            config_dir: output_root.join("runtime").join("mewx").join("config"),
            export_dir: output_root.join("runtime").join("mewx").join("export"),
            raw_dir: output_root.join("data").join("raw").join("mewx"),
            logs_dir: output_root.join("data").join("logs").join("mewx"),
            replays_dir: output_root.join("data").join("replays").join("mewx"),
            db_dir: output_root.join("data").join("postgres").join("mewx"),
            output_root,
            runtime_root,
        }
    }

    pub fn ensure(&self) {
        for dir in [
            &self.config_dir,
            &self.export_dir,
            &self.raw_dir,
            &self.logs_dir,
            &self.replays_dir,
            &self.db_dir,
        ] {
            let _ = fs::create_dir_all(dir);
        }
    }

    pub fn metadata(&self) -> Value {
        json!({
            "runtime_root": self.runtime_root,
            "config_dir": format!(r"{}\runtime\mewx\config", self.runtime_root),
            "export_dir": format!(r"{}\runtime\mewx\export", self.runtime_root),
            "raw_events_dir": format!(r"{}\data\raw\mewx", self.runtime_root),
            "logs_dir": format!(r"{}\data\logs\mewx", self.runtime_root),
            "replays_dir": format!(r"{}\data\replays\mewx", self.runtime_root),
            "db_exports_dir": format!(r"{}\data\postgres\mewx", self.runtime_root),
        })
    }
}

#[derive(Debug, Clone)]
struct AttemptState {
    started_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct SessionState {
    session_id: String,
    mint: String,
    creator: Option<String>,
    market: Option<String>,
    candidate_source: Option<String>,
    pool_id: Option<String>,
    observed_at: Option<DateTime<Utc>>,
    observed_open_price: Option<f64>,
    signal_at: Option<DateTime<Utc>>,
    signal_price: Option<f64>,
    signal_slot: Option<u64>,
    entry_fill_at: Option<DateTime<Utc>>,
    entry_fill_price: Option<f64>,
    exit_reason: Option<String>,
    exit_signal_at: Option<DateTime<Utc>>,
    exit_signal_price: Option<f64>,
    exit_fill_at: Option<DateTime<Utc>>,
    exit_fill_price: Option<f64>,
    peak_price: Option<f64>,
    peak_price_at: Option<DateTime<Utc>>,
    low_price: Option<f64>,
    attempts: HashMap<usize, AttemptState>,
    closed: bool,
}

impl SessionState {
    fn new(mint: &str, started_at: DateTime<Utc>) -> Self {
        Self {
            session_id: format!("{}-{}", mint, started_at.timestamp_millis()),
            mint: mint.to_string(),
            creator: None,
            market: None,
            candidate_source: None,
            pool_id: None,
            observed_at: None,
            observed_open_price: None,
            signal_at: None,
            signal_price: None,
            signal_slot: None,
            entry_fill_at: None,
            entry_fill_price: None,
            exit_reason: None,
            exit_signal_at: None,
            exit_signal_price: None,
            exit_fill_at: None,
            exit_fill_price: None,
            peak_price: None,
            peak_price_at: None,
            low_price: None,
            attempts: HashMap::new(),
            closed: false,
        }
    }
}

struct Inner {
    layout: RuntimeLayout,
    run_id: String,
    source_commit: String,
    mode: String,
    transport_mode: String,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    host_role: String,
    event_path: PathBuf,
    state_path: PathBuf,
    config_path: Option<PathBuf>,
    candidate_snapshot_paths: Vec<PathBuf>,
    session_summary_paths: Vec<PathBuf>,
    event_counts: HashMap<String, usize>,
    event_index: usize,
    attempt_counters: HashMap<String, usize>,
    sessions: HashMap<String, SessionState>,
    finalized: bool,
}

#[derive(Clone)]
pub struct MewInstrumentation {
    inner: Arc<Mutex<Inner>>,
}

impl MewInstrumentation {
    pub fn new(source_repo_root: impl AsRef<Path>) -> Self {
        let layout = RuntimeLayout::new();
        layout.ensure();
        let source_commit = env::var("MEWX_SOURCE_COMMIT")
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                Command::new("git")
                    .args(["rev-parse", "HEAD"])
                    .current_dir(source_repo_root.as_ref())
                    .output()
                    .ok()
                    .and_then(|output| String::from_utf8(output.stdout).ok())
                    .map(|stdout| stdout.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "unknown".to_string())
            });
        let tx_settings = config::get_tx_settings();
        let run_id = env::var("VEXTER_RUN_ID")
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("mewx-{}-{}", host_name(), Utc::now().format("%Y%m%dT%H%M%SZ")));
        let state_path = layout.export_dir.join(format!("{run_id}.state.json"));
        let inner = Inner {
            event_path: layout.raw_dir.join(format!("{run_id}.ndjson")),
            layout,
            run_id,
            source_commit,
            mode: resolve_mode(),
            transport_mode: resolve_transport_mode(&tx_settings),
            started_at: Utc::now(),
            ended_at: None,
            host_role: "windows_runtime".to_string(),
            state_path,
            config_path: None,
            candidate_snapshot_paths: Vec::new(),
            session_summary_paths: Vec::new(),
            event_counts: HashMap::new(),
            event_index: 0,
            attempt_counters: HashMap::new(),
            sessions: HashMap::new(),
            finalized: false,
        };
        let this = Self { inner: Arc::new(Mutex::new(inner)) };
        let mut guard = this.lock();
        Self::write_state_locked(&mut guard, "initialized");
        drop(guard);
        this
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("instrumentation mutex poisoned")
    }

    fn ensure_session_locked<'a>(
        inner: &'a mut Inner,
        mint_id: &str,
        creator: Option<&str>,
        market: Option<&str>,
        candidate_source: Option<&str>,
        pool_id: Option<&str>,
    ) -> &'a mut SessionState {
        let session = inner
            .sessions
            .entry(mint_id.to_string())
            .or_insert_with(|| SessionState::new(mint_id, inner.started_at));
        if let Some(creator) = creator {
            session.creator = Some(creator.to_string());
        }
        if let Some(market) = market {
            session.market = Some(market.to_string());
        }
        if let Some(candidate_source) = candidate_source {
            session.candidate_source = Some(candidate_source.to_string());
        }
        if let Some(pool_id) = pool_id {
            session.pool_id = Some(pool_id.to_string());
        }
        session
    }

    fn write_state_locked(inner: &mut Inner, status: &str) {
        write_json(
            &inner.state_path,
            &json!({
                "run_id": inner.run_id,
                "status": status,
                "source_system": "mewx",
                "source_commit": inner.source_commit,
                "mode": inner.mode,
                "transport_mode": inner.transport_mode,
                "host_role": inner.host_role,
                "started_at_utc": isoformat_utc(inner.started_at),
                "ended_at_utc": inner.ended_at.map(isoformat_utc),
                "paths": {
                    "runtime_layout": inner.layout.metadata(),
                    "raw_events_file": inner.event_path,
                    "config_snapshot": inner.config_path,
                    "candidate_snapshots": inner.candidate_snapshot_paths,
                    "session_summaries": inner.session_summary_paths,
                },
                "event_counts": inner.event_counts,
            }),
        );
    }

    fn emit_locked(
        inner: &mut Inner,
        event_type: &str,
        payload: Value,
        session_id: Option<&str>,
        mint: Option<&str>,
        creator: Option<&str>,
        ts: DateTime<Utc>,
    ) -> Option<String> {
        inner.event_index += 1;
        *inner.event_counts.entry(event_type.to_string()).or_insert(0) += 1;

        let mut event = json!({
            "run_id": inner.run_id,
            "event_id": format!("{}-{:06}", inner.run_id, inner.event_index),
            "event_type": event_type,
            "ts_utc": isoformat_utc(ts),
            "source_system": "mewx",
            "source_commit": inner.source_commit,
            "mode": inner.mode,
            "transport_mode": inner.transport_mode,
            "payload": payload,
        });
        if let Some(session_id) = session_id {
            event["session_id"] = json!(session_id);
        }
        if let Some(mint) = mint {
            event["mint"] = json!(mint);
        }
        if let Some(creator) = creator {
            event["creator"] = json!(creator);
        }

        if let Some(parent) = inner.event_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut handle) = OpenOptions::new().create(true).append(true).open(&inner.event_path) {
            let _ = writeln!(handle, "{}", event);
        }

        Self::write_state_locked(inner, "running");
        event.get("event_id").and_then(Value::as_str).map(ToOwned::to_owned)
    }

    fn export_session_summary_locked(inner: &mut Inner, session: &SessionState) {
        let entry_price = session.entry_fill_price.or(session.signal_price).or(session.observed_open_price);
        let exit_price = session.exit_fill_price.or(session.exit_signal_price).or(entry_price);
        let peak_price = session.peak_price.or(entry_price);
        let low_price = session.low_price.or(entry_price);
        let fill_time = session.entry_fill_at.or(session.signal_at).or(session.observed_at).unwrap_or(inner.started_at);
        let closed_time = session.exit_fill_at.or(session.exit_signal_at).unwrap_or(inner.ended_at.unwrap_or_else(Utc::now));
        let peak_time = session.peak_price_at.unwrap_or(fill_time);

        let sessions_dir = inner.export_dir().join("sessions");
        let path = sessions_dir.join(format!("{}.json", session.session_id));
        write_json(
            &path,
            &json!({
                "run_id": inner.run_id,
                "session_id": session.session_id,
                "mint": session.mint,
                "creator": session.creator,
                "candidate_source": session.candidate_source,
                "market": session.market,
                "pool_id": session.pool_id,
                "entry_price": entry_price,
                "exit_price": exit_price,
                "realized_return_pct": pct_change(entry_price, exit_price),
                "mfe_pct": pct_change(entry_price, peak_price),
                "mae_pct": pct_change(entry_price, low_price),
                "time_to_peak_ms": (peak_time - fill_time).num_milliseconds(),
                "session_duration_ms": (closed_time - fill_time).num_milliseconds(),
                "closed_at_utc": isoformat_utc(closed_time),
                "exit_reason": session.exit_reason,
            }),
        );
        inner.session_summary_paths.push(path);
    }

    pub fn export_masked_config(
        &self,
        config_payload: &Config,
        algo_config: &AlgoConfig,
        snipe_config: &SnipeConfig,
        strats_config: &StratsConfig,
        tx_settings: &TxSettings,
        initial_source_counts: &HashMap<String, usize>,
    ) -> Option<PathBuf> {
        let mut inner = self.lock();
        let path = inner.layout.config_dir.join(format!("{}.config.json", inner.run_id));
        write_json(
            &path,
            &json!({
                "run_id": inner.run_id,
                "captured_at_utc": isoformat_utc(Utc::now()),
                "source_commit": inner.source_commit,
                "paths": inner.layout.metadata(),
                "config": {
                    "mode": inner.mode,
                    "transport_mode": inner.transport_mode,
                    "rpc_url": mask_secret(Some(config_payload.rpc_url.clone())),
                    "ws_url": mask_secret(Some(config_payload.ws_url.clone())),
                    "grpc_url": mask_secret(config_payload.grpc_url.clone()),
                    "grpc_token": mask_secret(config_payload.grpc_token.clone()),
                    "nonce_account": config_payload.nonce_account.clone(),
                },
                "algorithm": {
                    "limit": algo_config.limit,
                    "min_mints": algo_config.min_mints,
                    "min_deagle_sol": algo_config.min_deagle_sol,
                    "use_vol_creators": algo_config.use_vc,
                    "use_deagles": algo_config.use_deagles,
                    "min_buys": algo_config.min_buys,
                    "min_volume": algo_config.min_volume,
                    "grand_chillers": algo_config.grand_chillers.as_ref().map(|gc| json!({
                        "use_gc": gc.use_gc,
                        "min_hmc": gc.min_hmc,
                        "min_mints": gc.min_mints,
                        "min_buys": gc.min_buys,
                    })),
                },
                "snipe": snipe_config,
                "strategies": strats_config,
                "tx_settings": {
                    "tx_strat": tx_settings.tx_strat.clone(),
                    "tip_lamports": tx_settings.tip_lamports,
                    "nextblock_key": mask_secret(Some(tx_settings.nextblock_key.clone())),
                    "zero_slot_key": mask_secret(Some(tx_settings.zero_slot_key.clone())),
                    "temporal_key": mask_secret(Some(tx_settings.temporal_key.clone())),
                    "blox_key": mask_secret(Some(tx_settings.blox_key.clone())),
                },
                "environment": {
                    "PRIVATE_KEY": mask_secret(env::var("PRIVATE_KEY").ok()),
                    "DB_URL": mask_secret(env::var("DB_URL").ok()),
                    "REGIONS": env::var("REGIONS").ok(),
                },
                "initial_source_counts": initial_source_counts,
            }),
        );
        inner.config_path = Some(path.clone());
        Self::write_state_locked(&mut inner, "config_exported");
        Some(path)
    }

    pub fn export_candidate_snapshot(
        &self,
        label: &str,
        report: &CandidateSelectionReport,
        added_keys: &[String],
        removed_keys: &[String],
    ) -> Option<PathBuf> {
        let mut inner = self.lock();
        let filename = format!("{}.candidates.{}.json", inner.run_id, label.replace(' ', "_"));
        let path = inner.layout.export_dir.join("candidates").join(filename);
        write_json(
            &path,
            &json!({
                "run_id": inner.run_id,
                "captured_at_utc": isoformat_utc(Utc::now()),
                "source_commit": inner.source_commit,
                "label": label,
                "source_counts": report.source_counts,
                "added_keys": added_keys,
                "removed_keys": removed_keys,
                "observations": report.observations,
            }),
        );
        inner.candidate_snapshot_paths.push(path.clone());
        Self::write_state_locked(&mut inner, "candidate_snapshot_exported");
        Some(path)
    }

    pub fn emit_creator_candidates(
        &self,
        report: &CandidateSelectionReport,
        reason_override: &str,
        filter_keys: Option<&HashSet<String>>,
    ) {
        let mut inner = self.lock();
        for observation in &report.observations {
            let key = format!("{}::{}", observation.source_label, observation.creator);
            if filter_keys.is_some_and(|keys| !keys.contains(&key)) {
                continue;
            }
            let payload = json!({
                "candidate_source": observation.source_label,
                "score_components": observation.score_components,
                "score_total": observation.score_total,
                "cohort_size": observation.cohort_size,
                "reason": reason_override,
                "source_reason": observation.reason,
            });
            let _ = Self::emit_locked(
                &mut inner,
                "creator_candidate",
                payload,
                None,
                None,
                Some(&observation.creator),
                Utc::now(),
            );
        }
    }

    pub fn observe_mint(
        &self,
        mint_id: &str,
        creator: &str,
        market: &str,
        candidate_source: Option<&str>,
        pool_id: &str,
        first_seen_slot: u64,
        open_price: f64,
        is_migrated: bool,
        extra: Value,
    ) -> String {
        let mut inner = self.lock();
        let session = Self::ensure_session_locked(&mut inner, mint_id, Some(creator), Some(market), candidate_source, Some(pool_id));
        let session_id = session.session_id.clone();
        let already_observed = session.observed_at.is_some();
        if !already_observed {
            let now = Utc::now();
            session.observed_at = Some(now);
            session.observed_open_price = Some(open_price);
            let mut payload = json!({
                "market": market,
                "pool_id": pool_id,
                "first_seen_slot": first_seen_slot,
                "open_price": as_f64(open_price),
                "is_migrated": is_migrated,
                "candidate_source": candidate_source,
            });
            if let (Some(target), Some(extra)) = (payload.as_object_mut(), extra.as_object()) {
                for (key, value) in extra {
                    target.insert(key.clone(), value.clone());
                }
            }
            let _ = Self::emit_locked(
                &mut inner,
                "mint_observed",
                payload,
                Some(&session_id),
                Some(mint_id),
                Some(creator),
                now,
            );
        }
        session_id
    }

    pub fn record_candidate_rejected(
        &self,
        mint_id: &str,
        creator: &str,
        candidate_source: &str,
        reject_reason: &str,
        gate_name: &str,
        details: Option<Value>,
    ) {
        let mut inner = self.lock();
        let session = Self::ensure_session_locked(&mut inner, mint_id, Some(creator), None, Some(candidate_source), None);
        let session_id = session.session_id.clone();
        let mut payload = json!({
            "candidate_source": candidate_source,
            "reject_reason": reject_reason,
            "gate_name": gate_name,
        });
        if let (Some(target), Some(details)) = (payload.as_object_mut(), details.and_then(|value| value.as_object().cloned())) {
            for (key, value) in details {
                target.insert(key, value);
            }
        }
        let _ = Self::emit_locked(
            &mut inner,
            "candidate_rejected",
            payload,
            Some(&session_id),
            Some(mint_id),
            Some(creator),
            Utc::now(),
        );
    }

    pub fn record_entry_signal(
        &self,
        mint_id: &str,
        creator: &str,
        candidate_source: &str,
        market: &str,
        signal_reason: &str,
        signal_price: f64,
        signal_slot: u64,
        pool_id: &str,
    ) {
        let mut inner = self.lock();
        let session = Self::ensure_session_locked(
            &mut inner,
            mint_id,
            Some(creator),
            Some(market),
            Some(candidate_source),
            Some(pool_id),
        );
        let session_id = session.session_id.clone();
        if session.signal_at.is_some() {
            return;
        }
        let signal_time = Utc::now();
        let observed_at = session.observed_at.unwrap_or(signal_time);
        session.signal_at = Some(signal_time);
        session.signal_price = Some(signal_price);
        session.signal_slot = Some(signal_slot);
        let signal_latency_ms = (signal_time - observed_at).num_milliseconds().max(0);
        let payload = json!({
            "candidate_source": candidate_source,
            "market": market,
            "signal_reason": signal_reason,
            "signal_price": as_f64(signal_price),
            "signal_slot": signal_slot,
            "signal_latency_ms": signal_latency_ms,
            "pool_id": pool_id,
        });
        let _ = Self::emit_locked(
            &mut inner,
            "entry_signal",
            payload,
            Some(&session_id),
            Some(mint_id),
            Some(creator),
            signal_time,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_entry_attempt(
        &self,
        mint_id: &str,
        creator: &str,
        candidate_source: &str,
        market: &str,
        route: &str,
        quote_price: f64,
        expected_tokens_out: f64,
        slippage_bps: u64,
        wallet_available_before: f64,
        tx_strategy: &str,
        transport_path: &str,
        priority_fee_lamports: u64,
        tip_lamports: u64,
        retries: u32,
    ) -> usize {
        let mut inner = self.lock();
        let attempt_index = inner.attempt_counters.get(mint_id).copied().unwrap_or(0) + 1;
        inner.attempt_counters.insert(mint_id.to_string(), attempt_index);
        let session = Self::ensure_session_locked(&mut inner, mint_id, Some(creator), Some(market), Some(candidate_source), None);
        let session_id = session.session_id.clone();
        session
            .attempts
            .insert(attempt_index, AttemptState { started_at: Utc::now() });
        let payload = json!({
            "attempt_index": attempt_index,
            "market": market,
            "route": route,
            "quote_price": as_f64(quote_price),
            "expected_tokens_out": as_f64(expected_tokens_out),
            "slippage_bps": slippage_bps,
            "wallet_available_before": as_f64(wallet_available_before),
            "tx_strategy": tx_strategy,
            "transport_path": transport_path,
            "candidate_source": candidate_source,
            "priority_fee_lamports": priority_fee_lamports,
            "tip_lamports": tip_lamports,
            "retries": retries,
        });
        let _ = Self::emit_locked(
            &mut inner,
            "entry_attempt",
            payload,
            Some(&session_id),
            Some(mint_id),
            Some(creator),
            Utc::now(),
        );
        attempt_index
    }

    pub fn record_entry_rejected(
        &self,
        mint_id: &str,
        creator: &str,
        reject_reason: &str,
        tx_signature: &str,
        attempt_index: usize,
        details: Option<Value>,
    ) {
        let mut inner = self.lock();
        let session = Self::ensure_session_locked(&mut inner, mint_id, Some(creator), None, None, None);
        let session_id = session.session_id.clone();
        let mut payload = json!({
            "attempt_index": attempt_index.max(1),
            "reject_reason": reject_reason,
            "tx_signature": tx_signature,
        });
        if let (Some(target), Some(details)) = (payload.as_object_mut(), details.and_then(|value| value.as_object().cloned())) {
            for (key, value) in details {
                target.insert(key, value);
            }
        }
        let _ = Self::emit_locked(
            &mut inner,
            "entry_rejected",
            payload,
            Some(&session_id),
            Some(mint_id),
            Some(creator),
            Utc::now(),
        );
    }

    pub fn record_entry_fill(
        &self,
        mint_id: &str,
        creator: &str,
        fill_price: f64,
        fill_qty: f64,
        tx_signature: &str,
        wallet_balance_after: f64,
        confirmation_source: &str,
        attempt_index: usize,
    ) {
        let mut inner = self.lock();
        let session = Self::ensure_session_locked(&mut inner, mint_id, Some(creator), None, None, None);
        let session_id = session.session_id.clone();
        let fill_time = Utc::now();
        let started_at = session
            .attempts
            .get(&attempt_index)
            .map(|attempt| attempt.started_at)
            .or(session.signal_at)
            .unwrap_or(fill_time);
        session.entry_fill_at = Some(fill_time);
        session.entry_fill_price = Some(fill_price);
        session.peak_price = session.peak_price.or(Some(fill_price));
        session.peak_price_at = session.peak_price_at.or(Some(fill_time));
        session.low_price = session.low_price.or(Some(fill_price));
        let payload = json!({
            "attempt_index": attempt_index.max(1),
            "fill_price": as_f64(fill_price),
            "fill_qty": as_f64(fill_qty),
            "fill_latency_ms": (fill_time - started_at).num_milliseconds().max(0),
            "tx_signature": tx_signature,
            "wallet_balance_after": as_f64(wallet_balance_after),
            "confirmation_source": confirmation_source,
        });
        let _ = Self::emit_locked(
            &mut inner,
            "entry_fill",
            payload,
            Some(&session_id),
            Some(mint_id),
            Some(creator),
            fill_time,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_session_update(
        &self,
        mint_id: &str,
        creator: &str,
        price: f64,
        highest_price: f64,
        buys: i64,
        sells: i64,
        liquidity: f64,
        state_reason: &str,
        creator_token_amount: f64,
        creator_sold: bool,
        txns_in_zero: i64,
        txns_in_n: i64,
    ) {
        let mut inner = self.lock();
        let session = Self::ensure_session_locked(&mut inner, mint_id, Some(creator), None, None, None);
        let session_id = session.session_id.clone();
        let now = Utc::now();
        let price = as_f64(price);
        let highest_price = as_f64(highest_price.max(price));
        if session.peak_price.is_none() || highest_price >= session.peak_price.unwrap_or_default() {
            session.peak_price = Some(highest_price);
            session.peak_price_at = Some(now);
        }
        if session.low_price.is_none() || price <= session.low_price.unwrap_or(price) {
            session.low_price = Some(price);
        }
        let entry_basis = session.entry_fill_price.or(session.signal_price).or(session.observed_open_price);
        let payload = json!({
            "price": price,
            "highest_price": highest_price,
            "buys": buys,
            "sells": sells,
            "liquidity": as_f64(liquidity),
            "mfe_pct": pct_change(entry_basis, session.peak_price),
            "mae_pct": pct_change(entry_basis, session.low_price),
            "state_reason": state_reason,
            "creator_token_amount": as_f64(creator_token_amount),
            "creator_sold": creator_sold,
            "txns_in_zero": txns_in_zero,
            "txns_in_n": txns_in_n,
        });
        let _ = Self::emit_locked(
            &mut inner,
            "session_update",
            payload,
            Some(&session_id),
            Some(mint_id),
            Some(creator),
            now,
        );
    }

    pub fn record_exit_signal(
        &self,
        mint_id: &str,
        creator: &str,
        exit_reason: &str,
        signal_price: f64,
        theoretical_best_price: f64,
    ) {
        let mut inner = self.lock();
        let session = Self::ensure_session_locked(&mut inner, mint_id, Some(creator), None, None, None);
        let session_id = session.session_id.clone();
        if session.exit_signal_at.is_some() {
            return;
        }
        let now = Utc::now();
        session.exit_reason = Some(exit_reason.to_string());
        session.exit_signal_at = Some(now);
        session.exit_signal_price = Some(signal_price);
        let payload = json!({
            "exit_reason": exit_reason,
            "signal_price": as_f64(signal_price),
            "theoretical_best_price": as_f64(theoretical_best_price),
            "realized_vs_peak_gap_pct": pct_change(Some(signal_price), Some(theoretical_best_price)),
        });
        let _ = Self::emit_locked(
            &mut inner,
            "exit_signal",
            payload,
            Some(&session_id),
            Some(mint_id),
            Some(creator),
            now,
        );
    }

    pub fn record_exit_fill(
        &self,
        mint_id: &str,
        creator: &str,
        fill_price: f64,
        fill_qty: f64,
        tx_signature: &str,
        exit_reason: &str,
        wallet_balance_after: f64,
    ) {
        let mut inner = self.lock();
        let session = Self::ensure_session_locked(&mut inner, mint_id, Some(creator), None, None, None);
        let session_id = session.session_id.clone();
        let now = Utc::now();
        session.exit_fill_at = Some(now);
        session.exit_fill_price = Some(fill_price);
        let signal_time = session.exit_signal_at.unwrap_or(now);
        let payload = json!({
            "fill_price": as_f64(fill_price),
            "fill_qty": as_f64(fill_qty),
            "fill_latency_ms": (now - signal_time).num_milliseconds().max(0),
            "tx_signature": tx_signature,
            "exit_reason": exit_reason,
            "wallet_balance_after": as_f64(wallet_balance_after),
        });
        let _ = Self::emit_locked(
            &mut inner,
            "exit_fill",
            payload,
            Some(&session_id),
            Some(mint_id),
            Some(creator),
            now,
        );
    }

    pub fn record_position_closed(
        &self,
        mint_id: &str,
        creator: &str,
        stale_position_flag: bool,
    ) {
        let mut inner = self.lock();
        let session = Self::ensure_session_locked(&mut inner, mint_id, Some(creator), None, None, None).clone();
        if session.closed {
            return;
        }
        let entry_price = session.entry_fill_price.or(session.signal_price).or(session.observed_open_price);
        let exit_price = session.exit_fill_price.or(session.exit_signal_price).or(entry_price);
        let peak_price = session.peak_price.or(entry_price);
        let low_price = session.low_price.or(entry_price);
        let fill_time = session.entry_fill_at.or(session.signal_at).or(session.observed_at).unwrap_or(inner.started_at);
        let closed_time = session.exit_fill_at.or(session.exit_signal_at).unwrap_or_else(Utc::now);
        let peak_time = session.peak_price_at.unwrap_or(fill_time);
        let payload = json!({
            "entry_price": entry_price,
            "exit_price": exit_price,
            "realized_return_pct": pct_change(entry_price, exit_price),
            "mfe_pct": pct_change(entry_price, peak_price),
            "mae_pct": pct_change(entry_price, low_price),
            "time_to_peak_ms": (peak_time - fill_time).num_milliseconds(),
            "session_duration_ms": (closed_time - fill_time).num_milliseconds(),
            "stale_position_flag": stale_position_flag,
        });
        let _ = Self::emit_locked(
            &mut inner,
            "position_closed",
            payload,
            Some(&session.session_id),
            Some(mint_id),
            Some(creator),
            closed_time,
        );
        if let Some(existing) = inner.sessions.get_mut(mint_id) {
            existing.closed = true;
        }
        Self::export_session_summary_locked(&mut inner, &session);
        Self::write_state_locked(&mut inner, "session_closed");
    }

    pub fn finalize(&self) {
        let mut inner = self.lock();
        if inner.finalized {
            return;
        }
        inner.finalized = true;
        inner.ended_at = Some(Utc::now());
        let event_count_before_summary: usize = inner.event_counts.values().sum();
        let payload = json!({
            "candidate_count": inner.event_counts.get("creator_candidate").copied().unwrap_or_default(),
            "entry_attempt_count": inner.event_counts.get("entry_attempt").copied().unwrap_or_default(),
            "entry_fill_count": inner.event_counts.get("entry_fill").copied().unwrap_or_default(),
            "position_closed_count": inner.event_counts.get("position_closed").copied().unwrap_or_default(),
            "event_count": event_count_before_summary + 1,
            "replayable": inner.config_path.is_some() && !inner.candidate_snapshot_paths.is_empty(),
        });
        let ended_at = inner.ended_at.unwrap_or_else(Utc::now);
        let _ = Self::emit_locked(&mut inner, "run_summary", payload, None, None, None, ended_at);
        Self::write_state_locked(&mut inner, "completed");
    }
}

impl Inner {
    fn export_dir(&self) -> PathBuf {
        self.layout.export_dir.clone()
    }
}

impl Drop for MewInstrumentation {
    fn drop(&mut self) {
        self.finalize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn instrumentation_writes_expected_artifacts() {
        let tmp = tempdir().unwrap();
        unsafe {
            env::set_var("VEXTER_OUTPUT_ROOT", tmp.path());
            env::set_var("VEXTER_RUNTIME_ROOT", FIXED_WINDOWS_ROOT);
            env::set_var("VEXTER_RUN_ID", "mewx-test-run");
            env::set_var("MEWX_SOURCE_COMMIT", "deadbeef");
            env::set_var("MODE", "sim");
            env::set_var("USE_GRPC", "false");
            env::set_var("TX_STRAT", "rpc");
            env::set_var("RPC_URL", "https://rpc.example.com");
            env::set_var("WS_URL", "wss://ws.example.com");
            env::set_var("PRIVATE_KEY", "super-secret-private-key");
            env::set_var("DB_URL", "postgres://db.example.com");
        }

        let writer = MewInstrumentation::new(tmp.path());
        let config_payload = Config::new(
            "https://rpc.example.com".to_string(),
            "secret".to_string(),
            "wss://ws.example.com".to_string(),
            Some("https://grpc.example.com".to_string()),
            Some("grpc-token".to_string()),
            Some("nonce-account".to_string()),
        );
        let algo_config = AlgoConfig {
            limit: 5,
            use_vc: true,
            use_deagles: true,
            min_volume: 10.0,
            min_buys: 2,
            min_mints: 1,
            min_deagle_sol: Some(20.0),
            grand_chillers: None,
        };
        let snipe_config = SnipeConfig {
            buy_amount: 0.01,
            slippage: 2.0,
            max_loss: 10.0,
            take_profit: 20.0,
            use_chp: true,
            chp_lower: 0.1,
            chp_upper: 0.2,
            use_tiz: true,
            tiz_lower: 3,
            tiz_upper: 21,
            max_no_activity_ms: 1000,
            max_na_on_start_ms: 250,
            min_dev_sold: 1,
        };
        let strats_config = StratsConfig {
            enable_abs: true,
            abs_min_buys: 2,
            abs_min_volume: 5.0,
            abs_n: 2,
            enable_mtd: true,
            mtd_pct: 10,
            mtd_stable_time: 10,
            mtd_max_loss: 10.0,
            mtd_take_profit: 15.0,
            mtd_min_mc: 100.0,
            enable_dbf: true,
            dbf_max_chp: 0.25,
        };
        let tx_settings = TxSettings {
            tx_strat: "rpc".to_string(),
            nextblock_key: "nextblock-key".to_string(),
            zero_slot_key: "zero-slot-key".to_string(),
            temporal_key: "temporal-key".to_string(),
            blox_key: "blox-key".to_string(),
            tip_lamports: 1234,
        };
        let initial_counts = HashMap::from([
            ("deagle".to_string(), 1usize),
            ("vol_creators".to_string(), 1usize),
        ]);
        let report = CandidateSelectionReport {
            creators: Vec::new(),
            observations: vec![crate::mew::deagle::deagle::CandidateObservation {
                creator: "creator-1".to_string(),
                source_label: "deagle".to_string(),
                score_components: json!({"deagle_sol_amount": 42.0}),
                score_total: 42.0,
                cohort_size: 1,
                reason: "deagle_transfer_threshold".to_string(),
            }],
            source_counts: initial_counts.clone(),
        };

        writer.export_masked_config(
            &config_payload,
            &algo_config,
            &snipe_config,
            &strats_config,
            &tx_settings,
            &initial_counts,
        );
        writer.export_candidate_snapshot("initial", &report, &[String::from("deagle::creator-1")], &[]);
        writer.emit_creator_candidates(&report, "initial_load", None);
        writer.observe_mint(
            "mint-1",
            "creator-1",
            "pump_fun",
            Some("deagle"),
            "pool-1",
            42,
            0.00012,
            false,
            json!({"name": "Mint One"}),
        );
        writer.record_entry_signal("mint-1", "creator-1", "deagle", "pump_fun", "matched_creator_candidate", 0.00013, 42, "pool-1");
        let attempt_index = writer.record_entry_attempt(
            "mint-1",
            "creator-1",
            "deagle",
            "pump_fun",
            "pump_fun",
            0.00013,
            1000.0,
            200,
            10.0,
            "rpc",
            "rpc",
            123,
            0,
            0,
        );
        writer.record_entry_fill("mint-1", "creator-1", 0.00014, 950.0, "buy-tx", 9.5, "transport_ack", attempt_index);
        writer.record_session_update("mint-1", "creator-1", 0.00016, 0.00018, 10, 2, 2500.0, "price_tick", 0.0, false, 1, 5);
        writer.record_exit_signal("mint-1", "creator-1", "take_profit", 0.00017, 0.00018);
        writer.record_exit_fill("mint-1", "creator-1", 0.00017, 950.0, "sell-tx", "take_profit", 10.1);
        writer.record_position_closed("mint-1", "creator-1", false);
        writer.finalize();

        let config_path = tmp.path().join("runtime/mewx/config/mewx-test-run.config.json");
        let event_path = tmp.path().join("data/raw/mewx/mewx-test-run.ndjson");
        let snapshot_path = tmp.path().join("runtime/mewx/export/candidates/mewx-test-run.candidates.initial.json");
        assert!(config_path.exists());
        assert!(event_path.exists());
        assert!(snapshot_path.exists());

        let events: Vec<Value> = fs::read_to_string(event_path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let event_types: Vec<String> = events
            .iter()
            .filter_map(|event| event.get("event_type").and_then(Value::as_str).map(ToOwned::to_owned))
            .collect();
        assert!(event_types.contains(&"creator_candidate".to_string()));
        assert!(event_types.contains(&"mint_observed".to_string()));
        assert!(event_types.contains(&"entry_fill".to_string()));
        assert!(event_types.contains(&"position_closed".to_string()));
        assert_eq!(event_types.last().map(String::as_str), Some("run_summary"));
    }
}
