use base64::prelude::*;
use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, Generate, Key, KeyInit},
};
use snafu::{OptionExt, ResultExt, ensure};

use crate::{
    Error, Result,
    error::{CipherSnafu, DecodeSnafu},
};

const DEFAULT_ENC_METHOD: &'static str = "chacha20";

pub fn generate_key() -> String {
    let key = Key::<ChaCha20Poly1305>::generate();
    BASE64_STANDARD.encode(key)
}

/// Encrypts data with a random key which is encrypted with the master key
/// Result format: enc_method:key_nonce:key_data|enc_method:input_nonce:input_data
pub fn encrypt(master_key: &str, data: &str) -> Result<String> {
    let master_key_bin = BASE64_STANDARD.decode(master_key).context(DecodeSnafu)?;
    let Ok(master_kb) = Key::<ChaCha20Poly1305>::try_from(master_key_bin.as_slice()) else {
        return Err(Error::Cipher {
            msg: "Master key is not valid".to_string(),
        });
    };

    // Create a random key and encrypt it with the main key
    let random_key = Key::<ChaCha20Poly1305>::generate();
    let cipher_key = chacha20_encrypt(master_kb, &random_key)?;

    // Now that we have a random key encrypted, encypt the data with it
    let cipher_data = chacha20_encrypt(random_key, data.as_bytes())?;
    Ok(format!("{}|{}", cipher_key, cipher_data))
}

/// Encrypts data with the provided key
/// Result format: enc_method:nonce:data
fn chacha20_encrypt(key: Key<ChaCha20Poly1305>, data: &[u8]) -> Result<String> {
    let c = ChaCha20Poly1305::new(&key);
    let nonce = Nonce::generate();
    match c.encrypt(&nonce, data) {
        Ok(res) => {
            let bres = BASE64_STANDARD.encode(res);
            let bnonce = BASE64_STANDARD.encode(nonce);
            Ok(format!("{}:{}:{}", DEFAULT_ENC_METHOD, bnonce, bres))
        }
        Err(e) => Err(Error::Cipher { msg: e.to_string() }),
    }
}

pub fn decrypt(master_key: &str, data: &str) -> Result<String> {
    let master_key_bin = BASE64_STANDARD.decode(master_key).context(DecodeSnafu)?;
    let Ok(master_kb) = Key::<ChaCha20Poly1305>::try_from(master_key_bin.as_slice()) else {
        return Err(Error::Cipher {
            msg: "Master key is not valid".to_string(),
        });
    };

    let mut chunks = data.split('|');
    let key_part = chunks.next().context(CipherSnafu {
        msg: "Cipher text format must be valid",
    })?;

    let data_part = chunks.next().context(CipherSnafu {
        msg: "Cipher text format must be valid",
    })?;

    // Decrypt the key first
    let data_key = decrypt_part(master_kb, key_part)?;
    let Ok(data_kb) = Key::<ChaCha20Poly1305>::try_from(data_key.as_slice()) else {
        return Err(Error::Cipher {
            msg: "Master key is not valid".to_string(),
        });
    };

    // Decrypt the data using the stored key
    let result = decrypt_part(data_kb, data_part)?;
    Ok(String::from_utf8_lossy(&result).to_string())
}

fn decrypt_part(key: Key<ChaCha20Poly1305>, data: &str) -> Result<Vec<u8>> {
    let mut chunks = data.split(':');
    let method = chunks.next().context(CipherSnafu {
        msg: "Cipher text part format must be valid",
    })?;

    ensure!(
        method == DEFAULT_ENC_METHOD,
        CipherSnafu {
            msg: "Encryption method not supported"
        }
    );

    let nonce = chunks.next().context(CipherSnafu {
        msg: "Cipher text part format must be valid",
    })?;
    let data_part = chunks.next().context(CipherSnafu {
        msg: "Cipher text part format must be valid",
    })?;

    chacha20_decrypt(
        key,
        &BASE64_STANDARD.decode(nonce).context(DecodeSnafu)?,
        &BASE64_STANDARD.decode(data_part).context(DecodeSnafu)?,
    )
}

/// Decrypts the data using the provided key and nonce using default enc method
fn chacha20_decrypt(key: Key<ChaCha20Poly1305>, nonce: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    let c = ChaCha20Poly1305::new(&key);
    let Ok(bnonce) = Nonce::try_from(nonce) else {
        return Err(Error::Cipher {
            msg: "Nonce is not valid".to_string(),
        });
    };

    match c.decrypt(&bnonce, data) {
        Ok(res) => Ok(res),
        Err(e) => Err(Error::Cipher { msg: e.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = generate_key();
        let plain = "the quick brown fox jumps over the lazy dog under the mango tree the quick brown fox jumps";

        let crypted = encrypt(&key, plain).unwrap();
        let plain_back = decrypt(&key, &crypted).unwrap();
        assert_eq!(plain, plain_back);
    }
}
