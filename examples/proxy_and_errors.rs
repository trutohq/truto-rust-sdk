//! Call the Proxy API and handle errors.
//!
//! TRUTO_API_TOKEN=... cargo run --example proxy_and_errors -- <integrated_account_id>

use truto::{Error, ErrorBody};

#[tokio::main]
async fn main() {
    let token = std::env::var("TRUTO_API_TOKEN").expect("set TRUTO_API_TOKEN");
    let account = std::env::args()
        .nth(1)
        .expect("usage: <integrated_account_id>");
    let client = truto::Client::new(token);

    match client
        .proxy_api()
        .get("contacts", "does-not-exist", &account)
        .no_retry()
        .await
    {
        Ok(record) => println!("found: {record:?}"),
        Err(e) if e.is_not_found() => println!("not found"),
        Err(Error::Api(api)) => {
            println!("API error {} {}", api.status, api.status_text);
            if let ErrorBody::Json(body) = &api.body {
                println!("{body:#}");
            }
        }
        Err(Error::Timeout) => println!("timed out"),
        Err(other) => println!("other error: {other}"),
    }
}
