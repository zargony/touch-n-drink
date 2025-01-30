use crate::json::{self, FromJson, ToJson};
use crate::time;
use crate::wifi::{DnsSocket, TcpClient, TcpConnection, Wifi};
use alloc::vec;
use alloc::vec::Vec;
use chrono::DateTime;
use core::convert::Infallible;
use core::{fmt, str};
use embedded_io_async::{BufRead, Read};
use log::debug;
use reqwless::client::{HttpClient, HttpResource, HttpResourceRequestBuilder};
use reqwless::client::{TlsConfig, TlsVerify};
use reqwless::headers::ContentType;
use reqwless::request::{RequestBody, RequestBuilder};
use reqwless::response::{BodyReader, StatusCode};

/// Maximum size of response headers from server
const MAX_RESPONSE_HEADER_SIZE: usize = 2048;

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
    read_buffer: Vec<u8>,
    write_buffer: Vec<u8>,
}

impl Resources {
    /// Create new HTTP client resources
    pub fn new() -> Self {
        Self {
            read_buffer: vec![0; READ_BUFFER_SIZE],
            write_buffer: vec![0; WRITE_BUFFER_SIZE],
        }
    }
}

/// HTTP client
pub struct Http<'a> {
    client: HttpClient<'a, TcpClient<'a>, DnsSocket<'a>>,
}

impl fmt::Debug for Http<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Http").finish()
    }
}

impl<'a> Http<'a> {
    /// Create new HTTP client using the given resources
    pub fn new(wifi: &'a Wifi, seed: u64, resources: &'a mut Resources) -> Self {
        // FIXME: reqwless with embedded-tls can't verify TLS certificates (though pinning is
        // supported)/ This is bad since it makes communication vulnerable to mitm attacks.
        // esp-mbedtls would work, but is only supported with git reqwless and nightly Rust atm.
        let tls_config = TlsConfig::new(
            seed,
            &mut resources.read_buffer,
            &mut resources.write_buffer,
            TlsVerify::None,
        );
        let client = HttpClient::new_with_tls(wifi.tcp(), wifi.dns(), tls_config);

        Self { client }
    }

    /// Connect to HTTP server
    pub async fn connect<'conn>(
        &'conn mut self,
        base_url: &'conn str,
    ) -> Result<Connection<'conn>, Error> {
        let resource = self.client.resource(base_url).await?;
        debug!("HTTP: Connected {}", base_url);

        Ok(Connection { resource })
    }
}

/// HTTP client connection
pub struct Connection<'a> {
    resource: HttpResource<'a, TcpConnection<'a>>,
}

impl fmt::Debug for Connection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("host", &self.resource.host)
            .field("base_path", &self.resource.base_path)
            .finish()
    }
}

impl<'a> Connection<'a> {
    /// Send GET request, deserialize JSON response
    pub async fn get<T: FromJson>(&mut self, path: &str) -> Result<T, Error> {
        let mut rx_buf = vec![0; MAX_RESPONSE_HEADER_SIZE];
        let mut json = self.get_json(path, &mut rx_buf).await?;
        json.read().await.map_err(Error::MalformedResponse)
    }

    /// Send GET request, return response body JSON reader
    pub async fn get_json<'req>(
        &'req mut self,
        path: &'req str,
        rx_buf: &'req mut [u8],
    ) -> Result<json::Reader<BodyReader<impl Read + BufRead + use<'a, 'req>>>, Error> {
        // FIXME: Return type of this function shouldn't be generic, but reqwless hides the
        // inner type `BufferingReader` so we can't use the full type signature for now

        debug!("HTTP: GET {}/{}", self.resource.base_path, path);
        let request = self
            .resource
            .get(path)
            .headers(&[("Accept", "application/json")]);

        Self::send_request(request, rx_buf).await
    }

    /// Serialize data to JSON, send POST request, deserialize JSON response
    pub async fn post<T: ToJson, U: FromJson>(&mut self, path: &str, data: &T) -> Result<U, Error> {
        let body = Self::prepare_body(data).await?;
        let mut rx_buf = vec![0; MAX_RESPONSE_HEADER_SIZE];
        let mut json = self.post_json(path, &body, &mut rx_buf).await?;
        json.read().await.map_err(Error::MalformedResponse)
    }

    /// Serialize data to JSON, send POST request, return response body JSON reader
    pub async fn post_json<'req>(
        &'req mut self,
        path: &'req str,
        data: &'req [u8],
        rx_buf: &'req mut [u8],
    ) -> Result<json::Reader<BodyReader<impl Read + BufRead + use<'a, 'req>>>, Error> {
        // FIXME: Return type of this function shouldn't be generic, but reqwless hides the
        // inner type `BufferingReader` so we can't use the full type signature for now

        debug!(
            "HTTP: POST {}/{} ({} bytes)",
            self.resource.base_path,
            path,
            data.len()
        );
        let request = self
            .resource
            .post(path)
            .content_type(ContentType::ApplicationJson)
            .headers(&[("Accept", "application/json")])
            .body(data);

        Self::send_request(request, rx_buf).await
    }

    /// Serialize data to JSON for request body
    pub async fn prepare_body<T: ToJson>(data: T) -> Result<Vec<u8>, Error> {
        // OPTIMIZE: Don't buffer but stream request body. Only needed if we start sending much data
        let mut body = Vec::new();
        let mut json = json::Writer::new(&mut body);
        json.write(data).await.map_err(Error::MalformedRequest)?;
        Ok(body)
    }
}

impl Connection<'_> {
    /// Send request, check response status and return response body JSON reader
    async fn send_request<'req, 'conn, B: RequestBody>(
        request: HttpResourceRequestBuilder<'req, 'conn, TcpConnection<'conn>, B>,
        rx_buf: &'req mut [u8],
    ) -> Result<json::Reader<BodyReader<impl Read + BufRead + use<'req, 'conn, B>>>, Error> {
        // FIXME: Return type of this function shouldn't be generic, but reqwless hides the
        // inner type `BufferingReader` so we can't use the full type signature for now

        // rx_buf is used to buffer response headers. The response body reader uses this only for
        // non-TLS connections. Body reader of TLS connections will use the TLS read_buffer for
        // buffering parts of the body. However, read_to_end will again always use this buffer.
        let response = request.send(rx_buf).await?;
        debug!("HTTP: Status {}", response.status.0);

        // Extract current date and time from response
        let time = response
            .headers()
            .find_map(|(k, v)| (k == "Date").then_some(v))
            .and_then(|v| str::from_utf8(v).ok())
            .and_then(|s| DateTime::parse_from_rfc2822(s).ok());
        if let Some(time) = time {
            time::set(&time);
        }

        // Check HTTP response status
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

        Ok(json::Reader::new(response.body().reader()))
    }
}
