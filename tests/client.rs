//! Mock-server tests (no network).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::TryStreamExt;
use serde_json::{json, Value};
use truto::models::{
    IntegratedAccountResponse, IntegratedAccountStatus, IntegrationCursorPagination,
    IntegrationPagination,
};
use truto::{Client, Error, ErrorBody, Record};
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn client(server: &MockServer) -> Client {
    Client::builder()
        .token("test-token")
        .base_url(format!("{}/", server.uri()))
        .build()
        .unwrap()
}

fn page(items: Value, cursor: Option<&str>) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({ "result": items, "next_cursor": cursor }))
}

fn query_of(req: &Request) -> Vec<(String, String)> {
    req.url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

#[tokio::test]
async fn sends_auth_accept_and_user_agent_headers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ping"))
        .and(header("authorization", "Bearer test-token"))
        .and(header("accept", "application/json"))
        .and(header(
            "user-agent",
            concat!("truto-rust-sdk/", env!("CARGO_PKG_VERSION")),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "message": "pong" })))
        .expect(1)
        .mount(&server)
        .await;

    let ping = client(&server).ping().await.unwrap();
    assert_eq!(ping.message.as_deref(), Some("pong"));
}

#[tokio::test]
async fn get_with_query_and_extra_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/integrated-account/ia%201"))
        .and(query_param("include", "integration"))
        .and(header("x-extra", "yes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "ia 1" })))
        .expect(1)
        .mount(&server)
        .await;

    let ia = client(&server)
        .integrated_accounts()
        .get("ia 1")
        .query(json!({ "include": "integration" }))
        .header("x-extra", "yes")
        .await
        .unwrap();
    assert_eq!(ia.id.as_deref(), Some("ia 1"));
}

#[tokio::test]
async fn post_sends_json_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/unified/crm/contacts"))
        .and(query_param("integrated_account_id", "ia_1"))
        .and(header("content-type", "application/json"))
        .and(body_json(json!({ "first_name": "Ada" })))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(json!({ "id": "c1", "first_name": "Ada" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let created = client(&server)
        .unified_api()
        .create("crm", "contacts", "ia_1", json!({ "first_name": "Ada" }))
        .await
        .unwrap();
    assert_eq!(created["id"], "c1");
}

#[tokio::test]
async fn no_content_gives_empty_result() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/documentation/d1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/daemon/d1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let c = client(&server);
    c.documentation().delete("d1").await.unwrap();
    let record: Record = c.daemons().delete("d1").await.unwrap();
    assert!(record.is_empty());
}

#[tokio::test]
async fn custom_api_raw_bytes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/custom/reports/export.csv"))
        .and(query_param("integrated_account_id", "ia_1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/csv")
                .set_body_bytes(b"id,name\n1,Ada\n".to_vec()),
        )
        .mount(&server)
        .await;

    let c = client(&server);
    let raw = c
        .custom_api()
        .get("/reports/export.csv", "ia_1")
        .raw()
        .await
        .unwrap();
    assert_eq!(raw.status, 200);
    assert_eq!(raw.content_type(), Some("text/csv"));
    assert_eq!(raw.body, b"id,name\n1,Ada\n");

    // Awaiting a JSON-typed request on a non-JSON body is a decode error.
    let err = c
        .custom_api()
        .get("reports/export.csv", "ia_1")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Decode { .. }), "{err:?}");
}

