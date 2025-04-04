use std::error::Error;
use std::io;
use std::sync::Arc;
use tokio;
use RAC::{check_single_instance, initialize_services, ClickService, ClickServiceConfig, Menu};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if !check_single_instance() {
        eprintln!("Application is already running!");
        println!("\nPress Enter to exit...");
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        std::process::exit(1);
    }

    match initialize_services() {
        Ok(()) => {
            let click_service = Arc::new(ClickService::new(ClickServiceConfig::default()));
            let click_executor = Arc::clone(&click_service.click_executor);
            let mut menu = Menu::new(Arc::clone(&click_service), click_executor);
            menu.show_main_menu();
        }
        Err(error_message) => {
            eprintln!("System validation failed: {}", error_message);
            println!("\nPress Enter to exit...");
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            std::process::exit(1);
        }
    }

    Ok(())
}