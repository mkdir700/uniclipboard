# uniclipboard

基于 WebDav 实现的剪切板共享程序，通过云端共享多台设备的剪切板。

## 功能

- 使用简单。程序启动后无需额外操作
- 配置方便。多台设备之间填写同一个分享码即可共享剪切板

## 快速开始

使用交互式的方式启动程序

```
./uniclipboard -i
```

键入连接地址、账户和密码：

```
请输入 WebDAV URL: https://example.com
请输入用户名: username
请输入密码: [hidden]
[2024-09-13T08:13:06Z INFO  uniclipboard] Connected to WebDAV server, device_id: be4cf1
```


或者使用一行命令搞定

```bash
uniclipboard --webdav-url="https://xxx.com" --username="test" --password="test"
```

## 配置

首次连接成功之后，配置文件将会在本地生成。

- Linux/Macos: `~/.config/uniclipboard/config.toml`
- Windows: `%appdata%/uniclipboard/config.toml`


```toml
device_id = "xxx"                    // 设备名称，保证唯一，由程序生成，请勿修改
webdav_url = "https://example.com"   // webdav 地址
username = "username"                // 用户名
password = "password"                // 密码
push_interval = 500                  // 推送周期，单位毫秒，500 即 0.5秒，下同
pull_interval = 500                  // 拉取周期
sync_interval = 500                  // 同步周期
enable_push = true                   // 开启推送，即将本地内容推送至云端
enable_pull = true                   // 启动拉取，即从云端拉取内容
```