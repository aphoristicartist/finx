### Summary
The crate compiles, but this Phase 9 implementation is not production-ready yet. There are several correctness/spec violations in the strategy pipeline (trait contract, signal->order flow, DSL handling) plus major validation/testing gaps that can lead to silent misconfiguration and incorrect trades.

### Critical Issues (must fix)
Signal generation/order conversion has hard blockers and DSL behavior can silently misconfigure trading logic.
- [ ] crates/ferrotick-strategies/src/traits/strategy.rs:50 — `Strategy` is not `Send + Sync`. This blocks safe integration with async/backtest adapters that require sendable strategy state. Suggested fix.
```diff
-pub trait Strategy {
+pub trait Strategy: Send + Sync {
```

- [ ] crates/ferrotick-strategies/src/signals/generator.rs:21 — `SignalGenerator` is missing `on_signal`, so the framework cannot convert generated signals into orders across strategies (required by the trait and Phase 9 flow). Suggested fix.
```diff
-use crate::traits::strategy::{Signal, Strategy};
+use crate::traits::strategy::{Order, Signal, Strategy};
@@
     pub fn on_bar(&mut self, bar: &Bar) -> Vec<Signal> {
         self.strategies
             .iter_mut()
             .filter_map(|strategy| strategy.on_bar(bar))
             .collect()
     }
+
+    pub fn on_signal(&mut self, signal: &Signal) -> Vec<Order> {
+        self.strategies
+            .iter_mut()
+            .filter_map(|strategy| strategy.on_signal(signal))
+            .collect()
+    }
```

- [ ] crates/ferrotick-strategies/src/dsl/parser.rs:23 — `IndicatorRule` cannot parse documented strategy examples (`name`/`condition` fields and non-scalar `value` like `[45,55]`), despite `deny_unknown_fields`. This causes valid documented specs to fail parsing. Suggested fix.
```diff
+#[derive(Debug, Clone, Deserialize)]
+#[serde(untagged)]
+pub enum RuleValue {
+    Scalar(f64),
+    Range([f64; 2]),
+}
+
 #[derive(Debug, Clone, Deserialize)]
 #[serde(deny_unknown_fields)]
 pub struct IndicatorRule {
+    #[serde(default)]
+    pub name: Option<String>,
+    #[serde(default)]
+    pub condition: Option<String>,
     pub indicator: String,
     #[serde(default)]
     pub period: Option<usize>,
     pub operator: String,
-    pub value: f64,
+    pub value: RuleValue,
     pub action: String,
 }
```

- [ ] crates/ferrotick-strategies/src/dsl/mod.rs:82 — Unknown `position_sizing.method` silently falls back to `Percent`. This can place orders with unintended risk when a config is misspelled. Suggested fix.
```diff
     let method = match spec.position_sizing.method.as_str() {
         "fixed" => PositionSizingMethod::Fixed,
         "percent" => PositionSizingMethod::Percent,
         "volatility" => PositionSizingMethod::Volatility,
         "kelly" => PositionSizingMethod::Kelly,
-        _ => PositionSizingMethod::Percent,
+        other => {
+            return Err(StrategyError::InvalidConfig(format!(
+                "unknown position sizing method: {other}"
+            )))
+        }
     };
```

- [ ] crates/ferrotick-strategies/src/dsl/mod.rs:66 — `bb_squeeze` `num_std` extraction is incorrect (`extract_value` matches `operator`, not `indicator`), so user-configured std-dev is ignored. This produces wrong signals while appearing configured correctly. Suggested fix.
```diff
-            let num_std = extract_value(&spec.entry_rules, "num_std", 2.0);
+            let num_std = extract_indicator_value(&spec.entry_rules, "num_std", 2.0);
@@
+fn extract_indicator_value(rules: &[IndicatorRule], indicator: &str, default: f64) -> f64 {
+    rules
+        .iter()
+        .find(|r| r.indicator.eq_ignore_ascii_case(indicator))
+        .and_then(|r| match &r.value {
+            RuleValue::Scalar(v) => Some(*v),
+            RuleValue::Range(_) => None,
+        })
+        .unwrap_or(default)
+}
```

- [ ] crates/ferrotick-strategies/src/dsl/mod.rs:41 — Strategy order quantity is derived from `position_sizing.value` directly for all strategies. This conflates signal generation with sizing and can produce nonsensical quantities (e.g., `percent=0.1` becomes `0.1` shares). Suggested fix.
```diff
-            let qty = spec.position_sizing.value;
+            let qty = 1.0;
```
Apply the same change at lines 50, 59, and 67; use `build_position_sizer` at execution time for final quantity.

