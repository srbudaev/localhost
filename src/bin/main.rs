use localhost::application::config::loader::ConfigLoader;
use localhost::application::server::server_manager::ServerManager;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <config_file>", args[0]);
        std::process::exit(1);
    }

    let config_path = &args[1];
    let config = match ConfigLoader::load(config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error parsing config file: {}", e);
            std::process::exit(1);
        }
    };

    let mut server_manager = match ServerManager::new(config) {
        Ok(sm) => sm,
        Err(e) => {
            eprintln!("Error starting server: {}", e);
            std::process::exit(1);
        }
    };

    // Print server information
    server_manager.print_server_info();

    if let Err(e) = server_manager.run() {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}

