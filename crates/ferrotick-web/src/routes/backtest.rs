use crate::error::WebError;
use crate::models::api::{BacktestMetrics, BacktestRequest, BacktestResponse};
use axum::{http::StatusCode, Json};
use ferrotick_backtest::{
    BacktestConfig, BacktestEngine, BacktestReport, BarEvent, Order as BacktestOrder,
    OrderSide as BacktestOrderSide, OrderType as BacktestOrderType, Portfolio as BacktestPortfolio,
    SignalAction as BacktestSignalAction, SignalEvent as BacktestSignalEvent,
    Strategy as BacktestStrategy,
};
use ferrotick_core::{Bar, Symbol, UtcDateTime};
use ferrotick_strategies::{
    BollingerBandSqueezeStrategy, MacdTrendStrategy, MovingAverageCrossoverStrategy,
    RsiMeanReversionStrategy, SignalAction as StrategySignalAction, Strategy as SignalStrategy,
};
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const MIN_SYNTHETIC_BARS: usize = 120;
const MAX_SYNTHETIC_BARS: usize = 750;
const EPSILON_QTY: f64 = 1e-9;

pub async fn run_backtest(
    Json(req): Json<BacktestRequest>,
) -> Result<(StatusCode, Json<BacktestResponse>), WebError> {
    if !req.initial_capital.is_finite() || req.initial_capital <= 0.0 {
        return Err(WebError::InvalidRequest(String::from(
            "initial_capital must be finite and > 0",
        )));
    }

    let symbol = Symbol::parse(&req.symbol)
        .map_err(|err| WebError::InvalidRequest(format!("invalid symbol '{}': {err}", req.symbol)))?;
    let start_date = parse_request_datetime(&req.start_date, false)?;
    let end_date = parse_request_datetime(&req.end_date, true)?;
    ensure_date_range(start_date, end_date)?;

    let (strategy_name, strategy, default_qty) = build_strategy(req.strategy_name.trim(), symbol.as_str())?;
    let bars = generate_synthetic_bar_events(&symbol, start_date, end_date)?;

    let config = BacktestConfig {
        initial_capital: req.initial_capital,
        start_date: Some(start_date),
        end_date: Some(end_date),
        ..BacktestConfig::default()
    };

    let mut engine = BacktestEngine::new(config);
    let mut strategy_adapter = StrategyAdapter { inner: RefCell::new(strategy), default_qty };
    let report: BacktestReport = engine
        .run(&mut strategy_adapter, &bars)
        .await
        .map_err(|err| WebError::Backtest(err.to_string()))?;

    let response = BacktestResponse {
        status: "success".to_string(),
        message: format!(
            "Backtest for strategy '{}' completed on {} synthetic bars",
            strategy_name,
            bars.len()
        ),
        metrics: BacktestMetrics {
            total_return: report.total_return,
            sharpe_ratio: report.sharpe_ratio,
            max_drawdown: report.max_drawdown,
            total_trades: report.trades,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

fn ensure_date_range(start_date: UtcDateTime, end_date: UtcDateTime) -> Result<(), WebError> {
    if end_date <= start_date {
        return Err(WebError::InvalidRequest(format!(
            "end_date ({}) must be after start_date ({})",
            end_date, start_date
        )));
    }
    Ok(())
}

fn parse_request_datetime(raw: &str, end_of_day_for_date_only: bool) -> Result<UtcDateTime, WebError> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(WebError::InvalidRequest(String::from(
            "date values must not be empty",
        )));
    }

    if let Ok(ts) = UtcDateTime::parse(value) {
        return Ok(ts);
    }

    if is_yyyy_mm_dd(value) {
        let suffix = if end_of_day_for_date_only {
            "T23:59:59Z"
        } else {
            "T00:00:00Z"
        };
        let composed = format!("{value}{suffix}");
        return UtcDateTime::parse(&composed).map_err(|_| {
            WebError::InvalidRequest(format!(
                "invalid date '{}': expected RFC3339 UTC or YYYY-MM-DD",
                value
            ))
        });
    }

    Err(WebError::InvalidRequest(format!(
        "invalid date '{}': expected RFC3339 UTC or YYYY-MM-DD",
        value
    )))
}

fn is_yyyy_mm_dd(value: &str) -> bool {
    if value.len() != 10 {
        return false;
    }
    value
        .chars()
        .enumerate()
        .all(|(idx, ch)| match idx {
            4 | 7 => ch == '-',
            _ => ch.is_ascii_digit(),
        })
}

fn build_strategy(
    strategy_name: &str,
    symbol: &str,
) -> Result<(String, Box<dyn SignalStrategy>, f64), WebError> {
    let normalized = strategy_name.trim().to_ascii_lowercase();
    let selected = match normalized.as_str() {
        "" | "ma_crossover" | "moving_average_crossover" => "ma_crossover",
        "rsi_mean_reversion" | "rsi_reversion" => "rsi_mean_reversion",
        "macd_trend" => "macd_trend",
        "bb_squeeze" | "bollinger_band_squeeze" => "bb_squeeze",
        _ => "ma_crossover",
    };

    let (strategy, qty): (Box<dyn SignalStrategy>, f64) = match selected {
        "ma_crossover" => (
            Box::new(
                MovingAverageCrossoverStrategy::new(symbol, 10, 30, 10.0)
                    .map_err(|err| WebError::Backtest(err.to_string()))?,
            ),
            10.0,
        ),
        "rsi_mean_reversion" => (
            Box::new(
                RsiMeanReversionStrategy::new(symbol, 14, 30.0, 70.0, 10.0)
                    .map_err(|err| WebError::Backtest(err.to_string()))?,
            ),
            10.0,
        ),
        "macd_trend" => (
            Box::new(
                MacdTrendStrategy::new(symbol, 12, 26, 9, 10.0)
                    .map_err(|err| WebError::Backtest(err.to_string()))?,
            ),
            10.0,
        ),
        "bb_squeeze" => (
            Box::new(
                BollingerBandSqueezeStrategy::new(symbol, 20, 2.0, 10.0)
                    .map_err(|err| WebError::Backtest(err.to_string()))?,
            ),
            10.0,
        ),
        _ => unreachable!("selected strategy must be one of known names"),
    };

    Ok((selected.to_string(), strategy, qty))
}

fn generate_synthetic_bar_events(
    symbol: &Symbol,
    start_date: UtcDateTime,
    end_date: UtcDateTime,
) -> Result<Vec<BarEvent>, WebError> {
    let start_ts = start_date.into_inner().unix_timestamp();
    let end_ts = end_date.into_inner().unix_timestamp();
    let range_secs = end_ts - start_ts;
    if range_secs <= 0 {
        return Err(WebError::InvalidRequest(String::from(
            "date range must be positive",
        )));
    }

    let approx_daily_bars = (range_secs / 86_400 + 1) as usize;
    let bar_count = approx_daily_bars.clamp(MIN_SYNTHETIC_BARS, MAX_SYNTHETIC_BARS);
    let step_seconds = range_secs as f64 / (bar_count.saturating_sub(1).max(1) as f64);

    let seed = deterministic_seed(symbol.as_str(), start_ts, end_ts);
    let mut rng = DeterministicRng::new(seed);

    let symbol_bucket = (seed % 25_000) as f64;
    let mut price = 40.0 + symbol_bucket / 100.0 + rng.next_f64() * 20.0;
    let drift = 0.0002 + (rng.next_f64() - 0.5) * 0.0002;
    let volatility = 0.006 + rng.next_f64() * 0.016;
    let base_volume = 50_000.0 + ((seed >> 8) % 250_000) as f64;

    let mut events = Vec::with_capacity(bar_count);
    for idx in 0..bar_count {
        let timestamp_seconds =
            start_ts + (step_seconds * idx as f64).round() as i64;
        let ts = UtcDateTime::from_unix_timestamp(timestamp_seconds.min(end_ts)).map_err(|err| {
            WebError::Internal(format!("failed generating synthetic timestamp: {err}"))
        })?;

        let cycle = ((idx as f64) / 18.0).sin() * 0.0015;
        let noise = (rng.next_f64() - 0.5) * 2.0 * volatility;
        let gap_return = drift * 0.35 + cycle * 0.4 + noise * 0.25;
        let intraday_return = drift + cycle + noise;

        let open = (price * (1.0 + gap_return)).max(0.01);
        let close = (open * (1.0 + intraday_return)).max(0.01);

        let wick_scale = open.max(close) * (0.002 + rng.next_f64() * 0.01);
        let high = (open.max(close) + wick_scale * (0.5 + rng.next_f64())).max(open.max(close));
        let low = (open.min(close) - wick_scale * (0.5 + rng.next_f64())).max(0.01);
        let volume = (base_volume * (0.7 + rng.next_f64() * 0.9) * (1.0 + intraday_return.abs() * 35.0))
            .round()
            .max(1.0) as u64;
        let vwap = Some(((open + high + low + close) / 4.0).clamp(low, high));

        let bar = Bar::new(ts, open, high, low, close, Some(volume), vwap).map_err(|err| {
            WebError::Internal(format!("failed generating synthetic bar {}: {err}", idx + 1))
        })?;

        events.push(BarEvent::new(symbol.clone(), bar));
        price = close;
    }

    Ok(events)
}

fn deterministic_seed(symbol: &str, start_ts: i64, end_ts: i64) -> u64 {
    let mut hasher = DefaultHasher::new();
    symbol.hash(&mut hasher);
    start_ts.hash(&mut hasher);
    end_ts.hash(&mut hasher);
    hasher.finish().max(1)
}

struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        // xorshift requires non-zero state.
        Self { state: seed.max(1) }
    }

    fn next_f64(&mut self) -> f64 {
        self.next_u64() as f64 / u64::MAX as f64
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}

