use crate::json::{self, FromJson, Reader as JsonReader, ToJson, Writer as JsonWriter};
use crate::wifi::{DnsSocket, TcpClient, TcpConnection, Wifi};
use alloc::vec::Vec;
use core::convert::Infallible;
use core::fmt;
use log::debug;
use reqwless::client::{
    HttpClient, HttpConnection, HttpResource, HttpResourceRequestBuilder, TlsConfig, TlsVerify,
};
use reqwless::headers::ContentType;
use reqwless::request::{RequestBody, RequestBuilder};
use reqwless::response::{Response, StatusCode};

/// Maximum size of response from server
const MAX_RESPONSE_SIZE: usize = 4096;

/// TLS read buffer size
const READ_BUFFER_SIZE: usize = 16640;

/// TLS write buffer size
const WRITE_BUFFER_SIZE: usize = 2048;

/// HTTP client error
#[derive(Debug)]
pub enum Error {
    /// Network / http client error
    Network(reqwless::Error),
    /// Request could not be built
    MalformedRequest(json::Error<Infallible>),
    /// Authorization required (HTTP status 401)
    Unauthorized,
    /// Server returned an error (HTTP status 4xx)
    BadRequest(StatusCode),
    /// Server returned an error (HTTP status 5xx)
    #[allow(clippy::enum_variant_names)]
    ServerError(StatusCode),
    /// Response could not be parsed
    MalformedResponse(json::Error<reqwless::Error>),
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
    pub async fn get<T: FromJson>(&mut self, path: &str) -> Result<T, Error> {
        let base_url = self.base_url;

        let mut resource = self.resource().await?;

        debug!("HTTP: GET {}/{}", base_url, path);
        let request = resource
            .get(path)
            .headers(&[("Accept", "application/json")]);

        Self::send_request_read_response(request).await
    }

    /// Serialize data to JSON, send POST request, deserialize JSON response
    pub async fn post<T: ToJson, U: FromJson>(&mut self, path: &str, data: &T) -> Result<U, Error> {
        let base_url = self.base_url;

        let mut resource = self.resource().await?;
        let body = Self::prepare_body(data).await?;

        debug!("HTTP: POST {}/{} ({} bytes)", base_url, path, body.len());
        let request = resource
            .post(path)
            .content_type(ContentType::ApplicationJson)
            .headers(&[("Accept", "application/json")])
            .body(&body[..]);

        Self::send_request_read_response(request).await
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

    /// Serialize data to JSON for request body
    async fn prepare_body<T: ToJson>(data: T) -> Result<Vec<u8>, Error> {
        // OPTIMIZE: Don't buffer but stream request body. Only needed if we start sending much data
        let mut body = Vec::new();
        let mut json_writer = JsonWriter::new(&mut body);
        json_writer
            .write(data)
            .await
            .map_err(Error::MalformedRequest)?;
        Ok(body)
    }

    /// Check response status and headers
    fn check_response(
        response: &Response<'_, '_, HttpConnection<'_, TcpConnection>>,
    ) -> Result<(), Error> {
        debug!("HTTP: Status {}", response.status.0);

        if response.status.0 == 401 {
            return Err(Error::Unauthorized);
        } else if response.status.is_server_error() {
            return Err(Error::ServerError(response.status));
        } else if !response.status.is_successful() {
            return Err(Error::BadRequest(response.status));
        }

        // Reqwless' content-type parsing is unreliable, so parse the body in any case. Parsing
        // will fail if it's not JSON.
        // if !matches!(response.content_type, Some(ContentType::ApplicationJson)) {
        //     return Err(Error::InvalidResponse);
        // }

        Ok(())
    }

    /// Send request, deserialize JSON response
    async fn send_request_read_response<B: RequestBody, T: FromJson>(
        request: HttpResourceRequestBuilder<'_, '_, TcpConnection<'_>, B>,
    ) -> Result<T, Error> {
        // rx_buf is used to buffer response headers. The response body reader uses this only for
        // non-TLS connections. Body reader of TLS connections will use the TLS read_buffer for
        // buffering parts of the body. However, read_to_end will again always use this buffer.
        let mut rx_buf = [0; MAX_RESPONSE_SIZE];
        let response = request.send(&mut rx_buf).await?;

        Self::check_response(&response)?;

        let mut json_reader = JsonReader::new(response.body().reader());
        json_reader.read().await.map_err(Error::MalformedResponse)
    }
}
