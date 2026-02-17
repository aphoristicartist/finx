use time::Duration;

use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, FundamentalsBatch, FundamentalsRequest, HealthState,
    HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
};
use crate::{
    AssetClass, Bar, BarSeries, Fundamental, Instrument, Interval, ProviderId, Quote, Symbol,
    UtcDateTime, ValidationError,
};

/// Deterministic Polygon adapter used by the Phase 2 routing pipeline.
#[derive(Debug, Clone)]
pub struct PolygonAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
}

impl Default for PolygonAdapter {
    fn default() -> Self {
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 90,
        }
    }
}

impl PolygonAdapter {
    pub fn with_health(health_state: HealthState, rate_available: bool) -> Self {
        Self {
            health_state,
            rate_available,
            ..Self::default()
        }
    }
}

impl DataSource for PolygonAdapter {
    fn id(&self) -> ProviderId {
        ProviderId::Polygon
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::full()
    }

    fn quote(&self, req: &QuoteRequest) -> Result<QuoteBatch, SourceError> {
        if req.symbols.is_empty() {
            return Err(SourceError::invalid_request(
                "polygon quote request requires at least one symbol",
            ));
        }
        if req.symbols.len() > 3 {
            return Err(SourceError::rate_limited(
                "polygon quote batch limit exceeded (max 3 symbols)",
            ));
        }

        let as_of = UtcDateTime::now();
        let quotes = req
            .symbols
            .iter()
            .map(|symbol| {
                let payload = PolygonQuotePayload::from_symbol(symbol, as_of);
                normalize_quote(payload)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(QuoteBatch { quotes })
    }

    fn bars(&self, req: &BarsRequest) -> Result<BarSeries, SourceError> {
        if req.limit == 0 {
            return Err(SourceError::invalid_request(
                "polygon bars request limit must be greater than zero",
            ));
        }
        if req.interval == Interval::OneMinute && req.limit > 120 {
            return Err(SourceError::rate_limited(
                "polygon free-tier minute bars limit exceeded (max 120)",
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
            let base = 95.0 + ((seed + index as u64 * 3) % 420) as f64 / 10.0;

            let raw = PolygonAggregatePayload {
                ts,
                open: base + 0.05,
                high: base + 1.35,
                low: base - 0.75,
                close: base + 0.42,
                volume: Some(35_000 + (index as u64) * 40),
                vwap: Some(base + 0.20),
            };

            bars.push(normalize_bar(raw)?);
        }

        Ok(BarSeries::new(req.symbol.clone(), req.interval, bars))
    }

    fn fundamentals(&self, req: &FundamentalsRequest) -> Result<FundamentalsBatch, SourceError> {
        if req.symbols.is_empty() {
            return Err(SourceError::invalid_request(
                "polygon fundamentals request requires at least one symbol",
            ));
        }
        if req.symbols.len() > 2 {
            return Err(SourceError::rate_limited(
                "polygon ticker metadata batch limit exceeded (max 2 symbols)",
            ));
        }

        let as_of = UtcDateTime::now();
        let fundamentals = req
            .symbols
            .iter()
            .map(|symbol| {
                let payload = PolygonTickerPayload::from_symbol(symbol, as_of);
                normalize_fundamentals(payload)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(FundamentalsBatch { fundamentals })
    }

    fn search(&self, req: &SearchRequest) -> Result<SearchBatch, SourceError> {
        let query = req.query.trim();
        if query.is_empty() {
            return Err(SourceError::invalid_request(
                "polygon search query must not be empty",
            ));
        }
        if req.limit == 0 {
            return Err(SourceError::invalid_request(
                "polygon search limit must be greater than zero",
            ));
        }

        let query_lower = query.to_ascii_lowercase();
        let results = polygon_catalog()
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
struct PolygonQuotePayload {
    ticker: String,
    last_trade_price: f64,
    bid_price: f64,
    ask_price: f64,
    session_volume: u64,
    currency: &'static str,
    timestamp: UtcDateTime,
}

impl PolygonQuotePayload {
    fn from_symbol(symbol: &Symbol, timestamp: UtcDateTime) -> Self {
        let seed = symbol_seed(symbol);
        let price = 93.5 + (seed % 540) as f64 / 10.0;
        Self {
            ticker: symbol.as_str().to_owned(),
            last_trade_price: price,
            bid_price: price - 0.06,
            ask_price: price + 0.06,
            session_volume: 65_000 + seed % 15_000,
            currency: "USD",
            timestamp,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PolygonAggregatePayload {
    ts: UtcDateTime,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: Option<u64>,
    vwap: Option<f64>,
}

#[derive(Debug, Clone)]
struct PolygonTickerPayload {
    ticker: String,
    as_of: UtcDateTime,
    market_cap: Option<f64>,
    pe_ratio: Option<f64>,
    dividend_yield: Option<f64>,
}

impl PolygonTickerPayload {
    fn from_symbol(symbol: &Symbol, as_of: UtcDateTime) -> Self {
        let seed = symbol_seed(symbol);
        Self {
            ticker: symbol.as_str().to_owned(),
            as_of,
            market_cap: Some(700_000_000_000.0 + (seed % 250_000) as f64 * 1_000_000.0),
            pe_ratio: Some(16.0 + (seed % 250) as f64 / 10.0),
            dividend_yield: Some(0.004 + (seed % 40) as f64 / 10_000.0),
        }
    }
}

fn normalize_quote(payload: PolygonQuotePayload) -> Result<Quote, SourceError> {
    let symbol = Symbol::parse(&payload.ticker).map_err(validation_to_error)?;
    Quote::new(
        symbol,
        payload.last_trade_price,
        Some(payload.bid_price),
        Some(payload.ask_price),
        Some(payload.session_volume),
        payload.currency,
        payload.timestamp,
    )
    .map_err(validation_to_error)
}

fn normalize_bar(payload: PolygonAggregatePayload) -> Result<Bar, SourceError> {
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

fn normalize_fundamentals(payload: PolygonTickerPayload) -> Result<Fundamental, SourceError> {
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

fn polygon_catalog() -> Vec<Instrument> {
    [
        ("AAPL", "Apple Inc.", Some("NASDAQ"), AssetClass::Equity),
        (
            "MSFT",
            "Microsoft Corporation",
            Some("NASDAQ"),
            AssetClass::Equity,
        ),
        (
            "NVDA",
            "NVIDIA Corporation",
            Some("NASDAQ"),
            AssetClass::Equity,
        ),
        ("TSLA", "Tesla, Inc.", Some("NASDAQ"), AssetClass::Equity),
        (
            "SPY",
            "SPDR S&P 500 ETF Trust",
            Some("ARCA"),
            AssetClass::Etf,
        ),
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
    symbol.as_str().bytes().fold(7_u64, |acc, byte| {
        acc.wrapping_mul(37).wrapping_add(byte as u64)
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