#[tokio::test]
async fn not_found_is_an_api_error_with_parsed_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tenant/missing"))
        .respond_with(
            ResponseTemplate::new(404)
                .insert_header("x-request-id", "req-1")
                .set_body_json(json!({ "message": "Tenant not found" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let err = client(&server).tenants().get("missing").await.unwrap_err();
    assert!(err.is_not_found());
    assert!(!err.is_rate_limited() && !err.is_unauthorized());
    assert_eq!(err.status(), Some(404));
    let api = err.api_error().unwrap();
    assert_eq!(api.status_text, "Not Found");
    assert_eq!(api.headers.get("x-request-id").unwrap(), "req-1");
    assert_eq!(
        api.body,
        ErrorBody::Json(json!({ "message": "Tenant not found" }))
    );
}

#[tokio::test]
async fn retries_503_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/environment/e1"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/environment/e1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "e1" })))
        .mount(&server)
        .await;

    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let env = client(&server)
        .environments()
        .get("e1")
        .on_retry(move |n, status| seen.lock().unwrap().push((n, status)))
        .await
        .unwrap();
    assert_eq!(env.id.as_deref(), Some("e1"));
    assert_eq!(*calls.lock().unwrap(), vec![(1, Some(503))]);
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn rate_limited_waits_for_retry_after() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/team/t1"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "1"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/team/t1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "t1" })))
        .mount(&server)
        .await;

    let limited = Arc::new(Mutex::new(Vec::new()));
    let seen = limited.clone();
    let retries = Arc::new(AtomicU32::new(0));
    let r = retries.clone();
    let start = Instant::now();
    let team = client(&server)
        .teams()
        .get("t1")
        .on_rate_limited(move |after, count| seen.lock().unwrap().push((after, count)))
        .on_retry(move |_, _| {
            r.fetch_add(1, Ordering::SeqCst);
        })
        .await
        .unwrap();
    assert_eq!(team["id"], "t1");
    assert!(start.elapsed() >= Duration::from_millis(950));
    assert_eq!(*limited.lock().unwrap(), vec![(1, 0)]);
    assert_eq!(retries.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retries_exhausted_returns_last_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user/u1"))
        .respond_with(ResponseTemplate::new(502).set_body_string("bad gateway"))
        .expect(3)
        .mount(&server)
        .await;

    let count = Arc::new(AtomicU32::new(0));
    let c = count.clone();
    let err = client(&server)
        .users()
        .get("u1")
        .max_retries(2)
        .on_retry(move |_, _| {
            c.fetch_add(1, Ordering::SeqCst);
        })
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(502));
    assert_eq!(err.body(), Some(&ErrorBody::Text("bad gateway".into())));
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn extra_retry_status_codes_are_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/alarm/a1"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/alarm/a1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "a1" })))
        .mount(&server)
        .await;

    let alarm = client(&server)
        .alarms()
        .get("a1")
        .retry_status_codes([500])
        .await
        .unwrap();
    assert_eq!(alarm["id"], "a1");
}

#[tokio::test]
async fn no_retry_disables_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user/u1"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;

    let err = client(&server)
        .users()
        .get("u1")
        .no_retry()
        .on_retry(|_, _| panic!("must not retry"))
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(503));
}

#[tokio::test]
async fn timeout_is_a_timeout_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ping"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "message": "pong" }))
                .set_delay(Duration::from_secs(2)),
        )
        .mount(&server)
        .await;

    let err = client(&server)
        .ping()
        .timeout(Duration::from_millis(200))
        .await
        .unwrap_err();
    assert!(err.is_timeout(), "{err:?}");
    assert!(matches!(err, Error::Timeout));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn paginates_across_three_pages() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/integrated-account"))
        .and(query_param("tenant_id", "t1"))
        .and(query_param_is_missing("next_cursor"))
        .respond_with(page(json!([{ "id": "a" }, { "id": "b" }]), Some("c1")))
        .expect(1)
        .mount(&server)
        .await;
    // An empty page that still carries a cursor: keep going.
    Mock::given(method("GET"))
        .and(path("/integrated-account"))
        .and(query_param("tenant_id", "t1"))
        .and(query_param("next_cursor", "c1"))
        .respond_with(page(json!([]), Some("c2")))
        .expect(1)
        .mount(&server)
        .await;
    // Last page uses the `results` spelling and an empty cursor.
    Mock::given(method("GET"))
        .and(path("/integrated-account"))
        .and(query_param("tenant_id", "t1"))
        .and(query_param("next_cursor", "c2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "results": [{ "id": "c" }], "next_cursor": "" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let caller_query = json!({ "tenant_id": "t1", "next_cursor": "" });
    let before = caller_query.clone();
    let c = client(&server);

    let all = c
        .integrated_accounts()
        .list()
        .query(&caller_query)
        .collect_all()
        .await
        .unwrap();
    let ids: Vec<_> = all.iter().map(|a| a.id.clone().unwrap()).collect();
    assert_eq!(ids, ["a", "b", "c"]);
    assert_eq!(caller_query, before, "caller query must not be mutated");

    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 3);
    assert_eq!(query_of(&reqs[0]), vec![("tenant_id".into(), "t1".into())]);
    assert_eq!(
        query_of(&reqs[2]),
        vec![
            ("next_cursor".into(), "c2".into()),
            ("tenant_id".into(), "t1".into())
        ]
    );
}

