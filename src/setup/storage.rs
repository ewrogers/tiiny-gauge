use core::net::Ipv4Addr;

use embassy_rp::{
    Peri,
    flash::{Blocking, ERASE_SIZE, Flash},
    peripherals::FLASH,
};

use super::{KEY_MAX, PASSWORD_MAX, SSID_MAX, Settings};

const FLASH_SIZE: usize = 2 * 1024 * 1024;
// Matches the two sectors reserved by memory.x.
const FIRST_SLOT: usize = FLASH_SIZE - 2 * ERASE_SIZE;
const HEADER: usize = 16;
const RECORD_SIZE: usize = HEADER + SSID_MAX + PASSWORD_MAX + KEY_MAX + 4;
const MAGIC: [u8; 4] = *b"TG01";

pub(crate) struct Storage {
    flash: Flash<'static, FLASH, Blocking, FLASH_SIZE>,
    newest: Option<(usize, u32)>,
    saved_this_boot: bool,
}

impl Storage {
    pub(crate) fn new(flash: Peri<'static, FLASH>) -> Self {
        Self {
            flash: Flash::new_blocking(flash),
            newest: None,
            saved_this_boot: false,
        }
    }

    pub(crate) fn load(&mut self) -> Option<Settings> {
        let mut latest = None;
        for index in 0..2 {
            let mut record = [0; RECORD_SIZE];
            if self.flash.blocking_read(slot(index), &mut record).is_ok()
                && let Some((generation, settings)) = decode(&record)
                && latest.as_ref().is_none_or(|&(_, previous, _)| {
                    generation != previous && generation.wrapping_sub(previous) < (1 << 31)
                })
            {
                latest = Some((index, generation, settings));
            }
        }
        let (index, generation, settings) = latest?;
        self.newest = Some((index, generation));
        Some(settings)
    }

    pub(super) fn save(&mut self, settings: &Settings) -> Result<(), ()> {
        let (index, generation) = self.newest.map_or((0, 0), |(index, generation)| {
            (1 - index, generation.wrapping_add(1))
        });
        let offset = slot(index);
        let record = encode(settings, generation);
        self.flash
            .blocking_erase(offset, offset + ERASE_SIZE as u32)
            .map_err(|_| ())?;
        self.flash.blocking_write(offset, &record).map_err(|_| ())?;
        let mut verify = [0; RECORD_SIZE];
        self.flash
            .blocking_read(offset, &mut verify)
            .map_err(|_| ())?;
        if record != verify {
            return Err(());
        }
        self.newest = Some((index, generation));
        self.saved_this_boot = true;
        Ok(())
    }

    pub(super) fn saved_this_boot(&self) -> bool {
        self.saved_this_boot
    }
}

fn slot(index: usize) -> u32 {
    (FIRST_SLOT + index * ERASE_SIZE) as u32
}

fn encode(settings: &Settings, generation: u32) -> [u8; RECORD_SIZE] {
    let mut data = [0xff; RECORD_SIZE];
    data[..4].copy_from_slice(&MAGIC);
    data[4..8].copy_from_slice(&generation.to_le_bytes());
    data[8..12].copy_from_slice(&settings.ip.octets());
    data[12] = settings.ssid.len() as u8;
    data[13] = settings.password.len() as u8;
    data[14..16].copy_from_slice(&(settings.key.len() as u16).to_le_bytes());
    let mut offset = HEADER;
    for field in [
        settings.ssid.as_bytes(),
        settings.password.as_bytes(),
        settings.key.as_bytes(),
    ] {
        data[offset..offset + field.len()].copy_from_slice(field);
        offset += field.len();
    }
    let checksum = crc32(&data[..RECORD_SIZE - 4]);
    data[RECORD_SIZE - 4..].copy_from_slice(&checksum.to_le_bytes());
    data
}

fn decode(data: &[u8; RECORD_SIZE]) -> Option<(u32, Settings)> {
    if data[..4] != MAGIC
        || crc32(&data[..RECORD_SIZE - 4])
            != u32::from_le_bytes(data[RECORD_SIZE - 4..].try_into().ok()?)
    {
        return None;
    }
    let generation = u32::from_le_bytes(data[4..8].try_into().ok()?);
    let ip = Ipv4Addr::new(data[8], data[9], data[10], data[11]);
    let ssid_end = HEADER + usize::from(data[12]);
    let password_end = ssid_end + usize::from(data[13]);
    let key_end = password_end + usize::from(u16::from_le_bytes([data[14], data[15]]));
    let payload = &data[..RECORD_SIZE - 4];
    let ssid = core::str::from_utf8(payload.get(HEADER..ssid_end)?).ok()?;
    let password = core::str::from_utf8(payload.get(ssid_end..password_end)?).ok()?;
    let key = core::str::from_utf8(payload.get(password_end..key_end)?).ok()?;
    Some((generation, Settings::new(ssid, password, ip, key).ok()?))
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
