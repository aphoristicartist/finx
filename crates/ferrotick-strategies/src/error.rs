use thiserror::Error;

#[derive(Debug, Error)]
pub enum StrategyError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("validation errors: {0:?}")]
    ValidationErrors(Vec<String>),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("yaml parse error: {0}")]
    YamlParse(String),
}
