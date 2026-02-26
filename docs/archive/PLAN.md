# Task: Add Financial Statements & Earnings Data

## Objective
Implement Yahoo-backed financial statements and earnings endpoints, extend fundamentals metrics, and persist the new datasets in DuckDB with full CLI support.

## Requirements
1. Add a new CLI command `ferrotick financials <SYMBOL> --statement <income|balance|cash-flow> --period <annual|quarterly>`.
2. Add a new CLI command `ferrotick earnings <SYMBOL>`.
3. Financials output must include statement metadata (`symbol`, `statement`, `period`) and a time-series list of per-period line-item maps.
4. Financials parsing must keep all numeric line items returned by Yahoo (not only a fixed subset).
5. Financials parsing must also provide canonical alias keys for required Phase 1 metrics (income, balance sheet, cash flow categories listed in scope).
6. Earnings output must include `symbol`, `next_earnings_date`, and historical quarter entries (up to 8) with `eps_estimate`, `eps_actual`, and `surprise_percent`.
7. Extend `Fundamental` domain model with: basic/diluted shares, forward P/E, PEG, price-to-book, price-to-sales, enterprise value, EV/EBITDA, gross/operating/net margins, ROE, ROA.
8. Add new core domain types in `ferrotick-core/src/domain/` for financial statements and earnings.
9. Extend `DataSource` trait and router to support `financials` and `earnings` endpoints.
10. Implement Yahoo adapter support for: enhanced fundamentals, financials, and earnings using quoteSummary modules.
11. Non-Yahoo adapters must explicitly return `source.unsupported_endpoint` for `financials` and `earnings`.
12. Add DuckDB tables `financials` and `earnings` via a new migration version (do not modify existing migration versions).
13. Add warehouse ingestion APIs and CLI warehouse sync functions for financials and earnings.
14. New commands must work with existing global `--format json|table|ndjson` and `--stream` behavior via envelope pipeline.
15. Add/adjust JSON schemas: new response schemas for financials and earnings, and extended fundamentals schema fields.
16. Add unit tests for Yahoo parsing (including missing/null field scenarios).
17. Add integration tests (ignored by default) that call real APIs for AAPL, MSFT, GOOGL for financials and earnings.
18. Maintain compile/test health (`cargo build`, `cargo test`) with zero new warnings/errors.

## Step-by-Step Implementation
This is the most important section. Write it as a numbered checklist of concrete actions. Each step must specify:
- WHICH file to open (full path)
- WHAT to do (create, modify, delete)
- WHERE in the file (after which function, which line range, which import block)
- HOW to do it (include code snippets the implementer can copy-paste or adapt)
- WHY (brief rationale so the implementer understands intent)

### Step 1: Add New Domain Types For Financials/Earnings
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/domain/models.rs`
**Action:** modify
**Location:** After `Fundamental` definition (after current line ~184), before `CorporateActionType`
**What to do:** Add new enums/structs used by the new endpoints.
**Code:**
```rust
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FinancialStatementType {
    Income,
    Balance,
    CashFlow,
}

impl FinancialStatementType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Income => "income",
            Self::Balance => "balance",
            Self::CashFlow => "cash-flow",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinancialPeriod {
    Annual,
    Quarterly,
}

