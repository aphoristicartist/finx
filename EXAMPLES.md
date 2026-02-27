# EXAMPLES.md - Ferrotick AI/ML Use Cases

This document provides comprehensive examples of using ferrotick's AI/ML-native features for quantitative finance workflows.

---

## Table of Contents

1. [Feature Engineering](#1-feature-engineering)
2. [ML Strategy Development](#2-ml-strategy-development)
3. [Backtesting Traditional Strategies](#3-backtesting-traditional-strategies)
4. [Backtesting ML Strategies](#4-backtesting-ml-strategies)
5. [Strategy Optimization](#5-strategy-optimization)
6. [Natural Language Strategy Creation](#6-natural-language-strategy-creation)
7. [Strategy Discovery with Genetic Algorithms](#7-strategy-discovery-with-genetic-algorithms)
8. [Performance Analysis & Reporting](#8-performance-analysis--reporting)
9. [Multi-Symbol Portfolio Backtesting](#9-multi-symbol-portfolio-backtesting)
10. [Real-Time Feature Pipeline](#10-real-time-feature-pipeline)

---

## 1. Feature Engineering

### Basic Feature Extraction

Extract technical indicators from historical data:

```bash
# Extract RSI, MACD, Bollinger Bands for AAPL
ferrotick ml features AAPL \
  --indicators rsi,macd,bollinger,atr \
  --start 2023-01-01 \
  --end 2024-01-01 \
  --output features.parquet

# Output:
# ✓ Extracted 252 bars with 12 features
# ✓ Saved to features.parquet (48 KB)
```

### Feature Engineering with Storage

Store features in the warehouse for reuse:

```bash
# Compute and store features for multiple symbols
ferrotick ml features AAPL,MSFT,GOOGL \
  --indicators rsi,macd,bollinger,atr,obv \
  --interval 1d \
  --start 2020-01-01 \
  --store

# Query features from warehouse
ferrotick sql "SELECT symbol, ts, close, rsi, macd_line 
               FROM features 
               WHERE rsi < 30 
               ORDER BY ts DESC 
               LIMIT 10"
```

### Feature Engineering Output Format

```json
{
  "symbol": "AAPL",
  "interval": "1d",
  "features": [
    {
      "ts": "2024-01-15T00:00:00Z",
      "open": 182.50,
      "high": 185.00,
      "low": 181.25,
      "close": 184.50,
      "volume": 50000000,
      "rsi_14": 65.3,
      "macd_line": 1.25,
      "macd_signal": 0.98,
      "macd_histogram": 0.27,
      "bb_upper": 188.50,
      "bb_middle": 182.00,
      "bb_lower": 175.50,
      "atr_14": 3.25,
      "returns": 0.012,
      "log_returns": 0.0119
    }
  ],
  "metadata": {
    "computed_at": "2024-01-15T16:00:00Z",
    "indicators": ["rsi_14", "macd", "bollinger_20_2", "atr_14"]
  }
}
```

---

## 2. ML Strategy Development

### End-to-End ML Workflow

```bash
# Step 1: Fetch historical data
ferrotick bars AAPL --interval 1d --limit 1000 --output aapl_raw.parquet

# Step 2: Engineer features
ferrotick ml features AAPL \
  --indicators rsi,macd,bollinger,atr,obv,sma_20,sma_50 \
  --input aapl_raw.parquet \
  --output aapl_features.parquet

# Step 3: Create labels (next-day returns > 1% = buy)
ferrotick ml label aapl_features.parquet \
  --method returns \
  --horizon 1 \
  --threshold 0.01 \
  --output aapl_labeled.parquet

# Step 4: Train model
ferrotick ml train \
  --data aapl_labeled.parquet \
  --model lstm \
  --features rsi_14,macd_line,bb_percent,atr_14 \
  --target signal \
  --test-split 0.2 \
  --output models/aapl_lstm.onnx

# Output:
# ✓ Training LSTM model...
# ✓ Epoch 1/50: loss=0.693, accuracy=0.52
# ✓ Epoch 10/50: loss=0.451, accuracy=0.71
# ✓ Epoch 25/50: loss=0.312, accuracy=0.79
# ✓ Epoch 50/50: loss=0.287, accuracy=0.82
# ✓ Test accuracy: 0.78
# ✓ Model saved to models/aapl_lstm.onnx

# Step 5: Evaluate model
ferrotick ml evaluate \
  --model models/aapl_lstm.onnx \
  --test-data aapl_labeled.parquet \
  --metrics accuracy,precision,recall,f1,auc

# Output:
# Model Evaluation Report
# =======================
# Accuracy:  0.78
# Precision: 0.76
# Recall:    0.81
# F1 Score:  0.78
# AUC-ROC:   0.84
```

### Training Different Model Types

```bash
# SVM Classifier
ferrotick ml train \
  --data features.parquet \
  --model svm \
  --kernel rbf \
  --target signal \
  --output svm_model.bin

# Random Forest
ferrotick ml train \
  --data features.parquet \
  --model random_forest \
  --trees 100 \
  --target signal \
  --output rf_model.bin

# LSTM for Price Forecasting
ferrotick ml train \
  --data features.parquet \
  --model lstm \
  --hidden-size 64 \
  --num-layers 2 \
  --target returns \
  --output lstm_model.onnx
```

---

## 3. Backtesting Traditional Strategies

### Moving Average Crossover

```yaml
# strategies/ma_crossover.yaml
name: moving_average_crossover
type: trend_following
timeframe: 1d

entry_rules:
  - name: golden_cross
    condition: sma_50_crosses_above_sma_200
    action: buy
    
  - name: death_cross
    condition: sma_50_crosses_below_sma_200
    action: sell

position_sizing:
  method: percent
  value: 0.10  # 10% of portfolio

risk_management:
  stop_loss: 0.08    # 8% stop loss
  take_profit: 0.20  # 20% take profit
  trailing_stop: 0.05  # 5% trailing stop
```

```bash
# Run backtest
ferrotick backtest \
  --strategy strategies/ma_crossover.yaml \
  --symbols SPY \
  --start 2015-01-01 \
  --end 2024-01-01 \
  --capital 100000 \
  --output results/ma_crossover.json

# Output:
# Backtest Results: MA Crossover (SPY)
# ====================================
# Period: 2015-01-01 to 2024-01-01
# Initial Capital: $100,000
# Final Value: $287,450
#
# Total Return: 187.45%
# Annualized Return: 12.34%
# Volatility: 15.2%
# Sharpe Ratio: 0.81
# Sortino Ratio: 1.12
# Max Drawdown: -22.3%
#
# Trades: 12 (8 winners, 4 losers)
# Win Rate: 66.7%
# Profit Factor: 2.45
#
# Benchmark (Buy & Hold): 165.2%
# Alpha: 22.25%
```

### RSI Mean Reversion

```yaml
# strategies/rsi_mean_reversion.yaml
name: rsi_mean_reversion
type: mean_reversion
timeframe: 1d

entry_rules:
  - name: oversold
    indicator: rsi
    period: 14
    operator: "<"
    value: 30
    action: buy
    
  - name: overbought
    indicator: rsi
    period: 14
    operator: ">"
    value: 70
    action: sell

exit_rules:
  - name: rsi_neutral
    indicator: rsi
    period: 14
    operator: between
    value: [45, 55]
    action: close

position_sizing:
  method: fixed
  value: 100  # $100 per trade

risk_management:
  stop_loss: 0.05
  take_profit: 0.08
```

```bash
ferrotick backtest \
  --strategy strategies/rsi_mean_reversion.yaml \
  --symbols AAPL,MSFT,GOOGL \
  --start 2020-01-01 \
  --end 2024-01-01 \
  --capital 100000
```

---

## 4. Backtesting ML Strategies

### ML-Based Signal Strategy

```yaml
# strategies/ml_predictor.yaml
name: ml_signal_predictor
type: ml
timeframe: 1d

model:
  path: models/aapl_lstm.onnx
  type: onnx
  
features:
  - rsi_14
  - macd_line
  - macd_signal
  - bb_percent
  - atr_14
  - returns_5d
  - volume_ratio

signal_threshold:
  buy: 0.6   # Model confidence > 60%
  sell: 0.4  # Model confidence < 40%

position_sizing:
  method: kelly
  fraction: 0.25  # Use 25% of Kelly criterion
  
risk_management:
  stop_loss: 0.05
  max_position: 0.20  # Max 20% in single position
```

```bash
# Backtest ML strategy
ferrotick backtest \
  --strategy strategies/ml_predictor.yaml \
  --model models/aapl_lstm.onnx \
  --symbols AAPL \
  --start 2022-01-01 \
  --end 2024-01-01 \
  --capital 100000

# Output:
# Backtest Results: ML Signal Predictor (AAPL)
# ============================================
# Model: LSTM (ONNX)
# Period: 2022-01-01 to 2024-01-01
# Initial Capital: $100,000
# Final Value: $142,875
#
# Total Return: 42.88%
# Annualized Return: 19.7%
# Sharpe Ratio: 1.24
# Max Drawdown: -12.3%
#
# Signal Accuracy: 68.5%
# True Positives: 145
# False Positives: 67
# Precision: 0.68
# Recall: 0.72
```

---

## 5. Strategy Optimization

### Grid Search Optimization

```bash
# Optimize RSI mean reversion parameters
ferrotick optimize \
  --strategy strategies/rsi_mean_reversion.yaml \
  --method grid \
  --params "rsi_period=[10,14,21],oversold=[25,30,35],overbought=[65,70,75]" \
  --symbols SPY \
  --start 2015-01-01 \
  --end 2023-01-01 \
  --metric sharpe_ratio \
  --output optimization_results.json

# Output:
# Grid Search Optimization
# =======================
# Total combinations: 27
# Evaluating...
#
# Best Parameters:
#   rsi_period: 14
#   oversold: 30
#   overbought: 70
#
# Best Sharpe Ratio: 0.92
# Total Return: 145.6%
# Max Drawdown: -18.2%
#
# Top 5 Results:
# 1. rsi=14, os=30, ob=70: Sharpe=0.92, Return=145.6%
# 2. rsi=14, os=25, ob=75: Sharpe=0.88, Return=138.2%
# 3. rsi=21, os=30, ob=70: Sharpe=0.85, Return=132.1%
```

### Genetic Algorithm Optimization

```bash
# Use genetic algorithm for complex strategy optimization
ferrotick optimize \
  --strategy strategies/momentum.yaml \
  --method genetic \
  --generations 100 \
  --population 50 \
  --mutation-rate 0.1 \
  --crossover-rate 0.7 \
  --symbols SPY,QQQ,IWM \
  --start 2010-01-01 \
  --end 2023-01-01 \
  --metric sortino_ratio

# Output:
# Genetic Algorithm Optimization
# =============================
# Generation 1/100: Best Fitness=0.45, Avg=0.32
# Generation 25/100: Best Fitness=0.78, Avg=0.61
# Generation 50/100: Best Fitness=0.92, Avg=0.75
# Generation 100/100: Best Fitness=0.98, Avg=0.82
#
# Best Individual:
#   lookback_period: 18
#   momentum_threshold: 0.03
#   position_size: 0.08
#   stop_loss: 0.06
#   take_profit: 0.15
#
# Performance:
#   Sortino Ratio: 1.45
#   Total Return: 234.5%
#   Max Drawdown: -15.8%
```

### Walk-Forward Validation

```bash
# Walk-forward validation to prevent overfitting
ferrotick validate \
  --strategy strategies/optimized_momentum.yaml \
  --method walk-forward \
  --windows 12 \
  --train-period 252 \
  --test-period 63 \
  --symbols SPY \
  --start 2010-01-01

# Output:
# Walk-Forward Validation
# ======================
# Windows: 12
# Train Period: 252 days (1 year)
# Test Period: 63 days (3 months)
#
# Window Results:
# 1. Train: 2010-01-01 to 2010-12-31, Test Return: 8.2%
# 2. Train: 2010-04-01 to 2011-03-31, Test Return: 5.7%
# 3. Train: 2010-07-01 to 2011-06-30, Test Return: -2.1%
# ...
# 12. Train: 2012-10-01 to 2013-09-30, Test Return: 11.4%
#
# Average Test Return: 6.8%
# Test Return Std: 4.2%
# Out-of-Sample Sharpe: 0.94
#
# Overfitting Score: Low (train-test gap: 2.1%)
```

---

## 6. Natural Language Strategy Creation

### Creating Strategies from Natural Language

```bash
# Create strategy from natural language description
ferrotick strategy create \
  --prompt "Mean reversion strategy using RSI oversold conditions with 2% position sizing. Buy when RSI is below 30, sell when above 70. Use a 5% stop loss and 10% take profit."

# Output:
# Generated Strategy: rsi_mean_reversion_auto
# ===========================================
# Type: mean_reversion
# Timeframe: 1d
#
# Entry Rules:
#   - Buy when RSI(14) < 30
#   - Sell when RSI(14) > 70
#
# Position Sizing: 2% of portfolio
# Stop Loss: 5%
# Take Profit: 10%
#
# Saved to: strategies/rsi_mean_reversion_auto.yaml

# Backtest generated strategy
ferrotick backtest \
  --strategy strategies/rsi_mean_reversion_auto.yaml \
  --symbols SPY \
  --start 2020-01-01 \
  --capital 100000
```

### Complex Strategy from Natural Language

```bash
# More complex strategy description
ferrotick strategy create \
  --prompt "Trend following strategy that uses a combination of 50-day and 200-day moving averages for trend direction, with ADX above 25 as a trend strength filter. Enter long when price is above both MAs and ADX > 25. Exit when price crosses below the 50-day MA. Use ATR-based position sizing with 2x ATR stop loss."

# Output:
# Generated Strategy: trend_following_adx
# =======================================
# Type: trend_following
# Timeframe: 1d
#
# Indicators:
#   - SMA(50), SMA(200)
#   - ADX(14)
#   - ATR(14)
#
# Entry Rules:
#   - Price > SMA(50) AND Price > SMA(200)
#   - ADX(14) > 25
#   - Action: buy
#
# Exit Rules:
#   - Price < SMA(50)
#   - Action: sell
#
# Position Sizing:
#   - Method: atr_based
#   - Risk per trade: 1% of portfolio
#   - Units = (Portfolio * 0.01) / (2 * ATR)
#
# Stop Loss: 2 * ATR (trailing)
```

---

## 7. Strategy Discovery with Genetic Algorithms

### Automated Strategy Discovery

```bash
# Discover new strategies using genetic programming
ferrotick strategy discover \
  --symbols SPY,QQQ \
  --start 2010-01-01 \
  --end 2023-01-01 \
  --method genetic \
  --generations 500 \
  --population 100 \
  --indicators rsi,macd,bollinger,sma,ema,atr \
  --output discovered_strategies/

# Output:
# Strategy Discovery (Genetic Programming)
# ========================================
# Generations: 500
# Population: 100
# Target: Maximize Sharpe Ratio
#
# Generation 100/500: Best Sharpe=0.65
# Generation 250/500: Best Sharpe=0.89
# Generation 500/500: Best Sharpe=1.12
#
# Top 3 Discovered Strategies:
#
# 1. strategy_gen_487.yaml
#    Sharpe: 1.12
#    Return: 198.4%
#    Drawdown: -12.3%
#    Logic: RSI(21) < 35 AND MACD > Signal AND Price > SMA(50)
#
# 2. strategy_gen_423.yaml
#    Sharpe: 1.05
#    Return: 175.2%
#    Drawdown: -14.8%
#    Logic: Bollinger %B < 0.2 AND ATR(14) > ATR(14)[1]
#
# 3. strategy_gen_391.yaml
#    Sharpe: 0.98
#    Return: 156.8%
#    Drawdown: -11.5%
#    Logic: SMA(20) > SMA(50) AND RSI(14) < 60
```

---

## 8. Performance Analysis & Reporting

### Detailed Performance Analysis

```bash
# Analyze backtest results
ferrotick analyze results/ma_crossover.json \
  --metrics sharpe,sortino,max-dd,win-rate,profit-factor,var,cvar \
  --benchmark SPY \
  --output analysis_report.md

# Output:
# Performance Analysis Report
# ==========================
# Strategy: MA Crossover
# Period: 2015-01-01 to 2024-01-01
#
# Returns Analysis
# ---------------
# Total Return: 187.45%
# Annualized Return: 12.34%
# Monthly Avg Return: 0.98%
# Daily Avg Return: 0.045%
#
# Risk Metrics
# -----------
# Volatility (Annual): 15.2%
# Sharpe Ratio: 0.81
# Sortino Ratio: 1.12
# Calmar Ratio: 0.55
# Max Drawdown: -22.3%
# Avg Drawdown: -5.2%
# Drawdown Duration: 45 days (avg)
#
# Tail Risk
# --------
# VaR (95%): -2.1%
# VaR (99%): -3.4%
# CVaR (95%): -2.8%
# CVaR (99%): -4.5%
#
# Trade Analysis
# -------------
# Total Trades: 12
# Win Rate: 66.7%
# Avg Win: $12,450
# Avg Loss: -$5,230
# Profit Factor: 2.45
# Expectancy: $4,280 per trade
#
# Benchmark Comparison (SPY)
# -------------------------
# Strategy Return: 187.45%
# Benchmark Return: 165.2%
# Alpha: 22.25%
# Beta: 0.85
# Information Ratio: 0.42
# Tracking Error: 8.2%
```

### Natural Language Explanation

```bash
# Generate natural language explanation of results
ferrotick explain results/ma_crossover.json \
  --query "Why did the strategy underperform in Q3 2022?"

# Output:
# Strategy Underperformance Analysis (Q3 2022)
# ============================================
#
# The MA Crossover strategy underperformed in Q3 2022 due to:
#
# 1. Whipsaw Market Conditions
#    The S&P 500 experienced multiple false trend signals during this period,
#    with the 50-day MA crossing above and below the 200-day MA 3 times.
#    Each crossover triggered a trade that resulted in small losses.
#
# 2. High Volatility Environment
#    Market volatility (VIX) averaged 28 during Q3 2022, compared to the
#    strategy's historical average of 18. Trend-following strategies
#    typically underperform during high-volatility mean-reverting markets.
#
# 3. Missed Recovery
#    The strategy exited positions in late July after a death cross signal,
#    missing the subsequent 8% rally in August.
#
# Recommendations:
# - Consider adding a volatility filter (e.g., VIX < 25) to avoid whipsaws
# - Implement a confirmation period (wait 3 days after crossover)
# - Add an ADX filter to confirm trend strength
```

---

## 9. Multi-Symbol Portfolio Backtesting

### Portfolio Strategy

```yaml
# strategies/portfolio_momentum.yaml
name: portfolio_momentum
type: multi_asset
timeframe: 1d

universe:
  - SPY    # US Large Cap
  - QQQ    # Nasdaq 100
  - IWM    # US Small Cap
  - EFA    # International Developed
  - EEM    # Emerging Markets
  - TLT    # Long-Term Treasuries
  - GLD    # Gold

allocation:
  method: momentum_rank
  top_n: 3
  rebalance: monthly

entry_rules:
  - name: momentum_rank
    indicator: returns_12m
    action: buy_top_n
    n: 3

exit_rules:
  - name: momentum_exit
    indicator: returns_12m
    action: sell_if_rank_below
    threshold: 3

position_sizing:
  method: equal_weight
  
risk_management:
  max_position: 0.35
  stop_loss: 0.15
```

```bash
# Backtest portfolio strategy
ferrotick backtest \
  --strategy strategies/portfolio_momentum.yaml \
  --start 2010-01-01 \
  --end 2024-01-01 \
  --capital 1000000

# Output:
# Portfolio Backtest Results
# =========================
# Strategy: Portfolio Momentum
# Universe: 7 assets (SPY, QQQ, IWM, EFA, EEM, TLT, GLD)
#
# Period: 2010-01-01 to 2024-01-01
# Initial Capital: $1,000,000
# Final Value: $4,287,500
#
# Total Return: 328.75%
# Annualized Return: 11.2%
# Sharpe Ratio: 0.92
# Sortino Ratio: 1.28
# Max Drawdown: -18.5%
#
# Current Holdings (as of 2024-01-01):
# 1. QQQ: 33.3% ($1,429,167)
# 2. SPY: 33.3% ($1,429,167)
# 3. IWM: 33.3% ($1,429,167)
#
# Annual Returns:
# 2010: 18.2%
# 2011: 5.4%
# 2012: 15.8%
# ...
# 2023: 24.5%
```

---

## 10. Real-Time Feature Pipeline

### Streaming Feature Computation

```bash
# Start real-time feature pipeline
ferrotick ml stream \
  --symbols AAPL,MSFT,GOOGL \
  --indicators rsi,macd,bollinger \
  --output websocket://localhost:8080/features

# Output (NDJSON stream):
{"type":"start","timestamp":"2024-01-15T09:30:00Z","symbols":["AAPL","MSFT","GOOGL"]}
{"type":"features","symbol":"AAPL","ts":"2024-01-15T09:30:00Z","close":182.50,"rsi_14":65.3,"macd_line":1.25}
{"type":"features","symbol":"MSFT","ts":"2024-01-15T09:30:00Z","close":378.25,"rsi_14":58.2,"macd_line":0.85}
{"type":"features","symbol":"GOOGL","ts":"2024-01-15T09:30:00Z","close":142.10,"rsi_14":71.5,"macd_line":2.15}
{"type":"signal","symbol":"GOOGL","action":"sell","confidence":0.72,"reason":"RSI overbought"}
```

### Integration with ML Inference

```bash
# Real-time ML inference pipeline
ferrotick ml infer \
  --model models/aapl_lstm.onnx \
  --symbols AAPL \
  --stream \
  --threshold 0.6 \
  --output signals.jsonl

# Output:
# Real-time ML Inference
# =====================
# Model: models/aapl_lstm.onnx
# Input: Live bar data (1m interval)
# Output: signals.jsonl
#
# 09:30:00 - AAPL: Buy signal (confidence: 0.72)
# 09:31:00 - AAPL: Hold (confidence: 0.52)
# 09:32:00 - AAPL: Hold (confidence: 0.48)
# 09:45:00 - AAPL: Sell signal (confidence: 0.68)
```

---

## Appendix: Common Workflows

### Workflow 1: Strategy Development Cycle

```bash
# 1. Explore data
ferrotick bars AAPL --interval 1d --limit 500 --pretty

# 2. Engineer features
ferrotick ml features AAPL --indicators rsi,macd,bollinger --store

# 3. Create strategy
ferrotick strategy create --prompt "RSI-based mean reversion"

# 4. Backtest
ferrotick backtest --strategy strategies/generated.yaml --symbols AAPL --start 2020-01-01

# 5. Optimize
ferrotick optimize --strategy strategies/generated.yaml --method bayesian --trials 100

# 6. Validate
ferrotick validate --strategy strategies/optimized.yaml --method walk-forward --windows 12

# 7. Analyze
ferrotick analyze backtest-results.json --metrics all

# 8. Deploy (paper trade)
ferrotick trade --strategy strategies/optimized.yaml --mode paper
```

### Workflow 2: ML Model Development

```bash
# 1. Prepare data
ferrotick bars SPY --interval 1d --limit 2000 --output spy_raw.parquet
ferrotick ml features SPY --input spy_raw.parquet --indicators all --output spy_features.parquet

# 2. Train model
ferrotick ml train --data spy_features.parquet --model lstm --target returns --output spy_lstm.onnx

# 3. Evaluate
ferrotick ml evaluate --model spy_lstm.onnx --metrics accuracy,precision,recall,f1

# 4. Backtest with ML
ferrotick backtest --strategy strategies/ml_predictor.yaml --model spy_lstm.onnx --symbols SPY

# 5. Compare with baseline
ferrotick analyze ml_backtest.json --compare baseline_backtest.json
```

### Workflow 3: Strategy Discovery

```bash
# 1. Discover strategies
ferrotick strategy discover --symbols SPY,QQQ --method genetic --generations 500

# 2. Filter promising strategies
ferrotick strategy filter discovered_strategies/ --min-sharpe 1.0 --max-dd 0.20

# 3. Validate discovered strategies
for strategy in discovered_strategies/*.yaml; do
  ferrotick validate --strategy "$strategy" --method walk-forward --windows 6
done

# 4. Select best strategy
ferrotick strategy rank validated_strategies/ --metric out_of_sample_sharpe

# 5. Final backtest
ferrotick backtest --strategy best_strategy.yaml --symbols SPY,QQQ,IWM --start 2015-01-01
```
