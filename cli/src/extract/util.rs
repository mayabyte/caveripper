use std::borrow::Cow;
use encoding_rs::SHIFT_JIS;

pub fn read_u16(data: &[u8], offset: u32) -> u16 {
    u16::from_be_bytes(data[offset as usize..offset as usize+2].try_into().unwrap())
}

pub fn read_u32(data: &[u8], offset: u32) -> u32 {
    u32::from_be_bytes(data[offset as usize..offset as usize+4].try_into().unwrap())
}

pub fn read_str(data: &[u8], offset: u32, len: u32) -> Cow<'_, str> {
    SHIFT_JIS.decode(&data[offset as usize .. (offset+len) as usize]).0
}

pub fn read_str_until_null(data: &[u8], offset: u32) -> Cow<'_, str> {
    let mut i = 0;
    while data[offset as usize + i] != b"\0"[0] {
        i += 1;
    }
    read_str(data, offset, i as u32)
}
