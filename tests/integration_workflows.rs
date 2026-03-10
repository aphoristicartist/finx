use std::collections::HashSet;

use ferrotick_backtest::{
    BacktestConfig, BacktestEngine, BarEvent, Order, Portfolio,
    SignalAction as BacktestSignalAction, SignalEvent, Strategy as BacktestStrategy,
};
use ferrotick_core::{Bar, Symbol, UtcDateTime};
use ferrotick_ml::{FeatureConfig, FeatureEngineer, IndicatorSelection, Model, SVMClassifier};
use ferrotick_optimization::GridSearchOptimizer;
use ferrotick_strategies::SignalAction as StrategySignalAction;
use ndarray::{s, Array1, Array2};

fn make_wave_bars(count: usize) -> Vec<Bar> {
    (0..count)
        .map(|i| {
            let ts = UtcDateTime::from_unix_timestamp((i as i64) * 86_400).expect("valid ts");
            let close = 100.0 + ((i as f64) / 6.0).sin() * 6.0 + (i as f64) * 0.03;
            Bar::new(
                ts,
                close - 1.0,
                close + 1.0,
                close - 2.0,
                close,
                Some(1_000),
                None,
            )
            .expect("valid bar")
        })
        .collect()
}

fn make_flat_bars(count: usize) -> Vec<Bar> {
    (0..count)
        .map(|i| {
            let ts = UtcDateTime::from_unix_timestamp((i as i64) * 86_400).expect("valid ts");
            Bar::new(ts, 100.0, 101.0, 99.0, 100.0, Some(1_000), None).expect("valid bar")
        })
        .collect()
}

fn build_ml_dataset(bars: &[Bar]) -> (Array2<f64>, Array1<f64>) {
    let symbol = Symbol::parse("PIPE").expect("valid symbol");
    let engineer = FeatureEngineer::new(FeatureConfig::default(), IndicatorSelection::all())
        .expect("feature engineer");
    let rows = engineer
        .compute_for_symbol(&symbol, bars)
        .expect("feature compute");

    let mut features = Vec::new();
    let mut targets = Vec::new();
    for row in rows {
        if let (Some(rsi), Some(macd), Some(atr), Some(ret_1d)) =
            (row.rsi, row.macd, row.atr, row.return_1d)
        {
            if rsi.is_finite() && macd.is_finite() && atr.is_finite() && ret_1d.is_finite() {
                features.extend_from_slice(&[rsi, macd, atr]);
                targets.push(if ret_1d >= 0.0 { 1.0 } else { -1.0 });
            }
        }
    }

    assert!(!targets.is_empty(), "dataset should not be empty");
    assert!(
        targets.iter().any(|t| *t > 0.0) && targets.iter().any(|t| *t < 0.0),
        "dataset should contain both classes"
    );

    (
        Array2::from_shape_vec((targets.len(), 3), features).expect("shape"),
        Array1::from_vec(targets),
    )
}

#[derive(Clone)]
struct MlSignalBacktestStrategy {
    symbol: Symbol,
    actions: Vec<BacktestSignalAction>,
    cursor: usize,
    emitted: usize,
}

impl MlSignalBacktestStrategy {
    fn new(symbol: Symbol, actions: Vec<BacktestSignalAction>) -> Self {
        Self {
            symbol,
            actions,
            cursor: 0,
            emitted: 0,
        }
    }
}

impl BacktestStrategy for MlSignalBacktestStrategy {
    fn on_bar(&mut self, bar: &BarEvent, _portfolio: &Portfolio) -> Option<SignalEvent> {
        let action = self
            .actions
            .get(self.cursor)
            .copied()
            .unwrap_or(BacktestSignalAction::Hold);
        self.cursor += 1;
        if matches!(
            action,
            BacktestSignalAction::Buy | BacktestSignalAction::Sell
        ) {
            self.emitted += 1;
        }

        Some(SignalEvent {
            symbol: self.symbol.clone(),
            ts: bar.bar.ts,
            action,
            strength: 1.0,
            reason: String::from("ml_prediction"),
        })
    }

