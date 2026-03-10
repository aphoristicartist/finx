use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::{Date, OffsetDateTime};

const SECONDS_PER_YEAR: f64 = 31_557_600.0;
const INV_SQRT_2PI: f64 = 0.398_942_280_401_432_7;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionContract {
    pub symbol: String,
    pub strike: f64,
    pub expiry: String,
    pub option_type: OptionType,
    pub underlying_price: f64,
    pub volatility: f64,
    pub risk_free_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptionType {
    Call,
    Put,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greeks {
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub vega: f64,
    pub rho: f64,
}

impl OptionContract {
    /// Black-Scholes option pricing
    pub fn price(&self) -> f64 {
        let s = self.underlying_price;
        let k = self.strike;
        let sigma = self.volatility.max(0.0);
        let r = self.risk_free_rate;
        let t = self.time_to_expiry_years();

        if s <= 0.0 || k <= 0.0 {
            return 0.0;
        }
        if t <= 0.0 {
            return self.intrinsic_value();
        }
        if sigma <= 0.0 {
            let discounted_strike = k * (-r * t).exp();
            return match self.option_type {
                OptionType::Call => (s - discounted_strike).max(0.0),
                OptionType::Put => (discounted_strike - s).max(0.0),
            };
        }

        let (d1, d2) = calculate_d1_d2(s, k, sigma, r, t);
        let discounted_strike = k * (-r * t).exp();
        let price = match self.option_type {
            OptionType::Call => s * standard_normal_cdf(d1) - discounted_strike * standard_normal_cdf(d2),
            OptionType::Put => {
                discounted_strike * standard_normal_cdf(-d2) - s * standard_normal_cdf(-d1)
            }
        };

        price.max(0.0)
    }

    /// Calculate Greeks
    pub fn greeks(&self) -> Greeks {
        let s = self.underlying_price;
        let k = self.strike;
        let sigma = self.volatility.max(0.0);
        let r = self.risk_free_rate;
        let t = self.time_to_expiry_years();

        if s <= 0.0 || k <= 0.0 {
            return Greeks {
                delta: 0.0,
                gamma: 0.0,
                theta: 0.0,
                vega: 0.0,
                rho: 0.0,
            };
        }

        if t <= 0.0 {
            return Greeks {
                delta: self.step_delta(k),
                gamma: 0.0,
                theta: 0.0,
                vega: 0.0,
                rho: 0.0,
            };
        }

        if sigma <= 0.0 {
            let discounted_strike = k * (-r * t).exp();
            return Greeks {
                delta: self.step_delta(discounted_strike),
                gamma: 0.0,
                theta: 0.0,
                vega: 0.0,
                rho: 0.0,
            };
        }

        let sqrt_t = t.sqrt();
        let (d1, d2) = calculate_d1_d2(s, k, sigma, r, t);
        let n_d1 = standard_normal_pdf(d1);
        let discounted_strike = k * (-r * t).exp();

        match self.option_type {
            OptionType::Call => Greeks {
                delta: standard_normal_cdf(d1),
                gamma: n_d1 / (s * sigma * sqrt_t),
                theta: -(s * n_d1 * sigma) / (2.0 * sqrt_t)
                    - r * discounted_strike * standard_normal_cdf(d2),
                vega: s * n_d1 * sqrt_t,
                rho: k * t * (-r * t).exp() * standard_normal_cdf(d2),
            },
            OptionType::Put => Greeks {
                delta: standard_normal_cdf(d1) - 1.0,
                gamma: n_d1 / (s * sigma * sqrt_t),
                theta: -(s * n_d1 * sigma) / (2.0 * sqrt_t)
                    + r * discounted_strike * standard_normal_cdf(-d2),
                vega: s * n_d1 * sqrt_t,
                rho: -k * t * (-r * t).exp() * standard_normal_cdf(-d2),
            },
        }
    }

    fn intrinsic_value(&self) -> f64 {
        let s = self.underlying_price;
        let k = self.strike;

        match self.option_type {
            OptionType::Call => (s - k).max(0.0),
            OptionType::Put => (k - s).max(0.0),
        }
    }

    fn step_delta(&self, boundary: f64) -> f64 {
        let s = self.underlying_price;

        match self.option_type {
            OptionType::Call => {
                if s > boundary {
                    1.0
                } else if s < boundary {
                    0.0
                } else {
                    0.5
                }
            }
            OptionType::Put => {
                if s < boundary {
                    -1.0
                } else if s > boundary {
                    0.0
                } else {
                    -0.5
                }
            }
        }
    }

    fn time_to_expiry_years(&self) -> f64 {
        let now = OffsetDateTime::now_utc();
        let expiry = parse_expiry(&self.expiry).unwrap_or(now);
        ((expiry - now).as_seconds_f64() / SECONDS_PER_YEAR).max(0.0)
    }
}

fn calculate_d1_d2(s: f64, k: f64, sigma: f64, r: f64, t: f64) -> (f64, f64) {
    let sqrt_t = t.sqrt();
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * sqrt_t);
    let d2 = d1 - sigma * sqrt_t;
    (d1, d2)
}

fn standard_normal_pdf(x: f64) -> f64 {
    INV_SQRT_2PI * (-0.5 * x * x).exp()
}

fn standard_normal_cdf(x: f64) -> f64 {
    let abs_x = x.abs();
    let k = 1.0 / (1.0 + 0.231_641_9 * abs_x);
    let poly = ((((1.330_274_429 * k - 1.821_255_978) * k + 1.781_477_937) * k
        - 0.356_563_782)
        * k
        + 0.319_381_530)
        * k;

    let approx = 1.0 - standard_normal_pdf(abs_x) * poly;
    let cdf = if x >= 0.0 { approx } else { 1.0 - approx };
    cdf.clamp(0.0, 1.0)
}

fn parse_expiry(expiry: &str) -> Option<OffsetDateTime> {
    if let Ok(date_time) = OffsetDateTime::parse(expiry, &Rfc3339) {
        return Some(date_time);
    }

    let date_format = time::format_description::parse("[year]-[month]-[day]").ok()?;
    let date = Date::parse(expiry, &date_format).ok()?;
    Some(date.with_hms(0, 0, 0).ok()?.assume_utc())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    fn one_year_expiry() -> String {
        (OffsetDateTime::now_utc() + Duration::seconds(SECONDS_PER_YEAR as i64))
            .format(&Rfc3339)
            .expect("one year expiry must format as RFC3339")
    }

    #[test]
    fn black_scholes_call_price_matches_reference_value() {
        let option = OptionContract {
            symbol: "TEST".to_string(),
            strike: 100.0,
            expiry: one_year_expiry(),
            option_type: OptionType::Call,
            underlying_price: 100.0,
            volatility: 0.2,
            risk_free_rate: 0.05,
        };

        let expected = 10.4506;
        let actual = option.price();
        assert!(
            (actual - expected).abs() < 0.05,
            "Expected call price near {expected:.4}, got {actual:.4}"
        );
    }

    #[test]
    fn black_scholes_put_price_matches_reference_value() {
        let option = OptionContract {
            symbol: "TEST".to_string(),
            strike: 100.0,
            expiry: one_year_expiry(),
            option_type: OptionType::Put,
            underlying_price: 100.0,
            volatility: 0.2,
            risk_free_rate: 0.05,
        };

        let expected = 5.5735;
        let actual = option.price();
        assert!(
            (actual - expected).abs() < 0.05,
            "Expected put price near {expected:.4}, got {actual:.4}"
        );
    }
}
