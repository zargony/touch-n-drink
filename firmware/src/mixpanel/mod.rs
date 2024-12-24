mod proto_event;

use crate::http::{self, Http};
use crate::telemetry::Event;
use crate::time;
use core::fmt;
use embassy_time::{with_timeout, Duration, Instant};
use log::{debug, warn};

/// Mixpanel API base URL
const BASE_URL: &str = "https://api-eu.mixpanel.com";

/// How long to wait for a server response
const TIMEOUT: Duration = Duration::from_secs(10);

/// Mixpanel API error
#[derive(Debug)]
pub enum Error {
    /// Current time is required but not set
    CurrentTimeNotSet,
    /// Failed to connect to API server
    Connect(http::Error),
    /// Failed to submit events to API server
    Submit(http::Error),
    /// Timeout waiting for response from API server
    Timeout,
}

impl From<embassy_time::TimeoutError> for Error {
    fn from(_err: embassy_time::TimeoutError) -> Self {
        Self::Timeout
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentTimeNotSet => write!(f, "Unknown current time"),
            Self::Connect(err) => write!(f, "Connect failed ({err})"),
            Self::Submit(err) => write!(f, "Submit failed ({err})"),
            Self::Timeout => write!(f, "Timeout"),
        }
    }
}

/// Mixpanel API client
#[derive(Debug)]
pub struct Mixpanel<'a> {
    token: &'a str,
    device_id: &'a str,
}

impl<'a> Mixpanel<'a> {
    /// Create new Mixpanel API client using the given project token
    pub fn new(token: &'a str, device_id: &'a str) -> Self {
        Self { token, device_id }
    }

    /// Connect to API server
    pub async fn connect<'conn>(
        &'conn mut self,
        http: &'conn mut Http<'_>,
    ) -> Result<Connection<'conn>, Error> {
        Connection::new(self, http).await
    }
}

/// Mixpanel API client connection
#[derive(Debug)]
pub struct Connection<'a> {
    http: http::Connection<'a>,
    token: &'a str,
    device_id: &'a str,
}

impl Connection<'_> {
    /// Submit tracked events
    pub async fn submit(&mut self, events: &[(Instant, Event)]) -> Result<(), Error> {
        use proto_event::{TrackRequest, TrackResponse};

        if time::now().is_none() {
            warn!("Mixpanel: Not submitting events. No current time set.");
            return Err(Error::CurrentTimeNotSet);
        }

        debug!("Mixpanel: Submitting {} events...", events.len());
        let response: TrackResponse = with_timeout(
            TIMEOUT,
            self.http.post(
                "track?verbose=1",
                &TrackRequest {
                    token: self.token,
                    device_id: self.device_id,
                    events,
                },
            ),
        )
        .await?
        .map_err(Error::Submit)?;
        debug!(
            "Mixpanel: Submit successul, status {} {}",
            response.status, response.error
        );
        Ok(())
    }
}

impl<'a> Connection<'a> {
    /// Connect to API server
    async fn new(mp: &'a Mixpanel<'_>, http: &'a mut Http<'_>) -> Result<Self, Error> {
        // Connect to API server
        let connection = with_timeout(TIMEOUT, http.connect(BASE_URL))
            .await?
            .map_err(Error::Connect)?;

        Ok(Self {
            http: connection,
            token: mp.token,
            device_id: mp.device_id,
        })
    }
}
