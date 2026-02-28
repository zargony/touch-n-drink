use crate::json::{self, StreamingJsonObjectReader};
use crate::time;
use crate::wifi::{DnsSocket, TcpClient, TcpConnection, Wifi};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use chrono::DateTime;
use core::{fmt, str};
use log::debug;
use reqwless::client::{HttpClient, HttpConnection, HttpResource, HttpResourceRequestBuilder};
use reqwless::client::{TlsConfig, TlsVerify};
use reqwless::headers::ContentType;
use reqwless::request::{RequestBody, RequestBuilder};
use reqwless::response::{Response, StatusCode};
use serde::{Serialize, de::DeserializeOwned};

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
    /// Response stream could not be parsed
    MalformedResponseStream(json::Error<reqwless::Error>),
}

impl From<reqwless::Error> for Error {
    fn from(err: reqwless::Error) -> Self {
        Self::Network(err)
    }
}

impl From<json::Error<reqwless::Error>> for Error {
    fn from(err: json::Error<reqwless::Error>) -> Self {
        match err {
            json::Error::Read(err) => Self::Network(err),
            json::Error::Json(err) => Self::MalformedResponse(err),
            err => Self::MalformedResponseStream(err),
        }
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
            Self::MalformedResponseStream(_err) => write!(f, "Malformed response stream"),
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
pub struct Http<'client> {
    client: HttpClient<'client, TcpClient<'client>, DnsSocket<'client>>,
}

impl fmt::Debug for Http<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Http").finish()
    }
}

impl<'client> Http<'client> {
    /// Create new HTTP client using the given resources
    pub fn new(wifi: &'client Wifi, seed: u64, resources: &'client mut Resources) -> Self {
        // FIXME: reqwless with embedded-tls can't verify TLS certificates (though pinning is
        // supported). This is bad since it makes communication vulnerable to MITM attacks.
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
        debug!("HTTP: Connected {base_url}");

        Ok(Connection { resource })
    }
}

/// HTTP client connection
pub struct Connection<'conn> {
    resource: HttpResource<'conn, TcpConnection<'conn>>,
}

impl fmt::Debug for Connection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("host", &self.resource.host)
            .field("base_path", &self.resource.base_path)
            .finish()
    }
}

impl Connection<'_> {
    /// Send GET request, deserialize JSON response
    pub async fn get<T: DeserializeOwned>(&mut self, path: &str) -> Result<T, Error> {
        debug!("HTTP: GET {}/{}", self.resource.base_path, path);

        let request = self
            .resource
            .get(path)
            .headers(&[("Accept", "application/json")]);

        Self::send_request_parse_response(request).await
    }

    /// Serialize data to JSON, send POST request, deserialize JSON response
    pub async fn post<T: Serialize, U: DeserializeOwned>(
        &mut self,
        path: &str,
        data: &T,
    ) -> Result<U, Error> {
        let body = Self::prepare_body(data)?;
        debug!(
            "HTTP: POST {}/{} ({} bytes)",
            self.resource.base_path,
            path,
            body.len()
        );

        let request = self
            .resource
            .post(path)
            .content_type(ContentType::ApplicationJson)
            .headers(&[("Accept", "application/json")])
            .body(body.as_ref());

        Self::send_request_parse_response(request).await
    }

    /// Serialize data to JSON, send POST request, streaming deserialize JSON response
    pub async fn post_fn<T: Serialize, U: DeserializeOwned, F: FnMut(String, U)>(
        &mut self,
        path: &str,
        data: &T,
        mut f: F,
    ) -> Result<(), Error> {
        let body = Self::prepare_body(data)?;
        debug!(
            "HTTP: POST {}/{} ({} bytes, streaming response)",
            self.resource.base_path,
            path,
            body.len()
        );

        let request = self
            .resource
            .post(path)
            .content_type(ContentType::ApplicationJson)
            .headers(&[("Accept", "application/json")])
            .body(body.as_ref());

        let mut rx_buf = vec![0; MAX_RESPONSE_HEADER_SIZE];
        let response = Self::send_request(request, &mut rx_buf).await?;
        let reader = response.body().reader();
        let mut stream = StreamingJsonObjectReader::<_, U, 2048>::new(reader);

        while let Some((key, value)) = stream.next().await? {
            f(key, value);
        }

        Ok(())
    }
}

impl Connection<'_> {
    /// Serialize data to JSON for request body
    pub fn prepare_body<T: Serialize>(data: &T) -> Result<Vec<u8>, Error> {
        let body = serde_json::to_vec(data).map_err(Error::MalformedRequest)?;
        Ok(body)
    }

    /// Send request, check response status and return deserialized JSON response
    /// Requires complete response body and parsed data to fit into memory
    async fn send_request_parse_response<'conn, B: RequestBody, T: DeserializeOwned>(
        request: HttpResourceRequestBuilder<'_, 'conn, TcpConnection<'conn>, B>,
    ) -> Result<T, Error> {
        // rx_buf is used to buffer response headers. The response body reader uses this only for
        // non-TLS connections. Body reader of TLS connections will use the TLS read_buffer for
        // buffering parts of the body. However, read_to_end will again always use this buffer.
        let mut rx_buf = vec![0; MAX_RESPONSE_HEADER_SIZE];
        let response = Self::send_request(request, &mut rx_buf).await?;
        let body = response.body().read_to_end().await?;
        serde_json::from_slice(body).map_err(Error::MalformedResponse)
    }

    /// Send request, check response status and return response
    async fn send_request<'req, 'conn, 'buf, B: RequestBody>(
        request: HttpResourceRequestBuilder<'req, 'conn, TcpConnection<'conn>, B>,
        rx_buf: &'buf mut [u8],
    ) -> Result<Response<'req, 'buf, HttpConnection<'conn, TcpConnection<'conn>>>, Error> {
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

        Ok(response)
    }
}
