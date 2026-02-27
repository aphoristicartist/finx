use super::parser::StrategySpec;

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub field: String,
    pub message: String,
}

pub fn validate_strategy_spec(spec: &StrategySpec) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if spec.name.trim().is_empty() {
        issues.push(ValidationIssue {
            field: "name".to_string(),
            message: "name must not be empty".to_string(),
        });
    }

    if spec.strategy_type.trim().is_empty() {
        issues.push(ValidationIssue {
            field: "type".to_string(),
            message: "type must not be empty".to_string(),
        });
    }

    if spec.timeframe.trim().is_empty() {
        issues.push(ValidationIssue {
            field: "timeframe".to_string(),
            message: "timeframe must not be empty".to_string(),
        });
    }

    if !matches!(
        spec.position_sizing.method.as_str(),
        "fixed" | "percent" | "volatility" | "kelly"
    ) {
        issues.push(ValidationIssue {
            field: "position_sizing.method".to_string(),
            message: format!(
                "invalid method '{}', must be one of: fixed, percent, volatility, kelly",
                spec.position_sizing.method
            ),
        });
    }

    if !spec.position_sizing.value.is_finite() || spec.position_sizing.value <= 0.0 {
        issues.push(ValidationIssue {
            field: "position_sizing.value".to_string(),
            message: "position_sizing.value must be a positive number".to_string(),
        });
    }

    for (idx, rule) in spec.entry_rules.iter().enumerate() {
        if rule.value.is_finite() == false {
            issues.push(ValidationIssue {
                field: format!("entry_rules[{}].value", idx),
                message: "value must be a finite number".to_string(),
            });
        }
    }

    for (idx, rule) in spec.exit_rules.iter().enumerate() {
        if rule.value.is_finite() == false {
            issues.push(ValidationIssue {
                field: format!("exit_rules[{}].value", idx),
                message: "value must be a finite number".to_string(),
            });
        }
    }

    if let Some(ref rm) = spec.risk_management {
        if let Some(sl) = rm.stop_loss {
            if !sl.is_finite() || sl <= 0.0 || sl >= 1.0 {
                issues.push(ValidationIssue {
                    field: "risk_management.stop_loss".to_string(),
                    message: "stop_loss must be between 0 and 1".to_string(),
                });
            }
        }
        if let Some(tp) = rm.take_profit {
            if !tp.is_finite() || tp <= 0.0 || tp >= 1.0 {
                issues.push(ValidationIssue {
                    field: "risk_management.take_profit".to_string(),
                    message: "take_profit must be between 0 and 1".to_string(),
                });
            }
        }
    }

    issues
}
