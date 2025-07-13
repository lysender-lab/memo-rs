use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

use crate::{
    Result,
    error::{HashPasswordSnafu, InvalidPasswordSnafu, VerifyPasswordHashSnafu},
};

pub fn hash_password(password: &str) -> Result<String> {
    let pwd = password.as_bytes();
    let salt = SaltString::generate(&mut OsRng);
    let gon = Argon2::default();
    match gon.hash_password(pwd, &salt) {
        Ok(hash) => Ok(hash.to_string()),
        Err(e) => HashPasswordSnafu {
            msg: format!("Error hashing password: {}", e),
        }
        .fail(),
    }
}

pub fn verify_password(password: &str, hash: &str) -> Result<()> {
    let Ok(parsed_hash) = PasswordHash::new(&hash) else {
        return VerifyPasswordHashSnafu {
            msg: "Invalid password hash".to_string(),
        }
        .fail();
    };
    let gone = Argon2::default();
    match gone.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(_) => Ok(()),
        Err(_) => InvalidPasswordSnafu {}.fail(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password() {
        let password = "password";
        let hash = hash_password(password).unwrap();
        assert!(hash.len() > 0);
    }

    #[test]
    fn test_verify_password() {
        let password = "password";
        let stored_hash = "$argon2id$v=19$m=19456,t=2,p=1$NxAcor94oNDtRqstYqRvmA$EtLJjVFPFz0hE5QLZ/ydx4Td4slp9GaXuwQX3vQU9Dc";

        let result = verify_password(password, &stored_hash);
        assert!(result.is_ok());

        // Try again
        let result = verify_password(password, &stored_hash);
        assert!(result.is_ok());
    }
}