#[tokio::test]
async fn pager_next_page_first_and_stream() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/integration"))
        .and(query_param_is_missing("next_cursor"))
        .respond_with(page(json!([{ "id": "i1" }]), Some("n1")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/integration"))
        .and(query_param("next_cursor", "n1"))
        .respond_with(page(json!([{ "id": "i2" }]), None))
        .mount(&server)
        .await;

    let c = client(&server);
    let mut pager = c.integrations().list();
    let p1 = pager.next_page().await.unwrap().unwrap();
    assert_eq!(p1.items.len(), 1);
    assert_eq!(p1.next_cursor.as_deref(), Some("n1"));
    let p2 = pager.next_page().await.unwrap().unwrap();
    assert_eq!(p2.items[0].id.as_deref(), Some("i2"));
    assert!(p2.next_cursor.is_none());
    assert!(pager.next_page().await.unwrap().is_none());

    let first = c.integrations().list().first().await.unwrap().unwrap();
    assert_eq!(first.id.as_deref(), Some("i1"));

    let streamed: Vec<_> = c
        .integrations()
        .list()
        .into_stream()
        .try_collect()
        .await
        .unwrap();
    assert_eq!(streamed.len(), 2);
}

#[tokio::test]
async fn unified_list_returns_records_and_typed_decode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/unified/crm/contacts"))
        .and(query_param("integrated_account_id", "ia_1"))
        .and(query_param("limit", "2"))
        .respond_with(page(
            json!([
                { "id": "1", "first_name": "Ada", "remote_data": { "x": 1 } },
                { "id": "2", "first_name": "Grace" }
            ]),
            None,
        ))
        .mount(&server)
        .await;

    let c = client(&server);
    let records: Vec<Record> = c
        .unified_api()
        .list("crm", "contacts", "ia_1")
        .query(truto::Query::new().set("limit", 2))
        .collect_all()
        .await
        .unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["first_name"], "Ada");
    assert_eq!(records[0]["remote_data"]["x"], 1);

    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct Contact {
        id: String,
        first_name: String,
    }
    let typed: Vec<Contact> = c
        .unified_api()
        .list("crm", "contacts", "ia_1")
        .query(json!({ "limit": 2 }))
        .typed::<Contact>()
        .collect_all()
        .await
        .unwrap();
    assert_eq!(
        typed[1],
        Contact {
            id: "2".into(),
            first_name: "Grace".into()
        }
    );
    let one: Contact = truto::decode(records[0].clone()).unwrap();
    assert_eq!(one.id, "1");
}

#[test]
fn typed_decode_is_lenient_and_keeps_unknown_enum_values() {
    let raw = json!({
        "id": "6d1c4d6e-1f7a-4bd7-9d0e-0a8c6b9a1c11",
        "tenant_id": "acme-co",
        "environment_integration_id": "ei_123",
        "context": { "subdomain": "acme" },
        "status": "quarantined",
        "last_error": null,
        "last_forbidden_error": null,
        "authentication_method": "oauth2",
        "region": "wnam",
        "is_sandbox": false,
        "created_at": "2026-03-01T10:00:00.000Z",
        "updated_at": "2026-03-02 10:00:00",
        "integration": { "id": "int_1", "name": "hubspot", "brand_new_field": [1, 2] },
        "some_future_field": { "nested": true }
    });
    let ia: IntegratedAccountResponse = serde_json::from_value(raw).unwrap();
    assert_eq!(ia.tenant_id.as_deref(), Some("acme-co"));
    assert_eq!(
        ia.status,
        Some(IntegratedAccountStatus::Other("quarantined".into()))
    );
    assert_eq!(ia.updated_at.as_deref(), Some("2026-03-02 10:00:00"));
    // The unknown enum value round-trips unchanged.
    let back = serde_json::to_value(&ia).unwrap();
    assert_eq!(back["status"], "quarantined");
    assert_eq!(back["authentication_method"], "oauth2");

    // Known values map to their variant.
    let known: IntegratedAccountResponse =
        serde_json::from_value(json!({ "status": "needs_reauth" })).unwrap();
    assert_eq!(known.status, Some(IntegratedAccountStatus::NeedsReauth));
}

