mod proto_event;

use crate::http::{self, Http};
use crate::telemetry::Event;
use crate::time::{self, DateTimeExt};
use crate::{GIT_SHA_STR, VERSION_STR};
use alloc::vec::Vec;
use core::fmt;
use embassy_time::{Duration, Instant, with_timeout};
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
pub struct Connection<'conn> {
    http: http::Connection<'conn>,
    token: &'conn str,
    device_id: &'conn str,
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
                    events: self.events(events)?,
                },
            ),
        )
        .await?
        .map_err(Error::Submit)?;
        debug!(
            "Mixpanel: Submit successul, status {} {}",
            response.status,
            response.error.as_deref().unwrap_or_default(),
        );
        Ok(())
    }
}

impl<'conn> Connection<'conn> {
    /// Connect to API server
    async fn new(mp: &'conn Mixpanel<'_>, http: &'conn mut Http<'_>) -> Result<Self, Error> {
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

    /// Helper function to create a list of tracking events from telemetry events
    fn events<'a>(
        &self,
        events: &'a [(Instant, Event)],
    ) -> Result<Vec<proto_event::Event<'a>>, Error>
    where
        'conn: 'a,
    {
        events
            .iter()
            .map(|(time, event)| self.event(*time, event))
            .collect()
    }

    /// Helper function to create a tracking event from a telemetry event
    fn event<'a>(&self, time: Instant, event: &'a Event) -> Result<proto_event::Event<'a>, Error>
    where
        'conn: 'a,
    {
        use proto_event::{
            DistinctId, EventProperties, EventPropertiesExtra, EventPropertiesExtraAuthentication,
            EventPropertiesExtraDataRefresh, EventPropertiesExtraError,
            EventPropertiesExtraPurchase,
        };

        Ok(proto_event::Event {
            event: event.event_name(),
            properties: EventProperties {
                token: self.token,
                // Convert relative `Instant` time to absolute `DateTime` (needs current time set)
                time: time.to_datetime().ok_or(Error::CurrentTimeNotSet)?,
                distinct_id: match event.user_id() {
                    Some(user_id) => DistinctId::User(user_id),
                    None => DistinctId::Device(self.device_id),
                },
                firmware_version: VERSION_STR,
                firmware_git_sha: GIT_SHA_STR,
                device_id: self.device_id,
                extra: match event {
                    Event::SystemStart => EventPropertiesExtra::None,
                    Event::DataRefreshed(article_count, uid_count, user_count) => {
                        EventPropertiesExtra::DataRefresh(EventPropertiesExtraDataRefresh {
                            article_count: *article_count,
                            uid_count: *uid_count,
                            user_count: *user_count,
                        })
                    }
                    Event::AuthenticationFailed(uid) | Event::UserAuthenticated(_, uid) => {
                        EventPropertiesExtra::Authentication(EventPropertiesExtraAuthentication {
                            uid,
                        })
                    }
                    Event::ArticlePurchased(_user_id, article_id, amount, total_price) => {
                        EventPropertiesExtra::Purchase(EventPropertiesExtraPurchase {
                            article_id,
                            amount: *amount,
                            total_price: *total_price,
                        })
                    }
                    Event::Error(_user_id, error_message) => {
                        EventPropertiesExtra::Error(EventPropertiesExtraError { error_message })
                    }
                },
            },
        })
    }
}
