//! List every integrated account of a tenant.
//!
//! TRUTO_API_TOKEN=... cargo run --example list_integrated_accounts -- <tenant_id>

use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), truto::Error> {
    let token = std::env::var("TRUTO_API_TOKEN").expect("set TRUTO_API_TOKEN");
    let tenant_id = std::env::args().nth(1).expect("usage: <tenant_id>");
    let client = truto::Client::new(token);

    let accounts = client
        .integrated_accounts()
        .list()
        .query(json!({ "tenant_id": tenant_id }))
        .collect_all()
        .await?;

    for account in accounts {
        println!(
            "{} {:?} {}",
            account.id.unwrap_or_default(),
            account.status.map(|s| s.to_string()),
            account.integration.and_then(|i| i.name).unwrap_or_default(),
        );
    }
    Ok(())
}
