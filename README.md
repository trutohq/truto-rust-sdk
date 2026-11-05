# truto

Rust SDK for the [Truto](https://truto.one) REST API: async (tokio), typed
models generated from Truto's OpenAPI spec, automatic retries and lazy cursor
pagination.

## Install

```sh
cargo add truto
cargo add tokio --features macros,rt-multi-thread
```

Guide: <https://truto.one/docs/sdks/rust>. API reference: <https://docs.rs/truto>.

Minimum supported Rust version: 1.88.

## Quick start

Every method returns a request builder. Chain options onto it, then `.await`
it. List methods return a lazy `Pager`.

```rust,no_run
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), truto::Error> {
    let client = truto::Client::new(std::env::var("TRUTO_API_TOKEN").unwrap());

    // List integrated accounts (typed: truto::models::IntegratedAccountResponse).
    let accounts = client
        .integrated_accounts()
        .list()
        .query(json!({ "tenant_id": "acme" }))
        .collect_all()
        .await?;
    for a in &accounts {
        println!("{:?} {:?}", a.id, a.status);
    }

    // Get one, typed.
    let account = client.integrated_accounts().get("integrated-account-id").await?;
    println!("{:?}", account.integration.and_then(|i| i.name));
    Ok(())
}
```

`Client` is cheap to clone (an `Arc` inside). Use `Client::builder()` for
settings:

```rust,no_run
use std::time::Duration;

let client = truto::Client::builder()
    .token("…")
    .base_url("https://api.truto.one") // default
    .timeout(Duration::from_secs(30)) // per attempt, default 30s
    .max_retries(3) // default 3
    .default_retry_after(Duration::from_secs(10)) // 429 without Retry-After
    .on_retry(|n, status| eprintln!("retry {n} after {status:?}"))
    .build()?;
# Ok::<(), truto::Error>(())
```

## Unified API with pagination

Unified, Proxy and Custom API items are `truto::Record`
(`serde_json::Map<String, Value>`), because their shape depends on the
integration.

```rust,no_run
use futures_util::TryStreamExt;
use truto::Query;

# async fn run(client: truto::Client) -> Result<(), truto::Error> {
// Stream every contact, fetching pages as needed.
let mut contacts = client
    .unified_api()
    .list("crm", "contacts", "integrated-account-id")
    .query(Query::new().set("limit", 100))
    .into_stream();
while let Some(contact) = contacts.try_next().await? {
    println!("{:?}", contact.get("first_name"));
}

// Or page by page.
let mut pager = client.unified_api().list("crm", "contacts", "integrated-account-id");
while let Some(page) = pager.next_page().await? {
    println!("{} items, next cursor {:?}", page.items.len(), page.next_cursor);
}

// Or everything at once / just the first item.
let all = client.unified_api().list("crm", "contacts", "ia").collect_all().await?;
let first = client.unified_api().list("crm", "contacts", "ia").first().await?;
# Ok(()) }
```

The pager copies your query and adds `next_cursor` for each following page; it
stops when the server returns no cursor. Responses carrying `results` instead
of `result` are handled too.

## Proxy and Custom APIs

```rust,no_run
use serde_json::json;

# async fn run(client: truto::Client) -> Result<(), truto::Error> {
let ticket = client.proxy_api().get("tickets", "123", "ia").await?;
let created = client
    .proxy_api()
    .create("tickets", "ia", json!({ "subject": "Hello" }))
    .await?;

// Custom API: path may contain `/`. Non-JSON responses: use `.raw()`.
let report = client.custom_api().get("reports/export.csv", "ia").raw().await?;
println!("{} bytes of {:?}", report.body.len(), report.content_type());
# Ok(()) }
```

## Query parameters

`.query(..)` takes a `serde_json::Value` object, a `serde_json::Map`, or a
`truto::Query`. Nested values are encoded like the `qs` package
(`filter[status]=open`, `ids[0]=a`), keys sorted. Repeated `.query` calls merge.

Admin list endpoints filter on related fields with a dotted key, not a nested
object: the API rejects `integration[name]=...` but accepts `integration.name=...`.

```rust,no_run
# use serde_json::json;
# async fn run(client: truto::Client) -> Result<(), truto::Error> {
let zoho_accounts = client
    .integrated_accounts()
    .list()
    .query(json!({ "tenant_id": "acme", "integration.name": "zohocrm" }))
    .collect_all()
    .await?;
# Ok(()) }
```

## Error handling

All methods return `Result<T, truto::Error>`:

```rust,no_run
use truto::{Error, ErrorBody};

# async fn run(client: truto::Client) {
match client.tenants().get("acme").await {
    Ok(tenant) => println!("{tenant:?}"),
    Err(e) if e.is_not_found() => println!("no such tenant"),
    Err(e) if e.is_unauthorized() => println!("bad token"),
    Err(Error::Api(api)) => {
        // status, status_text, parsed body (JSON or text), response headers
        println!("{} {}: {:?}", api.status, api.status_text, api.body);
    }
    Err(Error::Timeout) => println!("timed out"),
    Err(Error::Transport(e)) => println!("network: {e}"),
    Err(Error::Decode { message, .. }) => println!("unexpected response: {message}"),
    Err(e) => println!("{e}"),
}
# }
```

## Retries and timeouts

Requests are retried on 408, 409, 425, 429, 502, 503 and 504, and on
connection errors (never on timeouts), up to `max_retries` times. A 429 waits
for the `Retry-After` header (in seconds, default 10s); other statuses back off
exponentially with full jitter (500ms base, 10s cap). Dropping the future
cancels any wait.

Every request can override these:

```rust,no_run
use std::time::Duration;

# async fn run(client: truto::Client) -> Result<(), truto::Error> {
let ia = client
    .integrated_accounts()
    .get("ia")
    .timeout(Duration::from_secs(5))
    .max_retries(5)
    .retry_status_codes([500])
    .header("x-request-tag", "nightly-sync")
    .on_retry(|count, status| eprintln!("retry {count} ({status:?})"))
    .on_rate_limited(|after, count| eprintln!("429: waiting {after}s (retry {count})"))
    .await?;

let once = client.integrated_accounts().get("ia").no_retry().await?;
# Ok(()) }
```

Reuse a set of overrides with `truto::RequestOptions` and `.options(opts)`.

## Typed decode

Admin resources return generated models from `truto::models`. Every struct
decodes leniently (unknown fields ignored, missing fields defaulted), enums
are open (an unknown value becomes `Other(String)` and serialises back
unchanged), and union types keep the raw JSON with an `as_<variant>()`
accessor per variant.

For dynamic payloads, decode into your own types with `.typed::<T>()` on any
request or pager, or `truto::decode` on a record:

```rust,no_run
#[derive(serde::Deserialize)]
struct Contact {
    id: String,
    first_name: Option<String>,
}

# async fn run(client: truto::Client) -> Result<(), truto::Error> {
let contacts: Vec<Contact> = client
    .unified_api()
    .list("crm", "contacts", "ia")
    .typed::<Contact>()
    .collect_all()
    .await?;

let record = client.unified_api().get("crm", "contacts", "c1", "ia").await?;
let contact: Contact = truto::decode(record)?;
# Ok(()) }
```

## Resources

`client.<resource>()` for: `api_tokens`, `environments`, `categories`,
`teams`, `users`, `tenants`, `link_tokens`, `session_tokens`,
`integrated_accounts`, `sandbox_integrated_accounts`, `integrations`,
`environment_integrations`, `unified_models`, `environment_unified_models`,
`unified_model_resource_methods`, `environment_unified_model_resource_methods`,
`sync_jobs`, `sync_job_runs`, `sync_job_cron_triggers`, `sync_job_templates`,
`sync_job_run_states`, `daemons`, `daemon_jobs`, `daemon_job_runs`,
`daemon_job_cron_triggers`, `alarms`, `workflows`, `workflow_runs`, `webhooks`,
`datastores`, `documentation`, `logs`, `static_gates`,
`observability_destinations`, `analytics`, `batch_requests`,
`environment_integration_webhooks`, `integrated_account_webhooks`,
`unified_api`, `proxy_api`, `custom_api`, `mcp`; plus `client.ping()`.

`logs().list(log_type)` takes the `log_type` the server requires:
`unified_proxy_api`, `rapid_bridge`, `webhook`, `mcp` or `sync_job_cron_trigger`.

## Examples

```sh
TRUTO_API_TOKEN=... cargo run --example list_integrated_accounts -- <tenant_id>
TRUTO_API_TOKEN=... cargo run --example unified_api -- <integrated_account_id>
TRUTO_API_TOKEN=... cargo run --example proxy_and_errors -- <integrated_account_id>
```

## Regenerating the models

`src/models/generated.rs` is generated from `spec/openapi.json`. After
updating the spec:

```sh
python3 codegen/normalize_spec.py spec/openapi.json spec/overlay.json > codegen/models.json
python3 codegen/generate_rust.py codegen/models.json > src/models/generated.rs
```

Both scripts need only Python 3. Do not edit the generated file by hand.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tests run against a local mock server; no network or token is needed.

## License

MIT. See [LICENSE](LICENSE).
