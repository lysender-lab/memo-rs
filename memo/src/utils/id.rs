use uuid::Uuid;

pub fn generate_id() -> String {
    Uuid::now_v7().as_simple().to_string()
}

pub fn valid_id(id: &str) -> bool {
    let parsed = Uuid::parse_str(id);
    match parsed {
        Ok(val) => match val.get_version() {
            Some(uuid::Version::SortRand) => true,
            _ => return false,
        },
        Err(_) => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdPrefix {
    File,
    Dir,
    Any,
}

impl TryFrom<&str> for IdPrefix {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "fil" => Ok(Self::File),
            "dir" => Ok(Self::Dir),
            "any" => Ok(Self::Any),
            _ => Err(format!("Invalid ID Prefix: {value}")),
        }
    }
}

impl core::fmt::Display for IdPrefix {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::File => write!(f, "fil"),
            Self::Dir => write!(f, "dir"),
            Self::Any => write!(f, "any"),
        }
    }
}

pub fn generate_prefixed_id(prefix: IdPrefix) -> String {
    format!("{}_{}", prefix, Uuid::now_v7().as_simple())
}

pub fn valid_prefixed_id(id: &str) -> bool {
    if id.len() != 36 {
        return false;
    }

    let Some((prefix, raw_uuid)) = id.split_once('_') else {
        return false;
    };

    if IdPrefix::try_from(prefix).is_err() {
        return false;
    }

    let parsed = Uuid::parse_str(raw_uuid);
    match parsed {
        Ok(val) => matches!(val.get_version(), Some(uuid::Version::SortRand)),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        // Should be a 32-character uuid string
        let id = generate_id();
        assert_eq!(id.len(), 32);

        // Can be parsed back as uuid
        assert_eq!(valid_id(id.as_str()), true);
    }

    #[test]
    fn test_generate_prefixed_id() {
        // Should be a 36-character prefixed uuid string
        let id = generate_prefixed_id(IdPrefix::File);
        assert_eq!(id.len(), 36);
        assert!(id.starts_with("fil_"));

        // Can be parsed back as uuid
        assert_eq!(valid_prefixed_id(id.as_str()), true);
    }

    #[test]
    fn test_id_prefix_mapping() {
        assert_eq!(IdPrefix::File.to_string(), "fil");
        assert_eq!(IdPrefix::Dir.to_string(), "dir");
        assert_eq!(IdPrefix::Any.to_string(), "any");
    }

    #[test]
    fn test_invalid_prefix() {
        let id = generate_prefixed_id(IdPrefix::File);
        let invalid = id.replacen("fil_", "bad_", 1);
        assert!(!valid_id(invalid.as_str()));
    }
}
