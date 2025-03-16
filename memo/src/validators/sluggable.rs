use core::result::Result;
use validator::ValidationError;

pub fn sluggable(value: &str) -> Result<(), ValidationError> {
    if value.len() == 0 {
        return Err(ValidationError::new("sluggable"));
    }
    let mut valid = true;
    let mut dashes: i32 = 0;

    for (k, c) in value.chars().enumerate() {
        // Should be alphanumeric or dash
        if !c.is_alphanumeric() && c != '-' {
            valid = false;
            break;
        }
        // Should not start or end with a dash
        if (k == 0 && c == '-') || (k == value.len() - 1 && c == '-') {
            valid = false;
            break;
        }
        // Should not have consecutive dashes
        if c == '-' {
            dashes += 1;
            if dashes > 1 {
                valid = false;
                break;
            }
        } else {
            dashes = 0;
        }
    }

    match valid {
        true => Ok(()),
        false => Err(ValidationError::new("sluggable")),
    }
}

#[cfg(test)]
mod tests {
    use crate::validators::flatten_errors;

    use super::*;
    use validator::Validate;

    #[derive(Debug, Clone, Validate)]
    pub struct TestStruct {
        #[validate(length(min = 1, max = 50))]
        pub name: String,

        #[validate(length(min = 1, max = 100))]
        pub label: String,
    }

    #[test]
    fn test_sluggable() {
        assert!(sluggable("hello-world").is_ok());
        assert!(sluggable("Hello-World-123").is_ok());
        assert!(sluggable("hello_world").is_err());
        assert!(sluggable("-hello-world").is_err());
        assert!(sluggable("hello-world-").is_err());
        assert!(sluggable("hello--world").is_err());
        assert!(sluggable("hello world").is_err());
        assert!(sluggable("").is_err());
    }

    #[test]
    fn test_flatten_errors() {
        let data = TestStruct {
            name: "".to_string(),
            label: "".to_string(),
        };
        let errors = data.validate().unwrap_err();
        let flattened = flatten_errors(&errors);
        assert_eq!(
            flattened,
            "label: must be between 1 and 100 characters, name: must be between 1 and 50 characters"
        );
    }
}
