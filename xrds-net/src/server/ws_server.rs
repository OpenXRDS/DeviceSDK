use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};
use futures::{SinkExt, StreamExt};
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use tokio_tungstenite::WebSocketStream as WsStream;
use std::pin::Pin;
use std::future::Future;

struct WsConnection {
    sender: SplitSink<WsStream<TcpStream>, Message>,
    receiver: SplitStream<WsStream<TcpStream>>,
}

/*
    Defines a default handler for each message type
    This function echoes the received message
 */
async fn default_handler(msg: Vec<u8>) -> Option<Vec<u8>> {
    println!("Received message: {:?}", msg.clone());
    Some(msg)
}

pub struct WebSocketServer {
    // handler type: fn handler_name(Vec<u8>) -> Option<vec<u8>>
    handlers: HashMap<String, Arc<dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>> + Send + 'static>> + Send>>,  // <msg_type, handler>
}

impl WebSocketServer {
    pub fn new() -> Self {
        let handlers = HashMap::new();

        WebSocketServer {
            handlers,
        }
    }

    /**
     * Server user must provide handlers for each Owned Message type
     * - Text
     * - Binary
     * - Close
     * - Ping
     * - Pong
     */
    pub fn register_handler<F, Fut>(&mut self, msg_type: &str, handler: F)
    where
        F: Fn(Vec<u8>) -> Fut + Send + 'static,
        Fut: Future<Output = Option<Vec<u8>>> + Send + 'static,
    {
        let handler_arc: Arc<dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>> + Send + 'static>> + Send> =
            Arc::new(move |data| {
                let fut: Fut = handler(data);
                Box::pin(fut) as Pin<Box<dyn Future<Output = Option<Vec<u8>>> + Send + 'static>>
            });
        self.handlers.insert(msg_type.to_lowercase(), handler_arc);
    }

    /**
     * This method is to test 'register_handler' method
     * This runs the handler for 'test' message type, so tester must provide a handler for 'test' message type
     */
    pub fn test_handler(&self) -> Result<(), Box<dyn std::error::Error>> {
        let input_str = "hello world";
        let input = input_str.as_bytes().to_vec();

        let handler = self.handlers.get("test").unwrap();
        let fut = handler(input);
        let result = futures::executor::block_on(fut);
        let result_str = String::from_utf8(result.unwrap()).unwrap();
        println!("Result: {:?}", result_str);
        Ok(())
    }

    pub fn register_default_handlers(&mut self) {
        self.register_handler("text", default_handler);
        self.register_handler("binary", default_handler);
        self.register_handler("close", default_handler);
        self.register_handler("ping", default_handler);
        self.register_handler("pong", default_handler);
    }
    
    pub async fn run(&self, port: u32) {

    }
    /* Privates */

}