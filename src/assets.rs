use std::borrow::Cow;
use encoding_rs::SHIFT_JIS;
use rust_embed::RustEmbed;


#[derive(RustEmbed)]
#[folder="$CARGO_MANIFEST_DIR/assets"]
#[prefix="assets/"]
struct Assets;

#[allow(non_snake_case)]
pub fn get_file_JIS(path: &str) -> Option<String> {
    let file = Assets::get(path)?;
    Some(SHIFT_JIS.decode(&file.data).0.into_owned())
}

pub fn get_file_bytes(path: &str) -> Option<Cow<'static, [u8]>> {
    let file = Assets::get(path)?;
    Some(file.data)
}
