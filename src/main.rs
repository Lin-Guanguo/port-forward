use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::sync::{Arc, Mutex};
use thiserror::Error;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

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
}

#[derive(Debug)]
struct Server {
    users: HashMap<Uuid, User>,
    online_users: Mutex<HashSet<Uuid>>,
}

impl Server {
    fn new() -> Server {
        Server {
            users: HashMap::new(),
            online_users: Mutex::new(HashSet::new()),
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
            let _ = tokio::spawn(self.handle_connection(connection));
        }
    }

    async fn handle_connection(&self, mut connection: TcpStream) -> anyhow::Result<()> {
        let mut uuid = [0u8; 16];
        connection.read_exact(&mut uuid).await?;
        let uuid = Uuid::from_bytes(uuid);

        let not_existing = self.online_users.lock().unwrap().insert(uuid);
        if !not_existing {
            let r = [0u8];
            connection.write_all(&r).await?;
            return Err(Error::ConnectionDuplicate(uuid).into());
        }

        if let Some(user) = self.users.get(&uuid) {
            user.ports.iter().map(|(client_port, server_port)| todo!());
            Ok(())
        } else {
            Err(Error::UnkonwUser(uuid).into())
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
