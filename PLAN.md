# Task: Implement Ferrotick Phase 9 - Strategy Library

## Objective
Add a new `ferrotick-strategies` crate and wire it into `ferrotick-cli` so users can list built-in strategies, validate YAML specs, and run backtests from those specs.

## Requirements
1. A new workspace member `crates/ferrotick-strategies` exists and builds with dependencies: `ferrotick-core`, `ferrotick-ml`, `serde`, `serde_yaml`, and `thiserror`.
2. The new crate exposes a `Strategy` trait that is explicitly `Send + Sync`.
3. The new crate includes exactly four built-in strategies: `ma_crossover`, `rsi_mean_reversion`, `macd_trend`, and `bollinger_squeeze`.
4. Position sizing supports exactly four methods: `Fixed`, `Percent`, `VolatilityAdjusted`, and `Kelly`, with deterministic formulas and input validation.
5. YAML parsing is implemented via `serde_yaml` and supports file + string parsing.
6. Strategy validation exists as a separate component (parser and validator are not conflated).
7. A signal framework exists with composite methods: `all`, `any`, `majority`, `weighted`.
8. A compiler converts validated `StrategySpec` into executable runtime (`Box<dyn Strategy>` + sizing/backtest config).
9. `ferrotick-cli` adds `strategy` command group with subcommands `list`, `validate`, and `backtest`.
10. `strategy backtest` runs through `ferrotick-backtest::BacktestEngine` (not a custom loop).
11. Strategy/backtest command output is returned in existing envelope/JSON style via `CommandResult`.
12. Unit/integration tests are added for parser, validator, sizing, signal composition, and CLI argument parsing.

## Step-by-Step Implementation

### Step 1: Register the new crate in the workspace
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/Cargo.toml`
**Action:** modify
**Location:** In `[workspace].members` and `[workspace.dependencies]` blocks.
**What to do:**
Add `crates/ferrotick-strategies` to workspace members and add `serde_yaml` as a workspace dependency.
**Code:**
```toml
# BEFORE
members = [
  "crates/ferrotick-core",
  "crates/ferrotick-cli",
  "crates/ferrotick-warehouse",
  "crates/ferrotick-agent",
  "crates/ferrotick-ml",
  "crates/ferrotick-backtest",
]

[workspace.dependencies]
serde = { version = "1.0.218", features = ["derive"] }
serde_json = "1.0.139"
thiserror = "2.0.11"

# AFTER
members = [
  "crates/ferrotick-core",
  "crates/ferrotick-cli",
  "crates/ferrotick-warehouse",
  "crates/ferrotick-agent",
  "crates/ferrotick-ml",
  "crates/ferrotick-backtest",
  "crates/ferrotick-strategies",
]

[workspace.dependencies]
serde = { version = "1.0.218", features = ["derive"] }
serde_json = "1.0.139"
serde_yaml = "0.9.34"
thiserror = "2.0.11"
```
**Notes:** Keep existing ordering style; only append new member/dependency.

### Step 2: Create crate manifest
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/Cargo.toml`
**Action:** create
**Location:** new file
**What to do:**
Create crate metadata and dependencies. Keep dependency set minimal and aligned to requirement list.
**Code:**
```toml
[package]
name = "ferrotick-strategies"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
description = "Strategy library, YAML spec parser, and signal composition for ferrotick"

[dependencies]
ferrotick-core = { path = "../ferrotick-core" }
ferrotick-ml = { path = "../ferrotick-ml" }
serde.workspace = true
serde_yaml.workspace = true
thiserror.workspace = true

[dev-dependencies]
serde_yaml.workspace = true
```
**Notes:** Do not add direct `ta` dependency; use indicator functions from `ferrotick-ml`.

### Step 3: Create crate root and public API
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/lib.rs`
**Action:** create
**Location:** new file
**What to do:**
Declare modules and re-exports so CLI can consume one stable API surface.
**Code:**
```rust
pub mod error;
pub mod library;
pub mod signals;
pub mod sizing;
pub mod spec;
pub mod strategy;

pub use error::StrategyError;
pub use library::{
    built_in_strategy_catalog, BollingerSqueezeStrategy, MacdTrendStrategy,
    MaCrossoverStrategy, RsiMeanReversionStrategy, StrategyDescriptor,
};
pub use signals::{CompositeMethod, Signal, SignalAction, SignalComposer, SignalVote};
pub use sizing::{PositionSizingInput, PositionSizingMethod};
pub use spec::{
    compile_strategy, BacktestSpec, CompiledStrategy, StrategyConfig, StrategyParser,
    StrategySpec, StrategyValidator,
};
pub use strategy::{Strategy, StrategyContext};

