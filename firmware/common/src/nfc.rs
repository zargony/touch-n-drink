use const_hex::FromHex;
use core::fmt;
use core::str::FromStr;
use derive_more::Display;

/// NFC UID Error
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
#[display("Invalid NFC UID")]
#[must_use]
pub struct InvalidUid;

/// NFC UID
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[must_use]
pub enum Uid {
    /// Single Size UID (4 bytes), Mifare Classic
    Single([u8; 4]),
    /// Double Size UID (7 bytes), NXP NTAG Series
    Double([u8; 7]),
    /// Triple Size UID (10 bytes), not used yet
    Triple([u8; 10]),
}

impl TryFrom<&[u8]> for Uid {
    type Error = InvalidUid;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        match bytes.len() {
            // Note: Always safe to unwrap because of matching length check
            4 => Ok(Self::Single(bytes.try_into().unwrap())),
            7 => Ok(Self::Double(bytes.try_into().unwrap())),
            10 => Ok(Self::Triple(bytes.try_into().unwrap())),
            _ => Err(InvalidUid),
        }
    }
}

impl FromStr for Uid {
    type Err = InvalidUid;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.len() {
            8 => {
                let bytes = <[u8; 4]>::from_hex(s).map_err(|_e| InvalidUid)?;
                Ok(Self::Single(bytes))
            }
            14 => {
                let bytes = <[u8; 7]>::from_hex(s).map_err(|_e| InvalidUid)?;
                Ok(Self::Double(bytes))
            }
            20 => {
                let bytes = <[u8; 10]>::from_hex(s).map_err(|_e| InvalidUid)?;
                Ok(Self::Triple(bytes))
            }
            _ => Err(InvalidUid),
        }
    }
}

fn write_hex_bytes(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for b in bytes {
        write!(f, "{:02x}", *b)?;
    }
    Ok(())
}

impl fmt::Display for Uid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(bytes) => write_hex_bytes(f, bytes),
            Self::Double(bytes) => write_hex_bytes(f, bytes),
            Self::Triple(bytes) => write_hex_bytes(f, bytes),
        }
    }
}

impl AsRef<[u8]> for Uid {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Single(bytes) => bytes,
            Self::Double(bytes) => bytes,
            Self::Triple(bytes) => bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[tokio::test]
    async fn parse_uid() {
        assert_eq!(
            "11223344".parse(),
            Ok(Uid::Single([0x11, 0x22, 0x33, 0x44]))
        );
        assert_eq!(
            "11223344556677".parse(),
            Ok(Uid::Double([0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]))
        );
        assert_eq!(
            "112233445566778899aa".parse(),
            Ok(Uid::Triple([
                0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa
            ]))
        );
        assert_eq!("foo".parse::<Uid>(), Err(InvalidUid));
    }

    #[tokio::test]
    async fn display_uid() {
        assert_eq!(
            Uid::Single([0x11, 0x22, 0x33, 0x44]).to_string(),
            "11223344"
        );
        assert_eq!(
            Uid::Double([0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]).to_string(),
            "11223344556677"
        );
        assert_eq!(
            Uid::Triple([0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa]).to_string(),
            "112233445566778899aa"
        );
    }
}
