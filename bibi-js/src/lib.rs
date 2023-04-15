mod utils;

use utils::set_panic_hook;
use wasm_bindgen::prelude::*;

use bibi::{dump_bbcode, dump_markdown};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn to_bbcode(s: &str) -> Result<String, JsError> {
    set_panic_hook(); // run this once when the feature is enabled

    let mut writer = Vec::new();

    dump_bbcode(&mut writer, s)?;

    Ok(String::from_utf8(writer)?)
}

#[wasm_bindgen]
pub fn to_markdown(s: &str) -> Result<String, JsError> {
    set_panic_hook(); // see above

    let mut writer = Vec::new();

    dump_markdown(&mut writer, s)?;

    Ok(String::from_utf8(writer)?)
}