use core::result::Result;
use validator::ValidationError;

use crate::utils::valid_id;

pub fn uuid(value: &str) -> Result<(), ValidationError> {
    if value.len() == 0 {
        return Err(ValidationError::new("uuid"));
    }
    match valid_id(value) {
        true => Ok(()),
        false => Err(ValidationError::new("uuid")),
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::generate_id;

    use super::*;

    #[test]
    fn test_valid() {
        let id = generate_id();
        assert!(uuid(id.as_str()).is_ok());
    }

    #[test]
    fn test_invalid() {
        assert!(uuid("hello").is_err());
        assert!(uuid("").is_err());
    }
}
