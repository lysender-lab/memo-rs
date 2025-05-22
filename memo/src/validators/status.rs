use core::result::Result;
use validator::ValidationError;

pub fn status(value: &str) -> Result<(), ValidationError> {
    if value.len() == 0 {
        return Err(ValidationError::new("status"));
    }
    match value {
        "active" | "inactive" => Ok(()),
        _ => Err(ValidationError::new("status")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status() {
        assert!(status("active").is_ok());
        assert!(status("inactive").is_ok());
        assert!(status("active-inactive").is_err());
        assert!(status("").is_err());
    }
}
