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
    io,
    task,
    net::{TcpListener, TcpStream},
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{error::Result as WsResult, protocol::Message}
};


type Shared<T> = Arc<RwLock<T>>;
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

/// Basic room configurations read by every entity, but only host can write.
struct Room {
    code: String,
    capacity: usize,
    entity_map: SharedMap<SocketAddr, Entity>,
}

/// This pattern makes it impossible to keep the lock across an await.
/// https://users.rust-lang.org/t/mutable-struct-fields-with-async-await/45395/7
#[derive(Clone)]
struct RoomHandle {
    inner: Shared<Room>,
}

impl RoomHandle {
    pub fn default(entity_map: SharedMap<SocketAddr, Entity>) -> Self {
        let room = Arc::new(RwLock::new(
            Room {
                capacity: 10,
                code: gen_str(6),
                entity_map,
            }));
        Self {inner: room}
    }

    fn write_lock<F, T>(&self, func: F) -> T
    where F: FnOnce(&mut Room) -> T,
    {
        let mut guarded = self.inner.write().unwrap(); // acquire write guard, aka lock
        let result = func(&mut *guarded);
        drop(guarded); // release lock
        result
    }

    pub fn change_capacity(&self, new_capacity: usize) -> Result<(), RoomError> {
        self.write_lock(|room| {
            if new_capacity >= room.entity_map.read().unwrap().keys().len() {
                room.capacity = new_capacity;
                return Ok(());
            }
            Err(RoomError::CapacityTooLow)
        })
    }

    pub fn authorize(&self, input_code: String) -> Result<(), RoomError> {
        let mut guarded = self.inner.read().unwrap();
        if input_code != guarded.code {
            return Err(RoomError::WrongCode);
        }
        if guarded.entity_map.read().unwrap().keys().len() >= guarded.capacity {
            return Err(RoomError::CapacityExceeded);
        }
        Ok(())
    }
}

/// Generate random string of given length.
fn gen_str(len: usize) -> String {
    let mut rng = thread_rng();
    std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(len)
        .collect()
}

async fn handle_connection<T>(
    room: RoomHandle,
    state_tx: Sender<T>,
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
        // Rate limiting?
        info!("Received a message from {}", addr);
        tx.send_all(&mut stream).await.expect("Failed to send from tx");

        future::ok(())
    });
}

async fn handle_state<'a, T>(
    room: RoomHandle,
    state_rx: Receiver<T>,
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
    let tx_map = Arc::new(RwLock::new(HashMap::new()));

    // Entities updated by central thread to manage state
    let entity_map = Arc::new(RwLock::new(HashMap::new()));

    // Channel to communicate with state thread
    let (state_tx, state_rx) = channel::<Message>(100);

    // Room configurations
    let room: RoomHandle = RoomHandle::default(entity_map.clone());

    // Spawn task to manage room state
    task::spawn(handle_state(room.clone(), state_rx, entity_map.clone()));

    // Handle each new connection
    while let Ok((raw_stream, addr)) = listener.accept().await {
        // Authorize connection
        let mut authorized = false;
        let mut msg = vec![0; 1024];
        loop {
            raw_stream.readable().await.expect("Raw stream not readable");
            match raw_stream.try_read(&mut msg) {
                Ok(m) => {
                    authorized = match room.authorize(String::from("hello")) {
                        Ok(()) => true,
                        Err(e) => {
                            eprintln!("Error accepting connection: {}", e);
                            false
                        },
                    };
                    break;
                },
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    continue;
                },
                Err(e) => {
                    break;
                },
            }
        }

        if authorized {
            // Create entity and add it to entity_map
            let entity = Entity::new(String::from("placeholder"), false, (0.0, 0.0, 0.0));
            entity_map.write().unwrap().insert(addr, entity);

            // Spawn a task for the new connection
            let fut = task::spawn(handle_connection(room.clone(), state_tx.clone(), tx_map.clone(),
                                                    raw_stream, addr));
        }
    }

    Ok(())
}
