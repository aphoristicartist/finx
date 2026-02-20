use clap::{Args, Parser, Subcommand, ValueEnum};

/// ferrotick command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "ferrotick",
    author,
    version,
    about = "Provider-neutral financial data CLI",
    long_about = "A typed CLI skeleton for quote, bars, fundamentals, search, schema, and SQL workflows."
)]
pub struct Cli {
    /// Output shape.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,

    /// Pretty-print JSON output.
    #[arg(long, global = true, default_value_t = false)]
    pub pretty: bool,

    /// Treat warnings/errors as failure.
    #[arg(long, global = true, default_value_t = false)]
    pub strict: bool,

    /// Source selection strategy.
    #[arg(long, global = true, value_enum, default_value_t = SourceSelector::Auto)]
    pub source: SourceSelector,

    /// Request timeout budget in milliseconds.
    #[arg(long, global = true, default_value_t = 3000)]
    pub timeout_ms: u64,

    /// Enable profile metadata (placeholder in this phase).
    #[arg(long, global = true, default_value_t = false)]
    pub profile: bool,

    /// Enable NDJSON stream mode.
    #[arg(long, global = true, default_value_t = false)]
    pub stream: bool,

    /// Show query plan diagnostics.
    #[arg(long, global = true, default_value_t = false)]
    pub explain: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Ndjson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SourceSelector {
    Auto,
    Yahoo,
    Polygon,
    Alphavantage,
    Alpaca,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Fetch latest quote(s).
    Quote(QuoteArgs),

    /// Fetch OHLCV bars.
    Bars(BarsArgs),

    /// Fetch fundamentals snapshot(s).
    Fundamentals(FundamentalsArgs),

    /// Search instruments.
    Search(SearchArgs),

    /// Run local SQL query against the DuckDB warehouse.
    Sql(SqlArgs),

    /// Cache management commands.
    Cache(CacheArgs),

    /// Inspect bundled schemas.
    Schema(SchemaArgs),

    /// List source capability matrix.
    Sources(SourcesArgs),
}

#[derive(Debug, Args)]
pub struct QuoteArgs {
    /// One or more market symbols.
    #[arg(required = true, num_args = 1..)]
    pub symbols: Vec<String>,
}

#[derive(Debug, Args)]
pub struct BarsArgs {
    /// Market symbol.
    pub symbol: String,

    /// Interval (`1m`, `5m`, `15m`, `1h`, `1d`).
    #[arg(long, default_value = "1d")]
    pub interval: String,

    /// Number of bars to return.
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct FundamentalsArgs {
    /// One or more market symbols.
    #[arg(required = true, num_args = 1..)]
    pub symbols: Vec<String>,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Free-form instrument query string.
    pub query: String,

    /// Max returned matches.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct SqlArgs {
    /// SQL text.
    pub query: String,

    /// Allow write statements (INSERT/UPDATE/CREATE/DELETE).
    #[arg(long, default_value_t = false)]
    pub write: bool,

    /// Maximum row count.
    #[arg(long, default_value_t = 10_000)]
    pub max_rows: usize,

    /// Query timeout in milliseconds.
    #[arg(long, default_value_t = 5_000)]
    pub query_timeout_ms: u64,
}

#[derive(Debug, Args)]
pub struct CacheArgs {
    #[command(subcommand)]
    pub command: CacheCommand,
}

#[derive(Debug, Subcommand)]
pub enum CacheCommand {
    /// Sync local Parquet cache partitions into warehouse metadata.
    Sync,
}

#[derive(Debug, Args)]
pub struct SchemaArgs {
    #[command(subcommand)]
    pub command: SchemaCommand,
}

#[derive(Debug, Subcommand)]
pub enum SchemaCommand {
    /// List available schema names.
    List,

    /// Get a schema document by name.
    Get(SchemaGetArgs),
}

#[derive(Debug, Args)]
pub struct SchemaGetArgs {
    /// Schema file name. Accepts either `envelope` or full file name.
    pub name: String,
}

#[derive(Debug, Args)]
pub struct SourcesArgs {
    /// Include detailed capabilities.
    #[arg(long, default_value_t = false)]
    pub verbose: bool,
}
