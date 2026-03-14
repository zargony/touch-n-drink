use crate::nfc::Uid;
use crate::user::UserId;
use alloc::string::String;
use alloc::vec::Vec;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, TimestampMilliSeconds, serde_as};

/// `track` request
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct TrackRequest<'a> {
    pub events: Vec<Event<'a>>,
}

/// `track` response
#[derive(Debug, Deserialize)]
pub struct TrackResponse {
    pub error: Option<String>,
    pub status: u32,
}

/// Event
#[derive(Debug, Serialize)]
pub struct Event<'a> {
    pub event: &'a str,
    pub properties: EventProperties<'a>,
}

/// Event properties
#[serde_as]
#[derive(Debug, Serialize)]
pub struct EventProperties<'a> {
    // Reserved properties, see https://docs.mixpanel.com/docs/data-structure/property-reference/reserved-properties
    pub token: &'a str,
    #[serde_as(as = "TimestampMilliSeconds<i64>")]
    pub time: DateTime<Utc>,
    pub distinct_id: DistinctId<'a>,

    // Global custom properties
    pub firmware_version: &'a str,
    pub firmware_git_sha: &'a str,
    pub device_id: &'a str,

    // Event-specific custom properties
    #[serde(flatten)]
    pub extra: EventPropertiesExtra<'a>,
}

/// Distinct id is either a user id (if present) or the device id
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum DistinctId<'a> {
    User(UserId),
    Device(&'a str),
}

/// Event-specific custom properties
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum EventPropertiesExtra<'a> {
    None,
    DataRefresh(EventPropertiesExtraDataRefresh),
    Authentication(EventPropertiesExtraAuthentication<'a>),
    Purchase(EventPropertiesExtraPurchase<'a>),
    Error(EventPropertiesExtraError<'a>),
}

/// Event-specific custom properties (data refresh)
#[derive(Debug, Serialize)]
#[expect(clippy::struct_field_names)]
pub struct EventPropertiesExtraDataRefresh {
    pub article_count: usize,
    pub uid_count: usize,
    pub user_count: usize,
}

// Event-specific custom properties (authentication)
#[serde_as]
#[derive(Debug, Serialize)]
pub struct EventPropertiesExtraAuthentication<'a> {
    #[serde_as(as = "DisplayFromStr")]
    pub uid: &'a Uid,
}

/// Event-specific custom properties (purchase)
#[derive(Debug, Serialize)]
pub struct EventPropertiesExtraPurchase<'a> {
    pub article_id: &'a str,
    pub amount: f32,
    pub total_price: f32,
}

/// Event-specific custom properties (error)
#[derive(Debug, Serialize)]
pub struct EventPropertiesExtraError<'a> {
    pub error_message: &'a str,
}
