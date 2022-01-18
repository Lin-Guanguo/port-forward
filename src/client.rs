use byteorder::ByteOrder;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::protocols::*;

#[derive(Debug, Error)]
pub enum Error {
    #[error("man connection closed")]
    MainConnectionClose,

    #[error("unkown message type {0}")]
    UnkonwMessageType(u8),
}

#[derive(Debug)]
pub struct Client {
    server_addr: String,
    uuid: Uuid,
}

impl Client {
    pub fn new(server_addr: String, uuid: Uuid) -> Self {
        Client { server_addr, uuid }
    }

    pub async fn run(&'static self) -> anyhow::Result<()> {
        let mut main_connection = TcpStream::connect(&self.server_addr).await?;
        let mut msg = [MSG_FIRST_CONNECTION];
        main_connection.write_all(&msg).await?;
        main_connection.write_all(self.uuid.as_bytes()).await?;
        main_connection.flush().await?;

        loop {
            let read_len = main_connection.read(&mut msg).await?;
            if read_len == 0 {
                break Err(Error::MainConnectionClose.into());
            }
            let msg = msg[0];
            match msg {
                MSG_NEW_TUNNEL => {
                    let mut port = [0u8; 4];
                    let mut uuid = [0u8; 16];
                    main_connection.read_exact(&mut port).await?;
                    main_connection.read_exact(&mut uuid).await?;
                    let port = byteorder::NetworkEndian::read_i32(&port);
                    let uuid = Uuid::from_bytes(uuid);
                    let _ = tokio::spawn(async move {
                        let r = self.new_tunnel(port, uuid).await;
                        if let Err(e) = r {
                            println!("new tunnel error, port: {:?}, error: {:?}", port, e);
                        }
                    });
                }
                i => {
                    return Err(Error::UnkonwMessageType(i).into());
                }
            }
        }
    }

    async fn new_tunnel(&self, port: i32, uuid: Uuid) -> anyhow::Result<()> {
        let mut connection = TcpStream::connect(&self.server_addr).await?;
        let msg = [MSG_TUNNEL_CONNECTION];
        connection.write_all(&msg).await?;
        connection.write_all(uuid.as_bytes()).await?;
        connection.flush().await?;
        let local_connection = TcpStream::connect(format!("127.0.0.1:{}", port)).await?;

        let (r1, w1) = connection.into_split();
        let (r2, w2) = local_connection.into_split();
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
    }
}