### Important Issues (should fix)
These issues degrade correctness, resilience, and maintainability.
- [ ] crates/ferrotick-strategies/src/dsl/validator.rs:53 — Rule validation is incomplete (no checks for empty rule sets, invalid operators/actions, missing indicator names, or invalid periods). Invalid specs pass validation and fail later in less clear ways. Suggested fix.
```diff
+use super::parser::{RuleValue, StrategySpec};

+    if spec.entry_rules.is_empty() {
+        issues.push(ValidationIssue {
+            field: "entry_rules".to_string(),
+            message: "must contain at least one rule".to_string(),
+        });
+    }
+    if spec.exit_rules.is_empty() {
+        issues.push(ValidationIssue {
+            field: "exit_rules".to_string(),
+            message: "must contain at least one rule".to_string(),
+        });
+    }
@@
     for (idx, rule) in spec.entry_rules.iter().enumerate() {
-        if rule.value.is_finite() == false {
+        if rule.indicator.trim().is_empty() {
+            issues.push(ValidationIssue {
+                field: format!("entry_rules[{idx}].indicator"),
+                message: "indicator must not be empty".to_string(),
+            });
+        }
+        if rule.period == Some(0) {
+            issues.push(ValidationIssue {
+                field: format!("entry_rules[{idx}].period"),
+                message: "period must be > 0".to_string(),
+            });
+        }
+        if !matches!(rule.operator.as_str(), "<" | ">" | "<=" | ">=" | "==" | "between") {
+            issues.push(ValidationIssue {
+                field: format!("entry_rules[{idx}].operator"),
+                message: "invalid operator".to_string(),
+            });
+        }
+        if !matches!(rule.action.as_str(), "buy" | "sell" | "hold" | "close") {
+            issues.push(ValidationIssue {
+                field: format!("entry_rules[{idx}].action"),
+                message: "invalid action".to_string(),
+            });
+        }
+        let value_ok = match &rule.value {
+            RuleValue::Scalar(v) => v.is_finite(),
+            RuleValue::Range([low, high]) => low.is_finite() && high.is_finite() && low <= high,
+        };
+        if !value_ok {
             issues.push(ValidationIssue {
                 field: format!("entry_rules[{}].value", idx),
                 message: "value must be finite (and range low<=high)".to_string(),
             });
         }
     }
```

- [ ] crates/ferrotick-strategies/src/sizing/position.rs:67 — `PercentSizer` and `KellySizer` can allocate >100% equity with no cap, and Kelly input bounds are not validated (`win_rate`/`fraction`). This can generate extreme leverage unexpectedly. Suggested fix.
```diff
 impl PositionSizer for PercentSizer {
     fn size(&self, ctx: &PositionSizingContext) -> f64 {
@@
-        let allocation = ctx.equity * self.percent;
+        let percent = self.percent.clamp(0.0, 1.0);
+        let allocation = ctx.equity * percent;
         (allocation / ctx.price).max(0.0)
     }
 }
@@
 impl PositionSizer for KellySizer {
     fn size(&self, ctx: &PositionSizingContext) -> f64 {
@@
-        if !win_rate.is_finite() || !win_loss_ratio.is_finite() {
+        if !win_rate.is_finite()
+            || !(0.0..=1.0).contains(&win_rate)
+            || !win_loss_ratio.is_finite()
+            || win_loss_ratio <= 0.0
+        {
             return 0.0;
         }
@@
-        let adjusted_fraction = (kelly_fraction * self.fraction).max(0.0);
+        let adjusted_fraction =
+            (kelly_fraction * self.fraction.clamp(0.0, 1.0)).clamp(0.0, 1.0);
```

- [ ] crates/ferrotick-strategies/src/strategies/macd_trend.rs:78 — MACD warmup is off by one (`slow + signal` instead of `slow + signal - 1`), delaying first valid decision and shifting signals. Suggested fix.
```diff
-        if self.closes.len() < self.slow_period + self.signal_period {
+        if self.closes.len() < self.slow_period + self.signal_period - 1 {
             return None;
         }
```

- [ ] crates/ferrotick-strategies/src/strategies/rsi_reversion.rs:80 — RSI/MACD/BB strategies recompute indicators over full history each bar (`O(N^2)`) and grow `closes` unbounded. This will degrade long backtests significantly. Suggested fix (immediate safety cap).
```diff
 // rsi_reversion.rs
     fn on_bar(&mut self, bar: &Bar) -> Option<Signal> {
         self.closes.push(bar.close);
+        const MAX_HISTORY: usize = 2048;
+        if self.closes.len() > MAX_HISTORY {
+            self.closes.remove(0);
+        }
```
Apply the same cap in `macd_trend.rs` (after line 77) and `bb_squeeze.rs` (after line 72). Longer-term: migrate to incremental indicator state.

