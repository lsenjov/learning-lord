mod behaviour;
mod server;

#[tokio::main]
async fn main() {
    println!("Learning Lord tooling is ready.");
    // How do I make main wait here?
    //let _ = server::run_server().await;
    behaviour::run_sim();
}

// Park, wait for a websocket connection
