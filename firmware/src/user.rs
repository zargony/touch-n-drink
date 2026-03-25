use crate::nfc::Uid;
use crate::vereinsflieger;
use alloc::collections::BTreeMap;
use alloc::string::String;
use core::str::FromStr;
use log::warn;

/// Extra NFC card uids to add
static EXTRA_UIDS: [(Uid, UserId); 2] = [
    // Test card #1 (Mifare Classic 1k)
    (Uid::Single([0x13, 0xbd, 0x5b, 0x2a]), 3),
    // Test token #1 (Mifare Classic 1k)
    (Uid::Single([0xb7, 0xd3, 0x65, 0x26]), 3),
];

/// User id
/// Equivalent to the Vereinsflieger `memberid` attribute
pub type UserId = u32;

/// User information
#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub name: String,
}

/// User lookup table
/// Provides a look up of user information (member id and name) by NFC uid.
#[derive(Debug)]
pub struct Users {
    /// Look up NFC uid to user id
    uids: BTreeMap<Uid, UserId>,
    /// Look up user id to user information
    users: BTreeMap<UserId, User>,
}

impl Users {
    /// Create new user lookup table
    pub fn new() -> Self {
        let mut this = Self {
            uids: BTreeMap::new(),
            users: BTreeMap::new(),
        };
        this.clear();
        this
    }

    /// Clear all user information
    pub fn clear(&mut self) {
        self.uids.clear();
        self.users.clear();

        // Add extra uids and user for testing
        for (uid, id) in &EXTRA_UIDS {
            self.uids.insert(uid.clone(), *id);
            self.users.entry(*id).or_insert_with(|| User {
                name: String::from("Test-User"),
            });
        }
    }

    /// Add/update NFC uid to point to given user id
    pub fn update_uid(&mut self, uid: Uid, id: UserId) {
        self.uids.insert(uid, id);
    }

    /// Add/update user with given user id
    pub fn update_user(&mut self, id: UserId, name: String) {
        self.users.insert(id, User { name });
    }

    /// Add/update NFC uids and user using the given Vereinsflieger user
    pub fn update_vereinsflieger_user(&mut self, user: &vereinsflieger::User) {
        if !user.is_retired() {
            let mut has_valid_keys = false;
            // TODO: get prefix from system configuration
            for key in user.keys_named_with_prefix("NFC Transponder") {
                if let Ok(uid) = Uid::from_str(key) {
                    self.update_uid(uid, user.memberid);
                    has_valid_keys = true;
                } else {
                    warn!(
                        "Ignoring user key with invalid NFC uid ({}): {}",
                        user.memberid, key
                    );
                }
            }
            if has_valid_keys {
                self.update_user(user.memberid, user.firstname.clone());
            }
        }
    }

    /// Number of uids
    pub fn count_uids(&self) -> usize {
        self.uids.len()
    }

    /// Number of users
    pub fn count(&self) -> usize {
        self.users.len()
    }

    /// Look up user by user id
    pub fn get(&self, id: UserId) -> Option<&User> {
        self.users.get(&id)
    }

    /// Look up user by NFC uid
    pub fn get_by_uid(&self, uid: &Uid) -> Option<(UserId, &User)> {
        let id = self.uids.get(uid).copied()?;
        let user = self.get(id)?;
        Some((id, user))
    }
}
