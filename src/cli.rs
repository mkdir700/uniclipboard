use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub webdav_url: String,

    #[arg(short, long)]
    pub username: String,

    #[arg(short, long)]
    pub password: String,
}

pub fn parse_args() -> Args {
    Args::parse()
}