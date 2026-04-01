
use dotenv::dotenv;
use std::env;
use crate::mew::writing::{cc};
use serde::{Serialize, Deserialize};
use crate::mew::deagle::deagle::{AlgoConfig, GrandChillersConfig, DexterTrustConfig};
use crate::mew::sol_hook::sol::PriorityFeeLevel;

pub struct TxSettings {
    pub tx_strat: String,
    pub nextblock_key: String,
    pub zero_slot_key: String,
    pub temporal_key: String,
    pub blox_key: String,
    pub tip_lamports: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub rpc_url: String,
    pub private_key: String,
    pub ws_url: String,
    pub grpc_url: Option<String>,
    pub grpc_token: Option<String>,
    pub nonce_account: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SnipeConfig {
    pub buy_amount: f64,
    pub slippage: f64,
    pub max_loss: f64,
    pub take_profit: f64,
    pub use_chp: bool,
    pub chp_lower: f64,
    pub chp_upper: f64,
    pub use_tiz: bool,
    pub tiz_lower: i64,
    pub tiz_upper: i64,
    pub max_no_activity_ms: i64,
    pub max_na_on_start_ms: i64,
    pub min_dev_sold: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StratsConfig {
    pub enable_abs: bool,
    pub abs_min_buys: i64,
    pub abs_min_volume: f64,
    pub abs_n: i64,
    pub enable_mtd: bool,
    pub mtd_pct: i64,
    pub mtd_stable_time: i64,
    pub mtd_max_loss: f64,
    pub mtd_take_profit: f64,
    pub mtd_min_mc: f64,
    pub enable_dbf: bool,
    pub dbf_max_chp: f64,
}

impl Config {
    pub fn new(rpc_url: String, private_key: String, ws_url: String, grpc_url: Option<String>, grpc_token: Option<String>, nonce_account: Option<String>) -> Config {
        Config {
            rpc_url,
            private_key,
            ws_url,
            grpc_url,
            grpc_token,
            nonce_account,
        }
    }

    pub fn to_string(&self) -> String {
        format!("RPC_URL: {}\nPRIVATE_KEY: {}\nWS_URL: {}\nGRPC_URL: {}\nGRPC_TOKEN: {}", self.rpc_url, self.private_key, self.ws_url, self.grpc_url.as_ref().unwrap_or(&String::new()), self.grpc_token.as_ref().unwrap_or(&String::new()))
    }

    pub fn tail_mask(s: &str, mask: usize) -> String {
        if s.len() < mask {
            return s.to_string();
        }
        s.chars().take(s.len() - 8).collect::<String>()
    }
    
    pub fn to_string_masked(&self) -> String {
        let mask = 8;
        format!("RPC_URL: {}✕✕✕, PRIVATE_KEY: {}✕✕✕, WS_URL: {}✕✕✕, GRPC_URL: {}✕✕✕, GRPC_TOKEN: {}✕✕✕, Priority Fee Level: {:?}", Self::tail_mask(&self.rpc_url, mask), Self::tail_mask(&self.private_key, mask), Self::tail_mask(&self.ws_url, mask), Self::tail_mask(self.grpc_url.as_ref().unwrap_or(&String::new()), mask), Self::tail_mask(self.grpc_token.as_ref().unwrap_or(&String::new()), mask), get_priority_fee_lvl())
    }
}

pub fn load_env() {
    dotenv().ok();
}

pub fn check_empty_var(var: String, key: &str) -> String {
    if var.is_empty() {
        panic!("{}{}{} is invalid / not set{}", cc::BOLD, cc::RED, key, cc::RESET);
    }
    var
}

pub fn get_priority_fee_lvl() -> PriorityFeeLevel {
    let key: &'static str = "PRIORITY_FEE_LVL";
    match env::var(key).ok().unwrap_or_default().as_str() {
        "low" => PriorityFeeLevel::Low,
        "medium" => PriorityFeeLevel::Medium,
        "high" => PriorityFeeLevel::High,
        "turbo" => PriorityFeeLevel::Turbo,
        "max" => PriorityFeeLevel::Max,
        _ => PriorityFeeLevel::Medium,
    }
}

pub fn get_db_url() -> String {
    let key: &'static str = "DB_URL";
    check_empty_var(env::var(key).unwrap_or_else(|_| panic!("{} is invalid / not set", key)), key)
}

pub fn get_use_grpc() -> bool {
    let key: &'static str = "USE_GRPC";
    env::var(key).ok().unwrap_or_default().parse().unwrap_or(false)
}

pub fn get_max_tokens_at_once() -> i64 {
    let key: &'static str = "MAX_TOKENS_AT_ONCE";
    env::var(key).ok().unwrap_or_default().parse().unwrap_or(10)
}

pub fn get_mode() -> String {
    let key: &'static str = "MODE";
    check_empty_var(env::var(key).unwrap_or_else(|_| panic!("{} is invalid / not set", key)), key)
}

pub fn get_use_regions() -> bool {
    let key: &'static str = "USE_REGIONS";
    env::var(key).ok().unwrap_or_default().parse().unwrap_or(false)
}

pub fn get_rpc_url() -> String {
    let key: &'static str = "RPC_URL";
    check_empty_var(env::var(key).unwrap_or_else(|_| panic!("{} is invalid / not set", key)), key)
}

pub fn get_private_key() -> String {
    let key: &'static str = "PRIVATE_KEY";
    check_empty_var(env::var(key).unwrap_or_else(|_| panic!("{} is invalid / not set", key)), key)
}

pub fn get_ws_url() -> String {
    let key: &'static str = "WS_URL";
    check_empty_var(env::var(key).unwrap_or_else(|_| panic!("{} is invalid / not set", key)), key)
}

pub fn get_grpc_url() -> Option<String> {
    let key: &'static str = "GRPC_URL";
    env::var(key).ok()
}

pub fn get_grpc_token() -> Option<String> {
    let key: &'static str = "GRPC_TOKEN";
    env::var(key).ok()
}

pub fn get_nonce_account() -> Option<String> {
    let key: &'static str = "NONCE_ACCOUNT";
    env::var(key).ok()
}

pub fn get_deagle_debug() -> bool {
    let key: &'static str = "DEAGLE_DEBUG";
    env::var(key).ok().unwrap_or_default().parse().unwrap_or(false)
}

pub fn get_min_transfer_sol() -> f64 {
    let key: &'static str = "MIN_TRANSFER_SOL";
    env::var(key).ok().unwrap_or_default().parse().unwrap_or(20.0)
}

pub fn get_buy_amount() -> f64 {
    let key: &'static str = "BUY_AMOUNT_SOL";
    env::var(key).ok().unwrap_or_default().parse().unwrap_or(0.00001)
}

pub fn get_slippage() -> f64 {
    let key: &'static str = "SLIPPAGE";
    env::var(key).ok().unwrap_or_default().parse().unwrap_or(0.01)
}

pub fn get_regions() -> Option<Vec<String>> {
    let key: &'static str = "REGIONS";
    env::var(key).ok().map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
}

pub fn get_algo_config() -> AlgoConfig {
    let grand_chillers = if env::var("ALGO_GC_MIN_HMC").is_ok() {
        Some(GrandChillersConfig {
            use_gc: env::var("ALGO_USE_GRAND_CHILLERS").ok().unwrap_or_default().parse().unwrap_or(true),
            min_hmc: env::var("ALGO_GC_MIN_HMC").ok().unwrap_or_default().parse().unwrap_or(10_000.0),
            min_mints: env::var("ALGO_GC_MIN_MINTS").ok().unwrap_or_default().parse().unwrap_or(2),
            min_buys: env::var("ALGO_GC_MIN_BUYS").ok().unwrap_or_default().parse().unwrap_or(10),
        })
    } else {
        None
    };
    let dexter_trust = if env::var("DEXTER_TRUST_DB_URL").is_ok() {
        Some(DexterTrustConfig {
            use_dexter_trust: env::var("ALGO_USE_DEXTER_TRUST").ok().unwrap_or_default().parse().unwrap_or(true),
            db_url: env::var("DEXTER_TRUST_DB_URL").unwrap_or_default(),
            min_trust_factor: env::var("DEXTER_MIN_TRUST_FACTOR").ok().unwrap_or_default().parse().unwrap_or(0.0),
        })
    } else {
        None
    };

    AlgoConfig {
        limit: env::var("ALGO_LIMIT").ok().unwrap_or_default().parse().unwrap_or(400),
        min_mints: env::var("ALGO_MIN_MINTS").ok().unwrap_or_default().parse().unwrap_or(1),
        min_deagle_sol: env::var("ALGO_MIN_DEAGLE_SOL").ok().map(|v| v.parse().unwrap_or(15.0)),
        use_vc: env::var("ALGO_USE_VOLCREATORS").ok().unwrap_or_default().parse().unwrap_or(true),
        use_deagles: env::var("ALGO_USE_DEAGLES").ok().unwrap_or_default().parse().unwrap_or(true),
        min_buys: env::var("ALGO_MIN_BUYS").ok().unwrap_or_default().parse().unwrap_or(1),
        min_volume: env::var("ALGO_MIN_VOLUME").ok().unwrap_or_default().parse().unwrap_or(10.0),
        grand_chillers,
        dexter_trust,
    }
}

pub fn get_snipe_config() -> SnipeConfig {
    SnipeConfig {
        buy_amount: get_buy_amount(),
        slippage: get_slippage(),
        max_loss: env::var("MAX_LOSS").ok().unwrap_or_default().parse().unwrap_or(20.0),
        take_profit: env::var("TAKE_PROFIT").ok().unwrap_or_default().parse().unwrap_or(10.0),
        use_chp: env::var("USE_CHP").ok().unwrap_or_default().parse().unwrap_or(true),
        use_tiz: env::var("USE_TIZ").ok().unwrap_or_default().parse().unwrap_or(true),
        chp_lower: env::var("CHP_LOWER").ok().unwrap_or_default().parse().unwrap_or(0.09),
        chp_upper: env::var("CHP_UPPER").ok().unwrap_or_default().parse().unwrap_or(0.22),
        tiz_lower: env::var("TIZ_LOWER").ok().unwrap_or_default().parse().unwrap_or(3),
        tiz_upper: env::var("TIZ_UPPER").ok().unwrap_or_default().parse().unwrap_or(21),
        max_no_activity_ms: env::var("MAX_NO_ACTIVITY_MS").ok().unwrap_or_default().parse().unwrap_or(60000),
        max_na_on_start_ms: env::var("MAX_NA_ON_START_MS").ok().unwrap_or_default().parse().unwrap_or(5000),
        min_dev_sold: env::var("MIN_DEV_SOLD").ok().unwrap_or_default().parse().unwrap_or(20_000_000),
    }
}

pub fn get_strats_config() -> StratsConfig {
    StratsConfig {
        enable_abs: env::var("ENABLE_ABS").ok().unwrap_or_default().parse().unwrap_or(false),
        abs_min_buys: env::var("ABS_MIN_BUYS").ok().unwrap_or_default().parse().unwrap_or(10),
        abs_min_volume: env::var("ABS_MIN_VOL").ok().unwrap_or_default().parse().unwrap_or(10.0),
        abs_n: env::var("ABS_N").ok().unwrap_or_default().parse().unwrap_or(1),
        enable_mtd: env::var("ENABLE_MTD").ok().unwrap_or_default().parse().unwrap_or(false),
        mtd_pct: env::var("MTD_PCT").ok().unwrap_or_default().parse().unwrap_or(10),
        mtd_stable_time: env::var("MTD_STABLE_TIME").ok().unwrap_or_default().parse().unwrap_or(60),
        mtd_max_loss: env::var("MTD_MAX_LOSS").ok().unwrap_or_default().parse().unwrap_or(10.0),
        mtd_take_profit: env::var("MTD_TAKE_PROFIT").ok().unwrap_or_default().parse().unwrap_or(10.0),
        mtd_min_mc: env::var("MTD_MIN_MC").ok().unwrap_or_default().parse().unwrap_or(60.0),
        enable_dbf: env::var("ENABLE_DBF").ok().unwrap_or_default().parse().unwrap_or(false),
        dbf_max_chp: env::var("DBF_MAX_CHP").ok().unwrap_or_default().parse().unwrap_or(0.22),
    }
}

pub fn config() -> Config {
    load_env();
    Config::new(
        get_rpc_url(),
        get_private_key(),
        get_ws_url(),
        get_grpc_url(),
        get_grpc_token(),
        get_nonce_account(),
    )
}

pub fn get_tx_settings() -> TxSettings {
    TxSettings {
        tx_strat: env::var("TX_STRAT").ok().unwrap_or_default(),
        nextblock_key: env::var("NEXTBLOCK_KEY").ok().unwrap_or_default(),
        zero_slot_key: env::var("ZERO_SLOT_KEY").ok().unwrap_or_default(),
        temporal_key: env::var("TEMPORAL_KEY").ok().unwrap_or_default(),
        blox_key: env::var("BLOX_KEY").ok().unwrap_or_default(),
        tip_lamports: env::var("TIP_LAMPORTS").ok().unwrap_or_default().parse().unwrap_or(1_100_000),
    }
}
