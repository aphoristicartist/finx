pub fn simple_returns(values: &[f64], period: usize) -> Vec<Option<f64>> {
    if period == 0 {
        return vec![None; values.len()];
    }

    let mut output = vec![None; values.len()];
    for index in period..values.len() {
        let prev = values[index - period];
        if prev == 0.0 {
            output[index] = None;
        } else {
            output[index] = Some((values[index] - prev) / prev);
        }
    }

    output
}

pub fn log_returns(values: &[f64], period: usize) -> Vec<Option<f64>> {
    if period == 0 {
        return vec![None; values.len()];
    }

    let mut output = vec![None; values.len()];
    for index in period..values.len() {
        let prev = values[index - period];
        let curr = values[index];
        if prev <= 0.0 || curr <= 0.0 {
            output[index] = None;
        } else {
            output[index] = Some((curr / prev).ln());
        }
    }

    output
}

pub fn z_score(values: &[Option<f64>]) -> Vec<Option<f64>> {
    let non_null: Vec<f64> = values.iter().flatten().copied().collect();
    if non_null.is_empty() {
        return vec![None; values.len()];
    }

    let mean = non_null.iter().sum::<f64>() / non_null.len() as f64;
    let variance = non_null
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f64>()
        / non_null.len() as f64;
    let std = variance.sqrt();

    values
        .iter()
        .map(|value| match value {
            Some(v) if std > 0.0 => Some((v - mean) / std),
            Some(_) => Some(0.0),
            None => None,
        })
        .collect()
}

pub fn min_max(values: &[Option<f64>]) -> Vec<Option<f64>> {
    let non_null: Vec<f64> = values.iter().flatten().copied().collect();
    if non_null.is_empty() {
        return vec![None; values.len()];
    }

    let min = non_null
        .iter()
        .copied()
        .fold(f64::INFINITY, |acc, value| acc.min(value));
    let max = non_null
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, |acc, value| acc.max(value));

    values
        .iter()
        .map(|value| match value {
            Some(v) if max > min => Some((v - min) / (max - min)),
            Some(_) => Some(0.0),
            None => None,
        })
        .collect()
}
