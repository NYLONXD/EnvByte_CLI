use mac_address::get_mac_address;

/// Returns the MAC address of the primary network interface as a hex string.
///
/// This is a device identifier, not a secret and not a cryptographic factor.
/// Only legacy (pre-v2) ciphertext derives a key from it, so every current
/// operation must keep working when no interface is available — containers and
/// CI runners frequently report none.
pub fn get_device_mac() -> Option<String> {
    match get_mac_address() {
        Ok(Some(mac)) => Some(mac.to_string().replace(":", "").to_lowercase()),
        Ok(None) => None,
        Err(error) => {
            // Not fatal: the MAC is only needed to decrypt legacy payloads.
            eprintln!("Note: could not read a device MAC address ({error}).");
            None
        }
    }
}

/// Combines OTT + MAC address to form the refresher token.
/// refresher_token = SHA256(ott + mac_address)
pub fn make_refresher_token(ott: &str, mac: Option<&str>) -> String {
    use sha2::{Digest, Sha256};
    let input = format!("{}{}", ott, mac.unwrap_or_default());
    let hash = Sha256::digest(input.as_bytes());
    hex::encode(hash)
}