#[test]
fn union_accessor_decodes_variant() {
    let pagination: IntegrationPagination = serde_json::from_value(json!({
        "format": "cursor",
        "config": { "cursor_path": "meta.next", "limit_key": "per_page" }
    }))
    .unwrap();
    let cursor: IntegrationCursorPagination =
        pagination.as_integration_cursor_pagination().unwrap();
    assert_eq!(cursor.format.as_str(), "cursor");
    assert_eq!(cursor.config.cursor_path, "meta.next");
    assert_eq!(cursor.config.limit_key.as_deref(), Some("per_page"));

    // Constructing from a variant round-trips.
    let wrapped = IntegrationPagination::from(cursor.clone());
    assert_eq!(wrapped.as_integration_cursor_pagination().unwrap(), cursor);
}

#[tokio::test]
async fn mcp_payload_shapes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp/tok"))
        .and(body_json(json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "clientInfo": { "name": "my-app", "version": "1.0.0" }
            }
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "jsonrpc": "2.0", "result": {} })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp/tok"))
        .and(body_json(
            json!({ "jsonrpc": "2.0", "method": "tools/list", "id": 1 }),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "result": { "tools": [] } })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp/tok"))
        .and(body_json(json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "id": 2,
            "params": { "name": "list_contacts", "arguments": { "limit": 5 } }
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "result": { "content": [] } })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mcp = client(&server);
    let mcp = mcp.mcp();
    mcp.initialize("tok", "my-app", "1.0.0").await.unwrap();
    let tools = mcp.list_tools("tok").await.unwrap();
    assert_eq!(tools["result"]["tools"], json!([]));
    mcp.call_tool("tok", "list_contacts", json!({ "limit": 5 }))
        .await
        .unwrap();
}

#[tokio::test]
async fn integration_uninstall_lists_then_deletes_each() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/environment-integration"))
        .and(query_param("integration_id", "int_1"))
        .respond_with(page(json!([{ "id": "ei_1" }, { "id": "ei_2" }]), None))
        .expect(1)
        .mount(&server)
        .await;
    for id in ["ei_1", "ei_2"] {
        Mock::given(method("DELETE"))
            .and(path(format!("/environment-integration/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": id })))
            .expect(1)
            .mount(&server)
            .await;
    }

    let deleted = client(&server)
        .integrations()
        .uninstall("int_1")
        .await
        .unwrap();
    let ids: Vec<_> = deleted.into_iter().map(|d| d.id.unwrap()).collect();
    assert_eq!(ids, ["ei_1", "ei_2"]);
}

#[tokio::test]
async fn integration_install_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/environment-integration"))
        .and(body_json(
            json!({ "integration_id": "int_1", "is_enabled": true }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": "ei_1" })))
        .expect(1)
        .mount(&server)
        .await;

    let ei = client(&server)
        .integrations()
        .install("int_1", true)
        .await
        .unwrap();
    assert_eq!(ei.id.as_deref(), Some("ei_1"));
}

#[tokio::test]
async fn tools_filters_and_session_token_revoke() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/integrated-account/ia_1/tools"))
        .and(query_param("methods[0]", "get"))
        .and(query_param("methods[1]", "list"))
        .and(query_param("tags[0]", "crm"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "result": [] })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/session-token"))
        .and(body_json(json!({ "tenant_id": "t1" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(&server);
    c.integrated_accounts()
        .tools("ia_1", &["get", "list"], &["crm"])
        .await
        .unwrap();
    let body = truto::models::SessionTokenDeleteRequest {
        tenant_id: "t1".into(),
        ..Default::default()
    };
    c.session_tokens().revoke(&body).await.unwrap();
}

#[tokio::test]
async fn invalid_query_is_reported() {
    let server = MockServer::start().await;
    let err = client(&server)
        .ping()
        .query(json!(["not", "an", "object"]))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::InvalidRequest(_)));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn connection_errors_are_retried_then_reported() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let c = Client::builder()
        .token("t")
        .base_url(format!("http://127.0.0.1:{port}"))
        .build()
        .unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let err = c
        .ping()
        .max_retries(1)
        .on_retry(move |n, status| seen.lock().unwrap().push((n, status)))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Transport(_)), "{err:?}");
    assert_eq!(*calls.lock().unwrap(), vec![(1, None)]);
}
