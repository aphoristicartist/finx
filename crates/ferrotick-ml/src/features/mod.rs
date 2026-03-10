pub mod indicators;
pub mod store;
pub mod transforms;
pub mod windows;

use ferrotick_core::{Bar, Symbol};
use serde::{Deserialize, Serialize};

use crate::{MlError, MlResult};

pub use store::FeatureStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IndicatorSelection {
    pub rsi: bool,
    pub macd: bool,
    pub bb: bool,
    pub atr: bool,
}

impl IndicatorSelection {
    pub const fn all() -> Self {
        Self {
            rsi: true,
            macd: true,
            bb: true,
            atr: true,
        }
    }

    pub fn from_csv(raw: &str) -> MlResult<Self> {
        let trimmed = raw.trim().to_ascii_lowercase();
        if trimmed.is_empty() || trimmed == "all" {
            return Ok(Self::all());
        }

        let mut selection = Self {
            rsi: false,
            macd: false,
            bb: false,
            atr: false,
        };

        for token in trimmed.split(',').map(str::trim).filter(|v| !v.is_empty()) {
            match token {
                "all" => return Ok(Self::all()),
                "rsi" => selection.rsi = true,
                "macd" => selection.macd = true,
                "bb" | "bollinger" => selection.bb = true,
                "atr" => selection.atr = true,
                other => {
                    return Err(MlError::InvalidInput(format!(
                        "unsupported indicator '{other}'. Valid values: all,rsi,macd,bb,atr"
                    )))
                }
            }
        }

        Ok(selection)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    pub rsi_period: usize,
    pub macd_fast: usize,
    pub macd_slow: usize,
    pub macd_signal: usize,
    pub bb_period: usize,
    pub bb_std_dev: f64,
    pub atr_period: usize,
    pub window: usize,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            rsi_period: 14,
            macd_fast: 12,
            macd_slow: 26,
            macd_signal: 9,
            bb_period: 20,
            bb_std_dev: 2.0,
            atr_period: 14,
            window: 20,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureRow {
    pub symbol: String,
    pub timestamp: String,
    pub rsi: Option<f64>,
    pub macd: Option<f64>,
    pub macd_signal: Option<f64>,
    pub bb_upper: Option<f64>,
    pub bb_lower: Option<f64>,
    pub atr: Option<f64>,
    pub return_1d: Option<f64>,
    pub return_5d: Option<f64>,
    pub return_20d: Option<f64>,
    pub rolling_mean_20: Option<f64>,
    pub rolling_std_20: Option<f64>,
    pub lag_1: Option<f64>,
    pub lag_2: Option<f64>,
    pub lag_3: Option<f64>,
    pub rolling_momentum: Option<f64>,
}

pub struct FeatureEngineer {
    config: FeatureConfig,
    selection: IndicatorSelection,
}

impl FeatureEngineer {
    pub fn new(config: FeatureConfig, selection: IndicatorSelection) -> MlResult<Self> {
        if config.window == 0 {
            return Err(MlError::InvalidConfig(String::from(
                "window must be greater than zero",
            )));
        }
        Ok(Self { config, selection })
    }

    pub fn compute_for_symbol(&self, symbol: &Symbol, bars: &[Bar]) -> MlResult<Vec<FeatureRow>> {
        if bars.is_empty() {
            return Ok(Vec::new());
        }

        let closes: Vec<f64> = bars.iter().map(|bar| bar.close).collect();

        let rsi = if self.selection.rsi {
            indicators::compute_rsi(&closes, self.config.rsi_period)?
        } else {
            vec![None; bars.len()]
        };

        let macd_series = if self.selection.macd {
            Some(indicators::compute_macd(
                &closes,
                self.config.macd_fast,
                self.config.macd_slow,
                self.config.macd_signal,
            )?)
        } else {
            None
        };

        let bb_series = if self.selection.bb {
            Some(indicators::compute_bollinger(
                &closes,
                self.config.bb_period,
                self.config.bb_std_dev,
            )?)
        } else {
            None
        };

        let atr = if self.selection.atr {
            indicators::compute_atr(bars, self.config.atr_period)?
        } else {
            vec![None; bars.len()]
        };

        let return_1d = transforms::forward_simple_returns(&closes, 1);
        let return_5d = transforms::forward_simple_returns(&closes, 5);
        let return_20d = transforms::forward_simple_returns(&closes, 20);

        let rolling_mean_20 = windows::rolling_mean(&closes, self.config.window);
        let rolling_std_20 = windows::rolling_std(&closes, self.config.window);
        let (lag_1, lag_2, lag_3) = windows::lag_features(&closes);
        let rolling_momentum = windows::rolling_momentum(&closes, self.config.window);

        let mut rows = Vec::with_capacity(bars.len());
        for index in 0..bars.len() {
            rows.push(FeatureRow {
                symbol: symbol.as_str().to_string(),
                timestamp: bars[index].ts.format_rfc3339(),
                rsi: rsi[index],
                macd: macd_series.as_ref().and_then(|series| series.macd[index]),
                macd_signal: macd_series.as_ref().and_then(|series| series.signal[index]),
                bb_upper: bb_series.as_ref().and_then(|series| series.upper[index]),
                bb_lower: bb_series.as_ref().and_then(|series| series.lower[index]),
                atr: atr[index],
                return_1d: return_1d[index],
                return_5d: return_5d[index],
                return_20d: return_20d[index],
                rolling_mean_20: rolling_mean_20[index],
                rolling_std_20: rolling_std_20[index],
                lag_1: lag_1[index],
                lag_2: lag_2[index],
                lag_3: lag_3[index],
                rolling_momentum: rolling_momentum[index],
            });
        }

        Ok(rows)
    }
}