struct StrategyAdapter {
    inner: RefCell<Box<dyn SignalStrategy>>,
    default_qty: f64,
}

impl BacktestStrategy for StrategyAdapter {
    fn on_bar(
        &mut self,
        bar: &BarEvent,
        _portfolio: &BacktestPortfolio,
    ) -> Option<BacktestSignalEvent> {
        let signal = self.inner.borrow_mut().on_bar(&bar.bar)?;
        let action = map_strategy_action_to_backtest(signal.action);
        let ts = UtcDateTime::parse(&signal.ts).unwrap_or(bar.bar.ts);
        Some(BacktestSignalEvent {
            symbol: bar.symbol.clone(),
            ts,
            action,
            strength: signal.strength.clamp(0.0, 1.0),
            reason: signal.reason,
        })
    }

    fn create_order(
        &self,
        signal: &BacktestSignalEvent,
        portfolio: &BacktestPortfolio,
        _config: &BacktestConfig,
    ) -> Option<BacktestOrder> {
        let side = match signal.action {
            BacktestSignalAction::Buy => BacktestOrderSide::Buy,
            BacktestSignalAction::Sell => BacktestOrderSide::Sell,
            BacktestSignalAction::Hold => return None,
        };

        let mut quantity = self.default_qty;
        if !quantity.is_finite() || quantity <= EPSILON_QTY {
            return None;
        }

        match side {
            BacktestOrderSide::Buy => {
                let current_price = portfolio.current_price(&signal.symbol).max(1e-9);
                let affordable_quantity = portfolio.cash() / current_price;
                if !affordable_quantity.is_finite() || affordable_quantity <= EPSILON_QTY {
                    return None;
                }
                quantity = quantity.min(affordable_quantity);
            }
            BacktestOrderSide::Sell => {
                let available_quantity = portfolio.position(&signal.symbol);
                if !available_quantity.is_finite() || available_quantity <= EPSILON_QTY {
                    return None;
                }
                quantity = quantity.min(available_quantity);
            }
        }

        if quantity <= EPSILON_QTY {
            return None;
        }

        Some(BacktestOrder::new(
            signal.symbol.clone(),
            side,
            BacktestOrderType::Market,
            quantity,
            None,
            None,
            signal.ts,
        ))
    }
}

fn map_strategy_action_to_backtest(action: StrategySignalAction) -> BacktestSignalAction {
    match action {
        StrategySignalAction::Buy => BacktestSignalAction::Buy,
        StrategySignalAction::Sell => BacktestSignalAction::Sell,
        StrategySignalAction::Hold => BacktestSignalAction::Hold,
    }
}
