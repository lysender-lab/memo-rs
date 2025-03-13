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
}