impl FinancialPeriod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Annual => "annual",
            Self::Quarterly => "quarterly",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinancialStatementEntry {
    pub as_of: UtcDateTime,
    pub line_items: BTreeMap<String, Option<f64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinancialStatementReport {
    pub symbol: Symbol,
    pub statement: FinancialStatementType,
    pub period: FinancialPeriod,
    pub entries: Vec<FinancialStatementEntry>,
}

impl FinancialStatementReport {
    pub fn new(
        symbol: Symbol,
        statement: FinancialStatementType,
        period: FinancialPeriod,
        entries: Vec<FinancialStatementEntry>,
    ) -> Self {
        Self {
            symbol,
            statement,
            period,
            entries,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarningsEntry {
    pub earnings_date: Option<UtcDateTime>,
    pub period_end: Option<UtcDateTime>,
    pub eps_estimate: Option<f64>,
    pub eps_actual: Option<f64>,
    pub surprise_percent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarningsReport {
    pub symbol: Symbol,
    pub next_earnings_date: Option<UtcDateTime>,
    pub history: Vec<EarningsEntry>,
}

impl EarningsReport {
    pub fn new(
        symbol: Symbol,
        next_earnings_date: Option<UtcDateTime>,
        history: Vec<EarningsEntry>,
    ) -> Self {
        Self {
            symbol,
            next_earnings_date,
            history,
        }
    }
}
```
**Notes:** Use `BTreeMap` (not `HashMap`) to keep deterministic key order in JSON output.

### Step 2: Extend Fundamental Model With All Required Metrics
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/domain/models.rs`
**Action:** modify
**Location:** Replace `Fundamental` struct + `impl Fundamental::new` block (around lines ~154-184)
**What to do:** Add all new optional fields and validation.
**Code:**
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fundamental {
    pub symbol: Symbol,
    pub as_of: UtcDateTime,
    pub market_cap: Option<f64>,
    pub pe_ratio: Option<f64>,
    pub dividend_yield: Option<f64>,
    pub shares_outstanding_basic: Option<f64>,
    pub shares_outstanding_diluted: Option<f64>,
    pub forward_pe: Option<f64>,
    pub peg_ratio: Option<f64>,
    pub price_to_book: Option<f64>,
    pub price_to_sales: Option<f64>,
    pub enterprise_value: Option<f64>,
    pub ev_to_ebitda: Option<f64>,
    pub gross_margin: Option<f64>,
    pub operating_margin: Option<f64>,
    pub net_margin: Option<f64>,
    pub return_on_equity: Option<f64>,
    pub return_on_assets: Option<f64>,
}

impl Fundamental {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: Symbol,
        as_of: UtcDateTime,
        market_cap: Option<f64>,
        pe_ratio: Option<f64>,
        dividend_yield: Option<f64>,
        shares_outstanding_basic: Option<f64>,
        shares_outstanding_diluted: Option<f64>,
        forward_pe: Option<f64>,
        peg_ratio: Option<f64>,
        price_to_book: Option<f64>,
        price_to_sales: Option<f64>,
        enterprise_value: Option<f64>,
        ev_to_ebitda: Option<f64>,
        gross_margin: Option<f64>,
        operating_margin: Option<f64>,
        net_margin: Option<f64>,
        return_on_equity: Option<f64>,
        return_on_assets: Option<f64>,
    ) -> Result<Self, ValidationError> {
        validate_optional_non_negative("market_cap", market_cap)?;
        validate_optional_finite("pe_ratio", pe_ratio)?;
        validate_optional_non_negative("dividend_yield", dividend_yield)?;
        validate_optional_non_negative("shares_outstanding_basic", shares_outstanding_basic)?;
        validate_optional_non_negative("shares_outstanding_diluted", shares_outstanding_diluted)?;
        validate_optional_finite("forward_pe", forward_pe)?;
        validate_optional_finite("peg_ratio", peg_ratio)?;
        validate_optional_finite("price_to_book", price_to_book)?;
        validate_optional_finite("price_to_sales", price_to_sales)?;
        validate_optional_finite("enterprise_value", enterprise_value)?;
        validate_optional_finite("ev_to_ebitda", ev_to_ebitda)?;
        validate_optional_finite("gross_margin", gross_margin)?;
        validate_optional_finite("operating_margin", operating_margin)?;
        validate_optional_finite("net_margin", net_margin)?;
        validate_optional_finite("return_on_equity", return_on_equity)?;
        validate_optional_finite("return_on_assets", return_on_assets)?;

        Ok(Self {
            symbol,
            as_of,
            market_cap,
            pe_ratio,
            dividend_yield,
            shares_outstanding_basic,
            shares_outstanding_diluted,
            forward_pe,
            peg_ratio,
            price_to_book,
            price_to_sales,
            enterprise_value,
            ev_to_ebitda,
            gross_margin,
            operating_margin,
            net_margin,
            return_on_equity,
            return_on_assets,
        })
    }
}
```
**Notes:** Keep all new fields `Option<f64>`; missing/null upstream values must not fail construction.

### Step 3: Export New Domain Types
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/domain/mod.rs`
**Action:** modify
**Location:** `pub use models::{...}` block (around lines ~61-65)
**What to do:** Re-export the new types.
**Code:**
```rust
pub use models::{
    validate_currency_code, AssetClass, Bar, BarSeries, CorporateAction, CorporateActionType,
    EarningsEntry, EarningsReport, FinancialPeriod, FinancialStatementEntry,
    FinancialStatementReport, FinancialStatementType, Fundamental, Instrument, Quote,
};
```
**Notes:** Keep alphabetical-ish ordering for readability, matching current style.

### Step 4: Extend DataSource Contract For Financials/Earnings
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/data_source.rs`
**Action:** modify
**Location:** `Endpoint`, `CapabilitySet`, request structs area, and `DataSource` trait methods
**What to do:** Add endpoint variants, capability flags, request types, and trait methods.
**Code:**
```rust
use crate::{
    BarSeries, EarningsReport, FinancialPeriod, FinancialStatementReport, FinancialStatementType,
    Fundamental, Instrument, Interval, ProviderId, Quote, Symbol,
};

pub enum Endpoint {
    Quote,
    Bars,
    Fundamentals,
    Financials,
    Earnings,
    Search,
}

pub struct CapabilitySet {
    pub quote: bool,
    pub bars: bool,
    pub fundamentals: bool,
    pub financials: bool,
    pub earnings: bool,
    pub search: bool,
}

impl CapabilitySet {
    pub const fn new(
        quote: bool,
        bars: bool,
        fundamentals: bool,
        financials: bool,
        earnings: bool,
        search: bool,
    ) -> Self { /* assign fields */ }

    pub const fn full() -> Self {
        Self::new(true, true, true, true, true, true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinancialsRequest {
    pub symbol: Symbol,
    pub statement: FinancialStatementType,
    pub period: FinancialPeriod,
}

impl FinancialsRequest {
    pub fn new(
        symbol: Symbol,
        statement: FinancialStatementType,
        period: FinancialPeriod,
    ) -> Result<Self, SourceError> {
        Ok(Self {
            symbol,
            statement,
            period,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarningsRequest {
    pub symbol: Symbol,
}

impl EarningsRequest {
    pub fn new(symbol: Symbol) -> Result<Self, SourceError> {
        Ok(Self { symbol })
    }
}

pub trait DataSource: Send + Sync {
    fn financials<'a>(
        &'a self,
        req: FinancialsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<FinancialStatementReport, SourceError>> + Send + 'a>>;

    fn earnings<'a>(
        &'a self,
        req: EarningsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<EarningsReport, SourceError>> + Send + 'a>>;
}
```
**Notes:** Also update `as_str`, `supports`, and `supported_endpoints` match arms to include the two new endpoints.

### Step 5: Re-export New Core Types
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/lib.rs`
**Action:** modify
**Location:** `pub use data_source::{...}` and `pub use domain::{...}` and warehouse re-export block
**What to do:** Re-export new request/response/enum/record types.
**Code:**
```rust
pub use data_source::{
    BarsRequest, CapabilitySet, DataSource, EarningsRequest, Endpoint, FinancialsRequest,
    FundamentalsBatch, FundamentalsRequest, HealthState, HealthStatus, QuoteBatch, QuoteRequest,
    SearchBatch, SearchRequest, SourceError, SourceErrorKind,
};

pub use domain::{
    AssetClass, Bar, BarSeries, EarningsEntry, EarningsReport, FinancialPeriod,
    FinancialStatementEntry, FinancialStatementReport, FinancialStatementType,
    CorporateAction, CorporateActionType, Fundamental, Instrument, Interval, Quote, Symbol,
    UtcDateTime,
};

pub use ferrotick_warehouse::{
    BarRecord, CacheSyncReport, EarningsRecord, FinancialRecord, FundamentalRecord,
    QueryGuardrails, QueryResult, QuoteRecord, SqlColumn, Warehouse, WarehouseConfig,
    WarehouseError,
};
```
**Notes:** Keep this in sync with new structs in `ferrotick-warehouse` (Step 14).

### Step 6: Add Routing Methods For New Endpoints
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/routing.rs`
**Action:** modify
**Location:** imports block and `impl SourceRouter` section next to `route_fundamentals`/`route_search`
**What to do:** Add `route_financials` and `route_earnings` using existing `route_endpoint` pattern.
**Code:**
```rust
use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, EarningsRequest, Endpoint, FinancialsRequest,
    FundamentalsBatch, FundamentalsRequest, HealthState, HealthStatus, QuoteBatch, QuoteRequest,
    SearchBatch, SearchRequest, SourceError,
};
use crate::{BarSeries, EarningsReport, EnvelopeError, FinancialStatementReport, ProviderId};

pub async fn route_financials(
    &self,
    req: &FinancialsRequest,
    strategy: SourceStrategy,
) -> RouteResult<FinancialStatementReport> {
    let req = req.clone();
    self.route_endpoint(Endpoint::Financials, strategy, move |source| {
        source.financials(req.clone())
    })
    .await
}

pub async fn route_earnings(
    &self,
    req: &EarningsRequest,
    strategy: SourceStrategy,
) -> RouteResult<EarningsReport> {
    let req = req.clone();
    self.route_endpoint(Endpoint::Earnings, strategy, move |source| {
        source.earnings(req.clone())
    })
    .await
}
```
**Notes:** Add one router test asserting auto-chain for `Endpoint::Financials` and `Endpoint::Earnings` includes Yahoo and excludes Alpaca.

### Step 7: Update Non-Yahoo Adapters To Explicitly Not Support New Endpoints
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/adapters/alpaca.rs`
**Action:** modify
**Location:** `use crate::data_source::{...}` import list and `impl DataSource for AlpacaAdapter`
**What to do:** Add `financials` and `earnings` trait methods returning unsupported endpoint.
**Code:**
```rust
fn capabilities(&self) -> CapabilitySet {
    CapabilitySet::new(true, true, false, false, false, false)
}

fn financials<'a>(
    &'a self,
    req: FinancialsRequest,
) -> Pin<Box<dyn Future<Output = Result<FinancialStatementReport, SourceError>> + Send + 'a>> {
    Box::pin(async move {
        let _ = req;
        Err(SourceError::unsupported_endpoint(Endpoint::Financials))
    })
}

fn earnings<'a>(
    &'a self,
    req: EarningsRequest,
) -> Pin<Box<dyn Future<Output = Result<EarningsReport, SourceError>> + Send + 'a>> {
    Box::pin(async move {
        let _ = req;
        Err(SourceError::unsupported_endpoint(Endpoint::Earnings))
    })
}
```
**Notes:** Mirror this same pattern in Steps 8-9 for Polygon and Alpha Vantage.

### Step 8: Update Polygon Adapter Capabilities + Unsupported Methods
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/adapters/polygon.rs`
**Action:** modify
**Location:** imports block and `impl DataSource for PolygonAdapter`
**What to do:** Mark financials/earnings unsupported and add trait methods.
**Code:**
```rust
fn capabilities(&self) -> CapabilitySet {
    CapabilitySet::new(true, true, true, false, false, true)
}

fn financials<'a>(...) -> Pin<Box<dyn Future<Output = Result<FinancialStatementReport, SourceError>> + Send + 'a>> {
    Box::pin(async move {
        let _ = req;
        Err(SourceError::unsupported_endpoint(Endpoint::Financials))
    })
}

fn earnings<'a>(...) -> Pin<Box<dyn Future<Output = Result<EarningsReport, SourceError>> + Send + 'a>> {
    Box::pin(async move {
        let _ = req;
        Err(SourceError::unsupported_endpoint(Endpoint::Earnings))
    })
}
```
**Notes:** Do not change existing quote/bars/fundamentals/search behavior.

### Step 9: Update Alpha Vantage Adapter Capabilities + Unsupported Methods
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/adapters/alphavantage.rs`
**Action:** modify
**Location:** imports block and `impl DataSource for AlphaVantageAdapter`
**What to do:** Same as Step 8.
**Code:**
```rust
fn capabilities(&self) -> CapabilitySet {
    CapabilitySet::new(true, true, true, false, false, true)
}

fn financials<'a>(...) -> Pin<Box<dyn Future<Output = Result<FinancialStatementReport, SourceError>> + Send + 'a>> {
    Box::pin(async move {
        let _ = req;
        Err(SourceError::unsupported_endpoint(Endpoint::Financials))
    })
}

fn earnings<'a>(...) -> Pin<Box<dyn Future<Output = Result<EarningsReport, SourceError>> + Send + 'a>> {
    Box::pin(async move {
        let _ = req;
        Err(SourceError::unsupported_endpoint(Endpoint::Earnings))
    })
}
```
**Notes:** Add `Endpoint` import in this file (it currently does not import it).

### Step 10: Enhance Yahoo Fundamentals Parsing
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/adapters/yahoo.rs`
**Action:** modify
**Location:** `fetch_real_fundamentals` (~634+) and Yahoo quote-summary response structs (~955+)
**What to do:**
1. Update fundamentals module request to include `financialData`.
2. Add fields to quote-summary structs for all new metrics.
3. Build `Fundamental::new(...)` with all extended fields.
4. Rename `execute_fundamentals_request` to `execute_quote_summary_request` and use it for all quoteSummary calls.
**Code:**
```rust
let endpoint = format!(
    "https://query2.finance.yahoo.com/v10/finance/quoteSummary/{}?modules=price,summaryDetail,defaultKeyStatistics,financialData&crumb={}",
    urlencoding::encode(symbol.as_str()),
    urlencoding::encode(&crumb)
);

let shares_outstanding_basic = result
    .default_key_statistics
    .as_ref()
    .and_then(|s| s.shares_outstanding.as_ref().and_then(|v| v.to_option()));
let shares_outstanding_diluted = result
    .default_key_statistics
    .as_ref()
    .and_then(|s| s.implied_shares_outstanding.as_ref().and_then(|v| v.to_option()))
    .or(shares_outstanding_basic);

let fundamental = Fundamental::new(
    symbol.clone(),
    as_of,
    market_cap,
    pe_ratio,
    dividend_yield,
    shares_outstanding_basic,
    shares_outstanding_diluted,
    forward_pe,
    peg_ratio,
    price_to_book,
    price_to_sales,
    enterprise_value,
    ev_to_ebitda,
    gross_margin,
    operating_margin,
    net_margin,
    return_on_equity,
    return_on_assets,
)?;
```
**Notes:** Change `YahooRawValue::to_option()` to keep zero values (`0.0`) and only filter non-finite values.

### Step 11: Implement Yahoo Financials Endpoint
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/adapters/yahoo.rs`
**Action:** modify
**Location:** `impl DataSource for YahooAdapter` and real-methods section after `fetch_real_fundamentals`
**What to do:** Add `financials` trait method, request builder, response parser, alias mapping, and free-cash-flow calculation.
**Code:**
```rust
fn financials<'a>(
    &'a self,
    req: FinancialsRequest,
) -> Pin<Box<dyn Future<Output = Result<FinancialStatementReport, SourceError>> + Send + 'a>> {
    Box::pin(async move { self.fetch_real_financials(&req).await })
}

async fn fetch_real_financials(
    &self,
    req: &FinancialsRequest,
) -> Result<FinancialStatementReport, SourceError> {
    let crumb = self.auth_manager.get_crumb(&self.http_client).await?;
    let module = yahoo_financial_module(req.statement, req.period);
    let endpoint = format!(
        "https://query2.finance.yahoo.com/v10/finance/quoteSummary/{}?modules={}&crumb={}",
        urlencoding::encode(req.symbol.as_str()),
        module,
        urlencoding::encode(&crumb)
    );

    let response = self.execute_quote_summary_request(&endpoint).await?;
    self.parse_financials_response(&response.body, req)
}
```
Add helper decisions:
1. `yahoo_financial_module` mapping:
`income+annual -> incomeStatementHistory`
`income+quarterly -> incomeStatementHistoryQuarterly`
`balance+annual -> balanceSheetHistory`
`balance+quarterly -> balanceSheetHistoryQuarterly`
`cash-flow+annual -> cashflowStatementHistory`
`cash-flow+quarterly -> cashflowStatementHistoryQuarterly`
2. `line_items` key normalization: camelCase to snake_case.
3. Add canonical aliases (must always be attempted):
`revenue`, `cost_of_revenue`, `gross_profit`, `operating_income`, `net_income`, `eps_basic`, `eps_diluted`, `r_and_d_expenses`, `sg_and_a_expenses`, `total_assets`, `total_liabilities`, `total_equity`, `current_assets`, `current_liabilities`, `cash_and_equivalents`, `total_debt`, `operating_cash_flow`, `investing_cash_flow`, `financing_cash_flow`, `capital_expenditures`, `free_cash_flow`, `dividends_paid`, `stock_buybacks`.
4. Free cash flow formula (when both values exist):
`free_cash_flow = operating_cash_flow - capital_expenditures.abs()`.
**Notes:** If module/result is present but statement array is missing/empty, return `Ok(FinancialStatementReport { entries: vec![] })` (no hard error).

### Step 12: Implement Yahoo Earnings Endpoint
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/adapters/yahoo.rs`
**Action:** modify
**Location:** `impl DataSource for YahooAdapter` and real-methods section (after financials methods)
**What to do:** Add `earnings` trait method and parser based on quoteSummary modules `earningsTrend,earnings`.
**Code:**
```rust
fn earnings<'a>(
    &'a self,
    req: EarningsRequest,
) -> Pin<Box<dyn Future<Output = Result<EarningsReport, SourceError>> + Send + 'a>> {
    Box::pin(async move { self.fetch_real_earnings(&req).await })
}

async fn fetch_real_earnings(&self, req: &EarningsRequest) -> Result<EarningsReport, SourceError> {
    let crumb = self.auth_manager.get_crumb(&self.http_client).await?;
    let endpoint = format!(
        "https://query2.finance.yahoo.com/v10/finance/quoteSummary/{}?modules=earningsTrend,earnings&crumb={}",
        urlencoding::encode(req.symbol.as_str()),
        urlencoding::encode(&crumb)
    );
    let response = self.execute_quote_summary_request(&endpoint).await?;
    self.parse_earnings_response(&response.body, &req.symbol)
}
```
Parser rules (hard requirements):
1. Use `earningsTrend.trend` entries as primary history source.
2. Keep entries with at least one of `eps_estimate` or `eps_actual`.
3. Sort by `period_end` descending and take at most 8 entries.
4. `surprise_percent` source priority:
`surprisePercent.raw` -> computed `((actual-estimate)/abs(estimate))*100` when estimate != 0.
5. `next_earnings_date` = earliest future `period_end` without `eps_actual`; fallback = first history entry date.
6. Missing/null fields must remain `None`, not parsing errors.
**Notes:** If trend is missing, return `EarningsReport { history: vec![] }`.

### Step 13: Add Yahoo Parsing Unit Tests (Including Missing Data)
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/adapters/yahoo.rs`
**Action:** modify
**Location:** `#[cfg(test)] mod tests` near end of file
**What to do:** Add deterministic parsing tests with inline JSON fixtures.
**Code:**
```rust
#[test]
fn parse_financials_extracts_required_aliases_and_fcf() {
    // build JSON fixture with operatingCashflow + capitalExpenditures
    // assert line_items has operating_cash_flow, capital_expenditures, free_cash_flow
}

#[test]
fn parse_earnings_computes_surprise_when_missing() {
    // fixture has epsEstimate + epsActual but no surprisePercent
    // assert computed surprise_percent matches formula
}

#[test]
fn parse_fundamentals_handles_missing_null_fields() {
    // fixture intentionally omits several fields
    // assert parse returns Ok with None fields and no panic
}
```
**Notes:** Keep these as unit tests, not network tests.

### Step 14: Add CLI Args For New Commands
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/cli.rs`
**Action:** modify
**Location:** `Command` enum and args definitions (around lines ~150-280)
**What to do:** Add `Financials` and `Earnings` command variants and strongly-typed value enums.
**Code:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FinancialStatementArg {
    Income,
    Balance,
    #[value(name = "cash-flow")]
    CashFlow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FinancialPeriodArg {
    Annual,
    Quarterly,
}

#[derive(Debug, Args)]
pub struct FinancialsArgs {
    pub symbol: String,
    #[arg(long, value_enum, required = true)]
    pub statement: FinancialStatementArg,
    #[arg(long, value_enum, required = true)]
    pub period: FinancialPeriodArg,
}

#[derive(Debug, Args)]
pub struct EarningsArgs {
    pub symbol: String,
}
```
And add command variants:
```rust
Financials(FinancialsArgs),
Earnings(EarningsArgs),
```
**Notes:** Keep both `--statement` and `--period` required (no defaults).

### Step 15: Add New CLI Command Handlers
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/financials.rs`
**Action:** create
**Location:** new file
**What to do:** Implement `run()` mirroring `fundamentals.rs` pattern and sync to warehouse.
**Code:**
```rust
use serde::Serialize;
use ferrotick_core::{
    FinancialPeriod, FinancialStatementReport, FinancialStatementType, FinancialsRequest,
    SourceRouter, SourceStrategy, Symbol,
};

use crate::cli::{FinancialPeriodArg, FinancialStatementArg, FinancialsArgs};
use crate::error::CliError;
use super::{warehouse_sync, CommandResult};

#[derive(Debug, Serialize)]
struct FinancialsResponseData {
    financials: FinancialStatementReport,
}

fn map_statement(arg: FinancialStatementArg) -> FinancialStatementType { /* explicit match */ }
fn map_period(arg: FinancialPeriodArg) -> FinancialPeriod { /* explicit match */ }

pub async fn run(...) -> Result<CommandResult, CliError> { /* same route/success/failure pattern */ }
```
**Notes:** On route failure, return `FinancialStatementReport::new(symbol, statement, period, Vec::new())` and attach failure errors/warnings.

### Step 16: Add Earnings CLI Command Handler
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/earnings.rs`
**Action:** create
**Location:** new file
**What to do:** Implement `run()` for `earnings` command.
**Code:**
```rust
use serde::Serialize;
use ferrotick_core::{EarningsReport, EarningsRequest, SourceRouter, SourceStrategy, Symbol};

use crate::cli::EarningsArgs;
use crate::error::CliError;
use super::{warehouse_sync, CommandResult};

#[derive(Debug, Serialize)]
struct EarningsResponseData {
    earnings: EarningsReport,
}

pub async fn run(...) -> Result<CommandResult, CliError> { /* same pattern as fundamentals */ }
```
**Notes:** On failure, return `EarningsReport::new(symbol, None, Vec::new())`.

### Step 17: Wire New CLI Commands Into Dispatch
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/mod.rs`
**Action:** modify
**Location:** module declarations at top and `match &cli.command` block
**What to do:** Add module imports + dispatch arms.
**Code:**
```rust
mod earnings;
mod financials;

// in run() match:
Command::Financials(args) => financials::run(args, &router, &strategy).await?,
Command::Earnings(args) => earnings::run(args, &router, &strategy).await?,
```
**Notes:** Do not alter global envelope/rendering flow; this preserves `--format` and `--stream` automatically.

### Step 18: Extend Warehouse Sync Functions
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/warehouse_sync.rs`
**Action:** modify
**Location:** imports and after `sync_fundamentals`
**What to do:**
1. Extend `sync_fundamentals` to emit all new fundamental metrics.
2. Add `sync_financials` and `sync_earnings`.
**Code:**
```rust
use ferrotick_core::{
    Bar, BarRecord, EarningsRecord, EarningsReport, FinancialRecord, FinancialStatementReport,
    Fundamental, FundamentalRecord, Interval, ProviderId, Quote, QuoteRecord, Warehouse,
    WarehouseError,
};

pub fn sync_financials(...) -> Result<(), WarehouseError> { /* map entries->FinancialRecord */ }
pub fn sync_earnings(...) -> Result<(), WarehouseError> { /* map history->EarningsRecord */ }
```
Fundamentals metric list (exact):
`market_cap`, `pe_ratio`, `dividend_yield`, `shares_outstanding_basic`, `shares_outstanding_diluted`, `forward_pe`, `peg_ratio`, `price_to_book`, `price_to_sales`, `enterprise_value`, `ev_to_ebitda`, `gross_margin`, `operating_margin`, `net_margin`, `return_on_equity`, `return_on_assets`.
**Notes:** Skip `None` metrics; do not insert null metric rows.

### Step 19: Add New Warehouse Migration For `financials` and `earnings`
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-warehouse/src/migrations.rs`
**Action:** modify
**Location:** append new `Migration` entry after `0002_indexes`
**What to do:** Add `0003_financials_earnings` migration.
**Code:**
```sql
CREATE TABLE IF NOT EXISTS financials (
    symbol TEXT NOT NULL,
    statement_type TEXT NOT NULL,
    period_type TEXT NOT NULL,
    report_date TIMESTAMP NOT NULL,
    line_item TEXT NOT NULL,
    value DOUBLE NOT NULL,
    source TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(symbol, statement_type, period_type, report_date, line_item)
);

CREATE TABLE IF NOT EXISTS earnings (
    symbol TEXT NOT NULL,
    period_end TIMESTAMP NOT NULL,
    earnings_date TIMESTAMP,
    eps_estimate DOUBLE,
    eps_actual DOUBLE,
    surprise_pct DOUBLE,
    source TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(symbol, period_end)
);

CREATE INDEX IF NOT EXISTS idx_financials_symbol_stmt_period_date
    ON financials(symbol, statement_type, period_type, report_date);
CREATE INDEX IF NOT EXISTS idx_earnings_symbol_period_end
    ON earnings(symbol, period_end);
```
**Notes:** Do not edit existing migration versions or SQL in-place.

### Step 20: Add Warehouse Record Types And Ingestion Methods
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-warehouse/src/lib.rs`
**Action:** modify
**Location:** record struct section (~225+) and ingest methods section (~390+)
**What to do:** Add `FinancialRecord` + `EarningsRecord`, plus `ingest_financials` and `ingest_earnings`.
**Code:**
```rust
#[derive(Debug, Clone)]
pub struct FinancialRecord {
    pub symbol: String,
    pub statement_type: String,
    pub period_type: String,
    pub report_date: String,
    pub line_item: String,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct EarningsRecord {
    pub symbol: String,
    pub period_end: String,
    pub earnings_date: Option<String>,
    pub eps_estimate: Option<f64>,
    pub eps_actual: Option<f64>,
    pub surprise_pct: Option<f64>,
}

pub fn ingest_financials(&self, source: &str, request_id: &str, rows: &[FinancialRecord], latency_ms: u64) -> Result<(), WarehouseError> {
    // BEGIN TRANSACTION
    // INSERT OR REPLACE INTO financials ... parameterized
    // INSERT INTO ingest_log dataset='financials'
    // finalize_transaction
}

pub fn ingest_earnings(&self, source: &str, request_id: &str, rows: &[EarningsRecord], latency_ms: u64) -> Result<(), WarehouseError> {
    // BEGIN TRANSACTION
    // INSERT OR REPLACE INTO earnings ... parameterized
    // INSERT INTO ingest_log dataset='earnings'
    // finalize_transaction
}
```
**Notes:** Follow exact parameterized-query style used by `ingest_fundamentals`.

### Step 21: Update Core Re-exports For New Warehouse Records
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/lib.rs`
**Action:** modify
**Location:** `pub use ferrotick_warehouse::{...}` block (~159+)
**What to do:** Add `FinancialRecord` and `EarningsRecord`.
**Code:**
```rust
pub use ferrotick_warehouse::{
    BarRecord, CacheSyncReport, EarningsRecord, FinancialRecord, FundamentalRecord,
    QueryGuardrails, QueryResult, QuoteRecord, SqlColumn, Warehouse, WarehouseConfig,
    WarehouseError,
};
```
**Notes:** Keep this synchronized with Step 20.

### Step 22: Add Financials/Earnings Response Schemas And Extend Fundamentals Schema
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/schemas/v1/fundamentals.response.schema.json`
**Action:** modify
**Location:** `properties` under fundamentals item
**What to do:** Add optional schema fields for all new fundamentals metrics.
**Code:**
```json
"shares_outstanding_basic": { "type": ["number", "string", "null"] },
"shares_outstanding_diluted": { "type": ["number", "string", "null"] },
"forward_pe": { "type": ["number", "string", "null"] },
"peg_ratio": { "type": ["number", "string", "null"] },
"price_to_book": { "type": ["number", "string", "null"] },
"price_to_sales": { "type": ["number", "string", "null"] },
"enterprise_value": { "type": ["number", "string", "null"] },
"ev_to_ebitda": { "type": ["number", "string", "null"] },
"gross_margin": { "type": ["number", "string", "null"] },
"operating_margin": { "type": ["number", "string", "null"] },
"net_margin": { "type": ["number", "string", "null"] },
"return_on_equity": { "type": ["number", "string", "null"] },
"return_on_assets": { "type": ["number", "string", "null"] }
```
**Notes:** Keep `additionalProperties: false` unchanged.

**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/schemas/v1/financials.response.schema.json`
**Action:** create
**Location:** new file
**What to do:** Add envelope+financials schema.

**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/schemas/v1/earnings.response.schema.json`
**Action:** create
**Location:** new file
**What to do:** Add envelope+earnings schema.

### Step 23: Register New Schema Aliases
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/schema.rs`
**Action:** modify
**Location:** `resolve_schema_file_name` match (~75+)
**What to do:** map short names to new files.
**Code:**
```rust
"financials" => String::from("financials.response.schema.json"),
"earnings" => String::from("earnings.response.schema.json"),
```
**Notes:** No other behavior changes.

### Step 24: Add Real-API Integration Tests For New Endpoints
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/tests/cli_user_journeys.rs`
**Action:** modify
**Location:** add new ignored tests near other real-API journey tests
**What to do:** Add two tests: financials and earnings for AAPL/MSFT/GOOGL.
**Code:**
```rust
#[tokio::test]
#[ignore = "Requires real API data"]
async fn user_can_fetch_quarterly_income_statement() {
    // SourceRouterBuilder::new().with_real_clients().build()
    // financials request for AAPL quarterly income
    // assert entries not empty and required keys exist in first entry line_items
}

#[tokio::test]
#[ignore = "Requires real API data"]
async fn user_can_fetch_recent_earnings_history() {
    // earnings request for MSFT
    // assert history len in 1..=8 and eps_estimate/eps_actual fields parse
}
```
Add one multi-symbol loop test for `AAPL`, `MSFT`, `GOOGL`.
**Notes:** Keep ignored to avoid CI failures in offline environments.

### Step 25: Add Missing-Data And Capability Regression Tests
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/tests/data_provider_behavior.rs`
**Action:** modify
**Location:** endpoint support section near lines ~473+
**What to do:**
1. Add capability assertions for `Endpoint::Financials` and `Endpoint::Earnings`.
2. Add unsupported-endpoint assertions for Alpaca/Polygon/AlphaVantage on new endpoints.
**Code:**
```rust
assert!(yahoo.capabilities().supports(Endpoint::Financials));
assert!(yahoo.capabilities().supports(Endpoint::Earnings));
assert!(!alpaca.capabilities().supports(Endpoint::Financials));
assert!(!alpaca.capabilities().supports(Endpoint::Earnings));
```
**Notes:** Ensure imports include `FinancialsRequest`, `EarningsRequest`, `FinancialStatementType`, `FinancialPeriod`.

### Step 26: Add Warehouse Behavior Tests For New Tables
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/tests/warehouse_behavior.rs`
**Action:** modify
**Location:** ingestion behavior section near existing fundamentals ingestion test (~120+)
**What to do:** Add two tests:
1. `when_user_ingests_financials_they_are_queryable_by_statement_and_line_item`
2. `when_user_ingests_earnings_they_are_queryable_by_period_end`
**Code:**
```rust
let financial_rows = vec![FinancialRecord { ... }];
warehouse.ingest_financials("yahoo", "req-010", &financial_rows, 90)?;

let earnings_rows = vec![EarningsRecord { ... }];
warehouse.ingest_earnings("yahoo", "req-011", &earnings_rows, 80)?;
```
**Notes:** Query using `SELECT ... FROM financials` and `SELECT ... FROM earnings` and assert row counts.

### Step 27: Run Formatting, Build, And Tests
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick`
**Action:** modify (verification only)
**Location:** repository root
**What to do:** Execute all verification commands in this order.
**Code:**
```bash
cargo fmt
cargo build
cargo test
cargo run --bin ferrotick -- financials AAPL --statement income --period annual
cargo run --bin ferrotick -- earnings AAPL
cargo run --bin ferrotick -- fundamentals AAPL
```
**Notes:** If real API tests are ignored, run targeted parsing tests explicitly to verify new logic:
`cargo test -p ferrotick-core yahoo::tests -- --nocapture`.

## Existing Patterns to Follow
Use these exact patterns from the current codebase.

1. Command execution pattern with route success/failure handling:
**Path:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-cli/src/commands/fundamentals.rs`
```rust
match router.route_fundamentals(&request, strategy.clone()).await {
    Ok(route) => {
        let fundamentals = route.data.fundamentals;
        let warehouse_warning = warehouse_sync::sync_fundamentals(
            route.selected_source,
            fundamentals.as_slice(),
            route.latency_ms,
        )
        .err()
        .map(|error| format!("warehouse sync (fundamentals) failed: {error}"));
```

2. Router endpoint wrapper pattern:
**Path:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/routing.rs`
```rust
pub async fn route_fundamentals(
    &self,
    req: &FundamentalsRequest,
    strategy: SourceStrategy,
) -> RouteResult<FundamentalsBatch> {
    let req = req.clone();
    self.route_endpoint(Endpoint::Fundamentals, strategy, move |source| {
        source.fundamentals(req.clone())
    })
    .await
}
```

3. Unsupported endpoint behavior pattern:
**Path:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/adapters/alpaca.rs`
```rust
fn fundamentals<'a>(
    &'a self,
    req: FundamentalsRequest,
) -> Pin<Box<dyn Future<Output = Result<FundamentalsBatch, SourceError>> + Send + 'a>> {
    Box::pin(async move {
        let _ = req;
        Err(SourceError::unsupported_endpoint(Endpoint::Fundamentals))
    })
}
```

4. Warehouse parameterized insert pattern:
**Path:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-warehouse/src/lib.rs`
```rust
let params: [&dyn ToSql; 5] =
    [&row.symbol, &row.metric, &row.value, &row.date, &source];
connection.execute(
    "INSERT OR REPLACE INTO fundamentals \
     (symbol, metric, value, date, source, updated_at) \
     VALUES (?, ?, ?, TRY_CAST(? AS TIMESTAMP), ?, CURRENT_TIMESTAMP)",
    params.as_slice(),
)?;
```

5. Yahoo quoteSummary parsing style:
**Path:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-core/src/adapters/yahoo.rs`
```rust
let summary_response: YahooQuoteSummaryResponse = serde_json::from_str(&response.body)
    .map_err(|e| SourceError::internal(format!("failed to parse fundamentals: {}", e)))?;

if let Some(error) = &summary_response.quote_summary.error {
    if !error.is_empty() {
        return Err(SourceError::unavailable(format!(
            "yahoo fundamentals API error: {}",
            error
        )));
    }
}
```

## Edge Cases and Error Handling
For each edge case, specify the exact behavior:
1. Empty/missing quoteSummary `result`: return success with empty `entries`/`history` for financials/earnings; do not panic.
2. `quoteSummary.error` present and non-empty: return `SourceError::unavailable`.
3. Missing `raw` numeric wrappers: map to `None`; do not fail request.
4. Non-finite numbers (`NaN`, `inf`): drop to `None` before constructing domain models.
5. Earnings surprise percent missing: compute only when both EPS values exist and estimate is non-zero; else keep `None`.
6. Financial statement has no cash-flow CAPEX/OCF pair: omit `free_cash_flow` alias (set to `None` only if alias key already exists).
7. Non-Yahoo adapters called for new endpoints: must return `source.unsupported_endpoint` immediately.
8. Warehouse sync with empty vectors: return `Ok(())` without touching DB.
9. Financials line item keys must be deterministic (snake_case + stable map order via `BTreeMap`).
10. Timestamps from Yahoo Unix epoch values must be converted to UTC via `UtcDateTime::from_offset_datetime`; conversion failures become `SourceError::internal`.

## Dependencies and Imports
1. No new Cargo dependencies are required.
2. New std imports:
`std::collections::BTreeMap` in `domain/models.rs` and Yahoo parser helpers.
3. New ferrotick-core imports in CLI command modules:
`FinancialsRequest`, `EarningsRequest`, `FinancialStatementType`, `FinancialPeriod`, `FinancialStatementReport`, `EarningsReport`.
4. New ferrotick-warehouse imports where needed:
`FinancialRecord`, `EarningsRecord`.
5. Update adapter import lists to include:
`FinancialsRequest`, `EarningsRequest`, `FinancialStatementReport`, `EarningsReport`, and `Endpoint` where unsupported methods are added.

## Acceptance Criteria
- [ ] `cargo test` passes with 0 failures
- [ ] `cargo build` compiles without errors
- [ ] `cargo run --bin ferrotick -- financials AAPL --statement income --period annual` works
- [ ] `cargo run --bin ferrotick -- financials AAPL --statement balance --period quarterly` returns JSON with non-empty `data.financials.entries` (when Yahoo has data)
- [ ] `cargo run --bin ferrotick -- earnings AAPL` returns JSON with `data.earnings.history` length in `0..=8`
- [ ] `cargo run --bin ferrotick -- fundamentals AAPL` includes new fields (e.g., `forward_pe`, `peg_ratio`, `return_on_equity`) in output schema
- [ ] DuckDB has tables `financials` and `earnings` after startup migration
- [ ] Warehouse sync writes rows for new commands (verify with SQL count queries)
- [ ] Real API ignored tests exist for AAPL/MSFT/GOOGL financials+earnings paths
- [ ] Missing/null Yahoo fields are covered by unit tests and do not cause panics

## Out of Scope
1. Adding financials/earnings support for Polygon, Alpha Vantage, or Alpaca.
2. Adding new SQL analytical views for the `financials`/`earnings` tables.
3. Historical pagination beyond the latest 8 earnings entries.
4. Any breaking changes to existing envelope metadata format.
5. UI/UX table pretty-print customization beyond current generic table renderer.
6. Export-command enhancements for new tables (can be handled in a later phase).
