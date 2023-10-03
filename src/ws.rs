use leptos::*;

use serde::{Deserialize, Serialize};

pub type ClientMessage = String;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerMessage {
    pub sender: String,
    pub msg: String,
}

pub fn create_ws_signal() -> ReadSignal<Option<ServerMessage>> {
    let (get, set) = create_signal(None);
    create_server_ws_signal(set);

    get
}

use js_sys::{Function, JsString};
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::{MessageEvent, WebSocket};

fn create_server_ws_signal(set: WriteSignal<Option<ServerMessage>>) {
    let ws = use_context::<ServerWS>();

    match ws {
        Some(ServerWS(ws)) => {
            create_effect(move |_| {
                let callback = Closure::wrap(Box::new(move |event: MessageEvent| {
                    log::info!("Received a message from the server!");
                    let ws_string = event
                        .data()
                        .dyn_into::<JsString>()
                        .unwrap()
                        .as_string()
                        .unwrap();
                    let parsed = serde_json::from_str::<ServerMessage>(&ws_string);
                    match parsed {
                        Ok(parsed) => {
                            log::info!("Parsed: {parsed:?}");
                            set.set(Some(parsed));
                        }
                        Err(err) => {
                            log::error!("Failed to parse: {err:?}");
                        }
                    }
                }) as Box<dyn FnMut(_)>);
                let function: &Function = callback.as_ref().unchecked_ref();

                ws.set_onmessage(Some(function));
                callback.forget();
            });
        }
        None => {
            leptos::logging::error!(r#"No websocket provided at root of app"#);
        }
    }
}

type TypeFn<T> = fn(T, String);

pub struct FnStruct<T> {
    pub t: T,
    pub f: TypeFn<T>,
}

impl<T> Clone for FnStruct<T>
where
    T: Copy,
{
    fn clone(&self) -> Self {
        FnStruct {
            t: self.t,
            f: self.f,
        }
    }
}

pub fn send_msg(msg: &str) -> Result<(), ()> {
    let ws = use_context::<ServerWS>();
    match ws {
        Some(ServerWS(ws)) => {
            let msg: ClientMessage = msg.to_owned();
            let str = serde_json::to_string(&msg);
            log::info!("{str:?}");
            let _ = match str {
                Ok(str) => ws.send_with_str(str.as_str()),
                Err(_) => return Err(()),
            };
            Ok(())
        },
        None => Err(()),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ServerWS(WebSocket);

pub fn provide_websocket(url: &str) -> Result<(), JsValue> {
    if use_context::<ServerWS>().is_none() {
        let ws = WebSocket::new(url)?;
        provide_context(ServerWS(ws));
    }

    Ok(())
}