use anyhow::Result;
use clap::Parser;
use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    debug: Option<bool>,
    #[arg(long)]
    conf: Option<String>,
}

#[derive(Deserialize, Debug)]
struct AppConfig {
    debug: bool,
    log_file: String,
    log_level: String,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let config_path = args.conf.unwrap_or_else(|| "config.toml".to_string());
    let mut settings = Config::builder()
        .set_default("debug", false)?
        .set_default("log_file", "app.log")?
        .set_default("log_level", "info")?
        .add_source(File::with_name(&config_path).required(false))
        .add_source(Environment::with_prefix("CONF"))        
        .build()?;
    let config: AppConfig = settings.try_deserialize()?;
    println!("{:#?}", config);
    Ok(())
}
