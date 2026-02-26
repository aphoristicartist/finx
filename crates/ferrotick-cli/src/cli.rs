//! CLI argument definitions for Ferrotick.
//!
//! This module contains the command-line interface structure using Clap.
//! The CLI supports multiple commands for fetching market data, querying
//! the local warehouse, and managing configuration.
//!
//! # Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `quote` | Fetch latest quotes for symbols |
//! | `bars` | Fetch historical OHLCV bars |
//! | `fundamentals` | Fetch company fundamentals |
//! | `search` | Search for instruments |
//! | `sql` | Query the local DuckDB warehouse |
//! | `cache` | Manage local cache |
//! | `schema` | Inspect bundled JSON schemas |
//! | `sources` | List data source capabilities |
//!
//! # Global Options
//!
//! | Option | Default | Description |
//! |--------|---------|-------------|
//! | `--format` | `json` | Output format (json, ndjson, table) |
//! | `--pretty` | `false` | Pretty-print JSON output |
//! | `--strict` | `false` | Treat warnings as errors |
//! | `--source` | `auto` | Source selection strategy |
//! | `--timeout-ms` | `3000` | Request timeout in ms |
//! | `--stream` | `false` | Enable NDJSON streaming |
//!
//! # Examples
//!
//! ```bash
//! # Fetch a quote
//! ferrotick quote AAPL
//!
//! # Get daily bars with JSON output
//! ferrotick bars AAPL --interval 1d --limit 30 --pretty
//!
//! # Query the warehouse
//! ferrotick sql "SELECT * FROM bars_1d WHERE symbol='AAPL'"
//!
//! # Use strict mode for CI/CD
//! ferrotick quote AAPL --strict
//! ```

use clap::{Args, Parser, Subcommand, ValueEnum};

/// 🦀 Ferrotick - Provider-neutral financial data CLI
///
/// Fetch market data from multiple providers (Polygon, Yahoo, Alpha Vantage, Alpaca)
/// with unified output, local caching, and analytics via DuckDB.
///
/// For more information, see: <https://github.com/ferrotick/ferrotick>
#[derive(Debug, Parser)]
#[command(
    name = "ferrotick",
    author,
    version,
    about = "Provider-neutral financial data CLI",
    long_about = "Ferrotick is a high-performance financial data CLI that provides unified access \
to multiple market data providers. Features include:\n\
\n\
  • Multi-provider support (Polygon, Yahoo, Alpha Vantage, Alpaca)\n\
  • Local DuckDB warehouse for analytics\n\
  • Secure parameterized SQL queries\n\
  • AI-agent streaming mode\n\
  • Structured JSON output with metadata\n\
\n\
Use 'ferrotick <command> --help' for command-specific help."
)]
pub struct Cli {
    /// Output format for results.
    ///
    /// - json: Single JSON object (default)
    /// - ndjson: One JSON object per line
    /// - table: ASCII table format
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,

    /// Pretty-print JSON output with indentation.
    #[arg(long, global = true, default_value_t = false)]
    pub pretty: bool,

    /// Treat warnings and errors as failures (exit code 5).
    ///
    /// Useful for CI/CD pipelines that need strict validation.
    #[arg(long, global = true, default_value_t = false)]
    pub strict: bool,

    /// Source selection strategy for routing requests.
    #[arg(long, global = true, value_enum, default_value_t = SourceSelector::Auto)]
    pub source: SourceSelector,

    /// Request timeout budget in milliseconds.
    #[arg(long, global = true, default_value_t = 3000)]
    pub timeout_ms: u64,

    /// Enable profiling metadata in output (placeholder).
    #[arg(long, global = true, default_value_t = false)]
    pub profile: bool,

    /// Enable NDJSON streaming mode for AI agents.
    ///
    /// Outputs events as newline-delimited JSON:
    /// - start: Operation initiated
    /// - progress: Status updates
    /// - chunk: Data batches
    /// - end: Operation completed
    /// - error: Error occurred
    #[arg(long, global = true, default_value_t = false)]
    pub stream: bool,

    /// Show query plan diagnostics.
    #[arg(long, global = true, default_value_t = false)]
    pub explain: bool,

    #[command(subcommand)]
    pub command: Command,
}

/// Output format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// ASCII table format for terminal display.
    Table,
    /// Single JSON object output.
    Json,
    /// Newline-delimited JSON (one object per line).
    Ndjson,
}

/// Source selection strategy.
///
/// Controls which provider(s) handle requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SourceSelector {
    /// Automatic selection with priority scoring and fallback.
    Auto,
    /// Use Yahoo Finance directly.
    Yahoo,
    /// Use Polygon.io directly.
    Polygon,
    /// Use Alpha Vantage directly.
    Alphavantage,
    /// Use Alpaca directly.
    Alpaca,
}

