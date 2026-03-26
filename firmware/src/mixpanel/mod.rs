mod proto_event;

pub use self::proto_event::Event;

use crate::util::DisplayOption;
use alloc::vec;
use derive_more::{Display, From};
use embassy_time::{Duration, Instant, with_deadline, with_timeout};
use embedded_nal_async::{Dns, TcpConnect};
use log::debug;
use reqwless::client::{HttpClient, HttpResource};
use reqwless::headers::ContentType;
use reqwless::request::RequestBuilder;
use reqwless::response::StatusCode;
use serde::{Serialize, de::DeserializeOwned};

/// Mixpanel API base URL
const BASE_URL: &str = "https://api-eu.mixpanel.com";

/// How long to wait for a server response
const TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum size of response headers from server
const MAX_RESPONSE_HEADER_SIZE: usize = 2048;

/// Mixpanel API error
#[derive(Debug, Display, From)]
pub enum Error {
    /// Network error
    #[from]
    #[display("Network: {_0}")]
    Network(reqwless::Error),
    /// Request could not be built
    #[display("Malformed request")]
    MalformedRequest(serde_json::Error),
    /// Request failed
    #[display("Request failed: ({})", _0.0)]
    RequestFailed(StatusCode),
    /// Response could not be parsed
    #[display("Malformed response")]
    MalformedResponse(serde_json::Error),
    /// Timeout waiting for response
    #[from(embassy_time::TimeoutError)]
    #[display("Timeout")]
    Timeout,
}

/// Mixpanel API client
pub struct Mixpanel<'a> {
    token: &'a str,
}

impl<'a> Mixpanel<'a> {
    /// Create new Mixpanel API client using the given project token
    pub fn new(token: &'a str) -> Self {
        Self { token }
    }

    /// Connect to API server
    pub async fn connect<'conn, T: TcpConnect, D: Dns>(
        &'conn mut self,
        http: &'conn mut HttpClient<'_, T, D>,
    ) -> Result<Connection<'conn, T>, Error> {
        let resource = with_timeout(TIMEOUT, http.resource(BASE_URL)).await??;
        debug!("Mixpanel: Connected {BASE_URL}");

        Ok(Connection { resource })
    }

    /// Mixpanel API token
    pub fn token(&self) -> &str {
        self.token
    }
}

/// Mixpanel API client connection
pub struct Connection<'conn, T: TcpConnect + 'conn> {
    resource: HttpResource<'conn, T::Connection<'conn>>,
}

impl<T: TcpConnect> Connection<'_, T> {
    /// Submit tracked events
    pub async fn submit<P: Serialize>(&mut self, events: &[Event<'_, P>]) -> Result<(), Error> {
        use proto_event::{TrackRequest, TrackResponse};

        debug!("Mixpanel: Submitting {} events...", events.len());
        let response: TrackResponse = self
            .http_json_post("track?verbose=1", &TrackRequest { events })
            .await?;
        debug!(
            "Mixpanel: Submit successul, status {} {}",
            response.status,
            DisplayOption(response.error),
        );
        Ok(())
    }
}

impl<T: TcpConnect> Connection<'_, T> {
    /// Serialize data to JSON, send HTTP POST request, deserialize JSON response
    async fn http_json_post<D: Serialize, R: DeserializeOwned>(
        &mut self,
        path: &str,
        data: &D,
    ) -> Result<R, Error> {
        let deadline = Instant::now() + TIMEOUT;
        let mut rx_buf = vec![0; MAX_RESPONSE_HEADER_SIZE].into_boxed_slice();

        let body = serde_json::to_vec(data)
            .map_err(Error::MalformedRequest)?
            .into_boxed_slice();

        let request = self
            .resource
            .post(path)
            .content_type(ContentType::ApplicationJson)
            .headers(&[("Accept", "application/json")])
            .body(body.as_ref());

        debug!("Mixpanel: POST {BASE_URL}/{path} ({} bytes)", body.len());
        let response = with_deadline(deadline, request.send(&mut rx_buf)).await??;

        debug!("Mixpanel: HTTP status {}", response.status.0);
        if !response.status.is_successful() {
            return Err(Error::RequestFailed(response.status));
        }

        let body = with_deadline(deadline, response.body().read_to_end()).await??;

        serde_json::from_slice(body).map_err(Error::MalformedResponse)
    }
}
