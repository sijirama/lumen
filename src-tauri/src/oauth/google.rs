// src-tauri/src/auth/google.rs
use anyhow::{anyhow, Result};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use tiny_http::{Response, Server};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct GoogleAuth {
    client_id: String,
    client_secret: String,
    redirect_url: String,
}

impl GoogleAuth {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_url: "http://localhost:18247".to_string(), // Random port
        }
    }

    fn get_client(&self) -> Result<BasicClient> {
        Ok(BasicClient::new(
            ClientId::new(self.client_id.clone()),
            Some(ClientSecret::new(self.client_secret.clone())),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())?,
            Some(TokenUrl::new(
                "https://oauth2.googleapis.com/token".to_string(),
            )?),
        )
        .set_redirect_uri(RedirectUrl::new(self.redirect_url.clone())?))
    }

    pub async fn start_auth_flow(&self) -> Result<(String, String)> {
        let client = self.get_client()?;

        let (auth_url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/calendar".to_string(),
            ))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/gmail.send".to_string(),
            ))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/gmail.readonly".to_string(),
            ))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/tasks".to_string(),
            ))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/userinfo.email".to_string(),
            ))
            .add_extra_param("access_type", "offline")
            .add_extra_param("prompt", "consent")
            .url();

        Ok((auth_url.to_string(), csrf_token.secret().to_string()))
    }

    pub fn listen_for_code(&self, expected_state: String) -> Result<String> {
        let server = Server::http("127.0.0.1:18247")
            .map_err(|e| anyhow!("Failed to start local server: {}", e))?;

        for request in server.incoming_requests() {
            let url = format!("http://localhost:18247{}", request.url());
            let parsed_url = Url::parse(&url)?;

            let code = parsed_url
                .query_pairs()
                .find(|(key, _)| key == "code")
                .map(|(_, value)| value.into_owned());

            let state = parsed_url
                .query_pairs()
                .find(|(key, _)| key == "state")
                .map(|(_, value)| value.into_owned());

            match (code, state) {
                (Some(c), Some(s)) if s == expected_state => {
                    let response = Response::from_string(
                        "Authentication successful! You can close this window now.",
                    );
                    request.respond(response)?;
                    return Ok(c);
                }
                _ => {
                    let response = Response::from_string(
                        "Authentication failed. State mismatch or no code received.",
                    );
                    request.respond(response)?;
                    return Err(anyhow!("OAuth callback failed"));
                }
            }
        }
        Err(anyhow!("No request received"))
    }

    pub async fn exchange_code(&self, code: String) -> Result<GoogleTokens> {
        let client = self.get_client()?;

        let token_result = client
            .exchange_code(AuthorizationCode::new(code))
            .request_async(async_http_client)
            .await
            .map_err(|e| anyhow!("Failed to exchange token: {}", e))?;

        let expires_at = token_result.expires_in().map(|d| {
            chrono::Utc::now() + chrono::Duration::from_std(d).unwrap_or(chrono::Duration::zero())
        });

        Ok(GoogleTokens {
            access_token: token_result.access_token().secret().to_string(),
            refresh_token: token_result.refresh_token().map(|t| t.secret().to_string()),
            expires_at,
        })
    }

    pub async fn refresh_access_token(&self, refresh_token: String) -> Result<GoogleTokens> {
        let client = self.get_client()?;

        let token_result = client
            .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token))
            .request_async(async_http_client)
            .await
            .map_err(|e| anyhow!("Failed to refresh token: {}", e))?;

        let expires_at = token_result.expires_in().map(|d| {
            chrono::Utc::now() + chrono::Duration::from_std(d).unwrap_or(chrono::Duration::zero())
        });

        // NOTE: Refresh token might be None in refresh response, keep the old one if so
        Ok(GoogleTokens {
            access_token: token_result.access_token().secret().to_string(),
            refresh_token: token_result.refresh_token().map(|t| t.secret().to_string()),
            expires_at,
        })
    }
}
