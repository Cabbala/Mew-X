use {
    mew::{log, mew::{config::{self, get_use_grpc, get_db_url, get_use_regions}, deagle::deagle::{AlgoConfig, Deagle, DeagleConfig, Source}, snipe::handler::MewSnipe, sol_hook::{goldmine::Goldmine, pump_fun::PumpFun, pump_swap::PumpSwap, sol::SolHook, vacation::Vacation}, writing::{cc, Colors}}, warn}, solana_keypair::Keypair, std::{io::{self, StdoutLock}, sync::Arc, time::Duration}
};

pub const VERSION: &str = "0.1.0";
pub const AUTHOR: &str = "FLOCK4H";

const ART: &str = r#"
⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣠⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⢀⣴⣾⣿⡟⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⢀⣴⠿⢟⣛⣩⣤⣶⣶⣶⣿⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⢀⣴⣿⠿⠸⣿⣿⣿⣿⣿⣿⡿⢿⣿⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⢠⠞⠉⠀⠀⠀⣿⠋⠻⣿⣿⣿⠀⣦⣿⠏⠀⠀⠀⢀⣀⣀⣀⣀⣀⠀⠀
⢠⠏⠀⠀⠀⠀⠀⠻⣤⣷⣿⣿⣿⣶⢟⣁⣒⣒⡋⠉⠉⠁⠀⠀⠀⠈⠉⡧
⢻⡀⠀⠀⠀⠀⠀⣀⡤⠌⢙⣛⣛⣵⣿⣿⡛⠛⠿⠃⠀⠀⠀⠀⠀⢀⡜⠁
⠀⠉⠙⠒⠒⠛⠉⠁⠀⠸⠛⠉⠉⣿⣿⣿⣿⣦⣄⠀⠀⠀⢀⣠⠞⠁⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣿⣿⣿⡿⣿⣿⣷⡄⠞⠋⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⣿⣷⡻⣿⣿⣧⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢨⣑⡙⠻⠿⠿⠈⠙⣿⣧⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠸⣿⣷⡀⠀⠀⠀⠀⢹⣿⣆⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢻⣿⡇⠀⠀⠀⠀⠸⣿⣿⡄⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠁⠀⠀⠀⠀⠀⡿⣿⣿⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠙⠀⠀⠀⠀⠀
"#;

fn intro(colors: &mut Colors<'static>) {
    colors.cprint(&format!("{}{}{}{}", cc::BOLD, cc::LIGHT_WHITE, &ART.replace("{}", &AUTHOR), cc::RESET), cc::BLACK);
    colors.cprint(&format!("            {}{}Welcome to Mew! {}", cc::BOLD, cc::LIGHT_WHITE, cc::RESET), cc::RED);
    colors.cprint(&format!("{}v{} By {}{}", cc::BOLD, &VERSION, &AUTHOR, cc::RESET), cc::LIGHT_MAGENTA);
}

async fn init_dbs(sol: &SolHook, colors: &mut Colors<'static>, pump_fun: &PumpFun, pump_swap: &PumpSwap) -> (Vacation, Goldmine, Arc<Deagle>) {
        // `vacation` DB is used to store validator locations on the Solana cluster.
        let vacs = Vacation::new(sol.clone());
        let db_url = get_db_url();
        let vacs = vacs.initialize((db_url.clone() + "/vacation").as_str()).await;
        if get_use_regions() {
            vacs.fill(Some(500)).await.unwrap();
            colors.cprint("Vacation database filled 🌴", cc::LIGHT_MAGENTA);
        } else {
            colors.cprint("Regions are disabled, skipping vacation database fill 🌴", cc::LIGHT_MAGENTA);
        }
        // `goldmine` DB is used to store token data and duplicates.
        let goldmine = Goldmine::new(sol.clone());
        let mut goldmine = goldmine.initialize((db_url.clone() + "/goldmine").as_str()).await;
        colors.cprint("Goldmine database initialized 💰", cc::LIGHT_YELLOW);

        // Connect to Dexter-v3 trust factor DB if configured
        let algo_config_pre = config::get_algo_config();
        if let Some(ref dt) = algo_config_pre.dexter_trust {
            if dt.use_dexter_trust && !dt.db_url.is_empty() {
                match goldmine.connect_dexter_db(&dt.db_url).await {
                    Ok(_) => colors.cprint("DexterTrust database connected 🎯", cc::LIGHT_GREEN),
                    Err(e) => colors.cprint(&format!("DexterTrust DB connection failed: {e}"), cc::RED),
                }
            }
        }

        colors.cprint("Starting Deagle 🦅", cc::LIGHT_BLUE);
        let is_debug = config::get_deagle_debug();
        let deagle = Arc::new(Deagle::new(
            sol.clone(),
            DeagleConfig {
                min_transfer: config::get_min_transfer_sol(),
                max_transfer: None,
                exclude_accounts: None,
                debug: is_debug,
            },
            goldmine.clone(),
            Arc::new(pump_fun.clone()),
            Arc::new(pump_swap.clone()),
        ));

        (vacs, goldmine, deagle)
}

