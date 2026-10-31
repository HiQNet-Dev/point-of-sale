use std::fmt;

use super::*;

/// Identificador UUID canónico y estable asignado por la aplicación.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityId(String);

impl EntityId {
    /// Valida un UUID textual canónico en minúsculas.
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        let bytes = value.as_bytes();
        if bytes.len() != 36
            || ![8, 13, 18, 23]
                .into_iter()
                .all(|index| bytes[index] == b'-')
            || !bytes.iter().enumerate().all(|(index, byte)| {
                [8, 13, 18, 23].contains(&index)
                    || byte.is_ascii_digit()
                    || (b'a'..=b'f').contains(byte)
            })
        {
            return Err(DomainError::InvalidUuid);
        }
        Ok(Self(value.to_owned()))
    }

    /// Devuelve la representación canónica persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
