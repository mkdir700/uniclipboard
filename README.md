<image src="https://socialify.git.ci/mkdir700/uniclipboard/image?description=1&descriptionEditable=%E4%B8%80%E4%B8%AA%E5%89%AA%E5%88%87%E6%9D%BF%E5%85%B1%E4%BA%AB%E5%B7%A5%E5%85%B7%EF%BC%8C%E6%97%A8%E5%9C%A8%E6%89%93%E9%80%A0%E6%97%A0%E7%BC%9D%E7%9A%84%E5%89%AA%E8%B4%B4%E6%9D%BF%E4%BD%93%E9%AA%8C&font=Jost&logo=https%3A%2F%2Fs1.locimg.com%2F2024%2F09%2F27%2F778fb2adaebe7.png&name=1&owner=1&pattern=Floating%20Cogs&theme=Auto">

<div align="center">
  <br/>
    
  <a href="https://github.com/mkdir700/uniclipboard/releases">
    <img
      alt="Windows"
      src="https://img.shields.io/badge/-Windows-blue?style=flat-square&logo=data:image/svg+xml;base64,PHN2ZyB0PSIxNzI2MzA1OTcxMDA2IiBjbGFzcz0iaWNvbiIgdmlld0JveD0iMCAwIDEwMjQgMTAyNCIgdmVyc2lvbj0iMS4xIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHAtaWQ9IjE1NDgiIHdpZHRoPSIxMjgiIGhlaWdodD0iMTI4Ij48cGF0aCBkPSJNNTI3LjI3NTU1MTYxIDk2Ljk3MTAzMDEzdjM3My45OTIxMDY2N2g0OTQuNTEzNjE5NzVWMTUuMDI2NzU3NTN6TTUyNy4yNzU1NTE2MSA5MjguMzIzNTA4MTVsNDk0LjUxMzYxOTc1IDgwLjUyMDI4MDQ5di00NTUuNjc3NDcxNjFoLTQ5NC41MTM2MTk3NXpNNC42NzA0NTEzNiA0NzAuODMzNjgyOTdINDIyLjY3Njg1OTI1VjExMC41NjM2ODE5N2wtNDE4LjAwNjQwNzg5IDY5LjI1Nzc5NzUzek00LjY3MDQ1MTM2IDg0Ni43Njc1OTcwM0w0MjIuNjc2ODU5MjUgOTE0Ljg2MDMxMDEzVjU1My4xNjYzMTcwM0g0LjY3MDQ1MTM2eiIgcC1pZD0iMTU0OSIgZmlsbD0iI2ZmZmZmZiI+PC9wYXRoPjwvc3ZnPg=="
    />
  </a >  
  <a href="https://github.com/mkdir700/uniclipboard/releases">
    <img
      alt="MacOS"
      src="https://img.shields.io/badge/-MacOS-black?style=flat-square&logo=apple&logoColor=white"
    />
  </a >
  <a href="https://github.com/mkdir700/uniclipboard/releases">
    <img 
      alt="Linux"
      src="https://img.shields.io/badge/-Linux-purple?style=flat-square&logo=linux&logoColor=white" 
    />
  </a>

  <div>
    <a href="./LICENSE">
      <img
        src="https://img.shields.io/github/license/mkdir700/uniclipboard?style=flat-square"
      />
    </a >
    <a href="https://github.com/mkdir700/uniclipboard/releases">
      <img
        src="https://img.shields.io/github/v/release/mkdir700/uniclipboard?include_prereleases&style=flat-square"
      />
    </a >
    <a href="https://github.com/mkdir700/uniclipboard/releases">
      <img
        src="https://img.shields.io/github/downloads/mkdir700/uniclipboard/total?style=flat-square"
      />  
    </a >
  </div>

</div>

## 功能

- 使用简单。程序启动后无需额外操作，在后台静默运行
- 低资源消耗。仅占用极少的系统资源，不影响电脑性能
- 支持云端同步。不仅限于局域网，通过 WebDAV 协议实现跨设备剪贴板内容的实时同步
- 支持多设备。可同时在 Windows、macOS 和 Linux 系统上使用
- 安全可靠。使用加密传输确保数据安全
- 多媒体支持。不仅同步文本，还支持图片，其他格式待支持
- 开源免费。代码开源，用户可自由使用和贡献

## 快速开始

在仅用局域网的场景下，工具分为服务端和客户端，服务端负责接收客户端的剪切板内容以及转发其他客户端的剪切板内容，客户端负责将剪切板内容推送到服务端。

你可以选择任一台设备作为服务端（尽量选择长期在线的设备），其他设备作为客户端。

- 配置服务端

运行以下命令，根据提示进行配置:

```bash
./uniclipboard -i
```

示例输出:

```text
欢迎使用 UniClipboard！
版本: 0.1.1

欢迎使用配置向导！
✔ 请选择同步方式 · WebSocket
✔ 是否作为服务端？ · yes
✔ 请输入服务端 IP · 0.0.0.0
✔ 请输入服务端端口 · 8113

配置完成！
```

- 配置客户端

与服务端命令一致，仅在选择是否作为服务端时选择否（键盘输入 `n`），其他步骤与服务端一致。

首次启动后，会在本地生成配置文件:

- Linux: `~/.config/uniclipboard/config.toml`
- Windows: `%appdata%/uniclipboard/config.toml`
- MacOS: `~/Library/Application Support/uniclipboard/config.toml`

后续启动会自动加载配置文件，无需再次配置，直接输入以下命令即可:

```bash
./uniclipboard
```

## 使用

程序启动后，在任意设备上复制内容，然后在另一台设备上粘贴即可。

## TODO

- [ ] 支持文件/文件夹同步 
- [ ] UI 界面
- [ ] 支持自动发现设备，无需手动配置 IP 地址


## 限制

- 仅支持 Windows、macOS 和 Linux 系统

## 贡献

欢迎提交 PR 和 Issue，作为 Rust 新手还需要大家多多指教，欢迎大家提意见和建议。

## License

[Apache License 2.0](./LICENSE)