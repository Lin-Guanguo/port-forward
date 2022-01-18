use port_forward::server::{Server, User};
use uuid::Uuid;

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

    println!("server {:#?}", server);
    if let Err(e) = server.run().await {
        panic!("server run error {}", e);
    }
}
