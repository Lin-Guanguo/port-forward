struct Server {}

fn main() -> Result<(), uuid::Error> {
    let lua = rlua::Lua::new();
    let lua_ret = lua.context(|ctx| {
        let config_file = std::fs::read_to_string("server-config.lua")?;
        ctx.load(&config_file).exec()?;
        let g = ctx.globals();
        let config: rlua::Table = g.get("config")?;
        let users: rlua::Table = config.get("users")?;
        for i in 0..users.len()? {
            let user: rlua::Table = users.get(i + 1)?;
            let uuid: String = user.get("uuid")?;
            let ports_table: rlua::Table = user.get("ports")?;
            let mut ports: Vec<String> = vec![];
            for i in 0..ports_table.len()? {
                ports.push(ports_table.get(i + 1)?)
            }
            println!("uuid = {:?}", uuid);
            println!("ports = {:?}", ports);
        }
        anyhow::Ok(())
    });
    lua_ret.unwrap();

    Ok(())
}
