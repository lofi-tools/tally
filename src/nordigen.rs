use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
struct Account {
    id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Transaction {
    id: String,
    amount: f32,
    currency: String,
    date: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TransactionsResponse {
    transactions: Vec<Transaction>,
}

async fn obtain_jwt() -> anyhow::Result<()> {
    // TODO mv to config.rs
    let secret_id = env::var("NORDIGEN_API_SECRET_ID")?;
    let secret_key = env::var("NORDIGEN_API_SECRET_KEY")?;

    // Send a request to the Nordigen API to obtain an access token
    let client = Client::new();
    let response = client
        .post("https://ob.nordigen.com/api/v2/token")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "secret_id": secret_id,
            "secret_key": secret_key
        }))
        .send()
        .await?;

    // Parse the response and extract the access token
    let response_json: serde_json::Value = response.json().await?;
    let access_token = response_json["access_token"].as_str().unwrap_or("");
    println!("Access token: {}", access_token);

    Ok(())
}

async fn fetch_bank_transactions() -> Result<(), Box<dyn Error>> {
    // Read the API key from the environment variable
    let api_key = env::var("NORDIGEN_API_KEY")?;

    // Set up the request
    let account_id = "123456"; // Replace with your account ID
    let url = format!(
        "https://ob.nordigen.com/api/accounts/{}/transactions",
        account_id
    );
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", &format!("Bearer {}", api_key))
        .send()
        .await?;

    // Parse the response
    let response_text = response.text().await?;
    let transactions_response: TransactionsResponse = serde_json::from_str(&response_text)?;

    // Print the transactions
    for transaction in transactions_response.transactions {
        println!("{:?}", transaction);
    }

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[tokio::test]
    async fn test_obtain_jwt() -> anyhow::Result<()> {
        let jwt = obtain_jwt().await?;
        Ok(())
    }
}
