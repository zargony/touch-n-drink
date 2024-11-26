use super::AccessToken;
use crate::json::{self, FromJsonObject, ToJson};
use crate::nfc::Uid;
use crate::user::Users;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::str::FromStr;
use embedded_io_async::{BufRead, Write};
use log::warn;

/// `user/list` request
#[derive(Debug)]
pub struct UserListRequest<'a> {
    pub accesstoken: &'a AccessToken,
}

impl ToJson for UserListRequest<'_> {
    async fn to_json<W: Write>(
        &self,
        json: &mut json::Writer<W>,
    ) -> Result<(), json::Error<W::Error>> {
        json.write_object()
            .await?
            .field("accesstoken", self.accesstoken)
            .await?
            .finish()
            .await
    }
}

/// `user/list` response
#[derive(Debug, Default)]
pub struct UserListResponse {
    // pub *: User,
    // pub httpstatuscode: u16,
    //
    /// Total number of users
    pub total_users: u32,
}

impl FromJsonObject for UserListResponse {
    // Mutable reference to user lookup table
    type Context<'ctx> = RefCell<&'ctx mut Users>;

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        json: &mut json::Reader<R>,
        context: &Self::Context<'_>,
    ) -> Result<(), json::Error<R::Error>> {
        match u32::from_str(&key) {
            Ok(_key) => {
                let user: User = json.read().await?;
                self.total_users += 1;
                if !user.is_retired() {
                    let keys = user.keys_named_with_prefix("NFC Transponder");
                    if !keys.is_empty() {
                        // Instead of reading all users to a vector, this deserialization stores
                        // users directly to the user lookup table and only keeps the users needed,
                        // which heavily reduces memory consumption.
                        let mut users = context.borrow_mut();
                        for key in keys {
                            if let Ok(uid) = Uid::from_str(key) {
                                users.update_uid(uid, user.memberid);
                            } else {
                                warn!(
                                    "Ignoring user key with invalid NFC uid ({}): {}",
                                    user.memberid, key
                                );
                            }
                        }
                        users.update_user(user.memberid, user.firstname);
                    }
                }
            }
            _ => _ = json.read_any().await?,
        }
        Ok(())
    }
}

/// User
#[derive(Debug, Default)]
pub struct User {
    // pub uid: u32,
    // pub title: String,
    pub firstname: String,
    pub lastname: String,
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
    // pub directdebitauth: u32,
    // pub iban: String,
    // pub bic: String,
    // pub mandate: String,
    // pub roles: Vec<String>,
    // pub mandatedate: String, // "yyyy-mm-dd"
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

impl FromJsonObject for User {
    type Context<'ctx> = ();

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        json: &mut json::Reader<R>,
        _context: &Self::Context<'_>,
    ) -> Result<(), json::Error<R::Error>> {
        match &*key {
            "firstname" => self.firstname = json.read().await?,
            "lastname" => self.lastname = json.read().await?,
            "memberid" => self.memberid = json.read_any().await?.try_into()?,
            "memberstatus" => self.memberstatus = json.read().await?,
            "keymanagement" => self.keymanagement = json.read().await?,
            _ => _ = json.read_any().await?,
        }
        Ok(())
    }
}

impl User {
    /// Whether the user has/was retired ("ausgeschieden")
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
#[derive(Debug, Default)]
pub struct Key {
    /// Key label
    pub title: String,
    /// Key number
    pub keyname: String,
    // pub rfidkey: u32, // undocumented
}

impl FromJsonObject for Key {
    type Context<'ctx> = ();

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        json: &mut json::Reader<R>,
        _context: &Self::Context<'_>,
    ) -> Result<(), json::Error<R::Error>> {
        match &*key {
            "title" => self.title = json.read().await?,
            "keyname" => self.keyname = json.read().await?,
            _ => _ = json.read_any().await?,
        }
        Ok(())
    }
}
