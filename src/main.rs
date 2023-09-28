#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    //use axum::{routing::{post, get}, Router};
    use axum::routing::post;
    use leptos::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use web_app_axum::app::*;
    use web_app_axum::fileserv::file_and_error_handler;

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_chat=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Setting get_configuration(None) means we'll be using cargo-leptos's env values
    // For deployment these variables are:
    // <https://github.com/leptos-rs/start-axum#executing-a-server-on-a-remote-machine-without-the-toolchain>
    // Alternately a file can be specified such as Some("Cargo.toml")
    // The file would need to be included with the executable when moved to deployment
    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(|cx| view! { cx, <App/> }).await;

    let user_set = Mutex::new(HashSet::new());
    let (tx, _rx) = broadcast::channel(100);

    let app_state = Arc::new(AppState { user_set, tx });

    // build our application with a route
    let app = Router::new()
        .route("/api/*fn_name", post(leptos_axum::handle_server_fns))
        .route("/websocket", get(websocket_handler))
        .with_state(app_state)
        .leptos_routes(&leptos_options, routes, |cx| view! { cx, <App/> })
        .fallback(file_and_error_handler)
        .with_state(leptos_options);

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    log!("listening on http://{}", &addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("Failed to bind");
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for a purely client-side app
    // see lib.rs for hydration function instead
}

use cfg_if::cfg_if;

cfg_if! { if #[cfg(feature = "ssr")] {
    use axum::{
        extract::{
            ws::{Message, WebSocket, WebSocketUpgrade},
            State,
        },
        response::IntoResponse,
        routing::get,
        Router,
    };
    use futures::{sink::SinkExt, stream::StreamExt};
    use std::{
        collections::HashSet,
        sync::{Arc, Mutex},
    };
    use tokio::sync::broadcast;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    
    // Our shared state
    struct AppState {
        // We require unique usernames. This tracks which usernames have been taken.
        user_set: Mutex<HashSet<String>>,
        // Channel used to send messages to all connected clients.
        tx: broadcast::Sender<String>,
    }
    
    async fn websocket_handler(
        ws: WebSocketUpgrade,
        State(state): State<Arc<AppState>>,
    ) -> impl IntoResponse {
        ws.on_upgrade(|socket| websocket(socket, state))
    }
    
    // This function deals with a single websocket connection, i.e., a single
    // connected client / user, for which we will spawn two independent tasks (for
    // receiving / sending chat messages).
    async fn websocket(stream: WebSocket, state: Arc<AppState>) {
        // By splitting, we can send and receive at the same time.
        let (mut sender, mut receiver) = stream.split();

        // Username gets set in the receive loop, if it's valid.
        let mut username = String::new();
        // Loop until a text message is found.
        while let Some(Ok(message)) = receiver.next().await {
            if let Message::Text(name) = message {
                // If username that is sent by client is not taken, fill username string.
                check_username(&state, &mut username, &name);

                // If not empty we want to quit the loop else we want to quit function.
                if !username.is_empty() {
                    break;
                } else {
                    // Only send our client that username is taken.
                    let _ = sender
                        .send(Message::Text(String::from("Username already taken.")))
                        .await;

                    return;
                }
            }
        }

        // We subscribe *before* sending the "joined" message, so that we will also
        // display it to our client.
        let mut rx = state.tx.subscribe();

        // Now send the "joined" message to all subscribers.
        let msg = format!("{username} joined.");
        tracing::debug!("{msg}");
        let _ = state.tx.send(msg);

        // Spawn the first task that will receive broadcast messages and send text
        // messages over the websocket to our client.
        let mut send_task = tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                // In any websocket error, break loop.
                if sender.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
        });

        // Clone things we want to pass (move) to the receiving task.
        let tx = state.tx.clone();
        let name = username.clone();

        // Spawn a task that takes messages from the websocket, prepends the user
        // name, and sends them to all broadcast subscribers.
        let mut recv_task = tokio::spawn(async move {
            while let Some(Ok(Message::Text(text))) = receiver.next().await {
                // Add username before message.
                let _ = tx.send(format!("{{\"sender\": {name}, \"msg\": {text}}}"));
            }
        });

        // If any one of the tasks run to completion, we abort the other.
        tokio::select! {
            _ = (&mut send_task) => recv_task.abort(),
            _ = (&mut recv_task) => send_task.abort(),
        };

        // Send "user left" message (similar to "joined" above).
        let msg = format!("{username} left.");
        tracing::debug!("{msg}");
        let _ = state.tx.send(msg);

        // Remove username from map so new clients can take it again.
        state.user_set.lock().unwrap().remove(&username);
    }
    
    fn check_username(state: &AppState, string: &mut String, name: &str) {
        let mut user_set = state.user_set.lock().unwrap();
    
        if !user_set.contains(name) {
            user_set.insert(name.to_owned());
    
            string.push_str(name);
        }
    }  
}}