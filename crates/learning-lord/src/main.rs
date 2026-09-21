mod server;

#[tokio::main]
async fn main() {
    println!("Learning Lord tooling is ready.");
    // How do I make main wait here?
    let _ = server::run_server().await;
}

// Park, wait for a websocket connection
