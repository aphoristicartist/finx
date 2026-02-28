use serde::Serialize;

use ferrotick_core::{
    FinancialPeriod, FinancialsBatch, FinancialsRequest, SourceRouter, SourceStrategy,
    StatementType, Symbol,
};

use crate::cli::FinancialsArgs;
use crate::error::CliError;

use super::CommandResult;

#[derive(Debug, Serialize)]
struct FinancialsResponseData {
    financials: FinancialsBatch,
}

pub async fn run(
    args: &FinancialsArgs,
    router: &SourceRouter,
    strategy: &SourceStrategy,
) -> Result<CommandResult, CliError> {
    let symbol = Symbol::parse(&args.symbol)?;

    let statement_type = match args.statement.to_lowercase().as_str() {
        "income" => StatementType::Income,
        "balance" => StatementType::Balance,
        "cashflow" | "cash-flow" | "cash_flow" => StatementType::CashFlow,
        _ => {
            return Err(CliError::Command(format!(
                "invalid statement type: {}",
                args.statement
            )))
        }
    };

    let period = match args.period.to_lowercase().as_str() {
        "annual" => FinancialPeriod::Annual,
        "quarterly" | "quarter" => FinancialPeriod::Quarterly,
        _ => {
            return Err(CliError::Command(format!(
                "invalid period: {}",
                args.period
            )))
        }
    };

    let request = FinancialsRequest::new(symbol, statement_type, period, args.limit)
        .map_err(|error| CliError::Command(error.to_string()))?;

    match router.route_financials(&request, strategy.clone()).await {
        Ok(route) => {
            let data = serde_json::to_value(FinancialsResponseData {
                financials: route.data,
            })?;

            Ok(CommandResult::ok(data, route.source_chain)
                .with_errors(route.errors)
                .with_warnings(route.warnings)
                .with_latency(route.latency_ms)
                .with_cache_hit(false))
        }
        Err(failure) => {
            let data = serde_json::to_value(FinancialsResponseData {
                financials: FinancialsBatch { financials: vec![] },
            })?;
            Ok(CommandResult::ok(data, failure.source_chain)
                .with_errors(failure.errors)
                .with_warnings(failure.warnings)
                .with_latency(failure.latency_ms)
                .with_cache_hit(false))
        }
    }
}
