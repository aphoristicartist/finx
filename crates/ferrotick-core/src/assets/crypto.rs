use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoPair {
    pub base_asset: String,
    pub quote_asset: String,
    pub exchange: String,
    pub price: f64,
    pub volume_24h: f64,
}

impl CryptoPair {
    pub fn new(base: String, quote: String, exchange: String, price: f64) -> Self {
        Self {
            base_asset: base,
            quote_asset: quote,
            exchange,
            price,
            volume_24h: 0.0,
        }
    }

    pub fn symbol(&self) -> String {
        format!("{}/{}", self.base_asset, self.quote_asset)
    }

    pub fn calculate_value(&self, quantity: f64) -> f64 {
        self.price * quantity
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CryptoExchange {
    Binance,
    Coinbase,
    Kraken,
    FTX,
}
