//INFO: Shared HTTP client for outbound integration calls.
//      Building a new reqwest::Client per call defeats connection pooling and
//      forces a fresh TLS handshake every time. One process-wide client with
//      a sane default timeout is enough for every Google / Tavily / TTS call.

use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

static SHARED: OnceLock<Client> = OnceLock::new();

pub fn shared_client() -> Client {
    SHARED
        .get_or_init(|| {
            Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new())
        })
        .clone()
}
