//! Mat ma hoc va xac thuc cho CasxDuois Client (AES-128-CFB8, Mojang ServerHash SHA-1, UUIDv3).

use aes::Aes128;
use cfb8::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use cfb8::{Decryptor, Encryptor};
use md5::Md5;
use sha1::{Digest, Sha1};

type AesCfb8Enc = Encryptor<Aes128>;
type AesCfb8Dec = Decryptor<Aes128>;

/// Cap bo ma hoa va giai ma luong doi xung AES-128-CFB8 cho TCP Socket
pub struct StreamCipherPair {
    encryptor: AesCfb8Enc,
    decryptor: AesCfb8Dec,
}

impl StreamCipherPair {
    /// Khoi tao cap cipher voi khoa bi mat 16-bytes
    ///
    /// Theo dac ta Minecraft, IV ban dau su dung chinh gia tri cua shared secret key.
    pub fn new(key: &[u8; 16]) -> Self {
        let encryptor = AesCfb8Enc::new(key.into(), key.into());
        let decryptor = AesCfb8Dec::new(key.into(), key.into());
        Self { encryptor, decryptor }
    }

    /// Ma hoa du lieu tai cho (in-place) truoc khi ghi xuong TCP Socket
    pub fn encrypt(&mut self, data: &mut [u8]) {
        let (blocks, _) = cfb8::cipher::inout::InOutBuf::from(data).into_chunks();
        self.encryptor.encrypt_blocks_inout_mut(blocks);
    }

    /// Giai ma du lieu tai cho (in-place) ngay khi doc tu TCP Socket
    pub fn decrypt(&mut self, data: &mut [u8]) {
        let (blocks, _) = cfb8::cipher::inout::InOutBuf::from(data).into_chunks();
        self.decryptor.decrypt_blocks_inout_mut(blocks);
    }
}

/// Tinh toan chuoi bam ServerHash SHA-1 theo kieu Two's Complement Hex Digest cua Mojang
///
/// Cong thuc:
/// $$\text{Digest} = \text{SHA-1}(\text{ServerID}_{\text{ASCII}} \parallel \text{SharedSecret}_{16\text{B}} \parallel \text{PublicKey}_{\text{DER}})$$
pub fn mojang_server_hash(server_id: &str, shared_secret: &[u8], public_key_der: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key_der);
    let digest = hasher.finalize();

    format_mojang_twos_complement(&digest)
}

/// Dinh dang mang byte 20-bytes thanh chuoi Hex co dau Two's Complement theo quy uoc Mojang
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
        // Lay phan bu hai: dao tat ca cac bit va cong 1
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

/// Sinh UUIDv3 offline cho nguoi choi tu ten dang nhap (`OfflinePlayer:<username>`)
///
/// Dung de tao UUID co dinh cho cac may chu Offline (non-premium) hoac AuthMe.
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

/// Dinh dang u128 thanh chuoi UUID chuan 8-4-4-4-12
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
        // Truong hop 1: SHA-1 cho chuoi "Notch" (Duong)
        let hash_positive = mojang_server_hash("Notch", b"", b"");
        assert_eq!(hash_positive, "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");

        // Truong hop 2: SHA-1 cho chuoi "jeb_" (Am)
        let hash_negative = mojang_server_hash("jeb_", b"", b"");
        assert_eq!(hash_negative, "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1");

        // Truong hop 3: SHA-1 cho chuoi "simon" (Duong)
        let hash_simon = mojang_server_hash("simon", b"", b"");
        assert_eq!(hash_simon, "88e16a1019277b15d58faf0541e11910eb756f6");
    }

    #[test]
    fn test_aes_128_cfb8_roundtrip() {
        let key = [0x42u8; 16];
        let mut cipher_enc = StreamCipherPair::new(&key);
        let mut cipher_dec = StreamCipherPair::new(&key);

        let original = b"Day la goi tin du lieu Minecraft can ma hoa!".to_vec();
        let mut encrypted = original.clone();

        cipher_enc.encrypt(&mut encrypted);
        assert_ne!(original, encrypted, "Du lieu sau khi ma hoa phai thay doi");

        let mut decrypted = encrypted;
        cipher_dec.decrypt(&mut decrypted);
        assert_eq!(original, decrypted, "Du lieu sau khi giai ma phai trung khop ban dau");
    }
}
