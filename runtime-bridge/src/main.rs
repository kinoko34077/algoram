use algoram_runtime_bridge::{RuntimeBridgeConfig, RuntimeBridgeServer, RuntimeBridgeService};
use std::env;
use std::error::Error;
use std::fs;
use std::net::SocketAddr;

fn argument_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == name)
        .map(|window| window[1].clone())
}

fn required_argument(args: &[String], name: &str) -> Result<String, String> {
    argument_value(args, name).ok_or_else(|| format!("missing required argument {name}"))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();

    if args.get(1).map(String::as_str) == Some("__fixture_child") {
        println!("{}", args.get(2).map(String::as_str).unwrap_or("BRIDGE_OK"));
        return Ok(());
    }

    let config_path = required_argument(&args, "--config")?;
    let allowed_origin = required_argument(&args, "--origin")?;
    let bind = argument_value(&args, "--bind")
        .unwrap_or_else(|| "127.0.0.1:39091".to_owned())
        .parse::<SocketAddr>()?;
    let bearer_token = argument_value(&args, "--token")
        .or_else(|| env::var("ALGORAM_BRIDGE_TOKEN").ok())
        .ok_or("runtime bridge requires --token or ALGORAM_BRIDGE_TOKEN")?;

    let config_text = fs::read_to_string(config_path)?;
    let config: RuntimeBridgeConfig = serde_json::from_str(&config_text)?;
    let service = RuntimeBridgeService::from_config(config)?;
    let server = RuntimeBridgeServer::bind(service, bind, bearer_token, allowed_origin)?;
    let local_addr = server.local_addr()?;

    eprintln!("Algoram runtime bridge listening on http://{local_addr}");
    server.serve()?;
    Ok(())
}