/// Available CLI commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// 💰 Fetch latest quote(s) for one or more symbols.
    ///
    /// Returns real-time or delayed quotes including price, bid, ask,
    /// volume, and timestamp.
    ///
    /// # Examples
    ///
    ///   ferrotick quote AAPL
    ///   ferrotick quote AAPL MSFT GOOGL --pretty
    ///   ferrotick quote AAPL --source polygon
    Quote(QuoteArgs),

    /// 📊 Fetch historical OHLCV bars.
    ///
    /// Returns open, high, low, close, volume data for the specified
    /// interval and limit.
    ///
    /// # Examples
    ///
    ///   ferrotick bars AAPL
    ///   ferrotick bars AAPL --interval 5m --limit 100
    ///   ferrotick bars GOOGL --interval 1h --limit 48
    Bars(BarsArgs),

    /// 📈 Fetch company fundamentals snapshot(s).
    ///
    /// Returns fundamental data including market cap, P/E ratio,
    /// dividend yield, etc.
    ///
    /// # Examples
    ///
    ///   ferrotick fundamentals AAPL
    ///   ferrotick fundamentals AAPL MSFT --pretty
    Fundamentals(FundamentalsArgs),

    /// 🔍 Search for instruments.
    ///
    /// Search by symbol or company name to find matching instruments.
    ///
    /// # Examples
    ///
    ///   ferrotick search apple
    ///   ferrotick search microsoft --limit 5
    Search(SearchArgs),

    /// 📊 Fetch financial statements.
    ///
    /// Returns income statement, balance sheet, or cash flow data
    /// for a symbol.
    ///
    /// # Examples
    ///
    ///   ferrotick financials AAPL --statement income --period annual
    ///   ferrotick financials MSFT --statement balance --period quarterly
    Financials(FinancialsArgs),

    /// 💵 Fetch earnings data.
    ///
    /// Returns quarterly earnings history including EPS actual vs estimate.
    ///
    /// # Examples
    ///
    ///   ferrotick earnings AAPL
    ///   ferrotick earnings MSFT --limit 8
    Earnings(EarningsArgs),

    /// 🗄️ Run SQL queries against the DuckDB warehouse.
    ///
    /// Execute SQL queries against the local warehouse database.
    /// Default mode is read-only; use --write for data modifications.
    ///
    /// # Security
    ///
    /// All queries are executed with guardrails:
    /// - Row limits (default: 10,000)
    /// - Query timeout (default: 5,000ms)
    /// - Read-only by default
    ///
    /// # Examples
    ///
    ///   ferrotick sql "SELECT * FROM bars_1d WHERE symbol='AAPL' LIMIT 10"
    ///   ferrotick sql "SELECT COUNT(*) FROM bars_1d"
    Sql(SqlArgs),

    /// 📤 Export data from warehouse to Parquet or CSV.
    ///
    /// # Examples
    ///
    ///   ferrotick export --output data.csv --format csv --table bars --symbol AAPL
    ///   ferrotick export --output data.parquet --format parquet --query "SELECT * FROM bars_1d"
    Export(ExportArgs),

    /// 📦 Cache management commands.
    Cache(CacheArgs),

    /// 📋 Inspect bundled JSON schemas.
    Schema(SchemaArgs),

    /// 🔌 List data source capability matrix.
    Sources(SourcesArgs),
}

/// Arguments for the `quote` command.
#[derive(Debug, Args)]
pub struct QuoteArgs {
    /// One or more market symbols (e.g., AAPL, MSFT, GOOGL).
    #[arg(required = true, num_args = 1..)]
    pub symbols: Vec<String>,
}

/// Arguments for the `bars` command.
#[derive(Debug, Args)]
pub struct BarsArgs {
    /// Market symbol to fetch bars for.
    pub symbol: String,

    /// Bar interval.
    ///
    /// Supported intervals:
    /// - 1m: 1 minute
    /// - 5m: 5 minutes
    /// - 15m: 15 minutes
    /// - 1h: 1 hour
    /// - 1d: 1 day (default)
    #[arg(long, default_value = "1d")]
    pub interval: String,

    /// Number of bars to return (default: 10).
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
}

/// Arguments for the `fundamentals` command.
#[derive(Debug, Args)]
pub struct FundamentalsArgs {
    /// One or more market symbols.
    #[arg(required = true, num_args = 1..)]
    pub symbols: Vec<String>,
}

/// Arguments for the `search` command.
#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Free-form search query (symbol or company name).
    pub query: String,

    /// Maximum number of results to return.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

/// Arguments for the `financials` command.
#[derive(Debug, Args)]
pub struct FinancialsArgs {
    /// Market symbol to fetch financials for.
    pub symbol: String,

