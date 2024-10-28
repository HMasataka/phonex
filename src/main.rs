mod err;

use clap::Parser;
use dotenvy::dotenv;
use err::PhonexError;
use tracing::instrument;
use tracing_error::ErrorLayer;
use tracing_spanned::SpanErr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

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

#[instrument(skip_all, name = "initialize_tracing_subscriber", level = "trace")]
fn initialize_tracing_subscriber() -> Result<(), SpanErr<PhonexError>> {
    tracing_subscriber::Registry::default()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_filter(tracing_subscriber::filter::LevelFilter::INFO),
        )
        .with(ErrorLayer::default())
        .try_init()
        .map_err(PhonexError::InitializeTracingSubscriber)?;

    Ok(())
}

#[instrument(skip_all, name = "main", level = "trace")]
fn main() -> Result<(), SpanErr<PhonexError>> {
    initialize_tracing_subscriber()?;

    let _ = dotenv();

    let args = Args::parse();

    println!("{:?}", args);

    Ok(())
}
