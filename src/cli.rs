use clap::Parser;
use dialoguer::{Input, Password};
use anyhow::{Result, Context};
use crate::config::CONFIG;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub webdav_url: Option<String>,

    #[arg(short, long)]
    pub username: Option<String>,

    #[arg(short, long)]
    pub password: Option<String>,

    #[arg(short, long)]
    pub interactive: bool,
}

pub fn parse_args() -> Args {
    Args::parse()
}

#[allow(dead_code)]
pub fn interactive_input() -> Result<(String, String, String)> {
    let config = CONFIG.read().unwrap();

    let webdav_url: String = Input::new()
        .with_prompt("请输入 WebDAV URL")
        .with_initial_text(&config.webdav_url)
        .allow_empty(true)
        .interact_text()
        .context("无法读取 WebDAV URL")?;

    let webdav_url = if webdav_url.is_empty() {
        config.webdav_url.clone()
    } else {
        webdav_url
    };

    let username: String = Input::new()
        .with_prompt("请输入用户名")
        .with_initial_text(&config.username)
        .allow_empty(true)
        .interact_text()
        .context("无法读取用户名")?;

    let username = if username.is_empty() {
        config.username.clone()
    } else {
        username
    };

    let password: String = Password::new()
        .with_prompt("请输入密码")
        .with_confirmation("请确认密码", "密码不匹配")
        .allow_empty_password(true)
        .interact()
        .context("无法读取密码")?;

    let password = if password.is_empty() {
        config.password.clone()
    } else {
        password
    };

    Ok((webdav_url, username, password))
}