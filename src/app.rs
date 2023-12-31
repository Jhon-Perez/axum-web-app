use crate::error_template::{AppError, ErrorTemplate};
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    #[cfg(not(feature = "ssr"))]
    (move || {
        use crate::ws::provide_websocket;

        let location = window().location();
        let protocol = location.protocol().map(|protocol| match protocol.as_str() {
            "https:" => "wss:",
            _ => "ws:",
        });
        let protocol = match protocol {
            Ok(protocol) => protocol,
            Err(_) => return,
        };
        let host = match location.host() {
            Ok(host) => host,
            Err(_) => return,
        };
        match provide_websocket(format!("{protocol}//{host}/websocket").as_str()) {
            Ok(_) => logging::log!("Connected to {}//{}/websocket", protocol, host),
            Err(_) => log::error!("Failed to connect to WebSocket!"),
        };
    })();

    view! {

        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/web-app-axum.css"/>

        // sets the document title
        <Title text="Welcome to Leptos"/>

        // content for this welcome page
        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! {
                <ErrorTemplate outside_errors/>
            }
            .into_view()
        }>
            <main>
                <Routes>
                    <Route path="" view=|| view! { <HomePage/> }/>
                </Routes>
            </main>
        </Router>
    }
}

use crate::ws::{ServerMessage, create_ws_signal, send_msg};

use uuid::Uuid;
use web_sys::SubmitEvent;

#[derive(Debug, Clone)]
enum WsMessage {
    Me(String),
    Server(ServerMessage),
}

#[component]
fn HomePage() -> impl IntoView {
    //let mut disable_button = false;

    let last_message = create_ws_signal();
    let (messages, set_messages) = create_signal(Vec::<(Uuid, WsMessage)>::new());

    // update message to me if sent by someone else (problem is that it also does that if I
    // sent the message. Make sure to only do it when someone other than me sends a message)
    create_effect(move |_| match last_message.get() {
        Some(message) => {
            set_messages.update(move |messages| {
                (*messages).push((Uuid::new_v4(), WsMessage::Server(message)));
            });
        }
        None => (),
    });

    // get input and update it here
    let (message_input, set_message_input) = create_signal("".to_owned());

    // send message to everyone else if sent by me
    let send_message = move |ev: SubmitEvent| {
        ev.prevent_default();

        let msg = message_input.get();

        let _ = send_msg(msg.as_str());
        set_message_input.set("".to_owned());

        set_messages.update(move |messages| {
            (*messages).push((Uuid::new_v4(), WsMessage::Me(msg)));
        });
    };

    let (username, set_username_input) = create_signal("".to_owned());

    // send message to everyone else if sent by me
    let set_username = move |ev: SubmitEvent| {
        ev.prevent_default();

        let mut msg = username.get();

        let _ = send_msg(msg.as_str());
        set_username_input.set(msg.to_owned());

        msg.push_str(" has joined");
        let msg = ServerMessage{sender: "Server".to_owned(), msg};

        log::info!("{msg:?}");

        set_messages.update(move |messages| {
            (*messages).push((Uuid::new_v4(), WsMessage::Server(msg)));
        });
        //disable_button = true;
    };

    view! {
        <form on:submit=set_username>
            <input
                placeholder="Username"
                prop:value=username
                on:input=move |ev| {
                    set_username_input.set(event_target_value(&ev));
                }
            />
            // submit button
            <button type="submit" disabled=move || username.get() == "">
                "set name"
            </button>
        </form>

        <div class="chat__container">
        <ol class="chat">
            <For
                each=move || messages.get()
                key=move |message| message.0
                children=move |(_, message)| {
                    view! {
                        {
                            move || {
                                let message = message.clone();
                            // check if the message was sent by me or another client then update from there
                            match message { 
                            WsMessage::Me(msg) => view!{
                                <li class="chat-message__container chat-message__container--me">
                                <p class="chat-message__message chat-message__message--me">{
                                    msg
                                }</p>
                                </li>
                            }.into_view(),
                            WsMessage::Server(message) => {
                                if username.get() != message.sender {
                                    view! {
                                        <li class="chat-message__container">
                                    <p class="chat-message__sender">{move || {
                                        let sender = &message.sender;
                                        sender.to_owned()
                                    }}</p>
                                    <p class="chat-message__message chat-message__message--server">{move || {
                                        let msg = &message.msg;
                                        msg.to_owned()
                                    }}</p>
                                    </li>
                                }} else {view!{<li>""</li>}}
                            }.into_view(),
                        }}}
                }}
            />
        </ol>
        </div>

        // chat box that allows others to type on
        <form on:submit=send_message class="">
            <div class="chat-box">
                <div class="input-container">
                    <input
                        id="user-input"
                        placeholder="Message"
                        prop:value=message_input
                        on:input=move |ev| {
                            set_message_input.set(event_target_value(&ev));
                        }
                    />
                    // submit button
                    <button type="submit" id="send-button" disabled=move || message_input.get() == "" || username.get() == "">
                        "send"
                    </button>
                </div>
            </div>
        </form>
    }
}
