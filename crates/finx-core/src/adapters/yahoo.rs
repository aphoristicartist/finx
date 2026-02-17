use time::Duration;

use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, FundamentalsBatch, FundamentalsRequest, HealthState,
    HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
};
use crate::{
    AssetClass, Bar, BarSeries, Fundamental, Instrument, Interval, ProviderId, Quote, Symbol,
    UtcDateTime, ValidationError,
};

/// Deterministic Yahoo adapter used by the Phase 2 routing pipeline.
#[derive(Debug, Clone)]
pub struct YahooAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
}

impl Default for YahooAdapter {
    fn default() -> Self {
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 78,
        }
    }
}

impl YahooAdapter {
    pub fn with_health(health_state: HealthState, rate_available: bool) -> Self {
        Self {
            health_state,
            rate_available,
            ..Self::default()
        }
    }
}

impl DataSource for YahooAdapter {
    fn id(&self) -> ProviderId {
        ProviderId::Yahoo
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::full()
    }

    fn quote(&self, req: &QuoteRequest) -> Result<QuoteBatch, SourceError> {
        if req.symbols.is_empty() {
            return Err(SourceError::invalid_request(
                "yahoo quote request requires at least one symbol",
            ));
        }

        let as_of = UtcDateTime::now();
        let quotes = req
            .symbols
            .iter()
            .map(|symbol| {
                let payload = YahooQuotePayload::from_symbol(symbol, as_of);
                normalize_quote(payload)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(QuoteBatch { quotes })
    }

    fn bars(&self, req: &BarsRequest) -> Result<BarSeries, SourceError> {
        if req.limit == 0 {
            return Err(SourceError::invalid_request(
                "yahoo bars request limit must be greater than zero",
            ));
        }

        let step = interval_duration(req.interval);
        let now = UtcDateTime::now().into_inner();
        let seed = symbol_seed(&req.symbol);
        let mut bars = Vec::with_capacity(req.limit);

        for index in 0..req.limit {
            let offset = step * (req.limit.saturating_sub(index + 1) as i32);
            let ts =
                UtcDateTime::from_offset_datetime(now - offset).map_err(validation_to_error)?;
            let base = 90.0 + ((seed + index as u64) % 350) as f64 / 10.0;

            let raw = YahooBarPayload {
                ts,
                open: base,
                high: base + 1.20,
                low: base - 0.80,
                close: base + 0.30,
                volume: Some(20_000 + (index as u64) * 25),
                vwap: Some(base + 0.15),
            };

            bars.push(normalize_bar(raw)?);
        }

        Ok(BarSeries::new(req.symbol.clone(), req.interval, bars))
    }

    fn fundamentals(&self, req: &FundamentalsRequest) -> Result<FundamentalsBatch, SourceError> {
        if req.symbols.is_empty() {
            return Err(SourceError::invalid_request(
                "yahoo fundamentals request requires at least one symbol",
            ));
        }

        let as_of = UtcDateTime::now();
        let fundamentals = req
            .symbols
            .iter()
            .map(|symbol| {
                let payload = YahooFundamentalsPayload::from_symbol(symbol, as_of);
                normalize_fundamentals(payload)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(FundamentalsBatch { fundamentals })
    }

    fn search(&self, req: &SearchRequest) -> Result<SearchBatch, SourceError> {
        let query = req.query.trim();
        if query.is_empty() {
            return Err(SourceError::invalid_request(
                "yahoo search query must not be empty",
            ));
        }
        if req.limit == 0 {
            return Err(SourceError::invalid_request(
                "yahoo search limit must be greater than zero",
            ));
        }

        let query_lower = query.to_ascii_lowercase();
        let results = yahoo_catalog()
            .into_iter()
            .filter(|instrument| {
                instrument
                    .symbol
                    .as_str()
                    .to_ascii_lowercase()
                    .contains(&query_lower)
                    || instrument.name.to_ascii_lowercase().contains(&query_lower)
            })
            .take(req.limit)
            .collect::<Vec<_>>();

        Ok(SearchBatch {
            query: query.to_owned(),
            results,
        })
    }

    fn health(&self) -> HealthStatus {
        HealthStatus::new(self.health_state, self.rate_available, self.score)
    }
}

#[derive(Debug, Clone)]
struct YahooQuotePayload {
    ticker: String,
    regular_market_price: f64,
    regular_market_bid: f64,
    regular_market_ask: f64,
    regular_market_volume: u64,
    currency: &'static str,
    timestamp: UtcDateTime,
}

impl YahooQuotePayload {
    fn from_symbol(symbol: &Symbol, timestamp: UtcDateTime) -> Self {
        let seed = symbol_seed(symbol);
        let price = 92.0 + (seed % 500) as f64 / 10.0;
        Self {
            ticker: symbol.as_str().to_owned(),
            regular_market_price: price,
            regular_market_bid: price - 0.08,
            regular_market_ask: price + 0.08,
            regular_market_volume: 50_000 + seed % 10_000,
            currency: "USD",
            timestamp,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct YahooBarPayload {
    ts: UtcDateTime,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: Option<u64>,
    vwap: Option<f64>,
}

#[derive(Debug, Clone)]
struct YahooFundamentalsPayload {
    ticker: String,
    as_of: UtcDateTime,
    market_cap: Option<f64>,
    pe_ratio: Option<f64>,
    dividend_yield: Option<f64>,
}

impl YahooFundamentalsPayload {
    fn from_symbol(symbol: &Symbol, as_of: UtcDateTime) -> Self {
        let seed = symbol_seed(symbol);
        Self {
            ticker: symbol.as_str().to_owned(),
            as_of,
            market_cap: Some(500_000_000_000.0 + (seed % 300_000) as f64 * 1_000_000.0),
            pe_ratio: Some(14.0 + (seed % 200) as f64 / 10.0),
            dividend_yield: Some(0.005 + (seed % 50) as f64 / 10_000.0),
        }
    }
}

fn normalize_quote(payload: YahooQuotePayload) -> Result<Quote, SourceError> {
    let symbol = Symbol::parse(&payload.ticker).map_err(validation_to_error)?;
    Quote::new(
        symbol,
        payload.regular_market_price,
        Some(payload.regular_market_bid),
        Some(payload.regular_market_ask),
        Some(payload.regular_market_volume),
        payload.currency,
        payload.timestamp,
    )
    .map_err(validation_to_error)
}

fn normalize_bar(payload: YahooBarPayload) -> Result<Bar, SourceError> {
    Bar::new(
        payload.ts,
        payload.open,
        payload.high,
        payload.low,
        payload.close,
        payload.volume,
        payload.vwap,
    )
    .map_err(validation_to_error)
}

fn normalize_fundamentals(payload: YahooFundamentalsPayload) -> Result<Fundamental, SourceError> {
    let symbol = Symbol::parse(&payload.ticker).map_err(validation_to_error)?;
    Fundamental::new(
        symbol,
        payload.as_of,
        payload.market_cap,
        payload.pe_ratio,
        payload.dividend_yield,
    )
    .map_err(validation_to_error)
}

fn yahoo_catalog() -> Vec<Instrument> {
    [
        ("AAPL", "Apple Inc.", Some("NASDAQ"), AssetClass::Equity),
        (
            "MSFT",
            "Microsoft Corporation",
            Some("NASDAQ"),
            AssetClass::Equity,
        ),
        (
            "SPY",
            "SPDR S&P 500 ETF Trust",
            Some("ARCA"),
            AssetClass::Etf,
        ),
        ("QQQ", "Invesco QQQ Trust", Some("NASDAQ"), AssetClass::Etf),
    ]
    .into_iter()
    .map(|(symbol, name, exchange, asset_class)| {
        Instrument::new(
            Symbol::parse(symbol).expect("catalog symbols are valid"),
            name,
            exchange.map(str::to_owned),
            "USD",
            asset_class,
            true,
        )
        .expect("catalog entries are valid")
    })
    .collect::<Vec<_>>()
}

fn symbol_seed(symbol: &Symbol) -> u64 {
    symbol.as_str().bytes().fold(0_u64, |acc, byte| {
        acc.wrapping_mul(33).wrapping_add(byte as u64)
    })
}

fn interval_duration(interval: Interval) -> Duration {
    match interval {
        Interval::OneMinute => Duration::minutes(1),
        Interval::FiveMinutes => Duration::minutes(5),
        Interval::FifteenMinutes => Duration::minutes(15),
        Interval::OneHour => Duration::hours(1),
        Interval::OneDay => Duration::days(1),
    }
}

fn validation_to_error(error: ValidationError) -> SourceError {
    SourceError::internal(error.to_string())
}
