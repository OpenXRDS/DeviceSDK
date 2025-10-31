use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use futures::{SinkExt, StreamExt};
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use tokio_tungstenite::WebSocketStream as WsStream;
use std::pin::Pin;
use std::future::Future;
use std::sync::Mutex;
use tokio_tungstenite::tungstenite::error::Error;

use crate::common::generate_uuid;

type WsHandlers = HashMap<String, Arc<dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>> + Send + Sync + 'static>> + Send + Sync + 'static>>;
type WsHandler = Arc<dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>> + Send + Sync + 'static>> + Send + Sync + 'static>;

struct WsConnection {   // client auth info can be added here, e.g. client_id
    sender: SplitSink<WsStream<TcpStream>, Message>,
    receiver: SplitStream<WsStream<TcpStream>>,
}

#[derive(Clone)]
struct Client {
    client_id: String,
    peer_addr: String,
}

/*
    Defines a default handler for each message type
    This function echoes the received message
 */
async fn default_handler(msg: Vec<u8>) -> Option<Vec<u8>> {
    println!("Default Handler echoing: {:?}", msg.clone());
    Some(msg)
}

pub struct WebSocketServer {
    // handler type: fn handler_name(Vec<u8>) -> Option<vec<u8>>
    handlers: WsHandlers,  // <msg_type, handler>
    clients: Arc<Mutex<HashMap<String, Client>>>,  // <client_id, ws_connection>
}

#[allow(dead_code)]
impl WebSocketServer {

    /**
     * Create a new WebSocketServer instance with default handlers
     */
    pub fn new() -> Self {
        let handlers = HashMap::new();
        let mut wss = WebSocketServer {
            handlers,
            clients: Arc::new(Mutex::new(HashMap::new())),
        };

        wss.register_default_handlers();    // register default handlers for each message type
        wss
    }

    fn add_client(&self, client_id: &String, client: Client) {
        self.clients.lock().unwrap().insert(client_id.to_string(), client);
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
        F: Fn(Vec<u8>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Option<Vec<u8>>> + Send + Sync + 'static,
    {
        let handler_arc: WsHandler =
            Arc::new(move |data| {
                let fut: Fut = handler(data);
                Box::pin(fut) as Pin<Box<dyn Future<Output = Option<Vec<u8>>> + Send + Sync + 'static>>
            });
        self.handlers.insert(msg_type.to_lowercase(), handler_arc);
    }

    pub fn register_handler_arc(&mut self, msg_type: &str, handler: WsHandler) {
        self.handlers.insert(msg_type.to_lowercase(), handler);
    }

    pub fn set_handlers(&mut self, handlers: WsHandlers) {
        self.handlers = handlers;
    }

    /**
     * Register default handlers for each message type which echoes the received message
     */
    fn register_default_handlers(&mut self) {
        self.register_handler("text", default_handler);
        self.register_handler("binary", default_handler);
        self.register_handler("close", default_handler);
        self.register_handler("ping", default_handler);
        self.register_handler("pong", default_handler);
    }
    
    async fn handle_connection(&self, ws_connection: WsConnection) -> Result<(), Box<dyn std::error::Error>> {
        let (mut sender, mut receiver) = (ws_connection.sender, ws_connection.receiver);
        let handlers = self.handlers.clone();

        while let Some(msg) = receiver.next().await {
            println!("Ws Server Received message: {:?}", msg);
            let msg = match msg {
                Ok(msg) => msg,
                Err(e) => {
                    Self::log_error_connection(e);

                    if let Err(close_err) = sender.send(Message::Close(None)).await {
                        println!("[Server]Failed to send close frame: {}", close_err);
                    }
                    break;
                }
            };

            let msg_type = match msg.clone() {
                msg if msg.is_binary() => "binary",
                msg if msg.is_text() => "text",
                msg if msg.is_ping() => "ping",
                msg if msg.is_pong() => "pong",
                msg if msg.is_close() => {
                    println!("Close message received");
                    break;
                },
                _ => {
                    println!("Unknown message type");
                    continue;
                }
            };

            let handler = handlers.get(msg_type).unwrap();
            let input = msg.into_data();
            let input = input.to_vec();
            let fut = handler(input);
            let result = fut.await;
            let result = match result {
                Some(r) => r,
                None => {
                    println!("Handler returned None");
                    continue;
                }
            };

            println!("preparing message back to client");
            // prepare message back to client
            let msg = Message::Binary(result.into());
            if let Err(e) = sender.send(msg).await {
                println!("Error sending message: {}", e);
                continue;
            }
        }
        Ok(())
        
    }

    pub async fn run(self: Arc<Self>, port: u32) -> Result<(), Box<dyn std::error::Error>> {
        let host_addr = "0.0.0.0".to_owned() + ":" + &port.to_string();
        let try_socket = TcpListener::bind(host_addr.clone()).await;
        let listener = match try_socket { 
            Ok(l) => {
                println!("WebSocket server started on {}", host_addr);  // temporal log
                l
            }
            Err(e) => {
                println!("Error binding to {}: {}", host_addr, e);
                return Err(Box::new(e));
            }
        };
        
        while let Ok((stream, addr)) = listener.accept().await {
            println!("Accepted connection from {}", addr);  // temporal log
            
            let self_clone = Arc::clone(&self);
            tokio::spawn({
                async move {
                    match accept_async(stream).await {
                        Ok(ws_stream) => {  // connection established
                            let client_id = generate_uuid();
                            let peer_addr = addr.to_string();

                            let client = Client {
                                client_id: client_id.clone(),
                                peer_addr: peer_addr.clone(),
                            };
                            println!("New client connected: {} from {}", client.client_id, client.peer_addr);

                            self_clone.add_client(&client_id, client);

                            let (sender, receiver) = ws_stream.split();

                            let ws_connection = WsConnection {
                                sender,
                                receiver,
                            };

                            if let Err(e) = self_clone.handle_connection(ws_connection).await {
                                println!("Error handling connection from {}: {}", addr, e);
                            }
                        }
                        Err(e) => println!("Error accepting WebSocket connection from {}: {}", addr, e),
                    }
                }
            });
        }

        Ok(())
    }
    /* Privates */

    fn log_error_connection(error: Error) {
        match &error {
            Error::ConnectionClosed => {
                println!("Connection closed normally (but no Close frame?).");
            }
            Error::Io(io_err) => {
                match io_err.kind() {
                    std::io::ErrorKind::ConnectionReset => {
                        println!("Client rudely dropped connection (ConnectionReset).");
                    }
                    std::io::ErrorKind::BrokenPipe => {
                        println!("Client terminated socket without handshake (BrokenPipe).");
                    }
                    std::io::ErrorKind::ConnectionAborted => {
                        println!("Client terminated socket with handshake (ConnectionAborted).");
                    }
                    _ => {
                        println!("Unexpected I/O error: {}", io_err);
                    }
                }
            }
            Error::Protocol(proto_err) => {
                println!("Protocol violation by client: {}", proto_err);
            }
            _ => {
                println!("Other WebSocket error: {}", error);
            }
        }
    }
}