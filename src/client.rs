use uuid::Uuid;

#[derive(Debug)]
pub struct Client {
    server_addr: String,
    uuid: Uuid,
}

impl Client {
    pub fn new(server_addr: String, uuid: Uuid) -> Self {
        Client { server_addr, uuid }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
