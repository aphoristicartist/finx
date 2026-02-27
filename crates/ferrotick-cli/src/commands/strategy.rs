use crate::cli::{StrategyArgs, StrategyBacktestArgs, StrategyCommand, StrategyValidateArgs};
use crate::error::CliError;
use ferrotick_strategies::strategies::StrategyDescriptor;
use ferrotick_strategies::{built_in_strategies, parse_and_validate_file};

pub async fn run(args: &StrategyArgs) -> Result<(), CliError> {
    match &args.command {
        StrategyCommand::List => run_list(),
        StrategyCommand::Validate(validate_args) => run_validate(validate_args),
        StrategyCommand::Backtest(backtest_args) => run_backtest(backtest_args),
    }
}

fn run_list() -> Result<(), CliError> {
    let strategies = built_in_strategies();
    println!("Built-in Strategies:");
    for strategy in strategies {
        println!("  - {} (\"{}\")", strategy.name, strategy.description);
    }
    Ok(())
}

fn run_validate(args: &StrategyValidateArgs) -> Result<(), CliError> {
    let spec = parse_and_validate_file(std::path::Path::new(&args.file))?;
    println!("✅ Strategy spec is valid: {}", spec.name);
    println!("   Type: {}", spec.strategy_type);
    println!("   Timeframe: {}", spec.timeframe);
    println!("   Entry rules: {} rules", spec.entry_rules.len());
    println!("   Exit rules: {} rules", spec.exit_rules.len());
    println!(
        "   Position sizing: {} ({})",
        spec.position_sizing.method, spec.position_sizing.value
    );
    Ok(())
}

fn run_backtest(args: &StrategyBacktestArgs) -> Result<(), CliError> {
    let spec = parse_and_validate_file(std::path::Path::new(&args.file))?;
    let symbols: Vec<&str> = args.symbols.split(',').map(|s| s.trim()).collect();

    println!("🧪 Backtesting strategy: {}", spec.name);
    println!("   Symbols: {}", symbols.join(", "));
    println!("   Strategy: {}", spec.strategy_type);

    // TODO: Integrate with ferrotick-backtest engine
    println!("   ⚠️  Backtest integration with ferrotick-backtest is pending");
    println!("   Strategy validated and ready for backtesting");

    Ok(())
}
