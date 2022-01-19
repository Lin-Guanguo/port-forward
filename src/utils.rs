use tokio::io::{self, AsyncWriteExt};
use tokio::net::TcpStream;

pub async fn tunnel_transfer(mut c1: TcpStream, mut c2: TcpStream) -> anyhow::Result<()> {
    let (mut r1, mut w1) = c1.split();
    let (mut r2, mut w2) = c2.split();

    let client_to_server = async {
        io::copy(&mut r1, &mut w2).await?;
        w2.shutdown().await
    };

    let server_to_client = async {
        io::copy(&mut r2, &mut w1).await?;
        w1.shutdown().await
    };

    tokio::try_join!(client_to_server, server_to_client)?;

    Ok(())
}
