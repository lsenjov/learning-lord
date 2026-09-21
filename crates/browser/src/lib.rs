use wasm_bindgen::prelude::*;
use web_sys::MessageEvent;
use web_sys::console;

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

    let _socket = open_socket();

    Ok(())
}

fn open_socket() -> web_sys::WebSocket {
    let socket = web_sys::WebSocket::new("ws://127.0.0.1:1234").expect("connect");
    console::log_1(&format!("Opening").into());

    let onmessage = Closure::wrap(Box::new(|e: MessageEvent| {
        if let Some(text) = e.data().as_string() {
            console::log_1(&format!("Message: {text}").into());
        }
    }) as Box<dyn Fn(MessageEvent)>);

    socket.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let onopen = Closure::wrap(Box::new(|| {
        console::log_1(&"Connected!".into());
    }) as Box<dyn Fn()>);

    socket.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    let onclose = Closure::wrap(Box::new(|| {
        console::log_1(&"Disconnected!".into());
        open_socket();
    }) as Box<dyn Fn()>);

    socket.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    socket
}
