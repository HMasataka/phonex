use clap::Parser;
use dotenvy::dotenv;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[clap(short = 'p', long = "port", env, default_value = "3000")]
    port: u16,
    #[clap(short = 'c', long = "cert", env)]
    cert_path: String,
    #[clap(short = 'k', long = "private", env)]
    private_key_path: String,
}

fn main() {
    let _ = dotenv();

    let args = Args::parse();

    println!("{:?}", args);
}