async fn load_initial_creators(deagle: &Arc<Deagle>, algo_config: AlgoConfig) -> Vec<(String, Source)> {
    let mut attempt = 0u32;
    loop {
        match deagle.algo_choose_creators(algo_config.clone()).await {
            Ok(creators) if creators.is_empty() => {
                attempt += 1;
                let backoff = std::cmp::min(5 * attempt as u64, 60);
                warn!(
                    "Initial creator load returned 0 creators (attempt {attempt}); retrying in {backoff}s"
                );
                tokio::time::sleep(Duration::from_secs(backoff)).await;
            }
            Ok(creators) => return creators,
            Err(e) => {
                attempt += 1;
                let backoff = std::cmp::min(5 * attempt as u64, 60);
                warn!("Initial creator load failed: {e} (attempt {attempt}); retrying in {backoff}s");
                tokio::time::sleep(Duration::from_secs(backoff)).await;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let lock: StdoutLock<'static> = io::stdout().lock();
    let mut colors: Colors<'static> = Colors::new(lock);
    intro(&mut colors);

    let config: config::Config = mew::mew::config::config();
    let (rpc_url, private_key, _ws_url, grpc_url, grpc_token, _nonce_account) = (
        config.rpc_url.clone(), config.private_key.clone(), config.ws_url.clone(), config.grpc_url.clone().unwrap(), config.grpc_token.clone().unwrap(), config.nonce_account.clone().unwrap()
    );
    let use_grpc = get_use_grpc();
    log!("Using config: {}", config.to_string_masked());
    log!("Using regions: {:?}", config::get_regions().unwrap());
    let algo_config = config::get_algo_config();
    log!("{:?}", algo_config);

    let sol = SolHook::new(rpc_url);
    let keypair = Keypair::from_base58_string(&private_key);
    let pump_fun = PumpFun::new(Arc::new(keypair.insecure_clone()), Arc::new(sol.clone()));
    let pump_swap = PumpSwap::new(Arc::new(keypair.insecure_clone()), Arc::new(sol.clone()));

    let (vacs, goldmine, deagle) = init_dbs(&sol, &mut colors, &pump_fun, &pump_swap).await;

    let algo_creators = load_initial_creators(&deagle, algo_config.clone()).await;
    let mut deagles = 0;
    let mut vol_creators = 0;
    let mut grand_chillers = 0;
    let mut dexter_trust = 0;
    for (_, source) in &algo_creators {
        match source {
            Source::Deagle => deagles += 1,
            Source::VolCreators => vol_creators += 1,
            Source::GrandChillers => grand_chillers += 1,
            Source::DexterTrust => dexter_trust += 1,
            _ => {}
        }
    }

    colors.cprint(&format!("Deagles: {}\nVolume Creators: {}\nGrand Chillers: {}\nDexter Trust: {}\nTotal Creators: {}", deagles, vol_creators, grand_chillers, dexter_trust, algo_creators.len()), cc::LIGHT_BLUE);

    if let Err(e) = deagle.clone().analyze_profits().await {
        warn!("Deagle crashed: {e}");
    }

    let snipe = MewSnipe::new(
        sol.clone(), 
        goldmine.clone(), 
        vacs.clone(),
        deagle.clone(),
        pump_fun.clone(), 
        pump_swap.clone(), 
        grpc_url.clone(), 
        grpc_token.clone(), 
        algo_creators,
        algo_config,
    );

    tokio::spawn(async move {
        if let Err(e) = deagle.clone().run().await {
            warn!("Deagle crashed: {e}");
        }
    });

    let snipe_for_refresh = snipe.clone();
    let snipe_for_pumpfun = snipe.clone();
    let snipe_for_pumpswap = snipe.clone();

    let snipe_handle = tokio::spawn(async move {
        if let Err(e) = snipe_for_refresh.refresh_creators_loop().await {
            warn!("Snipe refresh crashed: {e}");
        }
    });
    
    let pump_fun_handle = tokio::spawn(async move {
        loop {
            if use_grpc {
                if let Err(e) = snipe_for_pumpfun.subscribe_grpc_pump_fun().await {
                    warn!("PumpFun snipe crashed: {e} — reconnecting in 5s");
                }
            } else {
                if let Err(e) = snipe_for_pumpfun.subscribe_ws_pump_fun().await {
                    warn!("PumpFun WS crashed: {e} — reconnecting in 5s");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            log!(cc::LIGHT_YELLOW, "Reconnecting PumpFun subscription...");
        }
    });

    let pump_swap_handle = tokio::spawn(async move {
        loop {
            if use_grpc {
                if let Err(e) = snipe_for_pumpswap.subscribe_grpc_pump_swap().await {
                    warn!("PumpSwap snipe crashed: {e} — reconnecting in 5s");
                }
            } else {
                if let Err(e) = snipe_for_pumpswap.subscribe_ws_pump_swap().await {
                    warn!("PumpSwap WS crashed: {e} — reconnecting in 5s");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            log!(cc::LIGHT_YELLOW, "Reconnecting PumpSwap subscription...");
        }
    });

    let _ = tokio::join!(snipe_handle, pump_fun_handle, pump_swap_handle);

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
