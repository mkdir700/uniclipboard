![uniclipboard](https://socialify.git.ci/mkdir700/uniclipboard/image?description=1&descriptionEditable=%E4%B8%80%E4%B8%AA%E8%B7%A8%E5%B9%B3%E5%8F%B0%E5%89%AA%E5%88%87%E6%9D%BF%E5%85%B1%E4%BA%AB%E5%B7%A5%E5%85%B7%EF%BC%8C%E6%97%A8%E5%9C%A8%E6%89%93%E9%80%A0%E6%97%A0%E7%BC%9D%E7%9A%84%E5%89%AA%E5%88%87%E6%9D%BF%E4%BD%93%E9%AA%8C&font=Raleway&language=1&name=1&owner=1&pattern=Circuit%20Board&theme=Auto)

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
    <a href="https://codecov.io/gh/mkdir700/uniclipboard">
      <img src="https://img.shields.io/codecov/c/github/mkdir700/uniclipboard/master?style=flat-square" />
    </a>
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
- 支持多设备。可同时在 Windows、macOS 和 Linux 系统上使用
- 安全可靠。使用加密传输确保数据安全
- 多媒体支持。不仅同步文本，还支持图片，其他格式待支持
- 开源免费。代码开源，用户可自由使用和贡献

## 快速开始

假设你想让两台设备 A 和 B 共享剪切板，他们的 IP 地址分别为 `172.11.1.175` 和 `172.12.0.12`。

在设备 A 的终端运行以下命令，根据提示进行配置:

```bash
./uniclipboard
```

输出:

```text
欢迎使用 UniClipboard！
版本: 0.1.1
本地 IP 地址:  172.11.1.175

欢迎使用配置向导！
✔ 请选择同步方式 · WebSocket
✔ 请输入本机服务端口 · 8113
✔ 是否连接到另一台设备？ · no
```

注意：按 `n` 键，选择不连接到另一台设备

在设备 B 的终端运行命令

```bash
./uniclipboard -i
```

与设备 A 的配置不同，设备 B 需要手动配置 A 设备的 IP 地址和端口（自动发现功能还没实现）。

输出:

```text
欢迎使用 UniClipboard！
版本: 0.1.1
本地 IP 地址:  172.12.0.12

欢迎使用配置向导！
✔ 请选择同步方式 · WebSocket
✔ 请输入本机服务端口 · 8113
✔ 是否连接到另一台设备？ · yes
✔ 请输入对等设备 IP · 172.11.1.175
✔ 请输入对等设备端口 · 8113
```


首次启动之后，后续直接使用以下命令启动即可:

```bash
./uniclipboard
```

如果 IP 地址变动，需要重新配置。

## 使用

程序启动后，在任意设备上复制内容，然后在另一台设备上粘贴即可。

## TODO

- [ ] 支持文件/文件夹同步 
- [ ] UI 界面
- [ ] 支持自动发现设备，无需手动配置 IP 地址
- [ ] 支持网页端同步，以覆盖移动端设备

## 限制

- 目前仅支持 Windows、macOS 和 Linux 系统
- WebDav 的同步功能暂时搁置，个人精力有限
- 暂时不支持开机自启动

## 贡献

欢迎提交 PR 和 Issue，作为 Rust 新手还需要大家多多指教，欢迎大家提意见和建议。

## License

[Apache License 2.0](./LICENSE)
