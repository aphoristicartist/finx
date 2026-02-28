use crate::error::TradingError;
use serde::{Deserialize, Serialize};

pub struct AlpacaClient {
    api_key: String,
    api_secret: String,
    pub base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlpacaOrder {
    pub symbol: String,
    pub qty: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub time_in_force: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlpacaAccount {
    pub buying_power: String,
    pub cash: String,
    pub portfolio_value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlpacaOrderResponse {
    pub id: String,
    pub symbol: String,
    pub qty: String,
    pub status: String,
}

impl AlpacaClient {
    pub fn new(api_key: String, api_secret: String, paper: bool) -> Self {
        let base_url = if paper {
            "https://paper-api.alpaca.markets/v2".to_string()
        } else {
            "https://api.alpaca.markets/v2".to_string()
        };

        Self {
            api_key,
            api_secret,
            base_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_account(&self) -> Result<AlpacaAccount, TradingError> {
        let url = format!("{}/account", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("APCA-API-KEY-ID", &self.api_key)
            .header("APCA-API-SECRET-KEY", &self.api_secret)
            .send()
            .await?;

        let account = response.json::<AlpacaAccount>().await?;

        Ok(account)
    }

    pub async fn submit_order(
        &self,
        order: &AlpacaOrder,
    ) -> Result<AlpacaOrderResponse, TradingError> {
        let url = format!("{}/orders", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("APCA-API-KEY-ID", &self.api_key)
            .header("APCA-API-SECRET-KEY", &self.api_secret)
            .json(order)
            .send()
            .await?;

        let order_response = response.json::<AlpacaOrderResponse>().await?;

        Ok(order_response)
    }
}
