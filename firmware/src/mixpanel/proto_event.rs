use alloc::borrow::Cow;
use alloc::string::String;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{TimestampMilliSeconds, serde_as};

/// `track` request
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct TrackRequest<'a, T: Serialize> {
    pub events: &'a [Event<'a, T>],
}

/// `track` response
#[derive(Debug, Deserialize)]
pub struct TrackResponse {
    pub error: Option<String>,
    pub status: u32,
}

/// Event
#[derive(Debug, Serialize)]
pub struct Event<'a, P: Serialize> {
    pub event: &'a str,
    pub properties: EventProperties<'a, P>,
}

/// Event properties
#[serde_as]
#[derive(Debug, Serialize)]
pub struct EventProperties<'a, P: Serialize> {
    // Reserved properties, see https://docs.mixpanel.com/docs/data-structure/property-reference/reserved-properties
    pub token: &'a str,
    #[serde_as(as = "TimestampMilliSeconds<i64>")]
    pub time: DateTime<Utc>,
    // FIXME: distinct_id should be &str, but that's a lot harder, e.g. with number types
    pub distinct_id: Cow<'a, str>,

    // User-defined properties (flattened)
    #[serde(flatten)]
    pub extra: P,
}

impl<'a, P: Serialize> Event<'a, P> {
    /// Create event with given properties
    pub fn new(
        name: &'a str,
        token: &'a str,
        time: DateTime<Utc>,
        distinct_id: impl Into<Cow<'a, str>>,
        extra: P,
    ) -> Self {
        Self {
            event: name,
            properties: EventProperties {
                token,
                time,
                distinct_id: distinct_id.into(),
                extra,
            },
        }
    }
}
