//! Cryptography and authentication for Minecraft network sessions (AES-128-CFB8, Mojang SHA-1, offline UUIDv3).

use aes::Aes128;
use cfb8::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use cfb8::{Decryptor, Encryptor};
use md5::Md5;
use sha1::{Digest, Sha1};

type AesCfb8Enc = Encryptor<Aes128>;
type AesCfb8Dec = Decryptor<Aes128>;

/// AES-128-CFB8 symmetric stream cipher pair for TCP socket encryption.
pub struct StreamCipherPair {
    encryptor: AesCfb8Enc,
    decryptor: AesCfb8Dec,
}

impl StreamCipherPair {
    /// Initializes cipher pair with 16-byte shared secret key and matching IV.
    pub fn new(key: &[u8; 16]) -> Self {
        let encryptor = AesCfb8Enc::new(key.into(), key.into());
        let decryptor = AesCfb8Dec::new(key.into(), key.into());
        Self {
            encryptor,
            decryptor,
        }
    }

    /// Encrypts byte slice in-place prior to TCP socket transmission.
    pub fn encrypt(&mut self, data: &mut [u8]) {
        let (blocks, _) = cfb8::cipher::inout::InOutBuf::from(data).into_chunks();
        self.encryptor.encrypt_blocks_inout_mut(blocks);
    }

    /// Decrypts byte slice in-place upon receiving from TCP socket.
    pub fn decrypt(&mut self, data: &mut [u8]) {
        let (blocks, _) = cfb8::cipher::inout::InOutBuf::from(data).into_chunks();
        self.decryptor.decrypt_blocks_inout_mut(blocks);
    }
}

/// Tinh toan chuoi bam ServerHash SHA-1 theo kieu Two's Complement Hex Digest cua Mojang
///
/// Formula:
/// $$\text{Digest} = \text{SHA-1}(\text{ServerID}_{\text{ASCII}} \parallel \text{SharedSecret}_{16\text{B}} \parallel \text{PublicKey}_{\text{DER}})$$
pub fn mojang_server_hash(server_id: &str, shared_secret: &[u8], public_key_der: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key_der);
    let digest = hasher.finalize();

    format_mojang_twos_complement(&digest)
}

/// Formats 20-byte digest as signed two's complement hex string following Mojang specification
pub fn format_mojang_twos_complement(digest: &[u8]) -> String {
    let is_negative = (digest[0] & 0x80) != 0;

    if !is_negative {
        let mut hex = String::with_capacity(40);
        let mut leading_zeros = true;
        for &byte in digest {
            if leading_zeros && byte == 0 {
                continue;
            }
            if leading_zeros {
                hex.push_str(&format!("{:x}", byte));
                leading_zeros = false;
            } else {
                hex.push_str(&format!("{:02x}", byte));
            }
        }
        if hex.is_empty() {
            "0".to_string()
        } else {
            hex
        }
    } else {
        // Two's complement: negate bits and add 1
        let mut carry = 1u16;
        let mut twos_comp = vec![0u8; digest.len()];
        for i in (0..digest.len()).rev() {
            let sum = (!digest[i] as u16) + carry;
            twos_comp[i] = (sum & 0xFF) as u8;
            carry = sum >> 8;
        }

        let mut hex = String::with_capacity(41);
        hex.push('-');
        let mut leading_zeros = true;
        for &byte in &twos_comp {
            if leading_zeros && byte == 0 {
                continue;
            }
            if leading_zeros {
                hex.push_str(&format!("{:x}", byte));
                leading_zeros = false;
            } else {
                hex.push_str(&format!("{:02x}", byte));
            }
        }
        if hex.len() == 1 {
            "-0".to_string()
        } else {
            hex
        }
    }
}

/// Generates offline UUIDv3 for player username (`OfflinePlayer:<username>`)
///
/// Used to create persistent UUID for offline (non-premium) servers or AuthMe authentication.
pub fn generate_offline_uuid(username: &str) -> u128 {
    let mut hasher = Md5::new();
    hasher.update(b"OfflinePlayer:");
    hasher.update(username.as_bytes());
    let mut md5_bytes = hasher.finalize();

    // Set version to 3 (UUIDv3 - MD5-based)
    md5_bytes[6] = (md5_bytes[6] & 0x0F) | 0x30;
    // Set variant to RFC 4122 (IETF)
    md5_bytes[8] = (md5_bytes[8] & 0x3F) | 0x80;

    u128::from_be_bytes(md5_bytes.into())
}

/// Formats u128 into standard 8-4-4-4-12 UUID string
pub fn format_uuid(uuid: u128) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (uuid >> 96) as u32,
        ((uuid >> 80) & 0xFFFF) as u16,
        ((uuid >> 64) & 0xFFFF) as u16,
        ((uuid >> 48) & 0xFFFF) as u16,
        uuid & 0xFFFF_FFFF_FFFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offline_uuid_notch_matches_vanilla() {
        let uuid = generate_offline_uuid("Notch");
        let formatted = format_uuid(uuid);
        assert_eq!(formatted, "b50ad385-829d-3141-a216-7e7d7539ba7f");
    }

    #[test]
    fn test_mojang_server_hash_known_cases() {
        // Case 1: SHA-1 for "Notch" (positive)
        let hash_positive = mojang_server_hash("Notch", b"", b"");
        assert_eq!(hash_positive, "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");

        // Case 2: SHA-1 for "jeb_" (negative)
        let hash_negative = mojang_server_hash("jeb_", b"", b"");
        assert_eq!(hash_negative, "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1");

        // Case 3: SHA-1 for "simon" (positive)
        let hash_simon = mojang_server_hash("simon", b"", b"");
        assert_eq!(hash_simon, "88e16a1019277b15d58faf0541e11910eb756f6");
    }

    #[test]
    fn test_aes_128_cfb8_roundtrip() {
        let key = [0x42u8; 16];
        let mut cipher_enc = StreamCipherPair::new(&key);
        let mut cipher_dec = StreamCipherPair::new(&key);

        let original = b"Minecraft protocol AES-128 CFB8 encryption test payload".to_vec();
        let mut encrypted = original.clone();

        cipher_enc.encrypt(&mut encrypted);
        assert_ne!(
            original, encrypted,
            "Encrypted data must differ from original"
        );

        let mut decrypted = encrypted;
        cipher_dec.decrypt(&mut decrypted);
        assert_eq!(original, decrypted, "Decrypted data must match original");
    }
}
