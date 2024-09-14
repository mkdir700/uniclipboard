# UniClipboard

UniClipboard 是一款基于 WebDAV 协议的跨设备剪贴板同步工具,让您轻松实现多台设备间的剪贴板共享。

## 功能

- 使用简单。程序启动后无需额外操作，自动在后台运行
- 云端同步。通过 WebDAV 协议实现跨设备剪贴板内容的实时同步
- 支持多设备。可同时在 Windows、macOS 和 Linux 系统上使用
- 安全可靠。使用加密传输确保数据安全
- 自定义同步间隔。可根据需求调整同步频率，平衡实时性和资源消耗
- 多媒体支持。不仅同步文本，还支持图片，其他格式待支持
- 开源免费。代码开源，用户可自由使用和贡献

## 快速开始

### 交互式启动

运行以下命令,然后按提示输入 WebDAV 连接信息:

```bash
./uniclipboard -i
```

示例输出:

```text
请输入 WebDAV URL: https://example.com
请输入用户名: username
请输入密码: [hidden]
[2024-09-13T08:13:06Z INFO  uniclipboard] Connected to WebDAV server, device_id: be4cf1
```

### 命令行启动

```bash
uniclipboard --webdav-url="https://xxx.com" --username="test" --password="test"
```

### WebDAV服务推荐

[坚果云](https://www.jianguoyun.com/)提供免费的 WebDAV 服务，空间足够日常使用。

[如何获取坚果云WebDAV地址和凭据?](https://help.jianguoyun.com/?p=2064)

## 配置

首次成功连接后，程序会在本地生成配置文件:

- Linux: `~/.config/uniclipboard/config.toml`
- Windows: `%appdata%/uniclipboard/config.toml`
- MacOS: `~/Library/Application Support/uniclipboard/config.toml`

配置文件内容示例:

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