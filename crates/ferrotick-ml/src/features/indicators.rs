use ferrotick_core::Bar;
use ta::indicators::{
    AverageTrueRange, BollingerBands, MovingAverageConvergenceDivergence, RelativeStrengthIndex,
};
use ta::{DataItem, Next};

use crate::{MlError, MlResult};

pub struct MacdSeries {
    pub macd: Vec<Option<f64>>,
    pub signal: Vec<Option<f64>>,
}

pub struct BollingerSeries {
    pub upper: Vec<Option<f64>>,
    pub lower: Vec<Option<f64>>,
}

pub fn compute_rsi(closes: &[f64], period: usize) -> MlResult<Vec<Option<f64>>> {
    if period == 0 {
        return Err(MlError::InvalidConfig(String::from(
            "rsi_period must be greater than zero",
        )));
    }

    let mut indicator = RelativeStrengthIndex::new(period)
        .map_err(|err| MlError::InvalidConfig(err.to_string()))?;

    let mut output = Vec::with_capacity(closes.len());
    for (index, close) in closes.iter().enumerate() {
        let value = indicator.next(*close);
        output.push((index + 1 >= period).then_some(value));
    }

    Ok(output)
}

pub fn compute_macd(
    closes: &[f64],
    fast: usize,
    slow: usize,
    signal: usize,
) -> MlResult<MacdSeries> {
    if fast == 0 || slow == 0 || signal == 0 || fast >= slow {
        return Err(MlError::InvalidConfig(String::from(
            "MACD config must satisfy fast > 0, slow > 0, signal > 0, fast < slow",
        )));
    }

    let mut indicator = MovingAverageConvergenceDivergence::new(fast, slow, signal)
        .map_err(|err| MlError::InvalidConfig(err.to_string()))?;

    let warmup = slow + signal - 1;
    let mut macd = Vec::with_capacity(closes.len());
    let mut signal_line = Vec::with_capacity(closes.len());

    for (index, close) in closes.iter().enumerate() {
        let value = indicator.next(*close);
        let valid = index + 1 >= warmup;
        macd.push(valid.then_some(value.macd));
        signal_line.push(valid.then_some(value.signal));
    }

    Ok(MacdSeries {
        macd,
        signal: signal_line,
    })
}

pub fn compute_bollinger(closes: &[f64], period: usize, stdev: f64) -> MlResult<BollingerSeries> {
    if period == 0 || !stdev.is_finite() || stdev <= 0.0 {
        return Err(MlError::InvalidConfig(String::from(
            "Bollinger config must satisfy period > 0 and stdev > 0",
        )));
    }

    let mut indicator = BollingerBands::new(period, stdev)
        .map_err(|err| MlError::InvalidConfig(err.to_string()))?;

    let mut upper = Vec::with_capacity(closes.len());
    let mut lower = Vec::with_capacity(closes.len());

    for (index, close) in closes.iter().enumerate() {
        let value = indicator.next(*close);
        let valid = index + 1 >= period;
        upper.push(valid.then_some(value.upper));
        lower.push(valid.then_some(value.lower));
    }

    Ok(BollingerSeries { upper, lower })
}

pub fn compute_atr(bars: &[Bar], period: usize) -> MlResult<Vec<Option<f64>>> {
    if period == 0 {
        return Err(MlError::InvalidConfig(String::from(
            "atr_period must be greater than zero",
        )));
    }

    let mut indicator = AverageTrueRange::new(period)
        .map_err(|err| MlError::InvalidConfig(err.to_string()))?;

    let mut output = Vec::with_capacity(bars.len());
    for (index, bar) in bars.iter().enumerate() {
        let item = DataItem::builder()
            .high(bar.high)
            .low(bar.low)
            .close(bar.close)
            .volume(bar.volume.unwrap_or(0) as f64)
            .build()
            .map_err(|err| MlError::Compute(err.to_string()))?;
        let value = indicator.next(&item);
        output.push((index + 1 >= period).then_some(value));
    }

    Ok(output)
}
