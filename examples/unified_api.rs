//! Stream CRM contacts through the Unified API, page by page.
//!
//! TRUTO_API_TOKEN=... cargo run --example unified_api -- <integrated_account_id>

use futures_util::TryStreamExt;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct Contact {
    id: String,
    #[serde(default)]
    first_name: Option<String>,
    #[serde(default)]
    last_name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), truto::Error> {
    let token = std::env::var("TRUTO_API_TOKEN").expect("set TRUTO_API_TOKEN");
    let account = std::env::args()
        .nth(1)
        .expect("usage: <integrated_account_id>");
    let client = truto::Client::builder()
        .token(token)
        .timeout(Duration::from_secs(60))
        .on_rate_limited(|after, n| eprintln!("rate limited, waiting {after}s (retry {n})"))
        .build()?;

    // Dynamic records.
    let mut contacts = client
        .unified_api()
        .list("crm", "contacts", &account)
        .query(truto::Query::new().set("limit", 100))
        .into_stream();
    while let Some(contact) = contacts.try_next().await? {
        println!("{:?}", contact.get("id"));
    }

    // Your own type.
    let first: Option<Contact> = client
        .unified_api()
        .list("crm", "contacts", &account)
        .typed::<Contact>()
        .first()
        .await?;
    if let Some(c) = first {
        println!(
            "first contact: {} {} {}",
            c.id,
            c.first_name.unwrap_or_default(),
            c.last_name.unwrap_or_default()
        );
    }
    Ok(())
}
