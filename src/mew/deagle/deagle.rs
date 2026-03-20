#![allow(deprecated, unused_imports)]
use {
    crate::mew::{config::{config, Config}, sol_hook::{goldmine::{DeagleRow, Token, Goldmine, Volume, Holder, Dupe, Sim}, sol::{SolHook, SYSTEM_PROGRAM, WSOL_MINT}}, writing::{cc, Colors}}, serde::{Deserialize, Serialize}, solana_commitment_config::CommitmentConfig, solana_program::{hash::Hash, instruction::Instruction, pubkey, pubkey::Pubkey}, solana_rpc_client_types::{config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter}, response::RpcLogsResponse}, solana_signature::Signature, solana_transaction_status::{
        EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction, UiInstruction, UiMessage,
        UiParsedInstruction,
    }, sqlx::{prelude::FromRow, Pool, Postgres}, std::{io, str::FromStr, time::Duration}, tokio::sync::mpsc::Receiver, std::sync::Arc, crate::mew::sol_hook::pump_fun::{PumpFun, PumpFunEvent, PUMP_FUN_ID}, pump_fun_types::events::{CreateEvent, TradeEvent},
    pump_swap_types::events::{BuyEvent, SellEvent, CreatePoolEvent},
    crate::mew::sol_hook::pump_swap::{PUMP_SWAP_ID, PumpSwap, PumpSwapEvent},
    std::collections::HashMap,
    serde_json::json,
    sqlx::types::Json,
    std::cmp::Ordering,
    crate::{log, warn}
};

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    VolCreators,
    Deagle,
    GrandChillers,
    Twitter,
    Dip,
    Digged,
    DevBestFriend
}