- [ ] crates/ferrotick-strategies/src/traits/strategy.rs:71 — `Portfolio::equity()` is mathematically incorrect (`cash + position` assumes `position` is currency, not quantity). If this type is used, risk/PnL will be wrong. Suggested fix.
```diff
-    pub fn equity(&self) -> f64 {
-        self.cash + self.position
+    pub fn equity(&self, mark_price: f64) -> f64 {
+        self.cash + (self.position * mark_price)
     }
```

- [ ] crates/ferrotick-strategies/src/lib.rs:1 — Test coverage is effectively absent (`cargo test -p ferrotick-strategies` runs 0 tests). Strategy logic, DSL validation, and sizing math are currently unguarded. Suggested fix.
```diff
*** Add File: crates/ferrotick-strategies/tests/phase9_strategy_library.rs
+use ferrotick_strategies::dsl::parse_and_validate_strategy_yaml;
+use ferrotick_strategies::strategies::MovingAverageCrossoverStrategy;
+use ferrotick_strategies::traits::Strategy;
+
+#[test]
+fn validates_known_good_yaml() {
+    let raw = r#"
+name: rsi_mean_reversion
+type: mean_reversion
+timeframe: 1d
+entry_rules:
+  - indicator: rsi
+    period: 14
+    operator: "<"
+    value: 30
+    action: buy
+exit_rules:
+  - indicator: rsi
+    period: 14
+    operator: ">"
+    value: 70
+    action: sell
+position_sizing:
+  method: percent
+  value: 0.1
+"#;
+    parse_and_validate_strategy_yaml(raw).expect("must parse/validate");
+}
+
+#[test]
+fn ma_crossover_constructs_with_valid_params() {
+    MovingAverageCrossoverStrategy::new("AAPL", 10, 20, 1.0).expect("must build");
+}
```

### Minor Issues (nice to have)
These are lower-risk cleanup items.
- [ ] crates/ferrotick-strategies/src/dsl/validator.rs:53 — Operator/action comparisons are case-sensitive, which is stricter than necessary and hurts UX for YAML authors.
```diff
+        let operator = rule.operator.trim().to_ascii_lowercase();
+        if !matches!(operator.as_str(), "<" | ">" | "<=" | ">=" | "==" | "between") {
+            // ...
+        }
+        let action = rule.action.trim().to_ascii_lowercase();
+        if !matches!(action.as_str(), "buy" | "sell" | "hold" | "close") {
+            // ...
+        }
```

- [ ] crates/ferrotick-strategies/src/strategies/ma_crossover.rs:51 — Signal construction logic is duplicated across all four strategies, increasing maintenance cost and drift risk.
```diff
*** Add File: crates/ferrotick-strategies/src/strategies/common.rs
+use ferrotick_core::Bar;
+use crate::traits::strategy::{Signal, SignalAction};
+
+pub fn build_signal(symbol: &str, bar: &Bar, action: SignalAction, strength: f64, reason: String) -> Signal {
+    Signal {
+        symbol: symbol.to_string(),
+        ts: bar.ts.format_rfc3339(),
+        action,
+        strength: strength.clamp(0.0, 1.0),
+        reason,
+    }
+}
```
Then replace local `fn signal(...)` bodies in each strategy with calls to `common::build_signal`.

- [ ] crates/ferrotick-strategies/src/lib.rs:1 — Public API has minimal rustdoc coverage, which makes DSL/sizing behavior harder to use correctly from CLI and downstream crates.
```diff
+/// Parse, validate, and compile YAML-based trading strategies.
 pub mod dsl;
+/// Position-sizing methods (fixed/percent/volatility/kelly).
 pub mod sizing;
+/// Signal generation and aggregation utilities.
 pub mod signals;
```

### Recommendations
1. Add a full strategy test matrix (warmup behavior, crossover boundaries, invalid configs, sizing edge-cases, parser/validator negative tests).
2. Separate concerns strictly: strategies should emit normalized intent; sizing should be applied once in execution adapter.
3. Replace full-history indicator recomputation with incremental indicator state to keep runtime near `O(N)`.
4. Align DSL schema and repository examples, then pin with compatibility tests to avoid future drift.
5. Add integration tests that bridge `ferrotick-strategies` outputs into `ferrotick-backtest` order types.

### Positive Findings
1. Constructors in all four strategies validate most obvious invalid parameters early and return typed errors.
2. `serde(deny_unknown_fields)` is used on DSL structs, which is good for typo detection.
3. `SignalAction`/`OrderSide` models are simple and coherent across strategies.
4. Position sizing implementations handle basic non-finite/zero guards instead of panicking.
5. Strategy registry (`built_in_strategies`) is clean and CLI-friendly.
