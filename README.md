# Port-Forward 一个简单的端口转发服务器/客户端

## 使用方法

### 服务端

配置好`server-config.lua`

```bash
cargo run --bin port-forward-server
```

### 客户端

配置好`client-config.lua`

```bash
cargo run --bin port-forward-client
```

### 注意事项

* 配置文件由`lua`语言编写，可支持编程
* 需要首先启动服务器，再启动客户端
* 服务器可同时服务多个客户端

## TODO

* 服务器客户端间长连接的心跳检测未完成
* 服务器异常时客户端的错误处理
* 服务器的优雅关闭
* 服务器对于执行恶意操作的客户端，没有对应的防护行为。如收到新连接请求但不响应的客户端，请求uuid会一直保存在服务器，超时功能未完成。
* 长连接断开重新建立连接的功能未完成
