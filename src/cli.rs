use crate::Config;
use anyhow::{Context, Result};
use clap::Parser;
use dialoguer::{Input, Password};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(long)]
    pub interactive: bool,

    #[arg(long)]
    pub webdav_url: Option<String>,

    #[arg(long)]
    pub webdav_username: Option<String>,

    #[arg(long)]
    pub webdav_password: Option<String>,

    #[arg(short = 's', long)]
    pub server: Option<bool>, // 是否作为 server 端

    #[arg(short = 'i', long)]
    pub server_ip: Option<String>,

    #[arg(short = 'p', long)]
    pub server_port: Option<u16>,
}

pub fn parse_args() -> Args {
    Args::parse()
}

#[allow(dead_code)]
pub fn interactive_input(config: &mut Config) -> Result<()> {
    let select_websocket_sync: String = Input::new()
        .with_prompt("请选择同步方式, 1-websocket, 2-webdav")
        .allow_empty(true)
        .interact_text()
        .context("无法读取同步方式")?;

    let select_websocket_sync = if select_websocket_sync.is_empty() {
        "1".to_string()
    } else {
        select_websocket_sync
    };

    // 如果选择 websocket 同步，则不需要输入 WebDAV URL、用户名和密码
    if select_websocket_sync == "1" {
        let is_server: bool = Input::new()
            .with_prompt("是否作为服务端")
            .with_initial_text(config.is_server.unwrap_or(true).to_string())
            .allow_empty(false)
            .interact_text()
            .context("无法读取是否作为 server 端")?;
        config.is_server = Some(is_server);

        if is_server == true {
            let default_server_ip: String;
            if config.websocket_server_addr.is_some() {
                default_server_ip = config
                    .websocket_server_addr
                    .clone()
                    .unwrap_or("0.0.0.0".to_string());
            } else {
                default_server_ip = "0.0.0.0".to_string();
            }
    
            let server_port: u16 = Input::new()
                .with_prompt("请输入服务端端口")
                .with_initial_text(config.websocket_server_port.unwrap_or(8113).to_string())
                .allow_empty(false)
                .interact_text()
                .context("无法读取 server 端端口")?;
    
            config.websocket_server_addr = Some(default_server_ip);
            config.websocket_server_port = Some(server_port);
        } else {
            let default_server_ip: String;
            if config.connect_websocket_server_addr.is_some() {
                default_server_ip = config
                    .connect_websocket_server_addr
                    .clone()
                    .unwrap_or("".to_string());
            } else {
                default_server_ip = "".to_string();
            }
            
            let server_ip: String = Input::new()
                .with_prompt("请输入服务端 IP")
                .with_initial_text(&default_server_ip)
                .allow_empty(false)
                .interact_text()
                .context("无法读取服务端 IP")?;

            let server_port: u16 = Input::new()
                .with_prompt("请输入服务端端口")
                .with_initial_text(config.websocket_server_port.unwrap_or(8113).to_string())
                .allow_empty(false)
                .interact_text()
                .context("无法读取服务端端口")?;

            config.connect_websocket_server_addr = Some(server_ip);
            config.connect_websocket_server_port = Some(server_port);
        }
        
    } else if select_websocket_sync == "2" {
        let default_webdav_url: String;
        if config.webdav_url.is_some() {
            default_webdav_url = config.webdav_url.clone().unwrap_or("".to_string());
        } else {
            default_webdav_url = "".to_string();
        }

        let webdav_url: String = Input::new()
            .with_prompt("请输入 WebDAV URL")
            .with_initial_text(&default_webdav_url)
            .allow_empty(true)
            .interact_text()
            .context("无法读取 WebDAV URL")?;

        // 判断 webdav_url 是否为空，如果为空，则抛出错误
        if webdav_url.is_empty() {
            return Err(anyhow::anyhow!("WebDAV URL 不能为空"));
        }

        let default_username: String;
        if config.username.is_some() {
            default_username = config.username.clone().unwrap_or("".to_string());
        } else {
            default_username = "".to_string();
        }

        let webdav_username: String = Input::new()
            .with_prompt("请输入用户名")
            .with_initial_text(&default_username)
            .allow_empty(true)
            .interact_text()
            .context("无法读取用户名")?;

        // 判断 webdav_username 是否为空，如果为空，则抛出错误
        if webdav_username.is_empty() {
            return Err(anyhow::anyhow!("用户名不能为空"));
        }

        let webdav_password: String = Password::new()
            .with_prompt("请输入密码")
            .with_confirmation("请确认密码", "密码不匹配")
            .allow_empty_password(true)
            .interact()
            .context("无法读取密码")?;

        // 判断 webdav_password 是否为空，如果为空，则抛出错误
        if webdav_password.is_empty() {
            return Err(anyhow::anyhow!("密码不能为空"));
        }

        if !webdav_url.is_empty() {
            config.webdav_url = Some(webdav_url);
        }
        if !webdav_username.is_empty() {
            config.username = Some(webdav_username);
        }
        if !webdav_password.is_empty() {
            config.password = Some(webdav_password);
        }
    } else {
        return Err(anyhow::anyhow!("同步方式错误"));
    }

    Ok(())
}