    fn create_order(
        &self,
        _signal: &SignalEvent,
        _portfolio: &Portfolio,
        _config: &BacktestConfig,
    ) -> Option<Order> {
        None
    }
}

#[derive(Clone)]
struct OptimizableStrategy {
    symbol: Symbol,
    threshold: f64,
}

impl BacktestStrategy for OptimizableStrategy {
    fn on_bar(&mut self, bar: &BarEvent, _portfolio: &Portfolio) -> Option<SignalEvent> {
        let action = if bar.bar.close > self.threshold {
            BacktestSignalAction::Buy
        } else if bar.bar.close < self.threshold {
            BacktestSignalAction::Sell
        } else {
            BacktestSignalAction::Hold
        };

        Some(SignalEvent {
            symbol: self.symbol.clone(),
            ts: bar.bar.ts,
            action,
            strength: 1.0,
            reason: format!("threshold={:.2}", self.threshold),
        })
    }

    fn create_order(
        &self,
        _signal: &SignalEvent,
        _portfolio: &Portfolio,
        _config: &BacktestConfig,
    ) -> Option<Order> {
        None
    }
}

#[derive(Default)]
struct MultiAssetTrackingStrategy {
    seen_symbols: HashSet<String>,
}

impl BacktestStrategy for MultiAssetTrackingStrategy {
    fn on_bar(&mut self, bar: &BarEvent, _portfolio: &Portfolio) -> Option<SignalEvent> {
        self.seen_symbols.insert(bar.symbol.as_str().to_string());
        Some(SignalEvent {
            symbol: bar.symbol.clone(),
            ts: bar.bar.ts,
            action: BacktestSignalAction::Hold,
            strength: 0.0,
            reason: String::from("track_assets"),
        })
    }

    fn create_order(
        &self,
        _signal: &SignalEvent,
        _portfolio: &Portfolio,
        _config: &BacktestConfig,
    ) -> Option<Order> {
        None
    }
}

#[test]
fn test_full_data_to_signal_pipeline() {
    let bars = make_wave_bars(260);
    let (features, targets) = build_ml_dataset(&bars);

    let split = ((features.nrows() as f64) * 0.8) as usize;
    assert!(split > 0 && split < features.nrows());

    let train_x = features.slice(s![..split, ..]).to_owned();
    let train_y = targets.slice(s![..split]).to_owned();
    let test_x = features.slice(s![split.., ..]).to_owned();
    let test_y = targets.slice(s![split..]).to_owned();

    let mut svm = SVMClassifier::new();
    svm.fit(&train_x, &train_y).expect("svm train");
    let predictions = Model::predict(&svm, &test_x).expect("svm predict");

    let signals: Vec<StrategySignalAction> = predictions
        .iter()
        .map(|pred| {
            if *pred > 0.0 {
                StrategySignalAction::Buy
            } else {
                StrategySignalAction::Sell
            }
        })
        .collect();

    let correct = predictions
        .iter()
        .zip(test_y.iter())
        .filter(|(pred, actual)| (**pred - **actual).abs() < f64::EPSILON)
        .count();
    let accuracy = correct as f64 / test_y.len() as f64;

    assert_eq!(signals.len(), test_y.len());
    assert!(
        accuracy >= 0.55,
        "expected reasonable generalization, got {accuracy:.2}"
    );
    assert!(signals
        .iter()
        .any(|s| matches!(s, StrategySignalAction::Buy)));
    assert!(signals
        .iter()
        .any(|s| matches!(s, StrategySignalAction::Sell)));
}

