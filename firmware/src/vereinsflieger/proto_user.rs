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
            _ => json.skip_any().await?,
        }
        Ok(())
    }
}

/// User
#[derive(Debug, Default)]
struct User {
    // uid: u32,
    // title: String,
    firstname: String,
    lastname: String,
    // careof: String,
    // street: String,
    // postofficebox: String, // undocumented
    // zipcode: String,
    // town: String,
    // email: String,
    // gender: String,
    // birthday: String, // "dd.mm.yyyy"
    // birthplace: String,
    // homenumber: String,
    // mobilenumber: String,
    // phonenumber: String,
    // phonenumber2: String,
    // carlicenseplate: String,
    // identification: String,
    // natoid: String,
    // policecert_validto: String, // "yyyy-mm-dd"
    // ice_contact1: String,
    // ice_contact2: String,
    memberid: u32,
    // msid: String, // undocumented
    // memberbegin: String, // "dd.mm.yyyy"
    // memberend: String, // "yyyy-mm-dd"
    // lettertitle: String,
    // cid: String, // undocumented
    // nickname: String, // undocumented
    // clid: String, // undocumented
    // flightrelease: String, // undocumented
    // flightreleasevalidto: String, // undocumented "yyyy-mm-dd"
    // flightdiscount: String, // undocumented
    // flightdiscount2: String, // undocumented
    // flightdiscount3: String, // undocumented
    // flightdiscount4: String, // undocumented
    // flightdiscount5: String, // undocumented
    // flightdiscount6: String, // undocumented
    memberstatus: String,
    // country: String,
    // bankaccountname: String,
    // bankaccountinfo: String, // undocumented
    // directdebitauth: u32,
    // iban: String,
    // bic: String,
    // mandate: String,
    // roles: Vec<String>,
    // mandatedate: String, // "yyyy-mm-dd"
    // mailrecipient: u32,
    // sector: Vec<String>,
    // functions: Vec<String>,
    // educations: Vec<String>,
    // prop0: [String, String],
    // prop1: [String, String],
    // prop2: [String, String],
    // accounts: Vec<UserAccountDescription>,
    keymanagement: Vec<Key>,
    // stateassociation: Vec<String>,
    // key1designation: String, // undocumented
    // key2designation: String, // undocumented
    // keyrfid: String, // undocumented
    // whtodo: UserWhTodoList, // undocumented
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
            _ => json.skip_any().await?,
        }
        Ok(())
    }
}

impl User {
    /// Whether the user has/was retired ("ausgeschieden")
    fn is_retired(&self) -> bool {
        self.memberstatus.to_lowercase().contains("ausgeschieden")
    }

    /// Get key numbers with the given label prefix
    fn keys_named_with_prefix(&self, prefix: &str) -> Vec<&str> {
        self.keymanagement
            .iter()
            .filter(|key| key.title.starts_with(prefix))
            .map(|key| key.keyname.as_str())
            .collect()
    }
}

/// User keymanagement
#[derive(Debug, Default)]
struct Key {
    /// Key label
    title: String,
    /// Key number
    keyname: String,
    // rfidkey: u32, // undocumented
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
            _ => json.skip_any().await?,
        }
        Ok(())
    }
}
