use uuid::Uuid;

use finx_core::{
    Bar, BarRecord, Fundamental, FundamentalRecord, Interval, ProviderId, Quote, QuoteRecord,
    Warehouse, WarehouseError,
};

pub fn sync_quotes(
    source: ProviderId,
    quotes: &[Quote],
    latency_ms: u64,
) -> Result<(), WarehouseError> {
    if quotes.is_empty() {
        return Ok(());
    }

    let warehouse = Warehouse::open_default()?;
    let request_id = format!("quote:{}", Uuid::new_v4());
    let rows = quotes
        .iter()
        .map(|quote| QuoteRecord {
            symbol: quote.symbol.as_str().to_string(),
            price: quote.price,
            bid: quote.bid,
            ask: quote.ask,
            volume: quote.volume,
            currency: quote.currency.clone(),
            as_of: quote.as_of.format_rfc3339(),
        })
        .collect::<Vec<_>>();
    warehouse.ingest_quotes(
        source.as_str(),
        request_id.as_str(),
        rows.as_slice(),
        latency_ms,
    )
}

pub fn sync_bars(
    source: ProviderId,
    interval: Interval,
    bars: &[Bar],
    symbol: &str,
    latency_ms: u64,
) -> Result<(), WarehouseError> {
    if bars.is_empty() {
        return Ok(());
    }

    let warehouse = Warehouse::open_default()?;
    let request_id = format!("bars:{}", Uuid::new_v4());
    let dataset = match interval {
        Interval::OneDay => "bars_1d",
        Interval::OneMinute
        | Interval::FiveMinutes
        | Interval::FifteenMinutes
        | Interval::OneHour => "bars_1m",
    };

    let rows = bars
        .iter()
        .map(|bar| BarRecord {
            symbol: symbol.to_string(),
            ts: bar.ts.format_rfc3339(),
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
        })
        .collect::<Vec<_>>();
    warehouse.ingest_bars(
        source.as_str(),
        dataset,
        request_id.as_str(),
        rows.as_slice(),
        latency_ms,
    )
}

pub fn sync_fundamentals(
    source: ProviderId,
    fundamentals: &[Fundamental],
    latency_ms: u64,
) -> Result<(), WarehouseError> {
    if fundamentals.is_empty() {
        return Ok(());
    }

    let warehouse = Warehouse::open_default()?;
    let request_id = format!("fundamentals:{}", Uuid::new_v4());
    let mut rows = Vec::new();

    for row in fundamentals {
        let date = row.as_of.format_rfc3339();
        if let Some(value) = row.market_cap {
            rows.push(FundamentalRecord {
                symbol: row.symbol.as_str().to_string(),
                metric: String::from("market_cap"),
                value,
                date: date.clone(),
            });
        }
        if let Some(value) = row.pe_ratio {
            rows.push(FundamentalRecord {
                symbol: row.symbol.as_str().to_string(),
                metric: String::from("pe_ratio"),
                value,
                date: date.clone(),
            });
        }
        if let Some(value) = row.dividend_yield {
            rows.push(FundamentalRecord {
                symbol: row.symbol.as_str().to_string(),
                metric: String::from("dividend_yield"),
                value,
                date: date.clone(),
            });
        }
    }

    warehouse.ingest_fundamentals(
        source.as_str(),
        request_id.as_str(),
        rows.as_slice(),
        latency_ms,
    )
}
