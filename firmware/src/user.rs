use crate::nfc::Uid;
use alloc::collections::BTreeMap;
use alloc::string::String;

/// Extra NFC card uids to add
static EXTRA_UIDS: [(Uid, UserId); 2] = [
    // Test card #1 (Mifare Classic 1k)
    (Uid::Single([0x13, 0xbd, 0x5b, 0x2a]), 3),
    // Test token #1 (Mifare Classic 1k)
    (Uid::Single([0xb7, 0xd3, 0x65, 0x26]), 3),
];

/// User id
/// Equivalent to the Vereinsflieger `memberid` attribute
#[allow(clippy::module_name_repetitions)]
pub type UserId = u32;

/// User information
#[derive(Debug, Clone, PartialEq)]
pub struct User {
    // pub uids: Vec<Uid>,
    // pub id: UserId,
    pub name: String,
}

/// User lookup table
/// Provides a look up of user information (member id and name) by NFC uid.
#[derive(Debug)]
pub struct Users {
    /// Look up NFC uid to user id
    uids: BTreeMap<Uid, UserId>,
    /// Look up user id to user details
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

    /// Add/update NFC uid for given user id
    pub fn update_uid(&mut self, uid: Uid, id: UserId) {
        self.uids.insert(uid, id);
    }

    /// Add/update user with given user id
    pub fn update_user(&mut self, id: UserId, name: String) {
        self.users.insert(id, User { name });
    }

    /// Number of uids
    #[allow(dead_code)]
    pub fn count_uids(&self) -> usize {
        self.uids.len()
    }

    /// Number of users
    pub fn count(&self) -> usize {
        self.users.len()
    }

    /// Look up user id by NFC uid
    pub fn id(&self, uid: &Uid) -> Option<UserId> {
        self.uids.get(uid).copied()
    }

    /// Look up user by user id
    pub fn get(&self, id: UserId) -> Option<&User> {
        self.users.get(&id)
    }
}
