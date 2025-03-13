use core::result::Result;
use std::collections::HashSet;
use validator::ValidationError;

use super::sluggable;

pub fn csvname(value: &str) -> Result<(), ValidationError> {
    if value.len() == 0 {
        return Err(ValidationError::new("csvname"));
    }

    let chunks: Vec<&str> = value.split(",").collect();
    let chunks_len = chunks.len();
    if chunks_len == 0 {
        return Err(ValidationError::new("csvname"));
    }

    // Validate that all parts are sluggable
    let valid = chunks.iter().all(|chunk| {
        if chunk.len() == 0 {
            return false;
        }
        sluggable(chunk).is_ok()
    });

    // Should contain no duplicate
    let list: HashSet<&str> = chunks.into_iter().collect();
    if list.len() != chunks_len {
        return Err(ValidationError::new("csvname"));
    }

    match valid {
        true => Ok(()),
        false => Err(ValidationError::new("csvname")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csvname() {
        assert!(csvname("hello-world").is_ok());
        assert!(csvname("Hello-World-123").is_ok());
        assert!(csvname("hello-world,other-world-bruh").is_ok());
        assert!(csvname("Hello-World-123,this,that").is_ok());
        assert!(csvname(",").is_err());
        assert!(csvname(",,").is_err());
        assert!(csvname("").is_err());
        assert!(csvname("foo,bar,baz").is_ok());
        assert!(csvname("foo,bar,foo").is_err());
    }
}