    /// Financial statement type.
    ///
    /// Supported types:
    /// - income: Income statement
    /// - balance: Balance sheet
    /// - cashflow: Cash flow statement
    #[arg(long, default_value = "income")]
    pub statement: String,

    /// Period type (annual, quarterly).
    #[arg(long, default_value = "annual")]
    pub period: String,

    /// Number of periods to return (default: 4).
    #[arg(long, default_value_t = 4)]
    pub limit: usize,
}

/// Arguments for the `earnings` command.
#[derive(Debug, Args)]
pub struct EarningsArgs {
    /// Market symbol to fetch earnings for.
    pub symbol: String,

    /// Number of quarters to return (default: 8).
    #[arg(long, default_value_t = 8)]
    pub limit: usize,
}

/// Arguments for the `sql` command.
#[derive(Debug, Args)]
pub struct SqlArgs {
    /// SQL query to execute.
    pub query: String,

    /// Allow write operations (INSERT, UPDATE, DELETE, CREATE, etc.).
    ///
    /// Without this flag, only SELECT and CTE queries are allowed.
    #[arg(long, default_value_t = false)]
    pub write: bool,

    /// Maximum number of rows to return (prevents memory exhaustion).
    #[arg(long, default_value_t = 10_000)]
    pub max_rows: usize,

    /// Query timeout in milliseconds.
    #[arg(long, default_value_t = 5_000)]
    pub query_timeout_ms: u64,
}

/// Arguments for the `cache` command group.
#[derive(Debug, Args)]
pub struct CacheArgs {
    #[command(subcommand)]
    pub command: CacheCommand,
}

/// Arguments for `cache load` command.
#[derive(Debug, Args)]
pub struct CacheLoadArgs {
    /// Stock symbol to load (e.g., AAPL).
    pub symbol: String,

    /// Number of days of historical data to fetch.
    #[arg(long, default_value = "30")]
    pub days: Option<String>,

    /// Bar interval (1m, 5m, 15m, 1h, 1d).
    #[arg(long, default_value = "1d")]
    pub interval: String,
}

/// Arguments for `export` command.
#[derive(Debug, Args)]
pub struct ExportArgs {
    /// SQL query to execute for export.
    ///
    /// If not provided, --table and --symbol will be used to generate a default query.
    #[arg(long)]
    pub query: Option<String>,

    /// Table to export (bars, quotes, fundamentals).
    ///
    /// Used when --query is not provided.
    #[arg(long)]
    pub table: Option<String>,

    /// Symbol to filter (optional).
    ///
    /// Used with --table to filter by symbol.
    #[arg(long)]
    pub symbol: Option<String>,

    /// Output format (parquet, csv).
    ///
    /// - parquet: Columnar format optimized for analytics
    /// - csv: Human-readable spreadsheet format
    #[arg(long, default_value = "parquet")]
    pub export_format: String,

    /// Output file path.
    #[arg(long)]
    pub output: String,

    /// Maximum number of rows to export.
    #[arg(long, default_value = "100000")]
    pub max_rows: Option<String>,

    /// Query timeout in milliseconds.
    #[arg(long, default_value = "30000")]
    pub query_timeout_ms: Option<String>,
}

/// Cache management subcommands.
#[derive(Debug, Subcommand)]
pub enum CacheCommand {
    /// Load historical data from providers into warehouse cache.
    ///
    /// Fetches OHLCV bars for a given symbol and interval,
    /// storing them in both the DuckDB warehouse and as Parquet files
    /// for analytics and ML workflows.
    Load(CacheLoadArgs),

    /// Sync local Parquet cache partitions into warehouse metadata.
    ///
    /// Scans the cache directory for parquet files and registers them
    /// in the warehouse manifest for query access.
    Sync,
}

/// Arguments for the `schema` command group.
#[derive(Debug, Args)]
pub struct SchemaArgs {
    #[command(subcommand)]
    pub command: SchemaCommand,
}

/// Schema inspection subcommands.
#[derive(Debug, Subcommand)]
pub enum SchemaCommand {
    /// List available schema names.
    ///
    /// Shows all bundled JSON schemas used for output validation.
    List,

    /// Get a schema document by name.
    ///
    /// Outputs the full JSON schema for the specified type.
    Get(SchemaGetArgs),
}

/// Arguments for `schema get` command.
#[derive(Debug, Args)]
pub struct SchemaGetArgs {
    /// Schema file name (e.g., 'envelope' or 'envelope.schema.json').
    pub name: String,
}

/// Arguments for the `sources` command.
#[derive(Debug, Args)]
pub struct SourcesArgs {
    /// Include detailed capability information.
    #[arg(long, default_value_t = false)]
    pub verbose: bool,
}