pub type StrategyResult<T> = Result<T, StrategyError>;
```
**Notes:** Keep re-exports explicit; avoid wildcard exports.

### Step 4: Add error type with thiserror
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/error.rs`
**Action:** create
**Location:** new file
**What to do:**
Define crate-wide error enum following existing `thiserror` pattern.
**Code:**
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StrategyError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("spec parse error: {0}")]
    Parse(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    #[error(transparent)]
    CoreValidation(#[from] ferrotick_core::ValidationError),

    #[error(transparent)]
    Ml(#[from] ferrotick_ml::MlError),
}
```
**Notes:** Keep user-facing strings lower-case and direct, consistent with existing crates.

### Step 5: Define strategy trait and execution context
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/strategy.rs`
**Action:** create
**Location:** new file
**What to do:**
Create the required `Strategy` trait and make `Send + Sync` explicit in the trait definition.
**Code:**
```rust
use ferrotick_core::{Bar, Symbol};

use crate::{Signal, StrategyResult};

#[derive(Debug, Clone)]
pub struct StrategyContext<'a> {
    pub symbol: &'a Symbol,
    pub bar: &'a Bar,
    pub position_qty: f64,
    pub cash: f64,
    pub equity: f64,
}

pub trait Strategy: Send + Sync {
    fn name(&self) -> &'static str;

    fn on_bar(&mut self, ctx: &StrategyContext<'_>) -> StrategyResult<Option<Signal>>;

    fn reset(&mut self);
}
```
**Notes:** `Strategy` does not depend on `ferrotick-backtest`; this keeps crate boundaries clean.

### Step 6: Implement signal framework and composite methods
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/signals.rs`
**Action:** create
**Location:** new file
**What to do:**
Add signal entities (`Signal`, `SignalAction`) and deterministic composition logic (`all`, `any`, `majority`, `weighted`).
**Code:**
```rust
use ferrotick_core::UtcDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalAction {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub ts: UtcDateTime,
    pub action: SignalAction,
    pub strength: f64,
    pub reason: String,
}

impl Signal {
    pub fn new(ts: UtcDateTime, action: SignalAction, strength: f64, reason: impl Into<String>) -> Self {
        Self {
            ts,
            action,
            strength: strength.clamp(0.0, 1.0),
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SignalVote {
    pub action: SignalAction,
    pub strength: f64,
    pub weight: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompositeMethod {
    All,
    Any,
    #[default]
    Majority,
    Weighted,
}

#[derive(Debug, Clone, Copy)]
pub struct SignalComposer {
    method: CompositeMethod,
}

impl SignalComposer {
    pub const fn new(method: CompositeMethod) -> Self {
        Self { method }
    }

    pub fn compose(&self, ts: UtcDateTime, votes: &[SignalVote]) -> Option<Signal> {
        let filtered: Vec<&SignalVote> = votes
            .iter()
            .filter(|vote| vote.action != SignalAction::Hold && vote.weight > 0.0)
            .collect();

        if filtered.is_empty() {
            return None;
        }

        match self.method {
            CompositeMethod::All => compose_all(ts, &filtered),
            CompositeMethod::Any => compose_any(ts, &filtered),
            CompositeMethod::Majority => compose_majority(ts, &filtered),
            CompositeMethod::Weighted => compose_weighted(ts, &filtered),
        }
    }
}

fn compose_all(ts: UtcDateTime, votes: &[&SignalVote]) -> Option<Signal> {
    let first = votes[0].action;
    if votes.iter().all(|vote| vote.action == first) {
        let avg_strength = votes.iter().map(|vote| vote.strength.clamp(0.0, 1.0)).sum::<f64>()
            / votes.len() as f64;
        let reasons = votes.iter().map(|vote| vote.reason.as_str()).collect::<Vec<_>>().join(" | ");
        return Some(Signal::new(ts, first, avg_strength, format!("all: {reasons}")));
    }
    None
}

fn compose_any(ts: UtcDateTime, votes: &[&SignalVote]) -> Option<Signal> {
    let best = votes.iter().max_by(|a, b| {
        let left = a.weight * a.strength.clamp(0.0, 1.0);
        let right = b.weight * b.strength.clamp(0.0, 1.0);
        left.total_cmp(&right)
    })?;

    Some(Signal::new(
        ts,
        best.action,
        best.strength,
        format!("any: {}", best.reason),
    ))
}

fn compose_majority(ts: UtcDateTime, votes: &[&SignalVote]) -> Option<Signal> {
    let mut buy = 0.0;
    let mut sell = 0.0;

    for vote in votes {
        match vote.action {
            SignalAction::Buy => buy += vote.weight,
            SignalAction::Sell => sell += vote.weight,
            SignalAction::Hold => {}
        }
    }

    if (buy - sell).abs() <= f64::EPSILON {
        return None;
    }

    let action = if buy > sell { SignalAction::Buy } else { SignalAction::Sell };
    let confidence = (buy - sell).abs() / (buy + sell);
    Some(Signal::new(ts, action, confidence, "majority"))
}

fn compose_weighted(ts: UtcDateTime, votes: &[&SignalVote]) -> Option<Signal> {
    let mut buy_score = 0.0;
    let mut sell_score = 0.0;

    for vote in votes {
        let score = vote.weight * vote.strength.clamp(0.0, 1.0);
        match vote.action {
            SignalAction::Buy => buy_score += score,
            SignalAction::Sell => sell_score += score,
            SignalAction::Hold => {}
        }
    }

    if (buy_score - sell_score).abs() <= 1e-12 {
        return None;
    }

    let total = buy_score + sell_score;
    let action = if buy_score > sell_score {
        SignalAction::Buy
    } else {
        SignalAction::Sell
    };
    let strength = if total <= f64::EPSILON {
        0.0
    } else {
        (buy_score - sell_score).abs() / total
    };

    Some(Signal::new(ts, action, strength, "weighted"))
}
```
**Notes:** Ties return `None` (no signal) to avoid churn.

### Step 7: Implement position sizing methods
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/sizing.rs`
**Action:** create
**Location:** new file
**What to do:**
Define the four required sizing methods and one deterministic sizing function with cash/position caps.
**Code:**
```rust
use serde::{Deserialize, Serialize};

use crate::{SignalAction, StrategyError, StrategyResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum PositionSizingMethod {
    Fixed {
        quantity: f64,
    },
    Percent {
        percent_of_equity: f64,
    },
    VolatilityAdjusted {
        risk_per_trade: f64,
        atr_period: usize,
        atr_multiplier: f64,
    },
    Kelly {
        win_probability: f64,
        win_loss_ratio: f64,
        fraction_cap: f64,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct PositionSizingInput {
    pub price: f64,
    pub cash: f64,
    pub equity: f64,
    pub current_position: f64,
    pub signal_strength: f64,
    pub volatility: Option<f64>,
}

impl PositionSizingMethod {
    pub const fn atr_period(&self) -> Option<usize> {
        match self {
            Self::VolatilityAdjusted { atr_period, .. } => Some(*atr_period),
            _ => None,
        }
    }

    pub fn quantity_for_action(
        &self,
        action: SignalAction,
        input: PositionSizingInput,
    ) -> StrategyResult<f64> {
        if !input.price.is_finite() || input.price <= 0.0 {
            return Ok(0.0);
        }

        let strength = input.signal_strength.clamp(0.0, 1.0);
        if strength <= f64::EPSILON {
            return Ok(0.0);
        }

        let raw = match self {
            Self::Fixed { quantity } => quantity * strength,
            Self::Percent { percent_of_equity } => {
                let notional = input.equity.max(0.0) * percent_of_equity * strength;
                notional / input.price
            }
            Self::VolatilityAdjusted {
                risk_per_trade,
                atr_multiplier,
                ..
            } => {
                let vol = input.volatility.unwrap_or(0.0);
                if !vol.is_finite() || vol <= 0.0 {
                    return Err(StrategyError::InvalidConfig(String::from(
                        "volatility-adjusted sizing requires positive ATR/volatility",
                    )));
                }
                let risk_budget = input.equity.max(0.0) * risk_per_trade * strength;
                risk_budget / (vol * atr_multiplier)
            }
            Self::Kelly {
                win_probability,
                win_loss_ratio,
                fraction_cap,
            } => {
                let p = *win_probability;
                let b = *win_loss_ratio;
                let f_star = p - ((1.0 - p) / b);
                let fraction = (f_star.max(0.0) * fraction_cap).clamp(0.0, *fraction_cap);
                let notional = input.equity.max(0.0) * fraction * strength;
                notional / input.price
            }
        };

        let qty = raw.max(0.0);
        let capped = match action {
            SignalAction::Buy => {
                let max_affordable = (input.cash.max(0.0) / input.price).max(0.0);
                qty.min(max_affordable)
            }
            SignalAction::Sell => qty.min(input.current_position.max(0.0)),
            SignalAction::Hold => 0.0,
        };

        Ok(capped)
    }
}
```
**Notes:** This function is the single source of truth for quantity sizing; do not duplicate sizing math in CLI.

### Step 8: Add strategy library module and catalog
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/library/mod.rs`
**Action:** create
**Location:** new file
**What to do:**
Register all four strategy modules and expose metadata used by CLI `strategy list`.
**Code:**
```rust
mod bollinger_squeeze;
mod ma_crossover;
mod macd_trend;
mod rsi_mean_reversion;

use serde::Serialize;

pub use bollinger_squeeze::BollingerSqueezeStrategy;
pub use ma_crossover::MaCrossoverStrategy;
pub use macd_trend::MacdTrendStrategy;
pub use rsi_mean_reversion::RsiMeanReversionStrategy;

#[derive(Debug, Clone, Serialize)]
pub struct StrategyDescriptor {
    pub strategy_type: &'static str,
    pub description: &'static str,
    pub default_yaml: &'static str,
}

pub fn built_in_strategy_catalog() -> Vec<StrategyDescriptor> {
    vec![
        StrategyDescriptor {
            strategy_type: "ma_crossover",
            description: "Fast/slow moving-average crossover trend follower",
            default_yaml: "name: ma_default\nsymbol: AAPL\ninterval: 1d\ntype: ma_crossover\nshort_period: 20\nlong_period: 50\nposition_sizing:\n  method: percent\n  percent_of_equity: 0.1\n",
        },
        StrategyDescriptor {
            strategy_type: "rsi_mean_reversion",
            description: "RSI oversold/overbought mean reversion",
            default_yaml: "name: rsi_default\nsymbol: AAPL\ninterval: 1d\ntype: rsi_mean_reversion\nperiod: 14\noversold: 30\noverbought: 70\nposition_sizing:\n  method: percent\n  percent_of_equity: 0.1\n",
        },
        StrategyDescriptor {
            strategy_type: "macd_trend",
            description: "MACD signal-line crossover trend strategy",
            default_yaml: "name: macd_default\nsymbol: AAPL\ninterval: 1d\ntype: macd_trend\nfast_period: 12\nslow_period: 26\nsignal_period: 9\nmin_histogram: 0.0\nposition_sizing:\n  method: percent\n  percent_of_equity: 0.1\n",
        },
        StrategyDescriptor {
            strategy_type: "bollinger_squeeze",
            description: "Bollinger squeeze breakout strategy",
            default_yaml: "name: bb_default\nsymbol: AAPL\ninterval: 1d\ntype: bollinger_squeeze\nperiod: 20\nstd_dev: 2.0\nsqueeze_threshold: 0.05\nposition_sizing:\n  method: percent\n  percent_of_equity: 0.1\n",
        },
    ]
}
```
**Notes:** Keep catalog strings static so CLI output is deterministic.

### Step 9: Implement MA crossover strategy
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/library/ma_crossover.rs`
**Action:** create
**Location:** new file
**What to do:**
Implement golden/death cross logic with composite signal output.
**Code:**
```rust
use crate::{
    CompositeMethod, Signal, SignalAction, SignalComposer, SignalVote, Strategy, StrategyContext,
    StrategyError, StrategyResult,
};

#[derive(Debug, Clone)]
pub struct MaCrossoverStrategy {
    short_period: usize,
    long_period: usize,
    composer: SignalComposer,
    closes: Vec<f64>,
}

impl MaCrossoverStrategy {
    pub fn new(short_period: usize, long_period: usize, composite: CompositeMethod) -> StrategyResult<Self> {
        if short_period == 0 || long_period == 0 || short_period >= long_period {
            return Err(StrategyError::InvalidConfig(String::from(
                "ma_crossover requires short_period > 0, long_period > 0, short_period < long_period",
            )));
        }

        Ok(Self {
            short_period,
            long_period,
            composer: SignalComposer::new(composite),
            closes: Vec::new(),
        })
    }

    fn sma(series: &[f64], period: usize) -> Option<f64> {
        if series.len() < period {
            return None;
        }
        let window = &series[series.len() - period..];
        Some(window.iter().sum::<f64>() / period as f64)
    }
}

impl Strategy for MaCrossoverStrategy {
    fn name(&self) -> &'static str {
        "ma_crossover"
    }

    fn on_bar(&mut self, ctx: &StrategyContext<'_>) -> StrategyResult<Option<Signal>> {
        self.closes.push(ctx.bar.close);
        if self.closes.len() < self.long_period + 1 {
            return Ok(None);
        }

        let curr_short = Self::sma(&self.closes, self.short_period).expect("checked len");
        let curr_long = Self::sma(&self.closes, self.long_period).expect("checked len");
        let prev_short = Self::sma(&self.closes[..self.closes.len() - 1], self.short_period)
            .expect("checked len");
        let prev_long = Self::sma(&self.closes[..self.closes.len() - 1], self.long_period)
            .expect("checked len");

        let mut votes = Vec::new();
        if prev_short <= prev_long && curr_short > curr_long {
            let strength = ((curr_short - curr_long) / curr_long.max(1e-9)).abs().clamp(0.0, 1.0);
            votes.push(SignalVote {
                action: SignalAction::Buy,
                strength,
                weight: 1.0,
                reason: format!("golden_cross short={curr_short:.4} long={curr_long:.4}"),
            });
        }

        if prev_short >= prev_long && curr_short < curr_long {
            let strength = ((curr_short - curr_long) / curr_long.max(1e-9)).abs().clamp(0.0, 1.0);
            votes.push(SignalVote {
                action: SignalAction::Sell,
                strength,
                weight: 1.0,
                reason: format!("death_cross short={curr_short:.4} long={curr_long:.4}"),
            });
        }

        Ok(self.composer.compose(ctx.bar.ts, &votes))
    }

    fn reset(&mut self) {
        self.closes.clear();
    }
}
```
**Notes:** Use previous and current MA values to detect true cross events only.

### Step 10: Implement RSI mean reversion strategy
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/library/rsi_mean_reversion.rs`
**Action:** create
**Location:** new file
**What to do:**
Use `ferrotick_ml::features::indicators::compute_rsi` and emit buy/sell signals on threshold breaches.
**Code:**
```rust
use ferrotick_ml::features::indicators::compute_rsi;

use crate::{
    CompositeMethod, Signal, SignalAction, SignalComposer, SignalVote, Strategy, StrategyContext,
    StrategyError, StrategyResult,
};

#[derive(Debug, Clone)]
pub struct RsiMeanReversionStrategy {
    period: usize,
    oversold: f64,
    overbought: f64,
    composer: SignalComposer,
    closes: Vec<f64>,
}

impl RsiMeanReversionStrategy {
    pub fn new(
        period: usize,
        oversold: f64,
        overbought: f64,
        composite: CompositeMethod,
    ) -> StrategyResult<Self> {
        if period == 0 || !(0.0..100.0).contains(&oversold) || !(0.0..100.0).contains(&overbought) {
            return Err(StrategyError::InvalidConfig(String::from(
                "invalid RSI config",
            )));
        }
        if oversold >= overbought {
            return Err(StrategyError::InvalidConfig(String::from(
                "oversold must be < overbought",
            )));
        }

        Ok(Self {
            period,
            oversold,
            overbought,
            composer: SignalComposer::new(composite),
            closes: Vec::new(),
        })
    }
}

impl Strategy for RsiMeanReversionStrategy {
    fn name(&self) -> &'static str {
        "rsi_mean_reversion"
    }

    fn on_bar(&mut self, ctx: &StrategyContext<'_>) -> StrategyResult<Option<Signal>> {
        self.closes.push(ctx.bar.close);

        let rsi = compute_rsi(&self.closes, self.period)?;
        let Some(current) = rsi.last().and_then(|value| *value) else {
            return Ok(None);
        };

        let mut votes = Vec::new();
        if current <= self.oversold {
            votes.push(SignalVote {
                action: SignalAction::Buy,
                strength: ((self.oversold - current) / self.oversold.max(1e-9)).clamp(0.0, 1.0),
                weight: 1.0,
                reason: format!("rsi_oversold={current:.2}"),
            });
        }

        if current >= self.overbought {
            votes.push(SignalVote {
                action: SignalAction::Sell,
                strength: ((current - self.overbought) / (100.0 - self.overbought).max(1e-9))
                    .clamp(0.0, 1.0),
                weight: 1.0,
                reason: format!("rsi_overbought={current:.2}"),
            });
        }

        Ok(self.composer.compose(ctx.bar.ts, &votes))
    }

    fn reset(&mut self) {
        self.closes.clear();
    }
}
```
**Notes:** No signal during RSI warmup (`None` from indicator).

### Step 11: Implement MACD trend strategy
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/library/macd_trend.rs`
**Action:** create
**Location:** new file
**What to do:**
Use MACD/signal crossover with histogram threshold filter.
**Code:**
```rust
use ferrotick_ml::features::indicators::compute_macd;

use crate::{
    CompositeMethod, Signal, SignalAction, SignalComposer, SignalVote, Strategy, StrategyContext,
    StrategyError, StrategyResult,
};

#[derive(Debug, Clone)]
pub struct MacdTrendStrategy {
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
    min_histogram: f64,
    composer: SignalComposer,
    closes: Vec<f64>,
}

impl MacdTrendStrategy {
    pub fn new(
        fast_period: usize,
        slow_period: usize,
        signal_period: usize,
        min_histogram: f64,
        composite: CompositeMethod,
    ) -> StrategyResult<Self> {
        if fast_period == 0 || slow_period == 0 || signal_period == 0 || fast_period >= slow_period {
            return Err(StrategyError::InvalidConfig(String::from(
                "macd_trend requires fast < slow and all periods > 0",
            )));
        }
        if !min_histogram.is_finite() || min_histogram < 0.0 {
            return Err(StrategyError::InvalidConfig(String::from(
                "min_histogram must be finite and >= 0",
            )));
        }

        Ok(Self {
            fast_period,
            slow_period,
            signal_period,
            min_histogram,
            composer: SignalComposer::new(composite),
            closes: Vec::new(),
        })
    }
}

impl Strategy for MacdTrendStrategy {
    fn name(&self) -> &'static str {
        "macd_trend"
    }

    fn on_bar(&mut self, ctx: &StrategyContext<'_>) -> StrategyResult<Option<Signal>> {
        self.closes.push(ctx.bar.close);

        let series = compute_macd(
            &self.closes,
            self.fast_period,
            self.slow_period,
            self.signal_period,
        )?;

        if self.closes.len() < 2 {
            return Ok(None);
        }

        let last = self.closes.len() - 1;
        let prev = last - 1;

        let Some(curr_macd) = series.macd[last] else { return Ok(None) };
        let Some(curr_signal) = series.signal[last] else { return Ok(None) };
        let Some(prev_macd) = series.macd[prev] else { return Ok(None) };
        let Some(prev_signal) = series.signal[prev] else { return Ok(None) };

        let histogram = curr_macd - curr_signal;
        let mut votes = Vec::new();

        if prev_macd <= prev_signal && curr_macd > curr_signal && histogram >= self.min_histogram {
            votes.push(SignalVote {
                action: SignalAction::Buy,
                strength: histogram.abs().min(1.0),
                weight: 1.0,
                reason: format!("macd_bullish_cross hist={histogram:.4}"),
            });
        }

        if prev_macd >= prev_signal && curr_macd < curr_signal && histogram <= -self.min_histogram {
            votes.push(SignalVote {
                action: SignalAction::Sell,
                strength: histogram.abs().min(1.0),
                weight: 1.0,
                reason: format!("macd_bearish_cross hist={histogram:.4}"),
            });
        }

        Ok(self.composer.compose(ctx.bar.ts, &votes))
    }

    fn reset(&mut self) {
        self.closes.clear();
    }
}
```
**Notes:** Histogram threshold prevents low-conviction noise signals.

### Step 12: Implement Bollinger squeeze strategy
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/library/bollinger_squeeze.rs`
**Action:** create
**Location:** new file
**What to do:**
Detect squeeze (`band_width <= threshold`) and emit breakout signal once squeeze releases.
**Code:**
```rust
use ferrotick_ml::features::indicators::compute_bollinger;

use crate::{
    CompositeMethod, Signal, SignalAction, SignalComposer, SignalVote, Strategy, StrategyContext,
    StrategyError, StrategyResult,
};

#[derive(Debug, Clone)]
pub struct BollingerSqueezeStrategy {
    period: usize,
    std_dev: f64,
    squeeze_threshold: f64,
    composer: SignalComposer,
    closes: Vec<f64>,
    was_in_squeeze: bool,
}

impl BollingerSqueezeStrategy {
    pub fn new(
        period: usize,
        std_dev: f64,
        squeeze_threshold: f64,
        composite: CompositeMethod,
    ) -> StrategyResult<Self> {
        if period == 0 || !std_dev.is_finite() || std_dev <= 0.0 {
            return Err(StrategyError::InvalidConfig(String::from(
                "bollinger_squeeze requires period > 0 and std_dev > 0",
            )));
        }
        if !squeeze_threshold.is_finite() || squeeze_threshold <= 0.0 || squeeze_threshold >= 1.0 {
            return Err(StrategyError::InvalidConfig(String::from(
                "squeeze_threshold must be in (0,1)",
            )));
        }

        Ok(Self {
            period,
            std_dev,
            squeeze_threshold,
            composer: SignalComposer::new(composite),
            closes: Vec::new(),
            was_in_squeeze: false,
        })
    }
}

impl Strategy for BollingerSqueezeStrategy {
    fn name(&self) -> &'static str {
        "bollinger_squeeze"
    }

    fn on_bar(&mut self, ctx: &StrategyContext<'_>) -> StrategyResult<Option<Signal>> {
        self.closes.push(ctx.bar.close);

        let bb = compute_bollinger(&self.closes, self.period, self.std_dev)?;
        let Some(upper) = bb.upper.last().and_then(|v| *v) else { return Ok(None) };
        let Some(lower) = bb.lower.last().and_then(|v| *v) else { return Ok(None) };

        let close = ctx.bar.close;
        if close <= 0.0 {
            return Ok(None);
        }

        let band_width = (upper - lower) / close;
        let in_squeeze = band_width <= self.squeeze_threshold;

        let mut votes = Vec::new();
        if self.was_in_squeeze && !in_squeeze {
            if close > upper {
                votes.push(SignalVote {
                    action: SignalAction::Buy,
                    strength: ((close - upper) / close).clamp(0.0, 1.0),
                    weight: 1.0,
                    reason: format!("squeeze_breakout_up width={band_width:.4}"),
                });
            } else if close < lower {
                votes.push(SignalVote {
                    action: SignalAction::Sell,
                    strength: ((lower - close) / close).clamp(0.0, 1.0),
                    weight: 1.0,
                    reason: format!("squeeze_breakout_down width={band_width:.4}"),
                });
            }
        }

        self.was_in_squeeze = in_squeeze;
        Ok(self.composer.compose(ctx.bar.ts, &votes))
    }

    fn reset(&mut self) {
        self.closes.clear();
        self.was_in_squeeze = false;
    }
}
```
**Notes:** Emit only when transitioning out of squeeze to avoid repeated duplicate signals.

### Step 13: Define strategy spec schema types
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/spec/types.rs`
**Action:** create
**Location:** new file
**What to do:**
Define YAML-deserializable types used by parser/validator/compiler.
**Code:**
```rust
use serde::{Deserialize, Serialize};

use crate::{CompositeMethod, PositionSizingMethod};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySpec {
    pub name: String,
    pub symbol: String,
    #[serde(default = "default_interval")]
    pub interval: String,
    #[serde(flatten)]
    pub strategy: StrategyConfig,
    pub position_sizing: PositionSizingMethod,
    #[serde(default)]
    pub signal: SignalSpec,
    #[serde(default)]
    pub backtest: BacktestSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StrategyConfig {
    MaCrossover {
        #[serde(default = "default_ma_short")]
        short_period: usize,
        #[serde(default = "default_ma_long")]
        long_period: usize,
    },
    RsiMeanReversion {
        #[serde(default = "default_rsi_period")]
        period: usize,
        #[serde(default = "default_oversold")]
        oversold: f64,
        #[serde(default = "default_overbought")]
        overbought: f64,
    },
    MacdTrend {
        #[serde(default = "default_macd_fast")]
        fast_period: usize,
        #[serde(default = "default_macd_slow")]
        slow_period: usize,
        #[serde(default = "default_macd_signal")]
        signal_period: usize,
        #[serde(default)]
        min_histogram: f64,
    },
    BollingerSqueeze {
        #[serde(default = "default_bb_period")]
        period: usize,
        #[serde(default = "default_bb_std")]
        std_dev: f64,
        #[serde(default = "default_bb_squeeze")]
        squeeze_threshold: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalSpec {
    #[serde(default)]
    pub composite: CompositeMethod,
}

impl Default for SignalSpec {
    fn default() -> Self {
        Self {
            composite: CompositeMethod::Majority,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestSpec {
    #[serde(default = "default_initial_capital")]
    pub initial_capital: f64,
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub end: Option<String>,
    #[serde(default = "default_risk_free_rate")]
    pub risk_free_rate: f64,
    #[serde(default = "default_trading_days")]
    pub trading_days_per_year: f64,
    #[serde(default = "default_max_bars")]
    pub max_bars: usize,
}

impl Default for BacktestSpec {
    fn default() -> Self {
        Self {
            initial_capital: default_initial_capital(),
            start: None,
            end: None,
            risk_free_rate: default_risk_free_rate(),
            trading_days_per_year: default_trading_days(),
            max_bars: default_max_bars(),
        }
    }
}

fn default_interval() -> String { String::from("1d") }

const fn default_ma_short() -> usize { 20 }
const fn default_ma_long() -> usize { 50 }
const fn default_rsi_period() -> usize { 14 }
const fn default_oversold() -> f64 { 30.0 }
const fn default_overbought() -> f64 { 70.0 }
const fn default_macd_fast() -> usize { 12 }
const fn default_macd_slow() -> usize { 26 }
const fn default_macd_signal() -> usize { 9 }
const fn default_bb_period() -> usize { 20 }
const fn default_bb_std() -> f64 { 2.0 }
const fn default_bb_squeeze() -> f64 { 0.05 }
const fn default_initial_capital() -> f64 { 100_000.0 }
const fn default_risk_free_rate() -> f64 { 0.02 }
const fn default_trading_days() -> f64 { 252.0 }
const fn default_max_bars() -> usize { 2_000 }
```
**Notes:** Keep `interval` default fixed to `1d` for Phase 9.

### Step 14: Implement YAML parser
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/spec/parser.rs`
**Action:** create
**Location:** new file
**What to do:**
Provide parser methods for both YAML strings and YAML files.
**Code:**
```rust
use std::path::Path;

use crate::{StrategyError, StrategyResult};

use super::StrategySpec;

pub struct StrategyParser;

impl StrategyParser {
    pub fn from_yaml_str(raw: &str) -> StrategyResult<StrategySpec> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(StrategyError::Parse(String::from("strategy YAML is empty")));
        }

        serde_yaml::from_str::<StrategySpec>(trimmed).map_err(StrategyError::from)
    }

    pub fn from_yaml_file(path: &Path) -> StrategyResult<StrategySpec> {
        let raw = std::fs::read_to_string(path)?;
        Self::from_yaml_str(&raw)
    }
}
```
**Notes:** Parser should not validate business rules; leave that to validator.

### Step 15: Implement strategy validator
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/spec/validator.rs`
**Action:** create
**Location:** new file
**What to do:**
Implement explicit validation rules and aggregate all errors in one pass.
**Code:**
```rust
use std::str::FromStr;

use ferrotick_core::{Interval, Symbol, UtcDateTime};

use crate::{PositionSizingMethod, StrategyError, StrategyResult};

use super::{StrategyConfig, StrategySpec};

pub struct StrategyValidator;

impl StrategyValidator {
    pub fn validate(spec: &StrategySpec) -> StrategyResult<()> {
        let mut issues = Vec::new();

        if spec.name.trim().is_empty() {
            issues.push(String::from("name must not be empty"));
        }

        if Symbol::parse(&spec.symbol).is_err() {
            issues.push(String::from("symbol is invalid"));
        }

        match Interval::from_str(&spec.interval) {
            Ok(interval) => {
                if interval != Interval::OneDay {
                    issues.push(String::from("Phase 9 supports interval=1d only"));
                }
            }
            Err(_) => issues.push(String::from("interval must be one of 1m,5m,15m,1h,1d")),
        }

        match &spec.strategy {
            StrategyConfig::MaCrossover { short_period, long_period } => {
                if *short_period == 0 || *long_period == 0 || short_period >= long_period {
                    issues.push(String::from("ma_crossover requires short_period > 0, long_period > 0, short_period < long_period"));
                }
            }
            StrategyConfig::RsiMeanReversion { period, oversold, overbought } => {
                if *period == 0 {
                    issues.push(String::from("rsi_mean_reversion.period must be > 0"));
                }
                if !oversold.is_finite() || !overbought.is_finite() || *oversold <= 0.0 || *overbought >= 100.0 || oversold >= overbought {
                    issues.push(String::from("rsi thresholds must satisfy 0 < oversold < overbought < 100"));
                }
            }
            StrategyConfig::MacdTrend { fast_period, slow_period, signal_period, min_histogram } => {
                if *fast_period == 0 || *slow_period == 0 || *signal_period == 0 || fast_period >= slow_period {
                    issues.push(String::from("macd_trend requires fast_period < slow_period and all periods > 0"));
                }
                if !min_histogram.is_finite() || *min_histogram < 0.0 {
                    issues.push(String::from("macd_trend.min_histogram must be finite and >= 0"));
                }
            }
            StrategyConfig::BollingerSqueeze { period, std_dev, squeeze_threshold } => {
                if *period == 0 {
                    issues.push(String::from("bollinger_squeeze.period must be > 0"));
                }
                if !std_dev.is_finite() || *std_dev <= 0.0 {
                    issues.push(String::from("bollinger_squeeze.std_dev must be finite and > 0"));
                }
                if !squeeze_threshold.is_finite() || *squeeze_threshold <= 0.0 || *squeeze_threshold >= 1.0 {
                    issues.push(String::from("bollinger_squeeze.squeeze_threshold must be in (0,1)"));
                }
            }
        }

        match &spec.position_sizing {
            PositionSizingMethod::Fixed { quantity } => {
                if !quantity.is_finite() || *quantity <= 0.0 {
                    issues.push(String::from("position_sizing.fixed.quantity must be finite and > 0"));
                }
            }
            PositionSizingMethod::Percent { percent_of_equity } => {
                if !percent_of_equity.is_finite() || *percent_of_equity <= 0.0 || *percent_of_equity > 1.0 {
                    issues.push(String::from("position_sizing.percent.percent_of_equity must be in (0,1]"));
                }
            }
            PositionSizingMethod::VolatilityAdjusted { risk_per_trade, atr_period, atr_multiplier } => {
                if !risk_per_trade.is_finite() || *risk_per_trade <= 0.0 || *risk_per_trade > 1.0 {
                    issues.push(String::from("volatility_adjusted.risk_per_trade must be in (0,1]"));
                }
                if *atr_period < 2 {
                    issues.push(String::from("volatility_adjusted.atr_period must be >= 2"));
                }
                if !atr_multiplier.is_finite() || *atr_multiplier <= 0.0 {
                    issues.push(String::from("volatility_adjusted.atr_multiplier must be > 0"));
                }
            }
            PositionSizingMethod::Kelly { win_probability, win_loss_ratio, fraction_cap } => {
                if !win_probability.is_finite() || *win_probability < 0.0 || *win_probability > 1.0 {
                    issues.push(String::from("kelly.win_probability must be in [0,1]"));
                }
                if !win_loss_ratio.is_finite() || *win_loss_ratio <= 0.0 {
                    issues.push(String::from("kelly.win_loss_ratio must be > 0"));
                }
                if !fraction_cap.is_finite() || *fraction_cap <= 0.0 || *fraction_cap > 1.0 {
                    issues.push(String::from("kelly.fraction_cap must be in (0,1]"));
                }
            }
        }

        let start = spec
            .backtest
            .start
            .as_deref()
            .map(UtcDateTime::parse)
            .transpose();
        let end = spec
            .backtest
            .end
            .as_deref()
            .map(UtcDateTime::parse)
            .transpose();

        if start.is_err() {
            issues.push(String::from("backtest.start must be RFC3339 or YYYY-MM-DD normalized by CLI"));
        }
        if end.is_err() {
            issues.push(String::from("backtest.end must be RFC3339 or YYYY-MM-DD normalized by CLI"));
        }
        if let (Ok(Some(start)), Ok(Some(end))) = (start, end) {
            if start > end {
                issues.push(String::from("backtest.start must be <= backtest.end"));
            }
        }

        if !spec.backtest.initial_capital.is_finite() || spec.backtest.initial_capital <= 0.0 {
            issues.push(String::from("backtest.initial_capital must be finite and > 0"));
        }
        if !spec.backtest.risk_free_rate.is_finite() {
            issues.push(String::from("backtest.risk_free_rate must be finite"));
        }
        if !spec.backtest.trading_days_per_year.is_finite() || spec.backtest.trading_days_per_year <= 0.0 {
            issues.push(String::from("backtest.trading_days_per_year must be finite and > 0"));
        }
        if spec.backtest.max_bars == 0 {
            issues.push(String::from("backtest.max_bars must be > 0"));
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(StrategyError::Validation(issues.join(" | ")))
        }
    }
}
```
**Notes:** Aggregate errors so `strategy validate` shows all problems in one run.

### Step 16: Implement compiler and spec module exports
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/spec/compiler.rs`
**Action:** create
**Location:** new file
**What to do:**
Compile `StrategySpec` into executable strategy + parsed core types.
**Code:**
```rust
use std::str::FromStr;

use ferrotick_core::{Interval, Symbol};

use crate::{
    BollingerSqueezeStrategy, MacdTrendStrategy, MaCrossoverStrategy, RsiMeanReversionStrategy,
    Strategy, StrategyResult,
};

use super::{BacktestSpec, StrategyConfig, StrategySpec, StrategyValidator};

pub struct CompiledStrategy {
    pub name: String,
    pub symbol: Symbol,
    pub interval: Interval,
    pub strategy: Box<dyn Strategy>,
    pub position_sizing: crate::PositionSizingMethod,
    pub backtest: BacktestSpec,
}

pub fn compile_strategy(spec: &StrategySpec) -> StrategyResult<CompiledStrategy> {
    StrategyValidator::validate(spec)?;

    let symbol = Symbol::parse(&spec.symbol)?;
    let interval = Interval::from_str(&spec.interval)?;
    let composite = spec.signal.composite;

    let strategy: Box<dyn Strategy> = match &spec.strategy {
        StrategyConfig::MaCrossover {
            short_period,
            long_period,
        } => Box::new(MaCrossoverStrategy::new(*short_period, *long_period, composite)?),
        StrategyConfig::RsiMeanReversion {
            period,
            oversold,
            overbought,
        } => Box::new(RsiMeanReversionStrategy::new(
            *period,
            *oversold,
            *overbought,
            composite,
        )?),
        StrategyConfig::MacdTrend {
            fast_period,
            slow_period,
            signal_period,
            min_histogram,
        } => Box::new(MacdTrendStrategy::new(
            *fast_period,
            *slow_period,
            *signal_period,
            *min_histogram,
            composite,
        )?),
        StrategyConfig::BollingerSqueeze {
            period,
            std_dev,
            squeeze_threshold,
        } => Box::new(BollingerSqueezeStrategy::new(
            *period,
            *std_dev,
            *squeeze_threshold,
            composite,
        )?),
    };

    Ok(CompiledStrategy {
        name: spec.name.clone(),
        symbol,
        interval,
        strategy,
        position_sizing: spec.position_sizing.clone(),
        backtest: spec.backtest.clone(),
    })
}
```
**Notes:** Compiler calls validator first; never compile unvalidated specs.

### Step 17: Create `spec/mod.rs`
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/src/spec/mod.rs`
**Action:** create
**Location:** new file
**What to do:**
Wire internal modules and public re-exports.
**Code:**
```rust
mod compiler;
mod parser;
mod types;
mod validator;

pub use compiler::{compile_strategy, CompiledStrategy};
pub use parser::StrategyParser;
pub use types::{BacktestSpec, StrategyConfig, StrategySpec};
pub use validator::StrategyValidator;
```
**Notes:** Keep module order fixed to match re-export order in `lib.rs`.

### Step 18: Add strategy crate tests
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/tests/phase9_strategy_library.rs`
**Action:** create
**Location:** new file
**What to do:**
Add deterministic tests for parser, validator, composer, sizing, and at least one signal path per strategy family.
**Code:**
```rust
use ferrotick_core::{Bar, Symbol, UtcDateTime};
use ferrotick_strategies::{
    compile_strategy, CompositeMethod, PositionSizingInput, PositionSizingMethod, SignalAction,
    SignalComposer, SignalVote, Strategy, StrategyContext, StrategyParser, StrategyValidator,
};

fn make_bar(day: usize, close: f64) -> Bar {
    let ts = UtcDateTime::parse(format!("2024-01-{:02}T00:00:00Z", day).as_str()).unwrap();
    Bar::new(ts, close - 1.0, close + 1.0, close - 2.0, close, Some(1_000), None).unwrap()
}

#[test]
fn parser_and_validator_accept_valid_yaml() {
    let yaml = r#"
name: rsi_ok
symbol: AAPL
interval: 1d
type: rsi_mean_reversion
period: 14
oversold: 30
overbought: 70
position_sizing:
  method: percent
  percent_of_equity: 0.1
"#;

    let spec = StrategyParser::from_yaml_str(yaml).expect("parse");
    StrategyValidator::validate(&spec).expect("validate");
    let _compiled = compile_strategy(&spec).expect("compile");
}

#[test]
fn validator_rejects_invalid_macd_periods() {
    let yaml = r#"
name: macd_bad
symbol: AAPL
interval: 1d
type: macd_trend
fast_period: 30
slow_period: 10
signal_period: 9
position_sizing:
  method: fixed
  quantity: 10
"#;

    let spec = StrategyParser::from_yaml_str(yaml).expect("parse");
    let err = StrategyValidator::validate(&spec).expect_err("must fail");
    assert!(err.to_string().contains("fast_period < slow_period"));
}

#[test]
fn composer_majority_resolves_buy() {
    let composer = SignalComposer::new(CompositeMethod::Majority);
    let ts = UtcDateTime::parse("2024-01-01T00:00:00Z").unwrap();
    let votes = vec![
        SignalVote { action: SignalAction::Buy, strength: 0.8, weight: 1.0, reason: "a".into() },
        SignalVote { action: SignalAction::Buy, strength: 0.7, weight: 1.0, reason: "b".into() },
        SignalVote { action: SignalAction::Sell, strength: 0.9, weight: 1.0, reason: "c".into() },
    ];
    let signal = composer.compose(ts, &votes).expect("signal");
    assert_eq!(signal.action, SignalAction::Buy);
}

#[test]
fn sizing_percent_caps_to_cash() {
    let method = PositionSizingMethod::Percent { percent_of_equity: 1.0 };
    let qty = method
        .quantity_for_action(
            SignalAction::Buy,
            PositionSizingInput {
                price: 100.0,
                cash: 500.0,
                equity: 10_000.0,
                current_position: 0.0,
                signal_strength: 1.0,
                volatility: None,
            },
        )
        .unwrap();
    assert_eq!(qty, 5.0);
}

#[test]
fn ma_crossover_emits_signal_after_cross() {
    let symbol = Symbol::parse("AAPL").unwrap();
    let yaml = r#"
name: ma_cross
symbol: AAPL
interval: 1d
type: ma_crossover
short_period: 2
long_period: 3
position_sizing:
  method: fixed
  quantity: 1
"#;
    let spec = StrategyParser::from_yaml_str(yaml).unwrap();
    let mut compiled = compile_strategy(&spec).unwrap();

    let closes = [10.0, 9.0, 8.0, 10.0, 12.0];
    let mut emitted = false;
    for (idx, close) in closes.iter().enumerate() {
        let bar = make_bar(idx + 1, *close);
        let ctx = StrategyContext {
            symbol: &symbol,
            bar: &bar,
            position_qty: 0.0,
            cash: 10_000.0,
            equity: 10_000.0,
        };
        if compiled.strategy.on_bar(&ctx).unwrap().is_some() {
            emitted = true;
        }
    }

    assert!(emitted);
}
```
**Notes:** Keep these tests synthetic and offline; do not call provider/network.

### Step 19: Add CLI dependencies for strategies and backtesting
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/Cargo.toml`
**Action:** modify
**Location:** `[dependencies]` block near existing crate deps.
**What to do:**
Add crate deps needed by new `strategy` command.
**Code:**
```toml
# BEFORE
ferrotick-agent = { path = "../ferrotick-agent" }
ferrotick-core = { path = "../ferrotick-core" }
ferrotick-ml = { path = "../ferrotick-ml" }
ferrotick-warehouse = { path = "../ferrotick-warehouse" }

# AFTER
ferrotick-agent = { path = "../ferrotick-agent" }
ferrotick-backtest = { path = "../ferrotick-backtest" }
ferrotick-core = { path = "../ferrotick-core" }
ferrotick-ml = { path = "../ferrotick-ml" }
ferrotick-strategies = { path = "../ferrotick-strategies" }
ferrotick-warehouse = { path = "../ferrotick-warehouse" }
```
**Notes:** Keep dependency order alphabetically by crate name.

### Step 20: Add CLI command definitions for `strategy`
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/cli.rs`
**Action:** modify
**Location:**
- `Command` enum (around existing `Ml`, `Cache`, `Schema` variants)
- New args/subcommand structs near existing grouped command structs (after `MlExportArgs`)
- Parser tests section (`#[cfg(test)]` module)
**What to do:**
Add command group and parse tests for `list`, `validate`, and `backtest`.
**Code:**
```rust
// in enum Command
/// 🧠 Strategy library commands.
///
/// # Examples
///
///   ferrotick strategy list
///   ferrotick strategy validate --file strategies/rsi.yaml
///   ferrotick strategy backtest --file strategies/rsi.yaml
Strategy(StrategyArgs),

// new structs/enums
#[derive(Debug, Args)]
pub struct StrategyArgs {
    #[command(subcommand)]
    pub command: StrategyCommand,
}

#[derive(Debug, Subcommand)]
pub enum StrategyCommand {
    /// List built-in strategy templates.
    List,

    /// Validate a strategy YAML spec.
    Validate(StrategyValidateArgs),

    /// Backtest a strategy YAML spec.
    Backtest(StrategyBacktestArgs),
}

#[derive(Debug, Args)]
pub struct StrategyValidateArgs {
    /// Path to strategy YAML file.
    #[arg(long)]
    pub file: String,
}

#[derive(Debug, Args)]
pub struct StrategyBacktestArgs {
    /// Path to strategy YAML file.
    #[arg(long)]
    pub file: String,

    /// Optional start date override (YYYY-MM-DD or RFC3339).
    #[arg(long)]
    pub start: Option<String>,

    /// Optional end date override (YYYY-MM-DD or RFC3339).
    #[arg(long)]
    pub end: Option<String>,

    /// Optional bar count cap override.
    #[arg(long)]
    pub max_bars: Option<usize>,
}
```
Add tests:
```rust
#[test]
fn parses_strategy_list_command() {
    let cli = Cli::try_parse_from(["ferrotick", "strategy", "list"]).expect("parse");
    match cli.command {
        Command::Strategy(args) => match args.command {
            StrategyCommand::List => {}
            _ => panic!("expected strategy list"),
        },
        _ => panic!("expected strategy command"),
    }
}

#[test]
fn parses_strategy_validate_command() {
    let cli = Cli::try_parse_from([
        "ferrotick",
        "strategy",
        "validate",
        "--file",
        "strategies/rsi.yaml",
    ])
    .expect("parse");

    match cli.command {
        Command::Strategy(args) => match args.command {
            StrategyCommand::Validate(validate_args) => {
                assert_eq!(validate_args.file, "strategies/rsi.yaml");
            }
            _ => panic!("expected strategy validate"),
        },
        _ => panic!("expected strategy command"),
    }
}

#[test]
fn parses_strategy_backtest_command() {
    let cli = Cli::try_parse_from([
        "ferrotick",
        "strategy",
        "backtest",
        "--file",
        "strategies/rsi.yaml",
        "--start",
        "2024-01-01",
        "--end",
        "2024-12-31",
        "--max-bars",
        "500",
    ])
    .expect("parse");

    match cli.command {
        Command::Strategy(args) => match args.command {
            StrategyCommand::Backtest(backtest_args) => {
                assert_eq!(backtest_args.file, "strategies/rsi.yaml");
                assert_eq!(backtest_args.max_bars, Some(500));
            }
            _ => panic!("expected strategy backtest"),
        },
        _ => panic!("expected strategy command"),
    }
}
```
**Notes:** Keep all new test style consistent with existing `Cli::try_parse_from` tests.

### Step 21: Create `strategy` command implementation
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/strategy.rs`
**Action:** create
**Location:** new file
**What to do:**
Implement `strategy list`, `strategy validate`, and `strategy backtest`.
**Code:**
```rust
use std::cell::RefCell;
use std::path::PathBuf;

use ferrotick_backtest::{
    BacktestConfig, BacktestEngine, BarEvent, Order, Portfolio, SignalAction as BtSignalAction,
    SignalEvent as BtSignalEvent, Strategy as BacktestStrategy,
};
use ferrotick_core::{ProviderId, Symbol, UtcDateTime};
use ferrotick_ml::FeatureStore;
use ferrotick_strategies::{
    built_in_strategy_catalog, compile_strategy, PositionSizingInput, PositionSizingMethod,
    Signal, SignalAction, Strategy as StrategyTrait, StrategyContext, StrategyParser,
    StrategyValidator,
};

use crate::cli::{StrategyArgs, StrategyBacktestArgs, StrategyCommand, StrategyValidateArgs};
use crate::error::CliError;

use super::CommandResult;

pub async fn run(
    args: &StrategyArgs,
    source_chain: Vec<ProviderId>,
) -> Result<CommandResult, CliError> {
    match &args.command {
        StrategyCommand::List => run_list(source_chain),
        StrategyCommand::Validate(validate_args) => run_validate(validate_args, source_chain),
        StrategyCommand::Backtest(backtest_args) => run_backtest(backtest_args, source_chain).await,
    }
}

fn run_list(source_chain: Vec<ProviderId>) -> Result<CommandResult, CliError> {
    Ok(CommandResult::ok(
        serde_json::json!({
            "strategies": built_in_strategy_catalog(),
            "count": built_in_strategy_catalog().len(),
        }),
        source_chain,
    ))
}

fn run_validate(
    args: &StrategyValidateArgs,
    source_chain: Vec<ProviderId>,
) -> Result<CommandResult, CliError> {
    let path = PathBuf::from(&args.file);
    let spec = StrategyParser::from_yaml_file(path.as_path())
        .map_err(|err| CliError::Command(err.to_string()))?;
    StrategyValidator::validate(&spec).map_err(|err| CliError::Command(err.to_string()))?;

    Ok(CommandResult::ok(
        serde_json::json!({
            "file": path,
            "valid": true,
            "name": spec.name,
            "symbol": spec.symbol,
            "interval": spec.interval,
            "type": match spec.strategy {
                ferrotick_strategies::StrategyConfig::MaCrossover { .. } => "ma_crossover",
                ferrotick_strategies::StrategyConfig::RsiMeanReversion { .. } => "rsi_mean_reversion",
                ferrotick_strategies::StrategyConfig::MacdTrend { .. } => "macd_trend",
                ferrotick_strategies::StrategyConfig::BollingerSqueeze { .. } => "bollinger_squeeze",
            }
        }),
        source_chain,
    ))
}

async fn run_backtest(
    args: &StrategyBacktestArgs,
    source_chain: Vec<ProviderId>,
) -> Result<CommandResult, CliError> {
    let path = PathBuf::from(&args.file);
    let mut spec = StrategyParser::from_yaml_file(path.as_path())
        .map_err(|err| CliError::Command(err.to_string()))?;

    if let Some(start) = &args.start {
        spec.backtest.start = Some(normalize_cli_date(start, false)?);
    }
    if let Some(end) = &args.end {
        spec.backtest.end = Some(normalize_cli_date(end, true)?);
    }
    if let Some(max_bars) = args.max_bars {
        spec.backtest.max_bars = max_bars;
    }

    StrategyValidator::validate(&spec).map_err(|err| CliError::Command(err.to_string()))?;
    let mut compiled = compile_strategy(&spec).map_err(|err| CliError::Command(err.to_string()))?;

    let start = parse_optional_spec_date(spec.backtest.start.as_deref())?;
    let end = parse_optional_spec_date(spec.backtest.end.as_deref())?;
    if let (Some(s), Some(e)) = (start, end) {
        if s > e {
            return Err(CliError::Command(String::from(
                "--start must be earlier than or equal to --end",
            )));
        }
    }

    let store = FeatureStore::open_default().map_err(|err| CliError::Command(err.to_string()))?;
    let mut bars = store
        .load_daily_bars(&compiled.symbol, start, end)
        .map_err(|err| CliError::Command(err.to_string()))?;

    let mut result = if bars.is_empty() {
        CommandResult::ok(
            serde_json::json!({
                "file": path,
                "name": compiled.name,
                "symbol": compiled.symbol.as_str(),
                "bars_used": 0,
                "backtest": null,
            }),
            source_chain,
        )
        .with_warning("no bars found in warehouse; run `ferrotick cache load <symbol>` first")
    } else {
        if bars.len() > spec.backtest.max_bars {
            let keep = spec.backtest.max_bars;
            bars = bars.split_off(bars.len() - keep);
        }

        let events: Vec<BarEvent> = bars
            .into_iter()
            .map(|bar| BarEvent::new(compiled.symbol.clone(), bar))
            .collect();

        let mut adapter = StrategyBacktestAdapter::new(compiled.strategy, compiled.position_sizing.clone());
        let mut engine = BacktestEngine::new(BacktestConfig {
            initial_capital: spec.backtest.initial_capital,
            start_date: start,
            end_date: end,
            risk_free_rate: spec.backtest.risk_free_rate,
            trading_days_per_year: spec.backtest.trading_days_per_year,
            ..BacktestConfig::default()
        });

        let report = engine
            .run(&mut adapter, events.as_slice())
            .await
            .map_err(|err| CliError::Command(err.to_string()))?;

        CommandResult::ok(
            serde_json::json!({
                "file": path,
                "name": compiled.name,
                "symbol": compiled.symbol.as_str(),
                "bars_used": events.len(),
                "start": start.map(UtcDateTime::format_rfc3339),
                "end": end.map(UtcDateTime::format_rfc3339),
                "backtest": report,
            }),
            source_chain,
        )
    };

    if args.max_bars.is_some() {
        result = result.with_warning("CLI --max-bars override applied");
    }

    Ok(result)
}

struct StrategyBacktestAdapter {
    strategy: Box<dyn StrategyTrait>,
    sizing: PositionSizingMethod,
    pending_signal: RefCell<Option<Signal>>,
    bars_for_atr: Vec<ferrotick_core::Bar>,
    latest_atr: f64,
}

impl StrategyBacktestAdapter {
    fn new(strategy: Box<dyn StrategyTrait>, sizing: PositionSizingMethod) -> Self {
        Self {
            strategy,
            sizing,
            pending_signal: RefCell::new(None),
            bars_for_atr: Vec::new(),
            latest_atr: 0.0,
        }
    }
}

impl BacktestStrategy for StrategyBacktestAdapter {
    fn on_bar(&mut self, bar: &BarEvent, portfolio: &Portfolio) -> Option<BtSignalEvent> {
        self.bars_for_atr.push(bar.bar.clone());

        if let Some(atr_period) = self.sizing.atr_period() {
            if let Ok(series) = ferrotick_ml::features::indicators::compute_atr(&self.bars_for_atr, atr_period) {
                self.latest_atr = series
                    .last()
                    .and_then(|value| *value)
                    .unwrap_or(0.0);
            }
        }

        let ctx = StrategyContext {
            symbol: &bar.symbol,
            bar: &bar.bar,
            position_qty: portfolio.position(&bar.symbol),
            cash: portfolio.cash(),
            equity: portfolio.equity(),
        };

        let signal = self.strategy.on_bar(&ctx).ok()??;
        *self.pending_signal.borrow_mut() = Some(signal.clone());

        Some(BtSignalEvent {
            symbol: bar.symbol.clone(),
            ts: signal.ts,
            action: to_backtest_action(signal.action),
            strength: signal.strength,
            reason: signal.reason,
        })
    }

    fn create_order(
        &self,
        signal: &BtSignalEvent,
        portfolio: &Portfolio,
        _config: &BacktestConfig,
    ) -> Option<Order> {
        let raw_signal = self.pending_signal.borrow_mut().take()?;

        if raw_signal.action == SignalAction::Hold {
            return None;
        }

        let price = portfolio.current_price(&signal.symbol);
        if !price.is_finite() || price <= 0.0 {
            return None;
        }

        let qty = self
            .sizing
            .quantity_for_action(
                raw_signal.action,
                PositionSizingInput {
                    price,
                    cash: portfolio.cash(),
                    equity: portfolio.equity(),
                    current_position: portfolio.position(&signal.symbol),
                    signal_strength: signal.strength,
                    volatility: (self.latest_atr > 0.0).then_some(self.latest_atr),
                },
            )
            .ok()?;

        if qty <= 0.0 {
            return None;
        }

        match raw_signal.action {
            SignalAction::Buy => Some(Order::market_buy(signal.symbol.clone(), qty)),
            SignalAction::Sell => Some(Order::market_sell(signal.symbol.clone(), qty)),
            SignalAction::Hold => None,
        }
    }
}

fn to_backtest_action(action: SignalAction) -> BtSignalAction {
    match action {
        SignalAction::Buy => BtSignalAction::Buy,
        SignalAction::Sell => BtSignalAction::Sell,
        SignalAction::Hold => BtSignalAction::Hold,
    }
}

fn normalize_cli_date(raw: &str, end_of_day: bool) -> Result<String, CliError> {
    if raw.contains('T') {
        return Ok(raw.to_string());
    }

    if end_of_day {
        Ok(format!("{}T23:59:59Z", raw))
    } else {
        Ok(format!("{}T00:00:00Z", raw))
    }
}

fn parse_optional_spec_date(raw: Option<&str>) -> Result<Option<UtcDateTime>, CliError> {
    match raw {
        Some(value) => UtcDateTime::parse(value)
            .map(Some)
            .map_err(CliError::Validation),
        None => Ok(None),
    }
}
```
**Notes:**
- Keep `strategy backtest` warehouse-only (no provider calls).
- ATR is computed in adapter only when `VolatilityAdjusted` sizing is selected.

### Step 22: Wire command module into dispatcher
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/mod.rs`
**Action:** modify
**Location:**
- module declarations at top
- `use crate::cli::{...}` import line
- `match &cli.command` dispatch block
**What to do:**
Register new `strategy` module and delegate command execution.
**Code:**
```rust
// add with other modules
mod strategy;

// keep import list including Command
use crate::cli::{CacheCommand, Cli, Command, SourceSelector};

// in run() match
Command::Strategy(args) => {
    strategy::run(args, non_provider_source_chain(&router, &strategy).await).await?
}
```
**Notes:** Use `non_provider_source_chain` (same pattern as `Ml`, `Schema`, `Sql`).

### Step 23: Add smoke tests for strategy CLI module (optional but recommended)
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/strategy.rs`
**Action:** modify
**Location:** bottom of file under `#[cfg(test)]`
**What to do:**
Add at least one test for `run_validate` success path and one for parser failure.
**Code:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn validate_accepts_valid_yaml_file() {
        let mut file = NamedTempFile::new().expect("tmp");
        std::io::Write::write_all(
            &mut file,
            br#"name: test
symbol: AAPL
interval: 1d
type: ma_crossover
short_period: 20
long_period: 50
position_sizing:
  method: fixed
  quantity: 1
"#,
        )
        .expect("write");

        let args = StrategyValidateArgs {
            file: file.path().display().to_string(),
        };

        let result = run_validate(&args, vec![]).expect("validate");
        assert_eq!(result.data.get("valid").and_then(|v| v.as_bool()), Some(true));
    }
}
```
**Notes:** Keep tests local-only and deterministic.

### Step 24: Format and verify
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick` (workspace root)
**Action:** modify (generated formatting artifacts only)
**Location:** terminal commands
**What to do:**
Run formatting and full verification.
**Code:**
```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
**Notes:** If clippy fails on pre-existing warnings outside touched files, fix only warnings in edited/new files for this phase and document remaining baseline warnings.

## Existing Patterns to Follow
Use the following patterns from the current codebase as templates.

1. `thiserror` error enum style
**Path:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/error.rs`
```rust
#[derive(Debug, Error)]
pub enum BacktestError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error(transparent)]
    Validation(#[from] ferrotick_core::ValidationError),
}
```

2. Indicator computation via `ferrotick-ml` (ta-backed)
**Path:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/src/features/indicators.rs`
```rust
pub fn compute_rsi(closes: &[f64], period: usize) -> MlResult<Vec<Option<f64>>> {
    if period == 0 {
        return Err(MlError::InvalidConfig(String::from(
            "rsi_period must be greater than zero",
        )));
    }
    // ... ta indicator usage
}
```

3. Tagged serde enums for runtime configuration
**Path:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/costs/slippage.rs`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum SlippageModel {
    None,
    FixedBps { bps: f64 },
    VolumeShare { max_volume_share: f64, max_impact_bps: f64 },
}
```

4. CLI command dispatch pattern
**Path:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/mod.rs`
```rust
let command_result = match &cli.command {
    Command::Quote(args) => quote::run(args, &router, &strategy).await?,
    Command::Ml(args) => {
        ml::run(args, non_provider_source_chain(&router, &strategy).await).await?
    }
    // ...
};
```

5. CLI date normalization helper pattern
**Path:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/ml.rs`
```rust
fn parse_cli_date(raw: &str, end_of_day: bool) -> Result<UtcDateTime, CliError> {
    let normalized = if raw.contains('T') {
        raw.to_string()
    } else if end_of_day {
        format!("{}T23:59:59Z", raw)
    } else {
        format!("{}T00:00:00Z", raw)
    };

    UtcDateTime::parse(&normalized).map_err(CliError::Validation)
}
```

## Edge Cases and Error Handling
1. Empty YAML file:
- Behavior: `StrategyParser::from_yaml_file` returns `StrategyError::Parse("strategy YAML is empty")`.

2. Unknown strategy type in YAML:
- Behavior: serde YAML parse fails; surface through `CliError::Command(parse_error_string)` in `strategy validate/backtest`.

3. Invalid symbol:
- Behavior: validator reports `symbol is invalid`; compile is not attempted.

4. Unsupported interval (`1m`, `5m`, etc.) in Phase 9:
- Behavior: validator rejects with `Phase 9 supports interval=1d only`.

5. Invalid period relationships (`short_period >= long_period`, `fast_period >= slow_period`):
- Behavior: validator rejects with explicit rule violation message.

6. Invalid position sizing inputs (negative quantity, percent > 1, Kelly invalid probabilities):
- Behavior: validator rejects spec before execution.

7. Volatility-adjusted sizing without ATR data:
- Behavior: sizing function returns `StrategyError::InvalidConfig("volatility-adjusted sizing requires positive ATR/volatility")`; adapter yields no order for that signal.

8. No bars in warehouse for requested symbol/range:
- Behavior: `strategy backtest` returns `bars_used: 0` and warning `no bars found in warehouse...`; no hard failure.

9. Start date greater than end date:
- Behavior: validator rejects spec; CLI override path also checks and returns `CliError::Command("--start must be earlier than or equal to --end")`.

10. `max_bars` truncation:
- Behavior: keep most recent `max_bars` bars only; add warning when CLI override is used.

11. Zero/invalid market price in create_order:
- Behavior: return `None` order (skip trade) instead of panic/error.

12. Signal tie in `majority` or `weighted` composition:
- Behavior: return `None` (no trade).

## Dependencies and Imports
Add/modify dependencies exactly as follows.

1. Workspace manifest:
- File: `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/Cargo.toml`
- Add: `serde_yaml = "0.9.34"`
- Add member: `"crates/ferrotick-strategies"`

2. New crate dependencies:
- File: `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-strategies/Cargo.toml`
- Add:
  - `ferrotick-core = { path = "../ferrotick-core" }`
  - `ferrotick-ml = { path = "../ferrotick-ml" }`
  - `serde.workspace = true`
  - `serde_yaml.workspace = true`
  - `thiserror.workspace = true`

3. CLI crate dependencies:
- File: `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/Cargo.toml`
- Add:
  - `ferrotick-backtest = { path = "../ferrotick-backtest" }`
  - `ferrotick-strategies = { path = "../ferrotick-strategies" }`

4. New imports in `strategy` command implementation:
- File: `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/strategy.rs`
- Required imports:
  - `ferrotick_backtest::{BacktestConfig, BacktestEngine, BarEvent, Order, Portfolio, SignalAction as BtSignalAction, SignalEvent as BtSignalEvent, Strategy as BacktestStrategy}`
  - `ferrotick_strategies::{built_in_strategy_catalog, compile_strategy, PositionSizingInput, PositionSizingMethod, Signal, SignalAction, Strategy as StrategyTrait, StrategyContext, StrategyParser, StrategyValidator}`
  - `ferrotick_ml::FeatureStore`
  - `std::cell::RefCell`

## Acceptance Criteria
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes
- [ ] No clippy warnings
- [ ] `ferrotick strategy list` returns 4 built-in strategies in JSON output
- [ ] `ferrotick strategy validate --file <valid.yaml>` returns `"valid": true`
- [ ] `ferrotick strategy validate --file <invalid.yaml>` fails with `CliError::Command` and explicit validation message
- [ ] `ferrotick strategy backtest --file <valid.yaml>` returns a backtest report when warehouse has bars

## Out of Scope
1. Live/paper trading execution.
2. Multi-symbol portfolio strategy specs (Phase 9 implementation is single-symbol per spec).
3. Intraday backtesting for strategy specs (`interval != 1d`).
4. Strategy optimization/tuning workflows.
5. Natural-language strategy generation (`strategy create`).
6. Genetic/discovery strategy workflows.
7. Additional strategy families beyond the required four.
