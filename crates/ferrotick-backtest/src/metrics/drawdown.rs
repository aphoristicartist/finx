use ferrotick_core::UtcDateTime;
use serde::{Deserialize, Serialize};

use crate::metrics::EquityPoint;

/// Drawdown point for each equity observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownPoint {
    pub ts: UtcDateTime,
    pub equity: f64,
    pub peak: f64,
    pub drawdown: f64,
}

/// Drawdown summary for a full equity curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownSummary {
    pub max_drawdown: f64,
    pub start: Option<UtcDateTime>,
    pub end: Option<UtcDateTime>,
    pub curve: Vec<DrawdownPoint>,
}

pub fn analyze_drawdown(equity_curve: &[EquityPoint]) -> DrawdownSummary {
    if equity_curve.is_empty() {
        return DrawdownSummary {
            max_drawdown: 0.0,
            start: None,
            end: None,
            curve: Vec::new(),
        };
    }

    let mut curve = Vec::with_capacity(equity_curve.len());
    let mut peak = equity_curve[0].equity;
    let mut peak_ts = equity_curve[0].ts;
    let mut max_drawdown = 0.0;
    let mut max_start = None;
    let mut max_end = None;

    for point in equity_curve {
        if point.equity > peak {
            peak = point.equity;
            peak_ts = point.ts;
        }

        let drawdown = if peak <= f64::EPSILON {
            0.0
        } else {
            ((peak - point.equity) / peak).max(0.0)
        };

        if drawdown > max_drawdown {
            max_drawdown = drawdown;
            max_start = Some(peak_ts);
            max_end = Some(point.ts);
        }

        curve.push(DrawdownPoint {
            ts: point.ts,
            equity: point.equity,
            peak,
            drawdown,
        });
    }

    DrawdownSummary {
        max_drawdown,
        start: max_start,
        end: max_end,
        curve,
    }
}

pub fn max_drawdown_from_values(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut peak = values[0];
    let mut max_drawdown = 0.0;

    for &value in values {
        if value > peak {
            peak = value;
        }

        let drawdown = if peak <= f64::EPSILON {
            0.0
        } else {
            ((peak - value) / peak).max(0.0)
        };

        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }

    max_drawdown
}