impl Source {
    pub fn label(&self) -> &'static str {
        match self {
            Source::VolCreators => "vol_creators",
            Source::Deagle => "deagle",
            Source::GrandChillers => "grand_chillers",
            Source::Twitter => "twitter",
            Source::Dip => "dip",
            Source::Digged => "digged",
            Source::DevBestFriend => "dev_best_friend",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChilledCreator {
    pub creator: Pubkey,
    pub median_token_high_mc: f64,
    pub median_buys: i64,
    pub mints: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SystemTransfer {
    pub source: String,
    pub destination: String,
    pub lamports: u64,
}

pub const CEX_IDS: &[Pubkey] = &[
    pubkey!("5tzFkiKscXHK5ZXCGbXZxdw7gTjjD1mBwuoFbhUvuAi9"), // BINANCE
    pubkey!("AobVSwdW9BbpMdJvTqeCN4hPAmh4rHm7vwLnQ5ATSyrS"), // CRYPTO.COM
    pubkey!("AC5RDfQFmDS1deWZos921JfqscXdByf8BKHs5ACWjtW2"), // BYBIT Hot Wallet
    pubkey!("is6MTRHEgyFLNTfYcuV4QBWLjrZBfmhVNYR6ccgr8KV"), // OKX
    pubkey!("53unSgGWqEWANcPYRF35B2Bgf8BkszUtcccKiXwGGLyr"), // BINANCE.us Hot Wallet
    pubkey!("FWznbcNXWQuHTawe9RxvQ2LdCENssh12dsznf4RiouN5"), // KRAKEN
    pubkey!("ASTyfSima4LLAdDgoFGkgqoKowG1LZFDr9fAQrg7iaJZ"), // MEXC
    pubkey!("9obNtb5GyUegcs3a1CbBkLuc5hEWynWfJC6gjz5uWQkE"), // Coinbase 4
    pubkey!("D89hHJT5Aqyx1trP6EnGY9jJUB3whgnq3aUvvCqedvzf"), // Coinbase 3
    pubkey!("2AQdpHJ2JpcEgPiATUXjQxA8QmafFegfQwSLWSprPicm"), // Coinbase 2
    pubkey!("FpwQQhQQoEaVu3WU2qZMfF1hx48YyfwsLoRgXG83E99Q"), // Coinbase 1
    pubkey!("A77HErqtfN1hLLpvZ9pCtu66FEtM8BveoaKbbMoZ4RiR"), // Bitget
    pubkey!("G9X7F4JzLzbSGMCndiBdWNi5YzZZakmtkdwq7xS3Q3FE"), // Stake Hot Wallet
    pubkey!("3vxheE5C46XzK4XftziRhwAf8QAfipD7HXXWj25mgkom"), // Coinbase Prime
    pubkey!("76iXe9yKFDjGv3HicUVVy8AYxHLC71L1wYa12zaZzHHp"), // Unknown
    pubkey!("DQ5JWbJyWdJeyBxZuuyu36sUBud6L6wo3aN1QC1bRmsR"), // Unknown
    pubkey!("BY4StcU9Y2BpgH8quZzorg31EGE4L1rjomN8FNsCBEcx"), // HTX: Hot Wallet
];

pub struct DeagleConfig {
    pub min_transfer: f64,
    pub max_transfer: Option<f64>,
    pub exclude_accounts: Option<Vec<String>>,
    pub debug: bool,
}

#[derive(FromRow)]
pub struct Deagle {
    pub sol_hook: SolHook,
    pub dconfig: DeagleConfig,
    lconfig: Config,
    goldmine: Goldmine,
    pump_fun: Arc<PumpFun>,
    pump_swap: Arc<PumpSwap>,
}

#[derive(Debug)]
pub struct VolumeCreator {
    pub creator: Pubkey,
    pub sol_in: f64,
    pub sol_out: f64,
    pub total: f64,
    pub mints: Vec<Pubkey>
}

#[derive(Debug, Clone)]
pub struct AlgoCreator {
    pub creator: Pubkey,
    pub mints: Option<Vec<Token>>,
    pub source: Source,
}

#[derive(Debug, Clone)]
pub struct GrandChillersConfig {
    pub use_gc: bool,
    pub min_hmc: f64,
    pub min_mints: i64,
    pub min_buys: i64,
}

#[derive(Debug, Clone)]
pub struct AlgoConfig {
    pub limit: i64,
    pub use_vc: bool,
    pub use_deagles: bool,
    pub min_volume: f64,
    pub min_buys: i64,
    pub min_mints: i64,
    pub min_deagle_sol: Option<f64>,
    pub grand_chillers: Option<GrandChillersConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CandidateObservation {
    pub creator: String,
    pub source_label: String,
    pub score_components: serde_json::Value,
    pub score_total: f64,
    pub cohort_size: usize,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct CandidateSelectionReport {
    pub creators: Vec<(String, Source)>,
    pub observations: Vec<CandidateObservation>,
    pub source_counts: HashMap<String, usize>,
}

impl Deagle {
    pub fn new(sol_hook: SolHook, dconfig: DeagleConfig, goldmine: Goldmine, pump_fun: Arc<PumpFun>, pump_swap: Arc<PumpSwap>) -> Self {
        Self { sol_hook, dconfig, lconfig: config(), goldmine, pump_fun, pump_swap }
    }

    pub async fn get_top_creators(&self, limit: i64) -> anyhow::Result<Vec<String>> {
        let tokens = self.goldmine.top_tokens_by_hmc(&PUMP_FUN_ID.to_string(), limit).await?;
        let mut creators = Vec::new();
        for token in tokens.iter().rev() {
            creators.push(token.dev_address.clone());
        }
        log!(cc::LIGHT_WHITE, "Top creators: {:?}", creators);
        Ok(creators)
    }

    /// Returns top creators sorted by volume
    pub async fn get_tc_by_volume(&self, min_mints: i64, limit: i64) -> anyhow::Result<HashMap<Pubkey, VolumeCreator>> {
        let tokens = self.goldmine
        .top_tokens_by_hmc(&PUMP_FUN_ID.to_string(), limit)
        .await?;

        let mut by_creator: HashMap<String, i64> = HashMap::new();
        for t in tokens.iter() {
            let v = &t.volume;
            if v.buys > 0 || v.sells > 0 {
                *by_creator.entry(t.dev_address.clone()).or_default() += v.sol_in;
            }
        }

        let mut items: Vec<(String, i64)> = by_creator.into_iter().collect();
        items.sort_unstable_by(|a, b| b.1.cmp(&a.1));

        let mut volume_creators = HashMap::new();
        for (c, _) in items.into_iter() {
            for token in tokens.iter() {
                if token.dev_address == c {
                    let creator = Pubkey::from_str(&c).unwrap();
                    if volume_creators.contains_key(&creator) {
                        let vc: &mut VolumeCreator = volume_creators.get_mut(&creator).unwrap();
                        vc.mints.push(Pubkey::from_str(&token.mint).unwrap());
                        vc.sol_in += token.volume.sol_in as f64 / 1e9;
                        vc.sol_out += token.volume.sol_out as f64 / 1e9;
                        vc.total += token.volume.sol_in as f64 / 1e9 + token.volume.sol_out as f64 / 1e9;
                    } else {
                        volume_creators.insert(creator, VolumeCreator {
                            creator: Pubkey::from_str(&c).unwrap(),
                            sol_in: token.volume.sol_in as f64 / 1e9,
                            sol_out: token.volume.sol_out as f64 / 1e9,
                            total: token.volume.sol_in as f64 / 1e9 + token.volume.sol_out as f64 / 1e9,
                            mints: vec![Pubkey::from_str(&token.mint).unwrap()],
                        });
                    }
                }
            }
        }

        let mut volume_creators_filtered: HashMap<Pubkey, VolumeCreator> = HashMap::new();
        for (k, v) in volume_creators.into_iter() {
            if v.mints.len() >= min_mints as usize {
                volume_creators_filtered.insert(k.clone(), v);
            }
        }

        Ok(volume_creators_filtered)
    }

    pub async fn grand_chillers(&self, limit: i64) -> anyhow::Result<Vec<ChilledCreator>> {
        let rows = self.goldmine.get_creators_medians(50000).await?; // memory cap

        let mut v: Vec<ChilledCreator> = rows.into_iter().filter_map(|r| {
            let pk = Pubkey::from_str(&r.creator).ok()?;
            if r.mints > 1 && r.median_buys >= 10 {
                Some(ChilledCreator {
                    creator: pk,
                    median_token_high_mc: r.median_token_high_mc,
                    median_buys: r.median_buys,
                    mints: r.mints,
                })
            } else {
                None
            }
        }).collect();

        fn chill_score(c: &ChilledCreator) -> f64 {
            let mints = c.mints.max(0) as f64;
            let buys  = c.median_buys.max(0) as f64;
            let hmc   = c.median_token_high_mc.max(0.0);

            (mints.ln_1p()) + (buys.ln_1p()) + (hmc.ln_1p())
        }

        v.sort_by(|a, b| {
            let sa = chill_score(a);
            let sb = chill_score(b);
            match sb.partial_cmp(&sa).unwrap_or(Ordering::Equal) {
                Ordering::Equal => {
                    b.mints.cmp(&a.mints)
                        .then(b.median_buys.cmp(&a.median_buys))
                        .then_with(|| b.median_token_high_mc
                            .partial_cmp(&a.median_token_high_mc)
                            .unwrap_or(Ordering::Equal))
                }
                ord => ord,
            }
        });

        if limit > 0 && (limit as usize) < v.len() {
            v.truncate(limit as usize);
        }
        Ok(v)
    }

    pub fn median_f64(xs: &mut [f64]) -> Option<f64> {
        if xs.is_empty() { return None; }
        let mid = xs.len() / 2;
        xs.select_nth_unstable_by(mid, |a, b| a.partial_cmp(b).unwrap());
        if xs.len() % 2 == 1 {
            Some(xs[mid])
        } else {
            let (lo, hi) = xs.split_at(mid);
            Some(( *lo.iter().max_by(|a,b| a.partial_cmp(b).unwrap()).unwrap() + hi[0]) / 2.0)
        }
    }

    pub async fn algo_choose_creators(
        &self, 
        config: AlgoConfig
    ) -> anyhow::Result<Vec<(String, Source)>> {
        Ok(self.algo_choose_creators_report(config).await?.creators)
    }

    pub async fn algo_choose_creators_report(
        &self,
        config: AlgoConfig,
    ) -> anyhow::Result<CandidateSelectionReport> {
        let limit = config.limit;
        let min_mints = config.min_mints;
        let min_deagle_sol = config.min_deagle_sol.unwrap_or(0.0);
        let min_buys = config.min_buys;
        let min_volume = config.min_volume;
        let grand_chillers = config.grand_chillers;
        let use_gc = grand_chillers.as_ref().unwrap().use_gc;
        let use_vc = config.use_vc;
        let use_deagles = config.use_deagles;

        let mut creators: Vec<AlgoCreator> = self._algo_choose_helper(limit, min_mints, min_deagle_sol, use_vc, use_deagles).await?;
        let gcv = if use_gc { self.grand_chillers(limit).await? } else { Vec::new() };
        let mut out_creators: Vec<(String, Source)> = Vec::new();
        let mut observations: Vec<CandidateObservation> = Vec::new();
        let mut source_counts: HashMap<String, usize> = HashMap::new();

        for creator in creators.iter_mut() {
            if creator.source == Source::Deagle {
                out_creators.push((creator.creator.to_string(), Source::Deagle));
                let deagle_amount = self
                    .goldmine
                    .get_deagle(&creator.creator.to_string())
                    .await?
                    .map(|row| row.sol_amount)
                    .unwrap_or_default();
                observations.push(CandidateObservation {
                    creator: creator.creator.to_string(),
                    source_label: Source::Deagle.label().to_string(),
                    score_components: json!({
                        "deagle_sol_amount": deagle_amount,
                        "min_deagle_sol": min_deagle_sol,
                    }),
                    score_total: deagle_amount,
                    cohort_size: 0,
                    reason: "deagle_transfer_threshold".to_string(),
                });
                *source_counts.entry(Source::Deagle.label().to_string()).or_insert(0) += 1;
            }
            let mut buys_vec: Vec<f64> = Vec::new();
            let mut vol_vec:  Vec<f64> = Vec::new();
        
            for token in creator.mints.iter().flatten() {
                buys_vec.push(token.volume.buys as f64);
                let vol = (token.volume.sol_in + token.volume.sol_out) as f64 / 1e9;
                vol_vec.push(vol);
            }
        
            let median_buys   = Self::median_f64(&mut buys_vec).unwrap_or(0.0);
            let median_volume = Self::median_f64(&mut vol_vec).unwrap_or(0.0);
        
            if median_buys >= min_buys as f64 && median_volume >= min_volume {
                out_creators.push((creator.creator.to_string(), Source::VolCreators));
                observations.push(CandidateObservation {
                    creator: creator.creator.to_string(),
                    source_label: Source::VolCreators.label().to_string(),
                    score_components: json!({
                        "median_buys": median_buys,
                        "median_volume": median_volume,
                        "mint_count": creator.mints.as_ref().map(|mints| mints.len()).unwrap_or_default(),
                        "min_buys": min_buys,
                        "min_volume": min_volume,
                    }),
                    score_total: median_volume + median_buys,
                    cohort_size: 0,
                    reason: "volume_creator_threshold".to_string(),
                });
                *source_counts.entry(Source::VolCreators.label().to_string()).or_insert(0) += 1;
            }
        }
        if let Some(config) = grand_chillers {
            for chiller in gcv.iter() {
                if chiller.median_token_high_mc >= config.min_hmc && chiller.mints >= config.min_mints && chiller.median_buys >= config.min_buys {
                    out_creators.push((chiller.creator.to_string(), Source::GrandChillers));
                    observations.push(CandidateObservation {
                        creator: chiller.creator.to_string(),
                        source_label: Source::GrandChillers.label().to_string(),
                        score_components: json!({
                            "median_token_high_mc": chiller.median_token_high_mc,
                            "median_buys": chiller.median_buys,
                            "mints": chiller.mints,
                            "min_hmc": config.min_hmc,
                            "min_mints": config.min_mints,
                            "min_buys": config.min_buys,
                        }),
                        score_total: chiller.median_token_high_mc + chiller.median_buys as f64 + chiller.mints as f64,
                        cohort_size: 0,
                        reason: "grand_chiller_threshold".to_string(),
                    });
                    *source_counts.entry(Source::GrandChillers.label().to_string()).or_insert(0) += 1;
                }
            }
        }
        for observation in &mut observations {
            observation.cohort_size = source_counts
                .get(&observation.source_label)
                .copied()
                .unwrap_or_default();
        }
        Ok(CandidateSelectionReport {
            creators: out_creators,
            observations,
            source_counts,
        })
    }

    pub async fn _algo_choose_helper(&self, limit: i64, min_mints: i64, min_deagle_sol: f64, use_vc: bool, use_deagles: bool) -> anyhow::Result<Vec<AlgoCreator>> {
        let mut mint_map: HashMap<String, Vec<Token>> = HashMap::new();
        let mut creators = Vec::new();
        if use_vc {
            let vol_creators = self.get_tc_by_volume(min_mints, limit).await?;
            for (creator, volume) in vol_creators.iter() {
                let mints = volume.mints.clone();
                let mut tokens = Vec::new();
                for mint in mints.iter() {
                    let token = self.goldmine.get_token(&mint.to_string()).await?;
                    if token.is_some() {
                        let token = token.unwrap();
                        tokens.push(token);
                    }
                }
                mint_map.insert(creator.to_string(), tokens);
            }
            for (creator, tokens) in mint_map.iter() {
                creators.push(AlgoCreator { creator: Pubkey::from_str(creator).unwrap(), mints: Some(tokens.clone()),  source: Source::VolCreators });
            }
        }
        if use_deagles {
        let deagles = self.goldmine.get_deagles_by_sol_amount(limit).await?;
            for deagle in deagles.iter() {
                let deagle_amount = deagle.sol_amount;
                if deagle_amount >= min_deagle_sol {
                    creators.push(AlgoCreator { creator: Pubkey::from_str(&deagle.wallet).unwrap(), mints: None, source: Source::Deagle });
                }
            }
        }

        Ok(creators)
    }

    pub async fn find_system_transfers(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> anyhow::Result<Vec<SystemTransfer>> {
        let mut out = Vec::new();
    
        let EncodedTransaction::Json(ui_tx) = &tx.transaction.transaction else { return Ok(out); };
        let UiMessage::Parsed(msg) = &ui_tx.message else { return Ok(out); };
    
        for ix in &msg.instructions {
            let UiInstruction::Parsed(UiParsedInstruction::Parsed(pi)) = ix else { continue };
            if pi.program.as_str() != "system" {
                continue;
            }
    
            let parsed = &pi.parsed;
            let ty = parsed.get("type").and_then(|v| v.as_str());
            if !matches!(ty, Some("transfer" | "transferWithSeed" | "transferChecked")) {
                continue;
            }
    
            let info = &parsed["info"];
            if let (Some(src), Some(dst), Some(lamports)) = (
                info.get("source").and_then(|v| v.as_str()),
                info.get("destination").and_then(|v| v.as_str()),
                info.get("lamports").and_then(|v| v.as_u64()),
            ) {
                if CEX_IDS.contains(&Pubkey::from_str(src).unwrap()) {
                    out.push(SystemTransfer {
                        source: src.to_string(),
                        destination: dst.to_string(),
                        lamports,
                    });
                } else {
                    let deagle = self.goldmine.get_deagle(src).await?;
                    if deagle.is_some() {
                            out.push(SystemTransfer {
                            source: src.to_string(),
                            destination: dst.to_string(),
                            lamports,
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    /// Increments sol amount for each transfer, if deagle already exists
    pub async fn fill_deagle(&self, transfers: &[SystemTransfer]) -> anyhow::Result<()> {
        for transfer in transfers {
            let tx_lams = transfer.lamports as f64 / 1e9;
            if self.dconfig.exclude_accounts.is_some() && self.dconfig.exclude_accounts.as_ref().unwrap().contains(&transfer.destination)
              || self.dconfig.max_transfer.is_some() && tx_lams > self.dconfig.max_transfer.unwrap()
              || tx_lams < self.dconfig.min_transfer {
                continue;
            }
            let deagle = self.goldmine.get_deagle(&transfer.destination).await?;
            let mut deagle_amount = tx_lams;
            if deagle.is_some() {
                deagle_amount += deagle.unwrap().sol_amount;
            }
            self.goldmine.upsert_deagle(&DeagleRow {
                wallet: transfer.destination.clone(),
                last_source: transfer.source.clone(),
                sol_amount: deagle_amount,
            }).await?;
            if self.dconfig.debug {
                log!(cc::DARK_GRAY, "Deagle spotted {} transferring: {} SOL", transfer.destination, deagle_amount);
            }
        }
        Ok(())
    }

    pub async fn analyze_profits(&self) -> anyhow::Result<()> {
        let sims = self.goldmine.get_sims().await?;
        let mut total_profit = 0.0;
        let mut best_sim: Option<&Sim> = None;
        for sim in sims.iter() {
            total_profit += sim.profit;
            if best_sim.is_none() || sim.profit > best_sim.unwrap().profit {
                best_sim = Some(sim);
            }
        };
        log!(cc::LIGHT_GREEN, "Total profit: {}", total_profit);
        log!(cc::LIGHT_GRAY, "Best sim: {:?}", best_sim);
        Ok(())
    }

    async fn check_is_dupe(&self, name: &str, symbol: &str, uri: &str) -> anyhow::Result<bool> {
        let mut dupe = self.goldmine.get_dupes(name, symbol, uri).await?;
        if dupe.is_some() {
            dupe.as_mut().unwrap().count += 1;
            self.goldmine.upsert_dupes(dupe.as_mut().unwrap()).await?;
            return Ok(true);
        }
        Ok(false)
    }

    async fn proc_create(&self, create: CreateEvent) -> anyhow::Result<()> {
        match self.check_is_dupe(&create.name, &create.symbol, &create.uri).await {
            Ok(true) => {
                if self.dconfig.debug {
                    log!(cc::LIGHT_RED, "Deagle | Dupe found: {} {}", create.mint, create.name);
                }
                return Ok(());
            }
            Ok(false) => {
                if self.dconfig.debug {
                    log!(cc::LIGHT_MAGENTA, "Deagle | New token created: {} {}", create.mint, create.name);
                }
            }
            Err(e) => {
                warn!("check_is_dupe error: {e}");
                return Err(e);
            }
        }

        let token = self.goldmine.get_token(&create.mint.to_string()).await;
        let is_db = match &token.as_ref().unwrap() {
            Some(token) => &token.current_id == &PUMP_FUN_ID.to_string(),
            None => false,
        };
        if is_db {
            return Ok(());
        }
        let decimals = self.sol_hook.get_token_decimals(&create.mint).await?;
        let open_price = PumpFun::get_open_price(&create);
        let token = Token {
            mint: create.mint.to_string(),
            pool: create.bonding_curve.to_string(),
            origin_id: PUMP_FUN_ID.to_string(),
            current_id: PUMP_FUN_ID.to_string(),
            name: create.name.to_string(),
            symbol: create.symbol.to_string(),
            decimals: decimals as i32,
            price: open_price,
            open_price: open_price,
            volume: Json(Volume {
                buys: 0,
                sells: 0,
                sol_in: 0,
                sol_out: 0,
            }),
            market_cap: 0.0,
            high_market_cap: 0.0,
            dev_sold: false,
            dev_buy_amount: 0.0,
            dev_address: create.creator.to_string(),
            holders: Json(HashMap::new()),
            seen_at: create.timestamp,
        };
        self.goldmine.upsert_token(&token).await?;
        self.goldmine.upsert_dupes(
            &Dupe {
                name: create.name.to_string(),
                symbol: create.symbol.to_string(),
                uri: create.uri.to_string(),
                creators: Json(vec![create.creator.to_string()]),
                count: 1,
            }
        ).await?;
        Ok(())
    }

    async fn proc_trade(&self, trade: TradeEvent, sol_price: f64) -> anyhow::Result<()> {
        let price = PumpFun::get_price(&trade);
        let mint = trade.mint.to_string();
        let market_cap = PumpFun::get_market_cap(&trade, sol_price).await?;
        let token = self.goldmine.get_token(&mint).await;
        let is_db = match &token.as_ref().unwrap() {
            Some(token) => &token.current_id == &PUMP_FUN_ID.to_string(),
            None => false,
        };
        if is_db {
            let mut token = token.unwrap_or_default().unwrap_or_default();
            let mut volume = token.volume.clone();
            let dev_address = token.dev_address.clone();
            let is_dev_trade = trade.user.to_string() == dev_address;
            let sol_amount = trade.sol_amount as f64 / 1e9;
            let token_amount = trade.token_amount as f64 / 1e6;

            let mut holders = token.holders;
            let mut holder = match holders.get(&trade.user.to_string()) {
                Some(h) => h.to_owned(),
                None => {
                    Holder {
                        total_sol_spent: 0.0,
                        total_token_bought: 0.0,
                        is_holding: true,
                    }
                },
            };

            if trade.is_buy {
                if is_dev_trade { token.dev_buy_amount += sol_amount; }
                volume.buys += 1;
                volume.sol_in += trade.sol_amount as i64;
                holder.total_sol_spent += sol_amount;
                holder.total_token_bought += token_amount;
            } else {
                if is_dev_trade { token.dev_buy_amount -= sol_amount; if token.dev_buy_amount < 0.01 { token.dev_sold = true; } }
                volume.sells += 1;
                volume.sol_out += trade.sol_amount as i64;
                holder.total_token_bought -= token_amount;
                if holder.total_token_bought <= 0.0 { holder.is_holding = false; }
            }

            holders.insert(trade.user.to_string(), holder);
            token.holders = holders;
            token.volume = volume;
            token.price = price;
            token.market_cap = market_cap;
            token.high_market_cap = if market_cap > token.high_market_cap { market_cap } else { token.high_market_cap };
            self.goldmine.upsert_token(&token).await?;
        } else {
            // index to db
            let token_info = self.sol_hook.get_token_info(&Pubkey::from_str(&mint).unwrap()).await?;
            let name = token_info.name.to_string();
            let symbol = token_info.symbol.to_string();
            let decimals = self.sol_hook.get_token_decimals(&trade.mint).await?;
            let pool = PumpFun::derive_bonding_curve(&Pubkey::from_str(&mint).unwrap()).await?;
            let mut creator = trade.creator.to_string();
            if creator == SYSTEM_PROGRAM.to_string() {
                creator = self.pump_fun.get_creator(&pool).await?.to_string();
            };
            let is_dev_trade = trade.user.to_string() == creator;
            let sol_amount = trade.sol_amount as f64 / 1e9;
            let token_amount = trade.token_amount as f64 / 1e6;
            let mut dev_buy_amount = 0.0;
            let mut volume = Volume::default();
            let mut holders = HashMap::<String, Holder>::new();
            let mut holder = Holder {
                total_sol_spent: 0.0,
                total_token_bought: 0.0,
                is_holding: true,
            };
            if trade.is_buy {
                if is_dev_trade { dev_buy_amount += sol_amount; }
                volume.buys += 1;
                volume.sol_in += trade.sol_amount as i64;
                holder.total_sol_spent += sol_amount;
                holder.total_token_bought += token_amount;
            } else {
                volume.sells += 1;
                volume.sol_out += trade.sol_amount as i64;
            }

            holders.insert(trade.user.to_string(), holder);

            let token = Token {
                mint: mint.clone(),
                pool: pool.to_string(),
                origin_id: PUMP_FUN_ID.to_string(),
                current_id: PUMP_FUN_ID.to_string(),
                name: name.clone(),
                symbol: symbol.clone(),
                decimals: decimals as i32,
                price: price,
                open_price: 0.0000000280,
                volume: Json(volume),
                market_cap: market_cap,
                high_market_cap: market_cap,
                dev_sold: false,
                dev_buy_amount: dev_buy_amount,
                dev_address: creator.clone(),
                holders: Json(holders),
                seen_at: trade.timestamp,
            };
            self.goldmine.upsert_token(&token).await?;
        }
        Ok(())
    }
    
    async fn proc_create_pool(&self, create: CreatePoolEvent, sol_price: f64) -> anyhow::Result<()> {
        let mint = create.base_mint;
        let quote = create.quote_mint;
        if quote != WSOL_MINT {
            return Ok(());
        }

        let bc = PumpFun::derive_bonding_curve(&mint).await?;
        let is_migrated = match self.pump_fun.fetch_state(&bc).await {
            Ok(state) => {
                state.complete
            }
            Err(e) => {
                return Err(e);
            }
        };
        if !is_migrated {
            return Ok(());
        }
        let price = PumpSwap::price_from_create(&create);
        let market_cap = PumpSwap::get_market_cap(create.pool_base_amount, create.pool_quote_amount, sol_price).await?;
        let high_market_cap = market_cap;

        let token = self.goldmine.get_token(&mint.to_string()).await;
        let is_db = match &token.as_ref().unwrap() {
            Some(token) => &token.current_id == &PUMP_FUN_ID.to_string(),
            None => false,
        };
        if is_db {
            let mut token = token.unwrap_or_default().unwrap_or_default();
            token.pool = create.pool.to_string();
            token.current_id = PUMP_SWAP_ID.to_string();
            token.price = price;
            token.market_cap = market_cap;
            token.high_market_cap = high_market_cap;
            self.goldmine.upsert_token(&token).await?;
            if self.dconfig.debug {
                log!(cc::LIGHT_WHITE, "Migrated token: {} {}", mint, token.name);
            }
        } else {
            let token_info: crate::mew::sol_hook::sol::TokenInfo = self.sol_hook.get_token_info(&mint).await?;
            let name = token_info.name.to_string();
            let symbol = token_info.symbol.to_string();
            let decimals = self.sol_hook.get_token_decimals(&mint).await?;
            let creator = create.creator.to_string();
            let token = Token {
                mint: mint.to_string(),
                pool: create.pool.to_string(),
                origin_id: PUMP_FUN_ID.to_string(),
                current_id: PUMP_SWAP_ID.to_string(),
                name: name.clone(),
                symbol: symbol.clone(),
                decimals: decimals as i32,
                price: price,
                open_price: 0.0000000280,
                volume: Json(Volume::default()),
                market_cap: market_cap,
                high_market_cap: high_market_cap,
                dev_sold: false,
                dev_buy_amount: 0.0,
                dev_address: creator.clone(),
                holders: Json(HashMap::new()),
                seen_at: create.timestamp,
            };
            self.goldmine.upsert_token(&token).await?;
            if self.dconfig.debug {
                log!(cc::LIGHT_MAGENTA, "New token migrated: {} {}", mint, name);
            }
        }
        Ok(())
    }
    
    async fn proc_buy(&self, buy: BuyEvent, sol_price: f64) -> anyhow::Result<()> {
        let state = self.pump_swap.fetch_state(&buy.pool).await?;
        let mint = state.base_mint;

        if state.quote_mint != WSOL_MINT {
            return Ok(());
        }

        let token = self.goldmine.get_token(&mint.to_string()).await;
        let price = PumpSwap::price_from_buy(&buy);
        let market_cap = PumpSwap::get_market_cap(buy.pool_base_token_reserves, buy.pool_quote_token_reserves, sol_price).await?;
        let is_db = match &token.as_ref().unwrap() {
            Some(token) => &token.current_id == &PUMP_SWAP_ID.to_string(),
            None => false,
        };
        if is_db {
            let mut token = token.unwrap_or_default().unwrap_or_default();
            let mut volume = token.volume.clone();
            let dev_address = token.dev_address.clone();
            let is_dev_trade = buy.user.to_string() == dev_address;
            let sol_amount = buy.quote_amount_in as f64 / 1e9;
            let token_amount = buy.base_amount_out as f64 / 1e6;

            let mut holders = token.holders;
            let mut holder = match holders.get(&buy.user.to_string()) {
                Some(h) => h.to_owned(),
                None => {
                    Holder {
                        total_sol_spent: 0.0,
                        total_token_bought: 0.0,
                        is_holding: true,
                    }
                },
            };

            if is_dev_trade { token.dev_buy_amount += sol_amount; }
            volume.buys += 1;
            volume.sol_in += buy.quote_amount_in as i64;
            holder.total_sol_spent += sol_amount;
            holder.total_token_bought += token_amount;

            holders.insert(buy.user.to_string(), holder);
            token.holders = holders;
            token.volume = volume;
            token.price = price;
            token.market_cap = market_cap;
            token.high_market_cap = if market_cap > token.high_market_cap { market_cap } else { token.high_market_cap };
            self.goldmine.upsert_token(&token).await?;
        } else {
            let token_info: crate::mew::sol_hook::sol::TokenInfo = match self.sol_hook.get_token_info(&mint).await {
                Ok(token_info) => token_info,
                Err(_) => {
                    return Ok(());
                }
            };
            let name = token_info.name.to_string();
            let symbol = token_info.symbol.to_string();
            let decimals = self.sol_hook.get_token_decimals(&mint).await?;
            let creator = buy.coin_creator.to_string();
            let token = Token {
                mint: mint.to_string(),
                pool: buy.pool.to_string(),
                origin_id: PUMP_SWAP_ID.to_string(),
                current_id: PUMP_SWAP_ID.to_string(),
                name: name.clone(),
                symbol: symbol.clone(),
                decimals: decimals as i32,
                price: price,
                open_price: 0.0000000280,
                volume: Json(Volume {
                    buys: 1,
                    sells: 0,
                    sol_in: buy.quote_amount_in as i64,
                    sol_out: 0,
                }),
                market_cap: market_cap,
                high_market_cap: market_cap,
                dev_sold: false,
                dev_buy_amount: 0.0,
                dev_address: creator.clone(),
                holders: Json(HashMap::new()),
                seen_at: buy.timestamp,
            };
            self.goldmine.upsert_token(&token).await?;
        }
            
        Ok(())
    }
    
    async fn proc_sell(&self, sell: SellEvent, sol_price: f64) -> anyhow::Result<()> {
        let state = self.pump_swap.fetch_state(&sell.pool).await?;
        let mint = state.base_mint;

        if state.quote_mint != WSOL_MINT {
            return Ok(());
        }

        let token = self.goldmine.get_token(&mint.to_string()).await;
        let price = PumpSwap::price_from_sell(&sell);
        let market_cap = PumpSwap::get_market_cap(sell.pool_base_token_reserves, sell.pool_quote_token_reserves, sol_price).await?;

        let is_db = match &token.as_ref().unwrap() {
            Some(token) => &token.current_id == &PUMP_SWAP_ID.to_string(),
            None => false,
        };
        if is_db {
            let mut token = token.unwrap_or_default().unwrap_or_default();
            let mut volume = token.volume.clone();
            let dev_address = token.dev_address.clone();
            let is_dev_trade = sell.user.to_string() == dev_address;
            let sol_amount = sell.quote_amount_out as f64 / 1e9;

            let mut holders = token.holders;
            let holder = match holders.get(&sell.user.to_string()) {
                Some(h) => h.to_owned(),
                None => {
                    Holder {
                        total_sol_spent: 0.0,
                        total_token_bought: 0.0,
                        is_holding: true,
                    }
                },
            };

            if is_dev_trade { token.dev_buy_amount -= sol_amount; if token.dev_buy_amount < 0.01 { token.dev_sold = true; } }
            volume.sells += 1;
            volume.sol_out += sell.quote_amount_out as i64;

            holders.insert(sell.user.to_string(), holder);
            token.holders = holders;
            token.volume = volume;
            token.price = price;
            token.market_cap = market_cap;
            token.high_market_cap = if market_cap > token.high_market_cap { market_cap } else { token.high_market_cap };
            self.goldmine.upsert_token(&token).await?;
        } else {
            let token_info: crate::mew::sol_hook::sol::TokenInfo = match self.sol_hook.get_token_info(&mint).await {
                Ok(token_info) => token_info,
                Err(_) => {
                    return Ok(());
                }
            };
            let name = token_info.name.to_string();
            let symbol = token_info.symbol.to_string();
            let decimals = self.sol_hook.get_token_decimals(&mint).await?;
            let creator = sell.coin_creator.to_string();
            let token = Token {
                mint: mint.to_string(),
                pool: sell.pool.to_string(),
                origin_id: PUMP_SWAP_ID.to_string(),
                current_id: PUMP_SWAP_ID.to_string(),
                name: name.clone(),
                symbol: symbol.clone(),
                decimals: decimals as i32,
                price: price,
                open_price: 0.0000000280,
                volume: Json(Volume {
                    buys: 0,
                    sells: 1,
                    sol_in: 0,
                    sol_out: sell.quote_amount_out as i64,
                }),
                market_cap: market_cap,
                high_market_cap: market_cap,
                dev_sold: false,
                dev_buy_amount: 0.0,
                dev_address: creator.clone(),
                holders: Json(HashMap::new()),
                seen_at: sell.timestamp,
            };
            self.goldmine.upsert_token(&token).await?;
        }

        Ok(())
    }

    async fn proc_pump_task(self: Arc<Self>, mut stream: Receiver<RpcLogsResponse>, sol_price: f64) {
        while let Some(msg) = stream.recv().await {
            if msg.err != None {
                continue;
            }
            let lines: std::slice::Iter<'_, String> = msg.logs
             .iter();

            let events = PumpFun::parse_logs(lines, Some(&msg.signature));
            for event in events {
                match event {
                    PumpFunEvent::Trade(Some(trade)) => {
                        let _ = self.proc_trade(trade, sol_price).await;
                    }
                    PumpFunEvent::Create(Some(create)) => {
                        let _ = self.proc_create(create).await;
                    }
                    _ => {}
                }
            }
        }
    }

    async fn proc_pump_swap_task(self: Arc<Self>, mut stream: Receiver<RpcLogsResponse>, sol_price: f64) {
        while let Some(msg) = stream.recv().await {
            if msg.err != None {
                continue;
            }
            let lines: std::slice::Iter<'_, String> = msg.logs
             .iter();

            let events = PumpSwap::parse_logs(lines, Some(&msg.signature.to_string()));
            for event in events {
                match event {
                    PumpSwapEvent::CreatePool(Some(create)) => {
                        let _ = self.proc_create_pool(create, sol_price).await;
                    }
                    PumpSwapEvent::Buy(Some(buy)) => {
                        let _ = self.proc_buy(buy, sol_price).await;
                    }
                    PumpSwapEvent::Sell(Some(sell)) => {
                        let _ = self.proc_sell(sell, sol_price).await;
                    }
                    _ => {}
                }
            }
        }
    }

    async fn proc_st_task(self: Arc<Self>, mut stream: Receiver<RpcLogsResponse>) {
        while let Some(msg) = stream.recv().await {
            let Ok(sig) = Signature::from_str(&msg.signature) else { continue; };
            match self.sol_hook.get_transaction_parsed(&sig).await {
                Ok(tx) => {
                    let transfers = self.find_system_transfers(&tx).await.unwrap_or_default();
                    if !transfers.is_empty() {
                        if let Err(e) = self.fill_deagle(&transfers).await {
                            warn!("fill_deagle error: {e}");
                        }
                    }
                }
                Err(e) => warn!("get_transaction_parsed error: {e} for {}", sig),
            }
        }
    }

    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        let ws_url = self.lconfig.ws_url.clone();
        let sol_price = self.sol_hook.fetch_sol_price().await?;

        let (rx_binance, _h1) = self.sol_hook
            .subscribe_logs_channel(&ws_url, RpcTransactionLogsFilter::Mentions(vec![CEX_IDS[0].to_string()]), CommitmentConfig::confirmed())
            .await?;
        let (rx_bybit,   _h2) = self.sol_hook
            .subscribe_logs_channel(&ws_url, RpcTransactionLogsFilter::Mentions(vec![CEX_IDS[1].to_string()]), CommitmentConfig::confirmed())
            .await?;
        let (rx_crypto,  _h3) = self.sol_hook
            .subscribe_logs_channel(&ws_url, RpcTransactionLogsFilter::Mentions(vec![CEX_IDS[2].to_string()]), CommitmentConfig::confirmed())
            .await?;
        let (rx_okx,     _h4) = self.sol_hook
            .subscribe_logs_channel(&ws_url, RpcTransactionLogsFilter::Mentions(vec![CEX_IDS[3].to_string()]), CommitmentConfig::confirmed())
            .await?;
        let (rx_binance_us, _h5) = self.sol_hook
            .subscribe_logs_channel(&ws_url, RpcTransactionLogsFilter::Mentions(vec![CEX_IDS[4].to_string()]), CommitmentConfig::confirmed())
            .await?;
        let (rx_kraken, _h6) = self.sol_hook
            .subscribe_logs_channel(&ws_url, RpcTransactionLogsFilter::Mentions(vec![CEX_IDS[5].to_string()]), CommitmentConfig::confirmed())
            .await?;
        let (rx_sysprog, _h7) = self.sol_hook
            .subscribe_logs_channel(&ws_url, RpcTransactionLogsFilter::Mentions(vec![SYSTEM_PROGRAM.to_string()]), CommitmentConfig::confirmed())
            .await?;

        let (rx_pump, _) = self.sol_hook
            .subscribe_logs_channel(
                &ws_url,
                RpcTransactionLogsFilter::Mentions(vec![PUMP_FUN_ID.to_string()]),
                CommitmentConfig::confirmed()
            ).await?;

        let (rx_pump_swap, _) = self.sol_hook
            .subscribe_logs_channel(
                &ws_url,
                RpcTransactionLogsFilter::Mentions(vec![PUMP_SWAP_ID.to_string()]),
                CommitmentConfig::confirmed()
            ).await?;

        let mut st_handles = Vec::new();
        for st_rx in [rx_bybit, rx_binance, rx_crypto, rx_okx, rx_binance_us, rx_kraken, rx_sysprog] {
            let me: Arc<Deagle> = Arc::clone(&self.clone());
            st_handles.push(tokio::spawn(async move {
                me.proc_st_task(st_rx).await 
            }));
        }

        let mut pump_handles = Vec::new();
        for pump_rx in [rx_pump] {
            let me: Arc<Deagle> = Arc::clone(&self.clone());
            pump_handles.push(tokio::spawn(async move {
                me.proc_pump_task(pump_rx, sol_price).await 
            }));
        }

        let mut pump_swap_handles = Vec::new();
        for pump_swap_rx in [rx_pump_swap] {
            let me: Arc<Deagle> = Arc::clone(&self.clone());
            pump_swap_handles.push(tokio::spawn(async move {
                me.proc_pump_swap_task(pump_swap_rx, sol_price).await 
            }));
        }

        for h in [st_handles, pump_handles, pump_swap_handles] {
            for h in h {
                let _ = h.await;
            }
        }
        Ok(())  
    }

}
