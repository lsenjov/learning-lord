use crate::citizens::behaviour;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    println!("Server starting");
    let listener = TcpListener::bind("127.0.0.1:1234").await?;
    let (tx, _) = broadcast::channel::<String>(256);
    eprintln!("Listening on 127.0.0.1:1234");

    loop {
        let (stream, addr) = listener.accept().await?;
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let Ok(ws) = accept_async(stream).await else {
                eprintln!("{addr}: handshake failed");
                return;
            };
            eprintln!("{addr} connected");

            let (mut sink, mut source) = ws.split();

            let _ = sink.send(Message::text("Test ping!"));
            let write = tokio::spawn(async move {
                while let Ok(msg) = rx.recv().await {
                    if sink.send(Message::text(msg)).await.is_err() {
                        break;
                    }
                }
            });

            while let Some(Ok(msg)) = source.next().await {
                if let Message::Text(text) = msg {
                    let _ = tx.send(text.to_string());
                }
            }
            write.abort();
            eprintln!("{addr} disconnected");
        });
    }
}
