use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let document = web_sys::window()
        .ok_or_else(|| JsValue::from_str("browser window is unavailable"))?
        .document()
        .ok_or_else(|| JsValue::from_str("browser document is unavailable"))?;
    let status = document
        .get_element_by_id("status")
        .ok_or_else(|| JsValue::from_str("#status element is missing"))?;

    status.set_text_content(Some("Hello world"));
    Ok(())
}