#[tokio::test]
async fn test_backtest_with_ml_strategy() {
    let bars = make_wave_bars(180);
    let (features, targets) = build_ml_dataset(&bars);

    let mut svm = SVMClassifier::new();
    svm.fit(&features, &targets).expect("svm train");
    let predictions = Model::predict(&svm, &features).expect("svm predict");
    assert!(!predictions.is_empty());

    let base_actions: Vec<BacktestSignalAction> = predictions
        .iter()
        .map(|pred| {
            if *pred > 0.0 {
                BacktestSignalAction::Buy
            } else {
                BacktestSignalAction::Sell
            }
        })
        .collect();
    let actions: Vec<BacktestSignalAction> = (0..bars.len())
        .map(|idx| base_actions[idx % base_actions.len()])
        .collect();

    let symbol = Symbol::parse("MLSTRAT").expect("valid symbol");
    let bar_events: Vec<BarEvent> = bars
        .iter()
        .cloned()
        .map(|bar| BarEvent::new(symbol.clone(), bar))
        .collect();
    let mut strategy = MlSignalBacktestStrategy::new(symbol, actions);

    let mut engine = BacktestEngine::new(BacktestConfig::default());
    let report = engine
        .run(&mut strategy, &bar_events)
        .await
        .expect("ml backtest");

    assert_eq!(report.equity_curve.len(), bars.len());
    assert!(strategy.emitted > 0);
    assert!(report.total_return.is_finite());
}

#[tokio::test]
async fn test_strategy_optimization_workflow() {
    let bars = make_flat_bars(150);
    let mut optimizer = GridSearchOptimizer::new();
    optimizer
        .add_param("threshold", vec![99.5, 100.0, 100.5])
        .add_param("bias", vec![0.0, 0.5]);

    let report = optimizer
        .optimize(
            |params| OptimizableStrategy {
                symbol: Symbol::parse("OPT").expect("valid symbol"),
                threshold: params["threshold"] + params["bias"],
            },
            &bars,
            BacktestConfig::default(),
        )
        .await;

    assert_eq!(report.combinations_tested, 6);
    assert_eq!(report.all_results.len(), 6);
    assert!(report.best_params.contains_key("threshold"));
    assert!(report.best_params.contains_key("bias"));

    let deployed = OptimizableStrategy {
        symbol: Symbol::parse("OPT").expect("valid symbol"),
        threshold: report.best_params["threshold"] + report.best_params["bias"],
    };
    let symbol = Symbol::parse("OPT").expect("valid symbol");
    let events: Vec<BarEvent> = bars
        .iter()
        .cloned()
        .map(|bar| BarEvent::new(symbol.clone(), bar))
        .collect();

    let mut strategy = deployed;
    let mut engine = BacktestEngine::new(BacktestConfig::default());
    let deploy_report = engine
        .run(&mut strategy, &events)
        .await
        .expect("deployed backtest");

    assert_eq!(deploy_report.equity_curve.len(), bars.len());
    assert!(deploy_report.final_equity.is_finite());
}

#[tokio::test]
async fn test_multi_asset_portfolio_backtest() {
    let assets = [("AAPL", 180.0), ("AAPLC150DEC24", 12.0), ("ESM26", 5200.0)];
    let mut events = Vec::new();

    for day in 0..40 {
        let ts = UtcDateTime::from_unix_timestamp((day as i64) * 86_400).expect("valid ts");
        for (symbol, base_price) in assets {
            let close = base_price + (day as f64) * 0.5;
            let bar = Bar::new(
                ts,
                close - 1.0,
                close + 1.0,
                close - 2.0,
                close,
                Some(10_000),
                None,
            )
            .expect("valid bar");
            events.push(BarEvent::new(
                Symbol::parse(symbol).expect("valid symbol"),
                bar,
            ));
        }
    }

    let mut strategy = MultiAssetTrackingStrategy::default();
    let mut engine = BacktestEngine::new(BacktestConfig::default());
    let report = engine
        .run(&mut strategy, &events)
        .await
        .expect("multi-asset backtest");

    assert_eq!(report.equity_curve.len(), events.len());
    assert_eq!(strategy.seen_symbols.len(), 3);
    assert!(strategy.seen_symbols.contains("AAPL"));
    assert!(strategy.seen_symbols.contains("AAPLC150DEC24"));
    assert!(strategy.seen_symbols.contains("ESM26"));
}
