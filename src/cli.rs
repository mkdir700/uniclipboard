use crate::Config;
use anyhow::Result;
use clap::Parser;
use console::{style, Emoji};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short = 'i', long)]
    pub interactive: bool,

    #[arg(long)]
    pub webdav_url: Option<String>,

    #[arg(long)]
    pub webdav_username: Option<String>,

    #[arg(long)]
    pub webdav_password: Option<String>,

    #[arg(long)]
    pub server: bool,

    #[arg(long)]
    pub server_ip: Option<String>,

    #[arg(long)]
    pub server_port: Option<u16>,
}

pub fn parse_args() -> Args {
    Args::parse()
}

#[allow(dead_code)]
pub fn interactive_input(config: &mut Config) -> Result<()> {
    println!("{}", style("欢迎使用配置向导！").cyan().bold());

    let theme = ColorfulTheme::default();

    let sync_options = vec!["WebSocket", "WebDAV"];
    let sync_selection = Select::with_theme(&theme)
        .with_prompt("请选择同步方式")
        .default(0)
        .items(&sync_options)
        .interact()?;

    if sync_selection == 0 {
        // WebSocket 配置
        config.is_server = Some(
            Confirm::with_theme(&theme)
                .with_prompt("是否作为服务端？")
                .default(config.is_server.unwrap_or(true))
                .interact()?,
        );

        if config.is_server.unwrap() {
            // 服务端配置
            config.websocket_server_addr = Some(
                Input::with_theme(&theme)
                    .with_prompt("请输入服务端 IP")
                    .default("0.0.0.0".to_string())
                    .interact_text()?,
            );

            config.websocket_server_port = Some(
                Input::with_theme(&theme)
                    .with_prompt("请输入服务端端口")
                    .default(8113)
                    .interact()?,
            );
        } else {
            // 客户端配置
            config.connect_websocket_server_addr = Some(
                Input::with_theme(&theme)
                    .with_prompt("请输入服务端 IP")
                    .interact_text()?,
            );

            config.connect_websocket_server_port = Some(
                Input::with_theme(&theme)
                    .with_prompt("请输入服务端端口")
                    .default(8113)
                    .interact()?,
            );
        }
    } else {
        // WebDAV 配置
        config.webdav_url = Some(
            Input::with_theme(&theme)
                .with_prompt("请输入 WebDAV URL")
                .interact_text()?,
        );

        config.username = Some(
            Input::with_theme(&theme)
                .with_prompt("请输入用户名")
                .interact_text()?,
        );

        config.password = Some(
            Password::with_theme(&theme)
                .with_prompt("请输入密码")
                .with_confirmation("请确认密码", "密码不匹配")
                .interact()?,
        );
    }

    // 新的配置完成提示
    println!("\n{}", style("配置完成！").green().bold());
    println!(
        "{}",
        style("===========================================").cyan()
    );
    println!(
        "{}  {}",
        Emoji("🎉", "!"),
        style("恭喜！您的 UniClipboard 已准备就绪！").green().bold()
    );
    println!(
        "{}  {}",
        Emoji("💡", "*"),
        style("提示：随时使用 --help 查看更多选项").italic()
    );
    println!(
        "{}",
        style("===========================================").cyan()
    );

    Ok(())
}
