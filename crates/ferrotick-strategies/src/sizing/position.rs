use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionSizingMethod {
    Fixed,
    Percent,
    Volatility,
    Kelly,
}

#[derive(Debug, Clone)]
pub struct PositionSizingContext {
    pub equity: f64,
    pub price: f64,
    pub volatility: Option<f64>,
    pub win_rate: Option<f64>,
    pub win_loss_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSizingConfig {
    pub method: PositionSizingMethod,
    pub value: f64,
}

pub trait PositionSizer: Send + Sync {
    fn size(&self, ctx: &PositionSizingContext) -> f64;
}

pub fn build_sizer(config: PositionSizingConfig) -> Box<dyn PositionSizer> {
    match config.method {
        PositionSizingMethod::Fixed => Box::new(FixedSizer {
            amount: config.value,
        }),
        PositionSizingMethod::Percent => Box::new(PercentSizer {
            percent: config.value,
        }),
        PositionSizingMethod::Volatility => Box::new(VolatilitySizer {
            target_risk: config.value,
        }),
        PositionSizingMethod::Kelly => Box::new(KellySizer {
            fraction: config.value,
        }),
    }
}

#[derive(Debug, Clone)]
pub struct FixedSizer {
    pub amount: f64,
}

impl PositionSizer for FixedSizer {
    fn size(&self, ctx: &PositionSizingContext) -> f64 {
        if !ctx.price.is_finite() || ctx.price <= 0.0 {
            return 0.0;
        }
        (self.amount / ctx.price).max(0.0)
    }
}

#[derive(Debug, Clone)]
pub struct PercentSizer {
    pub percent: f64,
}

impl PositionSizer for PercentSizer {
    fn size(&self, ctx: &PositionSizingContext) -> f64 {
        if !ctx.equity.is_finite()
            || ctx.equity <= 0.0
            || !ctx.price.is_finite()
            || ctx.price <= 0.0
        {
            return 0.0;
        }
        let allocation = ctx.equity * self.percent;
        (allocation / ctx.price).max(0.0)
    }
}

#[derive(Debug, Clone)]
pub struct VolatilitySizer {
    pub target_risk: f64,
}

impl PositionSizer for VolatilitySizer {
    fn size(&self, ctx: &PositionSizingContext) -> f64 {
        if !ctx.equity.is_finite()
            || ctx.equity <= 0.0
            || !ctx.price.is_finite()
            || ctx.price <= 0.0
        {
            return 0.0;
        }
        let volatility = ctx.volatility.unwrap_or(0.02);
        if !volatility.is_finite() || volatility <= 0.0 {
            return 0.0;
        }
        let risk_amount = ctx.equity * self.target_risk;
        (risk_amount / (ctx.price * volatility)).max(0.0)
    }
}

#[derive(Debug, Clone)]
pub struct KellySizer {
    pub fraction: f64,
}

impl PositionSizer for KellySizer {
    fn size(&self, ctx: &PositionSizingContext) -> f64 {
        if !ctx.equity.is_finite()
            || ctx.equity <= 0.0
            || !ctx.price.is_finite()
            || ctx.price <= 0.0
        {
            return 0.0;
        }
        let win_rate = ctx.win_rate.unwrap_or(0.5);
        let win_loss_ratio = ctx.win_loss_ratio.unwrap_or(1.0);
        if !win_rate.is_finite() || !win_loss_ratio.is_finite() {
            return 0.0;
        }
        // Kelly formula: f = p - (1-p)/b where p is win rate, b is win/loss ratio
        let kelly_fraction = win_rate - ((1.0 - win_rate) / win_loss_ratio.max(0.01));
        let adjusted_fraction = (kelly_fraction * self.fraction).max(0.0);
        let allocation = ctx.equity * adjusted_fraction;
        (allocation / ctx.price).max(0.0)
    }
}
