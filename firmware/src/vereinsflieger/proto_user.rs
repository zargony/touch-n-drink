use super::AccessToken;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

/// `user/list` request
#[derive(Debug, Serialize)]
pub struct UserListRequest<'a> {
    pub accesstoken: &'a AccessToken,
}

// /// `user/list` response
// #[derive(Debug, Deserialize)]
// pub struct UserListResponse {
//     #[serde(flatten)]
//     pub users: BTreeMap<String, User>,
//     // pub httpstatuscode: u16,
// }

/// User
#[serde_as]
#[derive(Debug, Deserialize)]
pub struct User {
    // #[serde_as(as = "DisplayFromStr")]
    // pub uid: u32,
    // pub title: String,
    pub firstname: String,
    // pub lastname: String,
    // pub careof: String,
    // pub street: String,
    // pub postofficebox: String, // undocumented
    // pub zipcode: String,
    // pub town: String,
    // pub email: String,
    // pub gender: String,
    // pub birthday: String, // "dd.mm.yyyy"
    // pub birthplace: String,
    // pub homenumber: String,
    // pub mobilenumber: String,
    // pub phonenumber: String,
    // pub phonenumber2: String,
    // pub carlicenseplate: String,
    // pub identification: String,
    // pub natoid: String,
    // pub policecert_validto: String, // "yyyy-mm-dd"
    // pub ice_contact1: String,
    // pub ice_contact2: String,
    #[serde_as(as = "DisplayFromStr")]
    pub memberid: u32,
    // pub msid: String, // undocumented
    // pub memberbegin: String, // "dd.mm.yyyy"
    // pub memberend: String, // "yyyy-mm-dd"
    // pub lettertitle: String,
    // pub cid: String, // undocumented
    // pub nickname: String, // undocumented
    // pub clid: String, // undocumented
    // pub flightrelease: String, // undocumented
    // pub flightreleasevalidto: String, // undocumented "yyyy-mm-dd"
    // pub flightdiscount: String, // undocumented
    // pub flightdiscount2: String, // undocumented
    // pub flightdiscount3: String, // undocumented
    // pub flightdiscount4: String, // undocumented
    // pub flightdiscount5: String, // undocumented
    // pub flightdiscount6: String, // undocumented
    pub memberstatus: String,
    // pub country: String,
    // pub bankaccountname: String,
    // pub bankaccountinfo: String, // undocumented
    // #[serde_as(as = "DisplayFromStr")]
    // pub directdebitauth: u32,
    // pub iban: String,
    // pub bic: String,
    // pub mandate: String,
    // pub roles: Vec<String>,
    // pub mandatedate: String, // "yyyy-mm-dd"
    // #[serde_as(as = "DisplayFromStr")]
    // pub mailrecipient: u32,
    // pub sector: Vec<String>,
    // pub functions: Vec<String>,
    // pub educations: Vec<String>,
    // pub prop0: [String, String],
    // pub prop1: [String, String],
    // pub prop2: [String, String],
    // pub accounts: Vec<UserAccountDescription>,
    pub keymanagement: Vec<Key>,
    // pub stateassociation: Vec<String>,
    // pub key1designation: String, // undocumented
    // pub key2designation: String, // undocumented
    // pub keyrfid: String, // undocumented
    // pub whtodo: UserWhTodoList, // undocumented
}

impl User {
    /// Whether the user has retired ("ausgeschieden")
    pub fn is_retired(&self) -> bool {
        self.memberstatus.to_lowercase().contains("ausgeschieden")
    }

    /// Get key numbers with the given label prefix
    pub fn keys_named_with_prefix(&self, prefix: &str) -> Vec<&str> {
        self.keymanagement
            .iter()
            .filter(|key| key.title.starts_with(prefix))
            .map(|key| key.keyname.as_str())
            .collect()
    }
}

/// User keymanagement
#[serde_as]
#[derive(Debug, Deserialize)]
pub struct Key {
    /// Key label
    pub title: String,
    /// Key number
    pub keyname: String,
    // #[serde_as(as = "DisplayFromStr")]
    // pub rfidkey: u32, // undocumented
}
