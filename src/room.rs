use crate::entity::Entity;
use futures::{
    channel::mpsc::{channel, Receiver, Sender},
    prelude::*,
};
use log::info;
use snafu::Snafu;
use rand::{Rng, thread_rng, distributions::Alphanumeric};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
};
use tokio::{
    task,
    net::{TcpListener, TcpStream},
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{error::Result as WsResult, protocol::Message}
};


type SharedMap<K, V> = Arc<RwLock<HashMap<K, V>>>;

#[derive(Debug, Snafu)]
pub enum RoomError {
    #[snafu(display("Room is at max capacity, connection failed"))]
    CapacityExceeded,
    #[snafu(display("Capacity must be at least current number of connections"))]
    CapacityTooLow,
    #[snafu(display("Wrong code entered, connection failed"))]
    WrongCode,
}

pub struct Room {
    capacity: RwLock<u32>,
    code: RwLock<String>,
}

impl Room {
    pub fn default() -> Self {
        Room {
            capacity: RwLock::new(10),
            code: RwLock::new(gen_str(6)),
        }
    }

    pub fn get_code(&self) -> String {
        self.code.read().unwrap().clone()
    }

    pub fn change_capacity(&self, n: i32) -> Result<(), RoomError> {
        Ok(())
    }
}

/// Generate random string of given length
fn gen_str(len: usize) -> String {
    let mut rng = thread_rng();
    std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(len)
        .collect()
}

async fn handle_connection<T>(
    room: Arc<Room>,
    room_tx: Sender<T>,
    tx_map: SharedMap<SocketAddr, Sender<T>>,
    raw_stream: TcpStream,
    addr: SocketAddr
) where T: Send + Sync
{
    let socket = accept_async(raw_stream).await.expect("Handshake failed");
    println!("WebSocket connection established: {}", addr);
    let (sink, stream) = socket.split();

    let (tx, rx) = channel::<T>(50);
    tx_map.write().unwrap().insert(addr, tx);

    let connection_event = stream.try_for_each(|msg| {
        info!("Received a message from {}", addr);
        let senders = tx_map.read().unwrap();

        future::ok(())
    });
}

async fn handle_state<'a, T>(
    room: Arc<Room>,
    room_rx: Receiver<T>,
    entity_map: SharedMap<SocketAddr, Entity>
) where T: Send + Sync
{
    // Get message from connections

    // Update state via simulation
    task::spawn_blocking(||{});
}

pub async fn room_process(listener: TcpListener) -> WsResult<()> {
    // Listener should be passed into this function
    // let listener = TcpListener::bind("127.0.0.1:0").await?;
    // info!("Listening on: {}", listener.local_addr().unwrap());

    // Sender end of channels for connection tasks to communicate
    let tx_map: SharedMap<SocketAddr, Sender<Message>> = Arc::new(RwLock::new(HashMap::new()));

    // Entities updated by central thread to manage state
    let entity_map: SharedMap<SocketAddr, Entity> = Arc::new(RwLock::new(HashMap::new()));

    let (room_tx, room_rx) = channel::<Message>(50);

    // Room configurations
    let room: Arc<Room> = Arc::new(Room::default());

    // Spawn task to manage room state
    task::spawn(handle_state(room.clone(), room_rx, entity_map.clone()));

    // Handle each new connection
    while let Ok((raw_stream, addr)) = listener.accept().await {
        // Validate connection

        // Create entity and add it to entity_map
        let entity = Entity::new(String::from("placeholder"), false, (0.0, 0.0, 0.0));
        entity_map.write().unwrap().insert(addr, entity);

        // Spawn a task for the new connection
        task::spawn(handle_connection(room.clone(), room_tx.clone(), tx_map.clone(),
                                      raw_stream, addr));
    }

    Ok(())
}
