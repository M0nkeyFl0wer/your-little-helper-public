use anyhow::{anyhow, Result};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use url::Url;

pub struct OAuthFlow {
    client: BasicClient,
    scopes: Vec<String>,
}

impl OAuthFlow {
    pub fn new(
        client_id: String,
        client_secret: Option<String>,
        auth_url: String,
        token_url: String,
        scopes: Vec<String>,
    ) -> Result<Self> {
        let client = BasicClient::new(
            ClientId::new(client_id),
            client_secret.map(ClientSecret::new),
            AuthUrl::new(auth_url)?,
            Some(TokenUrl::new(token_url)?),
        )
        .set_redirect_uri(RedirectUrl::new("http://localhost:8765/callback".to_string())?);

        Ok(Self { client, scopes })
    }

    pub async fn authenticate(&self) -> Result<(String, Option<String>)> {
        // Generate PKCE challenge for security
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Build authorization URL
        let mut auth_request = self.client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);

        for scope in &self.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, csrf_token) = auth_request.url();

        // Open browser to authorization URL
        println!("Opening browser for authentication...");
        println!("If the browser doesn't open automatically, visit:");
        println!("{}", auth_url);

        if let Err(e) = open::that(auth_url.as_str()) {
            eprintln!("Failed to open browser: {}", e);
            eprintln!("Please open the URL manually");
        }

        // Start local server to receive callback
        let listener = TcpListener::bind("127.0.0.1:8765")?;
        println!("Waiting for authorization...");

        let (code, state) = receive_callback(&listener)?;

        // Verify CSRF token
        if state != *csrf_token.secret() {
            return Err(anyhow!("CSRF token mismatch"));
        }

        // Exchange authorization code for access token
        let token_result = self.client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await?;

        let access_token = token_result.access_token().secret().clone();
        let refresh_token = token_result.refresh_token().map(|t| t.secret().clone());

        Ok((access_token, refresh_token))
    }
}

fn receive_callback(listener: &TcpListener) -> Result<(String, String)> {
    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            let mut reader = BufReader::new(&stream);
            let mut request_line = String::new();
            reader.read_line(&mut request_line)?;

            // Parse the request line to get the URL
            let redirect_url = request_line
                .split_whitespace()
                .nth(1)
                .ok_or_else(|| anyhow!("Invalid request"))?;

            let url = Url::parse(&format!("http://localhost{}", redirect_url))?;

            // Extract code and state from query parameters
            let code = url
                .query_pairs()
                .find(|(key, _)| key == "code")
                .map(|(_, value)| value.to_string())
                .ok_or_else(|| anyhow!("No authorization code in callback"))?;

            let state = url
                .query_pairs()
                .find(|(key, _)| key == "state")
                .map(|(_, value)| value.to_string())
                .ok_or_else(|| anyhow!("No state in callback"))?;

            // Send success response
            let response = "HTTP/1.1 200 OK\r\n\
                           Content-Type: text/html\r\n\r\n\
                           <html><body>\
                           <h1>Authentication successful!</h1>\
                           <p>You can close this window and return to Little Helper.</p>\
                           </body></html>";
            stream.write_all(response.as_bytes())?;
            stream.flush()?;

            return Ok((code, state));
        }
    }

    Err(anyhow!("Failed to receive callback"))
}
