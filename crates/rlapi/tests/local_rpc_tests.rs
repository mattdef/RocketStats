use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use rocketstats_rlapi::{PsyNetClient, PsyNetConfig};
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use url::Url;

#[tokio::test]
async fn authenticates_then_sends_typed_rpc_request_over_websocket() {
    let ws_addr = spawn_websocket_server().await;
    let http_addr = spawn_auth_server(ws_addr).await;

    let base_url = Url::parse(&format!("http://{http_addr}/rpc")).unwrap();
    let client = PsyNetClient::new(PsyNetConfig::default().with_base_url(base_url));

    let rpc = client
        .auth_player_eos("eos-token", "account-123", Some("RocketStats"))
        .await
        .expect("auth connects");

    let population = rpc.get_population().await.expect("population response");

    assert_eq!(population.len(), 1);
    assert_eq!(population[0].playlist_id, 13);
    assert_eq!(population[0].population, 42);

    rpc.close().await.expect("close succeeds");
}

async fn spawn_auth_server(ws_addr: SocketAddr) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = vec![0; 8192];
        let bytes = stream.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..bytes]);

        assert!(request.starts_with("POST /rpc/Auth/AuthPlayer/v2 HTTP/1.1"));
        assert!(request.contains("psybuildid: -1652286008"));
        assert!(request.contains("psyenvironment: Prod"));
        assert!(request.contains(r#""FeatureSet":"PrimeUpdate58_1""#));
        assert!(request.contains(r#""EpicAccountID":"account-123""#));

        let body = json!({
            "Result": {
                "SessionID": "session-123",
                "PerConURLv2": format!("ws://{ws_addr}"),
                "PsyToken": "psy-token"
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    });
    addr
}

async fn spawn_websocket_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (stream, request_addr) = listener.accept().await.unwrap();
        let mut ws = accept_async(stream).await.unwrap();
        let message = ws.next().await.unwrap().unwrap().into_text().unwrap();

        assert!(message.contains("PsyService: Population/GetPopulation v1\r\n"));
        assert!(message.contains("PsyRequestID: PsyNetMessage_X_1\r\n"));
        assert!(message.contains("PsySig: "));
        assert!(message.ends_with("{}"));

        let response = concat!(
            "PsyTime: 1\r\n",
            "PsySig: test\r\n",
            "PsyResponseID: PsyNetMessage_X_1\r\n",
            "\r\n",
            r#"{"Result":{"Playlists":[{"PlaylistID":13,"Population":42}],"Timestamp":1}}"#
        );
        ws.send(Message::Text(response.into())).await.unwrap();

        let _ = request_addr;
    });
    addr
}
