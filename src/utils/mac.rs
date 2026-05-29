use mac_address::get_mac_address;

/// Returns the MAC address of the primary network interface as a hex string.
/// This is used as part of device-bound encryption — the .env can only be
/// decrypted on a machine that was originally registered.
pub fn get_device_mac() -> Result<String, String> {
    match get_mac_address() {
        Ok(Some(mac)) => Ok(mac.to_string().replace(":", "").to_lowercase()),
        Ok(None) => Err("No MAC address found on this device".to_string()),
        Err(e) => Err(format!("Failed to get MAC address: {}", e)),
    }
}

/// Combines OTT + MAC address to form the refresher token.
/// refresher_token = SHA256(ott + mac_address)
pub fn make_refresher_token(ott: &str, mac: &str) -> String {
    use sha2::{Sha256, Digest};
    let input = format!("{}{}", ott, mac);
    let hash = Sha256::digest(input.as_bytes());
    hex::encode(hash)
}