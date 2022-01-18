use byteorder::ByteOrder;
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use thiserror::Error;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

const MSG_FIRST_CONNECTION: u8 = 0;
const MSG_TUNNEL_CONNECTION: u8 = 1;
const MSG_CLIENT_BEAT: u8 = 3;

const MSG_NEW_TUNNEL: u8 = 128;
const MSG_SERVER_BEAT: u8 = 129;

#[derive(Debug)]
struct User {
    uuid: Uuid,
    ports: Vec<(i32, i32)>,
}

impl User {
    fn new(uuid: Uuid) -> Self {
        User {
            uuid,
            ports: Vec::new(),
        }
    }

    fn add_port(&mut self, client_port: i32, server_port: i32) {
        self.ports.push((client_port, server_port));
    }
}

#[derive(Debug, Error)]
enum Error {
    #[error("duplicate conncetion with same uuid")]
    ConnectionDuplicate(Uuid),

    #[error("unkonw user")]
    UnkonwUser(Uuid),

    #[error("unkown message type {0}")]
    UnkonwMessageType(u8),
}

#[derive(Debug)]
struct Server {
    users: HashMap<Uuid, User>,
    online_users: Mutex<HashSet<Uuid>>,
    waiting_tunnel: Mutex<HashMap<Uuid, TcpStream>>,
}

impl Server {
    fn new() -> Server {
        Server {
            users: HashMap::new(),
            online_users: Mutex::new(HashSet::new()),
            waiting_tunnel: Mutex::new(HashMap::new()),
        }
    }

    fn add_user(&mut self, user: User) {
        self.users.insert(user.uuid.clone(), user);
    }

    async fn run(&'static self) -> anyhow::Result<()> {
        let main_listener = Server::listen(8077).await?;
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
            let r = [0u8];
            main_connection.write_all(&r).await?;
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
        todo!()
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
        TcpListener::bind(format!("127.0.0.1:{}", port)).await
    }
}

#[tokio::main]
async fn main() {
    let mut server = Box::new(Server::new());

    let lua = rlua::Lua::new();
    let lua_ret = lua.context(|ctx| {
        let config_file = std::fs::read_to_string("server-config.lua")?;
        ctx.load(&config_file).exec()?;
        let g = ctx.globals();
        let config: rlua::Table = g.get("config")?;
        let users: rlua::Table = config.get("users")?;
        for i in 0..users.len()? {
            let user_tbl: rlua::Table = users.get(i + 1)?;
            let uuid: String = user_tbl.get("uuid")?;
            let ports_tbl: rlua::Table = user_tbl.get("ports")?;

            let mut user = User::new(Uuid::parse_str(&uuid)?);
            for i in 0..ports_tbl.len()? {
                let s: String = ports_tbl.get(i + 1)?;
                let mut s = s.split(":");
                user.add_port(s.next().unwrap().parse()?, s.next().unwrap().parse()?)
            }
            server.add_user(user);
        }
        anyhow::Ok(())
    });
    if let Err(e) = lua_ret {
        panic!("lua config file error {}", e);
    }

    let server = Box::leak::<'static>(server);

    println!("server {:?}", server);
    if let Err(e) = server.run().await {
        panic!("server run error {}", e);
    }
}
