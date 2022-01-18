use byteorder::ByteOrder;
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Mutex;
use thiserror::Error;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use crate::protocols::*;

#[derive(Debug)]
pub struct User {
    uuid: Uuid,
    ports: Vec<(i32, i32)>,
}

impl User {
    pub fn new(uuid: Uuid) -> Self {
        User {
            uuid,
            ports: Vec::new(),
        }
    }

    pub fn add_port(&mut self, client_port: i32, server_port: i32) {
        self.ports.push((client_port, server_port));
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("duplicate conncetion with same uuid")]
    ConnectionDuplicate(Uuid),

    #[error("unkonw user")]
    UnkonwUser(Uuid),

    #[error("unkown message type {0}")]
    UnkonwMessageType(u8),

    #[error("connect to error tunnel session id")]
    ErrorSessionId(Uuid),
}

#[derive(Debug)]
pub struct Server {
    server_port: i32,
    users: HashMap<Uuid, User>,
    online_users: Mutex<HashSet<Uuid>>,
    waiting_tunnel: Mutex<HashMap<Uuid, TcpStream>>,
}

impl Server {
    pub fn new() -> Server {
        Server {
            server_port: 8077,
            users: HashMap::new(),
            online_users: Mutex::new(HashSet::new()),
            waiting_tunnel: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_port(&mut self, port: i32) {
        self.server_port = port;
    }

    pub fn add_user(&mut self, user: User) {
        self.users.insert(user.uuid.clone(), user);
    }

    pub async fn run(&'static self) -> anyhow::Result<()> {
        let main_listener = Server::listen(self.server_port).await?;
        loop {
            let (connection, addr) = main_listener.accept().await?;
            println!("connetion from {:?}", addr);
            let _ = tokio::spawn(async move {
                let r = self.handle_connection(connection).await;
                if let Err(e) = r {
                    println!("handle connection, addr: {:?}, error: {:?}", addr, e);
                }
            });
        }
    }

    async fn handle_connection(&self, mut connection: TcpStream) -> anyhow::Result<()> {
        let mut request_type = [0u8];
        connection.read_exact(&mut request_type).await?;
        let request_type = request_type[0];
        match request_type {
            MSG_FIRST_CONNECTION => self.handle_first_conncetion(connection).await,
            MSG_TUNNEL_CONNECTION => self.handle_tunnel_conncetion(connection).await,
            _ => Err(Error::UnkonwMessageType(request_type).into()),
        }
    }

    async fn handle_first_conncetion(&self, mut main_connection: TcpStream) -> anyhow::Result<()> {
        let mut uuid = [0u8; 16];
        main_connection.read_exact(&mut uuid).await?;
        let uuid = Uuid::from_bytes(uuid);

        let not_existing = self.online_users.lock().unwrap().insert(uuid);
        if !not_existing {
            // TODO: respond error msg
            return Err(Error::ConnectionDuplicate(uuid).into());
        }

        let (shutdown_sender, shutdown_receiver) = broadcast::channel::<()>(1);
        let (new_tunnel_sender, mut new_tunnel_receiver) =
            mpsc::channel::<(i32, TcpStream, SocketAddr)>(16);

        if let Some(user) = self.users.get(&uuid) {
            user.ports.iter().for_each(|(client_port, server_port)| {
                let shutdown = shutdown_sender.subscribe();
                let new_tunnel = new_tunnel_sender.clone();
                let client_port = *client_port;
                let server_port = *server_port;
                let _ = tokio::spawn(async move {
                    let r = Server::tunnle_listener(shutdown, new_tunnel, client_port, server_port)
                        .await;
                    if let Err(e) = r {
                        println!(
                            "tunnel listner error, listen port: {:?}, error: {:?}",
                            server_port, e
                        );
                    }
                });
            });
            drop(shutdown_receiver);
            drop(new_tunnel_sender);
        } else {
            return Err(Error::UnkonwUser(uuid).into());
        }

        loop {
            let mut byte_buf = [0u8];
            tokio::select! {
                new_tunnel = new_tunnel_receiver.recv() => {
                    if let Some((client_port, connection, addr)) = new_tunnel {
                        println!("new tunnel from: {:?}", addr);
                        let mut msg_buf = [0u8; 1+4];
                        let session_id: [u8; 16] = rand::thread_rng().gen();

                        msg_buf[0] = MSG_NEW_TUNNEL;
                        byteorder::BigEndian::write_i32(&mut msg_buf[1..], client_port);
                        main_connection.write_all(&msg_buf).await?;
                        main_connection.write_all(&session_id).await?;
                        main_connection.flush().await?;

                        let session_id = Uuid::from_bytes(session_id);
                        self.waiting_tunnel.lock().unwrap().insert(session_id, connection);
                    } else { todo!() }
                }
                read_len = main_connection.read(&mut byte_buf) => { // use read, because read_exect is not cancel safe
                    let read_len = read_len?;
                    if read_len == 0 {
                        shutdown_sender.send(())?;
                        break Ok(())
                    } else {
                        todo!("beat check")
                    }
                }
            }
        }
        // TODO: send shut down
    }

    async fn handle_tunnel_conncetion(&self, mut connection: TcpStream) -> anyhow::Result<()> {
        let mut session_id = [0u8; 16];
        connection.read_exact(&mut session_id).await.unwrap();
        let session_id = Uuid::from_bytes(session_id);

        let lock = self.waiting_tunnel.lock().unwrap().remove(&session_id);
        if let Some(connection2) = lock {
            let (r1, w1) = connection.into_split();
            let (r2, w2) = connection2.into_split();
            let j1 = tokio::spawn(async move {
                let (mut r1, mut w2) = (r1, w2);
                let r1 = tokio::io::copy(&mut r1, &mut w2).await;
                let r2 = w2.shutdown().await;
                r1?;
                r2?;
                anyhow::Ok(())
            });
            let j2 = tokio::spawn(async move {
                let (mut r2, mut w1) = (r2, w1);
                let r1 = tokio::io::copy(&mut r2, &mut w1).await;
                let r2 = w1.shutdown().await;
                r1?;
                r2?;
                anyhow::Ok(())
            });
            let r = tokio::join!(j1, j2);
            if let Err(e) = r.0 {
                println!("tunnel transfer error1: {:?}", e)
            }
            if let Err(e) = r.1 {
                println!("tunnel transfer error2: {:?}", e)
            }
            Ok(())
        } else {
            Err(Error::ErrorSessionId(session_id).into())
        }
    }

    async fn tunnle_listener(
        mut shutdown_receiver: broadcast::Receiver<()>,
        new_tunnel_sender: mpsc::Sender<(i32, TcpStream, SocketAddr)>,
        client_port: i32,
        server_port: i32,
    ) -> anyhow::Result<()> {
        let tunnel_listener = Server::listen(server_port).await?;
        loop {
            tokio::select! {
                tunnel = tunnel_listener.accept() => {
                    let tunnel = tunnel?;
                    new_tunnel_sender.send((client_port, tunnel.0, tunnel.1)).await?;
                }
                _ = shutdown_receiver.recv() => {
                    break Ok(())
                }
            }
        }
    }

    async fn listen(port: i32) -> tokio::io::Result<TcpListener> {
        TcpListener::bind(format!("0.0.0.0:{}", port)).await
    }
}
