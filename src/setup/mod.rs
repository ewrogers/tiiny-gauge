use core::net::Ipv4Addr;
use serde_json_core::heapless::String;

mod serial;
mod storage;

pub(crate) use serial::run;
pub(crate) use storage::Storage;

const SSID_MAX: usize = 32;
const PASSWORD_MAX: usize = 63;
const KEY_MAX: usize = 504;

#[derive(Debug, PartialEq, Eq)]
enum InvalidField {
    Ssid,
    Password,
    Address,
    Key,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Settings {
    pub ssid: String<SSID_MAX>,
    pub password: String<PASSWORD_MAX>,
    pub ip: Ipv4Addr,
    pub key: String<KEY_MAX>,
}

impl Settings {
    fn new(ssid: &str, password: &str, ip: Ipv4Addr, key: &str) -> Result<Self, InvalidField> {
        Ok(Self {
            ssid: parse_ssid(ssid)?,
            password: parse_password(password)?,
            ip: validate_ip(ip)?,
            key: parse_key(key)?,
        })
    }
}

fn parse_ssid(ssid: &str) -> Result<String<SSID_MAX>, InvalidField> {
    if ssid.is_empty() || ssid.chars().any(char::is_control) {
        return Err(InvalidField::Ssid);
    }
    String::try_from(ssid).map_err(|_| InvalidField::Ssid)
}

fn parse_password(password: &str) -> Result<String<PASSWORD_MAX>, InvalidField> {
    if !password.is_empty()
        && (password.len() < 8 || !password.bytes().all(|byte| (32..=126).contains(&byte)))
    {
        return Err(InvalidField::Password);
    }
    String::try_from(password).map_err(|_| InvalidField::Password)
}

fn validate_ip(ip: Ipv4Addr) -> Result<Ipv4Addr, InvalidField> {
    if ip.is_unspecified() || ip.is_broadcast() || ip.is_multicast() || ip.is_loopback() {
        return Err(InvalidField::Address);
    }
    Ok(ip)
}

fn parse_key(key: &str) -> Result<String<KEY_MAX>, InvalidField> {
    if key.is_empty() || !key.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(InvalidField::Key);
    }
    String::try_from(key).map_err(|_| InvalidField::Key)
}

#[cfg(feature = "baked-config")]
mod config {
    include!(concat!(env!("OUT_DIR"), "/local_config.rs"));
}

pub(crate) fn baked() -> Option<Settings> {
    #[cfg(feature = "baked-config")]
    {
        Settings::new(
            config::WIFI_SSID,
            config::WIFI_PASSWORD,
            config::TIINY_IP.parse().ok()?,
            config::TIINY_API_KEY,
        )
        .ok()
    }
    #[cfg(not(feature = "baked-config"))]
    None
}
