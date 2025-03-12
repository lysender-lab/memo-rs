use core::result::Result;
use validator::ValidationError;

pub fn anyname(value: &str) -> Result<(), ValidationError> {
    if value.len() == 0 {
        return Err(ValidationError::new("anyname"));
    }
    let mut valid = true;
    let mut spaces: i32 = 0;

    for (k, c) in value.chars().enumerate() {
        // Should be alphanumeric or dash or a space or underscore
        if !c.is_alphanumeric() && c != '-' && c != ' ' && c != '_' {
            valid = false;
            break;
        }
        // Should not start or end with a space
        if (k == 0 && c == ' ') || (k == value.len() - 1 && c == ' ') {
            valid = false;
            break;
        }
        // Should not have consecutive dashes
        if c == ' ' {
            spaces += 1;
            if spaces > 1 {
                valid = false;
                break;
            }
        } else {
            spaces = 0;
        }
    }

    match valid {
        true => Ok(()),
        false => Err(ValidationError::new("anyname")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anyname() {
        assert!(anyname("hello-world").is_ok());
        assert!(anyname("Hello World_123-").is_ok());
        assert!(anyname("hello world?").is_err());
        assert!(anyname(" hello-world").is_err());
        assert!(anyname("hello-world ").is_err());
        assert!(anyname("hello  world").is_err());
        assert!(anyname("").is_err());
    }
}
