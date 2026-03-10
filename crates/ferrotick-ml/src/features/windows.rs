pub fn rolling_mean(values: &[f64], window: usize) -> Vec<Option<f64>> {
    if window == 0 {
        return vec![None; values.len()];
    }

    let mut output = vec![None; values.len()];
    for index in (window - 1)..values.len() {
        let slice = &values[index + 1 - window..=index];
        output[index] = Some(slice.iter().sum::<f64>() / window as f64);
    }
    output
}

pub fn rolling_std(values: &[f64], window: usize) -> Vec<Option<f64>> {
    if window < 2 {
        return vec![None; values.len()];
    }

    let mut output = vec![None; values.len()];
    for index in (window - 1)..values.len() {
        let slice = &values[index + 1 - window..=index];
        let mean = slice.iter().sum::<f64>() / window as f64;
        let variance = slice
            .iter()
            .map(|value| {
                let delta = value - mean;
                delta * delta
            })
            .sum::<f64>()
            / (window as f64 - 1.0);
        output[index] = Some(variance.sqrt());
    }

    output
}

pub fn rolling_min(values: &[f64], window: usize) -> Vec<Option<f64>> {
    if window == 0 {
        return vec![None; values.len()];
    }

    let mut output = vec![None; values.len()];
    for index in (window - 1)..values.len() {
        let slice = &values[index + 1 - window..=index];
        let min = slice
            .iter()
            .copied()
            .fold(f64::INFINITY, |acc, value| acc.min(value));
        output[index] = Some(min);
    }

    output
}

pub fn rolling_max(values: &[f64], window: usize) -> Vec<Option<f64>> {
    if window == 0 {
        return vec![None; values.len()];
    }

    let mut output = vec![None; values.len()];
    for index in (window - 1)..values.len() {
        let slice = &values[index + 1 - window..=index];
        let max = slice
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, |acc, value| acc.max(value));
        output[index] = Some(max);
    }

    output
}

pub fn lag_features(values: &[f64]) -> LagFeatures {
    let mut lag_1 = vec![None; values.len()];
    let mut lag_2 = vec![None; values.len()];
    let mut lag_3 = vec![None; values.len()];

    for index in 0..values.len() {
        if index >= 1 {
            lag_1[index] = Some(values[index - 1]);
        }
        if index >= 2 {
            lag_2[index] = Some(values[index - 2]);
        }
        if index >= 3 {
            lag_3[index] = Some(values[index - 3]);
        }
    }

    (lag_1, lag_2, lag_3)
}

pub fn rolling_momentum(values: &[f64], window: usize) -> Vec<Option<f64>> {
    if window == 0 {
        return vec![None; values.len()];
    }

    let mut output = vec![None; values.len()];
    for index in window..values.len() {
        let prev = values[index - window];
        if prev != 0.0 {
            output[index] = Some(values[index] / prev - 1.0);
        }
    }

    output
}
pub type LagFeatures = (Vec<Option<f64>>, Vec<Option<f64>>, Vec<Option<f64>>);
