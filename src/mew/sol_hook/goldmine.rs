#[allow(unused_imports)]
use {
    sqlx::{Pool, Postgres, FromRow},
    sqlx::postgres::PgPoolOptions,
    crate::mew::sol_hook::sol::SolHook,
    crate::mew::writing::{cc, Colors},
    crate::mew::deagle::deagle::Deagle,
    sqlx::types::Json,
    std::collections::HashMap,
    serde::{Serialize, Deserialize}
};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CreatorMediansRow {
    pub creator: String,
    pub median_token_high_mc: f64,
    pub median_buys: i64,
    pub mints: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Volume {
    pub buys: i64,
    pub sells: i64,
    pub sol_in: i64,
    pub sol_out: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Holder {
    pub total_sol_spent: f64,
    pub total_token_bought: f64,
    pub is_holding: bool,
}

#[derive(Debug, Clone, sqlx::FromRow, Default)]
pub struct Token {
    pub mint: String,
    pub pool: String,
    pub origin_id: String,
    pub current_id: String,
    pub name: String,
    pub symbol: String,
    pub decimals: i32,
    pub price: f64,
    pub open_price: f64,
    pub volume: Json<Volume>,
    pub market_cap: f64,
    pub high_market_cap: f64,
    pub dev_sold: bool,
    pub dev_buy_amount: f64,
    pub dev_address: String,
    pub holders: Json<HashMap<String, Holder>>,
    pub seen_at: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DeagleRow {
    pub wallet: String,
    pub last_source: String,
    pub sol_amount: f64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Dupe {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub creators: Json<Vec<String>>,
    pub count: i32,
}

#[derive(Clone)]
pub struct Goldmine {
    pub sol_hook: SolHook,
    pub db_pool: Option<Pool<Postgres>>,
    pub dexter_pool: Option<Pool<Postgres>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CreatorVolume {
    pub creator: String,
    pub sol_out: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BlacklistRow {
    pub creator: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TwitterRow {
    pub twitter: String,
    pub score: i32,
    pub creators: Json<Vec<String>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Sim {
    pub mint: String,
    pub bonding_curve: String,
    pub price: f64,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub creator: String,
    pub creator_sold: bool,
    pub creator_token_amount: f64,
    pub buys: i64,
    pub sells: i64,
    pub volume: f64,
    pub liquidity: f64,
    pub txns_in_zero: i64,
    pub first_seen_slot: i64,
    pub last_seen_slot: i64,
    pub profit: f64,
    pub highest_profit: f64,
}

impl Goldmine {
    pub fn new(sol_hook: SolHook) -> Self {
        Self {
            sol_hook,
            db_pool: None,
            dexter_pool: None,
        }
    }

    pub async fn initialize(mut self, db_url: &str) -> Self {
        let db_res = self.connect_db(&db_url).await;
        match db_res {
            Ok(_) => {
                self.ensure_table().await.unwrap();
            }
            Err(e) => {
                eprintln!("Error connecting to database: {}", e);
            }
        }
        self
    }

    pub async fn connect_db(&mut self, database_url: &str) -> anyhow::Result<()> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        self.db_pool = Some(pool);
        Ok(())
    }

    fn db(&self) -> anyhow::Result<&Pool<Postgres>> {
        match &self.db_pool {
            Some(p) => Ok(p),
            None => anyhow::bail!("database is not connected; call connect_db() first"),
        }
    }

    pub async fn ensure_table(&self) -> anyhow::Result<()> {
        let pool = self.db()?;
        let create_tokens = r#"
            CREATE TABLE IF NOT EXISTS tokens (
                mint            TEXT PRIMARY KEY,
                pool            TEXT NOT NULL,
                origin_id       TEXT NOT NULL,
                current_id      TEXT NOT NULL,
                name            TEXT NOT NULL,
                symbol          TEXT NOT NULL,
                decimals        INTEGER NOT NULL,
    
                price           DOUBLE PRECISION NOT NULL DEFAULT 0,
                open_price      DOUBLE PRECISION NOT NULL DEFAULT 0,
                volume          JSONB NOT NULL DEFAULT '{}'::jsonb,
    
                market_cap      DOUBLE PRECISION NOT NULL DEFAULT 0,
                high_market_cap DOUBLE PRECISION NOT NULL DEFAULT 0,
    
                dev_sold        BOOLEAN NOT NULL DEFAULT FALSE,
                dev_buy_amount  DOUBLE PRECISION NOT NULL DEFAULT 0,
                dev_address     TEXT NOT NULL,
    
                holders         JSONB NOT NULL DEFAULT '{}'::jsonb,
    
                seen_at         BIGINT NOT NULL,
    
                created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
            );
        "#;
    
        let create_dupes = r#"
            CREATE TABLE IF NOT EXISTS dupes (
                name        TEXT PRIMARY KEY,
                symbol      TEXT NOT NULL,
                uri         TEXT NOT NULL,
                creators    JSONB NOT NULL DEFAULT '{}'::jsonb,
                count       INTEGER NOT NULL,
                created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
            );
        "#;
    
        let create_deagle = r#"
            CREATE TABLE IF NOT EXISTS deagle (
                wallet      TEXT PRIMARY KEY,
                last_source TEXT NOT NULL,
                sol_amount  DOUBLE PRECISION NOT NULL DEFAULT 0,
                created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
            );
        "#;

        let create_blacklist = r#"
            CREATE TABLE IF NOT EXISTS blacklist (
                creator TEXT PRIMARY KEY,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );
        "#;

        let create_twitters = r#"
            CREATE TABLE IF NOT EXISTS twitters (
                twitter TEXT PRIMARY KEY,
                score INT NOT NULL DEFAULT 0,
                creators JSONB NOT NULL DEFAULT '{}'::jsonb,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );
        "#;

        let create_sims = r#"
            CREATE TABLE IF NOT EXISTS sims (
                mint TEXT PRIMARY KEY,
                bonding_curve TEXT NOT NULL,
                price DOUBLE PRECISION NOT NULL,
                name TEXT NOT NULL,
                symbol TEXT NOT NULL,
                uri TEXT NOT NULL,
                creator TEXT NOT NULL,
                creator_sold BOOLEAN NOT NULL,
                creator_token_amount DOUBLE PRECISION NOT NULL,
                buys BIGINT NOT NULL,
                sells BIGINT NOT NULL,
                volume DOUBLE PRECISION NOT NULL,
                liquidity DOUBLE PRECISION NOT NULL,
                txns_in_zero BIGINT NOT NULL,
                first_seen_slot BIGINT NOT NULL,
                last_seen_slot BIGINT NOT NULL,
                profit DOUBLE PRECISION NOT NULL,
                highest_profit DOUBLE PRECISION NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            );
        "#;

        let index_tokens = r#"
            CREATE INDEX IF NOT EXISTS tokens_current_hmc_idx
                ON tokens (current_id, high_market_cap DESC, mint DESC);
        "#;
    
        sqlx::query(create_tokens).execute(pool).await?;
        sqlx::query(create_dupes).execute(pool).await?;
        sqlx::query(create_deagle).execute(pool).await?;
        sqlx::query(create_blacklist).execute(pool).await?;
        sqlx::query(create_twitters).execute(pool).await?;
        sqlx::query(index_tokens).execute(pool).await?;
        sqlx::query(create_sims).execute(pool).await?;
        Ok(())
    }

    pub async fn upsert_token(&self, t: &Token) -> anyhow::Result<()> {
        let sql = r#"
            INSERT INTO tokens (
                mint, pool, origin_id, current_id, name, symbol, decimals,
                price, open_price, volume, market_cap, high_market_cap,
                dev_sold, dev_buy_amount, dev_address, holders, seen_at
            )
            VALUES (
                $1,$2,$3,$4,$5,$6,$7,
                $8,$9,$10,$11,$12,
                $13,$14,$15,$16,$17
            )
            ON CONFLICT (mint) DO UPDATE SET
                pool             = EXCLUDED.pool,
                current_id       = EXCLUDED.current_id,
                name             = EXCLUDED.name,
                symbol           = EXCLUDED.symbol,
                decimals         = EXCLUDED.decimals,
                price            = EXCLUDED.price,
                open_price       = EXCLUDED.open_price,
                volume           = EXCLUDED.volume,
                market_cap       = EXCLUDED.market_cap,
                high_market_cap  = EXCLUDED.high_market_cap,
                dev_sold         = EXCLUDED.dev_sold,
                dev_buy_amount   = EXCLUDED.dev_buy_amount,
                dev_address      = EXCLUDED.dev_address,
                holders          = EXCLUDED.holders,
                seen_at          = EXCLUDED.seen_at,
                updated_at       = now()
        "#;
    
        sqlx::query(sql)
            .bind(&t.mint)
            .bind(&t.pool)
            .bind(&t.origin_id)
            .bind(&t.current_id)
            .bind(&t.name)
            .bind(&t.symbol)
            .bind(t.decimals)
            .bind(t.price)
            .bind(t.open_price)
            .bind(t.volume.clone())
            .bind(t.market_cap)
            .bind(t.high_market_cap)
            .bind(t.dev_sold)
            .bind(t.dev_buy_amount)
            .bind(&t.dev_address)
            .bind(t.holders.clone())
            .bind(t.seen_at)
            .execute(self.db()?)
            .await?;
        Ok(())
    }
    
    pub async fn upsert_twitters(&self, twitter: &str, score: i32, creators: Json<Vec<String>>) -> anyhow::Result<()> {
        let sql = r#"
            INSERT INTO twitters (twitter, score, creators) VALUES ($1, $2, $3)
            ON CONFLICT (twitter) DO UPDATE SET score = EXCLUDED.score, creators = EXCLUDED.creators;
        "#;
        sqlx::query(sql).bind(twitter).bind(score).bind(creators.clone()).execute(self.db()?).await?;
        Ok(())
    }

    pub async fn get_twitter(&self, twitter: &str) -> anyhow::Result<Option<TwitterRow>> {
        let sql = r#"SELECT * FROM twitters WHERE twitter = $1"#;
        let row = sqlx::query_as::<_, TwitterRow>(sql).bind(twitter).fetch_optional(self.db()?).await?;
        Ok(row)
    }

    pub async fn upsert_blacklist(&self, creator: &str) -> anyhow::Result<()> {
        let sql = r#"
            INSERT INTO blacklist (creator) VALUES ($1)
            ON CONFLICT (creator) DO UPDATE SET updated_at = now();
        "#;
        sqlx::query(sql).bind(creator).execute(self.db()?).await?;
        Ok(())
    }

    pub async fn blacklist_check(&self, creator: &str) -> anyhow::Result<bool> {
        let sql = r#"
            SELECT * FROM blacklist WHERE creator = $1
        "#;
        let row = sqlx::query_as::<_, BlacklistRow>(sql).bind(creator).fetch_optional(self.db()?).await?;
        Ok(row.is_some())
    }

    pub async fn get_token(&self, mint: &str) -> anyhow::Result<Option<Token>> {
        let sql = r#"SELECT * FROM tokens WHERE mint = $1"#;
        let row = sqlx::query_as::<_, Token>(sql)
            .bind(mint)
            .fetch_optional(self.db()?)
            .await?;
        Ok(row)
    }
    

    pub async fn get_tokens(&self, creator: &str) -> anyhow::Result<Vec<Token>> {
        let sql = r#"SELECT * FROM tokens WHERE dev_address = $1"#;
        let rows = sqlx::query_as::<_, Token>(sql)
            .bind(creator)
            .fetch_all(self.db()?)
            .await?;
        Ok(rows)
    }

    pub async fn get_creators_medians(&self, limit: i64) -> anyhow::Result<Vec<CreatorMediansRow>> {
        let sql = r#"
            SELECT
                dev_address                           AS creator,
                COUNT(*)::bigint                      AS mints,
                percentile_disc(0.5) WITHIN GROUP (ORDER BY high_market_cap)
                                                     AS median_token_high_mc,
                percentile_disc(0.5) WITHIN GROUP (
                    ORDER BY COALESCE((volume->>'buys')::bigint, 0)
                )                                    AS median_buys
            FROM tokens
            GROUP BY dev_address
            LIMIT $1
        "#;

        let rows = sqlx::query_as::<_, CreatorMediansRow>(sql)
            .bind(limit)
            .fetch_all(self.db()?)
            .await?;
        Ok(rows)
    }

    pub async fn top_tokens_by_hmc(
        &self,
        current_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<Token>> {
        let sql = r#"
            SELECT *
            FROM tokens
            WHERE current_id = $1
            ORDER BY high_market_cap DESC
            LIMIT $2
        "#;
        let rows = sqlx::query_as::<_, Token>(sql)
            .bind(current_id)
            .bind(limit)
            .fetch_all(self.db()?)
            .await?;
        Ok(rows)
    }

    pub async fn upsert_dupes(&self, d: &Dupe) -> anyhow::Result<()> {
        let sql = r#"
            INSERT INTO dupes (name, symbol, uri, creators, count)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (name) DO UPDATE SET count = EXCLUDED.count, creators = EXCLUDED.creators;
        "#;

        sqlx::query(sql)
            .bind(&d.name)
            .bind(&d.symbol)
            .bind(&d.uri)
            .bind(d.creators.clone())
            .bind(d.count)
            .execute(self.db()?)
            .await?;
        Ok(())
    }

    pub async fn get_dupes(&self, name: &str, symbol: &str, uri: &str) -> anyhow::Result<Option<Dupe>> {
        let sql = r#"
            SELECT * FROM dupes WHERE name = $1 OR symbol = $2 OR uri = $3
        "#;
        let res = sqlx::query_as::<_, Dupe>(sql).bind(name).bind(symbol).bind(uri).fetch_optional(self.db()?).await?;
        Ok(res)
    }

    pub async fn upsert_deagle(&self, w: &DeagleRow) -> anyhow::Result<()> {
        let sql = r#"
            INSERT INTO deagle (wallet, last_source, sol_amount)
            VALUES ($1, $2, $3)
            ON CONFLICT (wallet) DO UPDATE SET last_source = EXCLUDED.last_source, sol_amount = EXCLUDED.sol_amount;
        "#;

        sqlx::query(sql).bind(&w.wallet).bind(&w.last_source).bind(&w.sol_amount).execute(self.db()?).await?;
        Ok(())
    }

    pub async fn get_deagle(&self, wallet: &str) -> anyhow::Result<Option<DeagleRow>> {
        let sql = r#"
            SELECT * FROM deagle WHERE wallet = $1
        "#;
        let res = sqlx::query_as::<_, DeagleRow>(sql)
            .bind(wallet)
            .fetch_optional(self.db()?)
            .await?;
        Ok(res)
    }

    pub async fn get_deagles_by_sol_amount(&self, limit: i64) -> anyhow::Result<Vec<DeagleRow>> {
        let sql = r#"
            SELECT * FROM deagle
            ORDER BY sol_amount DESC
            LIMIT $1
        "#;
        let rows = sqlx::query_as::<_, DeagleRow>(sql).bind(limit).fetch_all(self.db()?).await?;
        Ok(rows)
    }

    pub async fn upsert_sim(&self, s: &Sim) -> anyhow::Result<()> {
        let sql = r#"
            INSERT INTO sims (
                mint,
                bonding_curve,
                price,
                name,
                symbol,
                uri,
                creator,
                creator_sold,
                creator_token_amount,
                buys,
                sells,
                volume,
                liquidity,
                txns_in_zero,
                first_seen_slot,
                last_seen_slot,
                profit,
                highest_profit  
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            ON CONFLICT (mint) DO UPDATE SET
                bonding_curve = EXCLUDED.bonding_curve,
                price = EXCLUDED.price,
                name = EXCLUDED.name,
                symbol = EXCLUDED.symbol,
                uri = EXCLUDED.uri,
                creator = EXCLUDED.creator,
                creator_sold = EXCLUDED.creator_sold,
                creator_token_amount = EXCLUDED.creator_token_amount,
                buys = EXCLUDED.buys,
                sells = EXCLUDED.sells,
                volume = EXCLUDED.volume,
                liquidity = EXCLUDED.liquidity,
                txns_in_zero = EXCLUDED.txns_in_zero,
                first_seen_slot = EXCLUDED.first_seen_slot,
                last_seen_slot = EXCLUDED.last_seen_slot,
                profit = EXCLUDED.profit,
                highest_profit = EXCLUDED.highest_profit;
        "#;

        sqlx::query(sql).bind(&s.mint).bind(&s.bonding_curve).bind(&s.price).bind(&s.name).bind(&s.symbol).bind(&s.uri).bind(&s.creator).bind(&s.creator_sold).bind(&s.creator_token_amount).bind(&s.buys).bind(&s.sells).bind(&s.volume).bind(&s.liquidity).bind(&s.txns_in_zero).bind(&s.first_seen_slot).bind(&s.last_seen_slot).bind(&s.profit).bind(&s.highest_profit).execute(self.db()?).await?;
        Ok(())
    }

    pub async fn get_sims(&self) -> anyhow::Result<Vec<Sim>> {
        let sql = r#"SELECT * FROM sims"#;
        let rows = sqlx::query_as::<_, Sim>(sql).fetch_all(self.db()?).await?;
        Ok(rows)
    }

    // --- DexterTrust integration ---

    pub async fn connect_dexter_db(&mut self, database_url: &str) -> anyhow::Result<()> {
        let pool = PgPoolOptions::new()
            .max_connections(3)
            .connect(database_url)
            .await?;
        self.dexter_pool = Some(pool);
        Ok(())
    }

    fn dexter_db(&self) -> anyhow::Result<&Pool<Postgres>> {
        match &self.dexter_pool {
            Some(p) => Ok(p),
            None => anyhow::bail!("Dexter trust database is not connected"),
        }
    }

    /// Fetch high-trust creator addresses from Dexter-v3's latest leaderboard.
    pub async fn get_dexter_trusted_creators(&self, min_trust_factor: f64) -> anyhow::Result<Vec<String>> {
        let pool = self.dexter_db()?;
        let sql = r#"
            SELECT cs.creator
            FROM phase2_creator_snapshots cs
            WHERE cs.leaderboard_version = (
                SELECT version FROM phase2_leaderboard_generations
                ORDER BY generated_at DESC LIMIT 1
            )
            AND cs.trust_factor >= $1
            ORDER BY cs.performance_score DESC
        "#;

        let rows: Vec<(String,)> = sqlx::query_as(sql)
            .bind(min_trust_factor)
            .fetch_all(pool)
            .await?;

        Ok(rows.into_iter().map(|(c,)| c).collect())
    }
}