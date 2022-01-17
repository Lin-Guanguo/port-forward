#[derive(Debug)]
struct User {
    uuid: uuid::Uuid,
    ports: Vec<(i32, i32)>,
}

impl User {
    fn new(uuid: uuid::Uuid) -> Self {
        User {
            uuid,
            ports: Vec::new(),
        }
    }

    fn add_port(&mut self, client_port: i32, server_port: i32) {
        self.ports.push((client_port, server_port));
    }
}

#[derive(Debug)]
struct Server {
    users: Vec<User>,
}

impl Server {
    fn new() -> Server {
        Server { users: Vec::new() }
    }

    fn add_user(&mut self, user: User) {
        self.users.push(user);
    }

    fn run(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

fn main() -> Result<(), uuid::Error> {
    let mut server = Server::new();

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

            let mut user = User::new(uuid::Uuid::parse_str(&uuid)?);
            for i in 0..ports_tbl.len()? {
                let s: String = ports_tbl.get(i + 1)?;
                let mut s = s.split(":");
                user.add_port(s.next().unwrap().parse()?, s.next().unwrap().parse()?)
            }
            (&mut server).add_user(user);
        }
        anyhow::Ok(())
    });
    if let Err(e) = lua_ret {
        panic!("lua config file error {}", e);
    }

    println!("server {:?}", server);
    if let Err(e) = server.run() {
        panic!("server run error {}", e);
    }

    Ok(())
}
