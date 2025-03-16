use core::result::Result;
use validator::ValidationError;

pub fn alphanumeric(value: &str) -> Result<(), ValidationError> {
    if value.len() == 0 {
        return Err(ValidationError::new("alphanumeric"));
    }
    match value.chars().all(char::is_alphanumeric) {
        true => Ok(()),
        false => Err(ValidationError::new("alphanumeric")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alphanumeric() {
        assert!(alphanumeric("helloworld").is_ok());
        assert!(alphanumeric("HelloWorld123").is_ok());
        assert!(alphanumeric("hello world").is_err());
        assert!(alphanumeric("hello_world").is_err());
        assert!(alphanumeric("-hello-world").is_err());
        assert!(alphanumeric("").is_err());
    }
}
