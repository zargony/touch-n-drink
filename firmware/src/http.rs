use crate::wifi::{DnsSocket, TcpClient, TcpConnection, Wifi};
use core::fmt;
use embedded_io_async::{Read, Write};
use log::debug;
use reqwless::client::{
    HttpClient, HttpResource, HttpResourceRequestBuilder, TlsConfig, TlsVerify,
};
use reqwless::headers::ContentType;
use reqwless::request::{RequestBody, RequestBuilder};
use reqwless::response::StatusCode;
use serde::{de::DeserializeOwned, Serialize};

/// Maximum size of response from server
const MAX_RESPONSE_SIZE: usize = 4096;

/// TLS read buffer size
const READ_BUFFER_SIZE: usize = 4096;

/// TLS write buffer size
const WRITE_BUFFER_SIZE: usize = 2048;

/// HTTP client error
#[derive(Debug)]
pub enum Error {
    /// Network / http client error
    Network(reqwless::Error),
    /// Request could not be built
    MalformedRequest(serde_json::Error),
    /// Authorization required (HTTP status 401)
    Unauthorized,
    /// Server returned an error (HTTP status 4xx)
    BadRequest(StatusCode),
    /// Server returned an error (HTTP status 5xx)
    #[allow(clippy::enum_variant_names)]
    ServerError(StatusCode),
    /// Response could not be parsed
    MalformedResponse(serde_json::Error),
}

impl From<reqwless::Error> for Error {
    fn from(err: reqwless::Error) -> Self {
        Self::Network(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(_err) => write!(f, "Network error"),
            Self::MalformedRequest(_err) => write!(f, "Malformed request"),
            Self::Unauthorized => write!(f, "Unauthorized"),
            Self::BadRequest(status) => write!(f, "Bad request ({})", status.0),
            Self::ServerError(status) => write!(f, "Server error ({})", status.0),
            Self::MalformedResponse(_err) => write!(f, "Malformed response"),
        }
    }
}

/// HTTP client resources
pub struct Resources {
    read_buffer: [u8; READ_BUFFER_SIZE],
    write_buffer: [u8; WRITE_BUFFER_SIZE],
}

impl Resources {
    /// Create new HTTP client resources
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            read_buffer: [0; READ_BUFFER_SIZE],
            write_buffer: [0; WRITE_BUFFER_SIZE],
        }
    }
}

/// HTTP client
pub struct Http<'a> {
    client: HttpClient<'a, TcpClient<'a>, DnsSocket<'a>>,
    base_url: &'a str,
}

impl<'a> fmt::Debug for Http<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Http")
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl<'a> Http<'a> {
    /// Create new HTTP client using the given resources
    #[allow(dead_code)]
    pub fn new(wifi: &'a Wifi, seed: u64, resources: &'a mut Resources, base_url: &'a str) -> Self {
        // FIXME: embedded-tls can't verify TLS certificates (though pinning is supported)
        // This is bad since it makes communication vulnerable to mitm attacks. esp-mbedtls would
        // be an alternative, but is atm only supported with git reqwless and nightly Rust.
        let tls_config = TlsConfig::new(
            seed,
            &mut resources.read_buffer,
            &mut resources.write_buffer,
            TlsVerify::None,
        );
        let client = HttpClient::new_with_tls(wifi.tcp(), wifi.dns(), tls_config);

        Self { client, base_url }
    }

    /// Send GET request, deserialize JSON response
    #[allow(dead_code)]
    pub async fn get<T: DeserializeOwned>(&mut self, path: &str) -> Result<T, Error> {
        let base_url = self.base_url;
        let mut resource = self.resource().await?;

        debug!("HTTP: GET {}/{}", base_url, path);
        Self::send_request_parse_response(
            resource
                .get(path)
                .headers(&[("Accept", "application/json")]),
        )
        .await
    }

    /// Serialize data to JSON, send POST request, deserialize JSON response
    #[allow(dead_code)]
    pub async fn post<T: Serialize, U: DeserializeOwned>(
        &mut self,
        path: &str,
        data: &T,
    ) -> Result<U, Error> {
        let body = serde_json::to_vec(&data).map_err(Error::MalformedRequest)?;

        let base_url = self.base_url;
        let mut resource = self.resource().await?;

        debug!("HTTP: POST {}/{} ({} bytes)", base_url, path, body.len());
        Self::send_request_parse_response(
            resource
                .post(path)
                .content_type(ContentType::ApplicationJson)
                .headers(&[("Accept", "application/json")])
                .body(&body[..]),
        )
        .await
    }
}

impl<'a> Http<'a> {
    /// Returns a connected http resource client
    async fn resource(&mut self) -> Result<HttpResource<'_, TcpConnection<'_>>, Error> {
        // TODO: keep resource cached so that we stay connected (and reconnect only when required)?
        let resource = self.client.resource(self.base_url).await?;
        debug!("HTTP: Connect {}", self.base_url);
        Ok(resource)
    }

    /// Parse response status code
    fn parse_status_code(status: StatusCode) -> Result<(), Error> {
        if status.is_successful() {
            Ok(())
        } else if status.0 == 401 {
            Err(Error::Unauthorized)
        } else if status.is_server_error() {
            Err(Error::ServerError(status))
        } else {
            Err(Error::BadRequest(status))
        }
    }

    /// Send request, deserialize JSON response
    async fn send_request_parse_response<C: Read + Write, B: RequestBody, T: DeserializeOwned>(
        request: HttpResourceRequestBuilder<'_, '_, C, B>,
    ) -> Result<T, Error> {
        // rx_buf is used to buffer response headers. The response body reader uses this only for
        // non-TLS connections. Body reader of TLS connections will use the TLS read_buffer for
        // buffering parts of the body. However, read_to_end will again always use this buffer.
        let mut rx_buf = [0; MAX_RESPONSE_SIZE];
        let response = request.send(&mut rx_buf).await?;

        let status = response.status;
        Self::parse_status_code(status)?;

        // Reqwless' content-type parsing is unreliable, so parse the body in any case. Parsing
        // will fail if it's not JSON.
        // if !matches!(response.content_type, Some(ContentType::ApplicationJson)) {
        //     return Err(Error::InvalidResponse);
        // }

        // TODO: Use streaming JSON parser so that we don't need to buffer the full response body
        let body = response.body().read_to_end().await?;
        debug!("HTTP: Status {} ({} bytes)", status.0, body.len());

        serde_json::from_slice(body).map_err(Error::MalformedResponse)
    }
}
