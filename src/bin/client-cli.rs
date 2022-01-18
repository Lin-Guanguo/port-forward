use port_forward::client::Client;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let lua = rlua::Lua::new();
    let lua_ret = lua.context(|ctx| {
        let config_file = std::fs::read_to_string("client-config.lua")?;
        ctx.load(&config_file).exec()?;
        let g = ctx.globals();
        let config: rlua::Table = g.get("config")?;

        let server_addr: String = config.get("server_addr")?;
        let uuid: String = config.get("uuid")?;
        let uuid = Uuid::parse_str(&uuid)?;

        anyhow::Ok(Box::new(Client::new(server_addr, uuid)))
    });

    let client = match lua_ret {
        Err(e) => panic!("lua config file error {}", e),
        Ok(client) => Box::leak::<'static>(client),
    };

    println!("client config: {:#?}", client);

    if let Err(e) = client.run().await {
        panic!("client run error {}", e);
    }
}
