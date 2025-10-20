use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use serde_json::Value;
use std::process::Command;

#[derive(Parser)]
#[command(name = "wash-manager")]
#[command(about = "Manage wasmCloud development environment", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check if wash is currently running
    Status,
    /// Start the development environment
    Start {
        /// Path to the component WASM file
        #[arg(short, long)]
        component: String,
        /// Component ID to use
        #[arg(short, long, default_value = "mcp-multi-tools")]
        id: String,
        /// Port to bind HTTP server to
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Stop the development environment and clean up
    Stop {
        /// Component ID to stop
        #[arg(short, long, default_value = "mcp-multi-tools")]
        id: String,
        /// Clean up configs
        #[arg(short, long, default_value = "true")]
        cleanup: bool,
    },
    /// Clean up persistent configurations and links
    Clean,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Status => check_status()?,
        Commands::Start { component, id, port } => start_env(&component, &id, port)?,
        Commands::Stop { id, cleanup } => stop_env(&id, cleanup)?,
        Commands::Clean => clean_configs()?,
    }

    Ok(())
}

fn wash_cmd() -> Command {
    Command::new("/opt/homebrew/Cellar/wash/0.42.0/bin/wash")
}

fn check_status() -> Result<()> {
    println!("{}", "Checking wasmCloud status...".cyan());

    // Check if wash host is actually running by trying to get hosts
    let hosts_output = wash_cmd()
        .args(["get", "hosts", "--output", "json"])
        .output()
        .context("Failed to check hosts")?;

    let wash_running = hosts_output.status.success() && !hosts_output.stdout.is_empty();

    if wash_running {
        println!("{} {}", "✓".green(), "wash is running".green());

        // Get hosts in human-readable format
        let output = wash_cmd()
            .args(["get", "hosts"])
            .output()
            .context("Failed to get hosts")?;

        if output.status.success() {
            println!("\n{}", "Active hosts:".cyan());
            println!("{}", String::from_utf8_lossy(&output.stdout));
        }

        // Get inventory if we can find a host
        if let Ok(json_str) = String::from_utf8(hosts_output.stdout) {
            if let Ok(json) = serde_json::from_str::<Value>(&json_str) {
                if let Some(hosts) = json["hosts"].as_array() {
                    if let Some(first_host) = hosts.first() {
                        if let Some(host_id) = first_host["id"].as_str() {
                            let inv_output = wash_cmd()
                                .args(["get", "inventory", host_id])
                                .output()
                                .context("Failed to get inventory")?;

                            if inv_output.status.success() {
                                println!("{}", String::from_utf8_lossy(&inv_output.stdout));
                            }
                        }
                    }
                }
            }
        }
    } else {
        println!("{} {}", "✗".red(), "wash is not running".red());
        println!("\n{}", "To start wash, run: wash up".yellow());
    }

    Ok(())
}

fn start_env(component_path: &str, component_id: &str, port: u16) -> Result<()> {
    println!("{}", format!("Starting development environment for component: {}", component_id).cyan());

    // Step 1: Start wash if needed
    let hosts_check = wash_cmd()
        .args(["get", "hosts"])
        .output()
        .context("Failed to check hosts")?;

    if !hosts_check.status.success() {
        println!("{}", "wash is not running, starting it...".yellow());

        let wash_up = wash_cmd()
            .env("WASMCLOUD_MAX_CORE_INSTANCES_PER_COMPONENT", "50")
            .args(["up", "-d"])
            .output()
            .context("Failed to start wash")?;

        if !wash_up.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to start wash: {}",
                String::from_utf8_lossy(&wash_up.stderr)
            ));
        }

        println!("{} wash started", "✓".green());

        // Wait a moment for wash to fully initialize
        std::thread::sleep(std::time::Duration::from_secs(2));
    } else {
        println!("{} {}", "✓".green(), "wash is running");
    }

    // Step 2: Ensure HTTP server config exists
    let config_name = "httpserver-config";
    let check_config = wash_cmd()
        .args(["config", "get", config_name])
        .output()
        .context("Failed to check config")?;

    if !check_config.status.success() {
        let create_config = wash_cmd()
            .args([
                "config",
                "put",
                config_name,
                &format!("address=0.0.0.0:{}", port),
            ])
            .output()
            .context("Failed to create config")?;

        if !create_config.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to create config: {}",
                String::from_utf8_lossy(&create_config.stderr)
            ));
        }
    }

    // Validate config exists and is readable
    let verify_config = wash_cmd()
        .args(["config", "get", config_name])
        .output()
        .context("Failed to verify config")?;

    if !verify_config.status.success() {
        return Err(anyhow::anyhow!("Config validation failed"));
    }
    println!("{} Config ready", "✓".green());

    // Step 4: Start component (check if already running first)
    let check_component = wash_cmd()
        .args(["get", "inventory", "--output", "json"])
        .output()
        .context("Failed to check components")?;

    let component_exists = if check_component.status.success() {
        let inventory = String::from_utf8_lossy(&check_component.stdout);
        inventory.contains(component_id)
    } else {
        false
    };

    if component_exists {
        // Stop existing component
        let stop_component = wash_cmd()
            .args(["stop", "component", component_id])
            .output()
            .context("Failed to stop existing component")?;

        if !stop_component.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to stop existing component: {}",
                String::from_utf8_lossy(&stop_component.stderr)
            ));
        }
    }

    // Start component
    let start_component = wash_cmd()
        .args(["start", "component", component_path, component_id])
        .output()
        .context("Failed to start component")?;

    if !start_component.status.success() {
        return Err(anyhow::anyhow!(
            "Failed to start component: {}",
            String::from_utf8_lossy(&start_component.stderr)
        ));
    }
    println!("{} Component ready", "✓".green());

    // Step 5: Start HTTP provider (check if already running first)
    let provider_id = "httpserver";
    let check_provider = wash_cmd()
        .args(["get", "inventory", "--output", "json"])
        .output()
        .context("Failed to check providers")?;

    let provider_exists = if check_provider.status.success() {
        let inventory = String::from_utf8_lossy(&check_provider.stdout);
        inventory.contains(provider_id)
    } else {
        false
    };

    if !provider_exists {
        let start_provider = wash_cmd()
            .args([
                "start",
                "provider",
                "ghcr.io/wasmcloud/http-server:0.22.0",
                provider_id,
            ])
            .output()
            .context("Failed to start provider")?;

        if !start_provider.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to start provider: {}",
                String::from_utf8_lossy(&start_provider.stderr)
            ));
        }
    }
    println!("{} Provider ready", "✓".green());

    // Wait for provider to fully initialize
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Step 6: Create link and validate
    let link = wash_cmd()
        .args([
            "link",
            "put",
            "httpserver",
            component_id,
            "wasi",
            "http",
            "--source-config",
            config_name,
            "--interface",
            "incoming-handler",
        ])
        .output()
        .context("Failed to create link")?;

    if !link.status.success() {
        return Err(anyhow::anyhow!(
            "Failed to create link: {}",
            String::from_utf8_lossy(&link.stderr)
        ));
    }

    // Validate link exists
    let verify_link = wash_cmd()
        .args(["get", "links", "--output", "json"])
        .output()
        .context("Failed to verify links")?;

    if verify_link.status.success() {
        let link_output = String::from_utf8_lossy(&verify_link.stdout);
        if link_output.contains(component_id) && link_output.contains("httpserver") {
            println!("{} Link ready", "✓".green());
        } else {
            return Err(anyhow::anyhow!("Link not found in validation"));
        }
    } else {
        return Err(anyhow::anyhow!("Failed to validate link"));
    }

    println!(
        "\n{} {}",
        "Development environment ready!".green().bold(),
        format!("HTTP server listening on http://localhost:{}/mcp", port).cyan()
    );

    Ok(())
}

fn stop_env(component_id: &str, cleanup: bool) -> Result<()> {
    println!("{}", format!("Stopping environment for component: {}", component_id).cyan());

    // Delete link
    println!("{}", "Deleting link...".cyan());
    let _ = wash_cmd()
        .args(["link", "del", component_id, "wasi", "http"])
        .output();
    println!("{} Link deleted", "✓".green());

    // Stop provider
    println!("{}", "Stopping HTTP provider...".cyan());
    let _ = wash_cmd()
        .args(["stop", "provider", "httpserver"])
        .output();
    println!("{} Provider stopped", "✓".green());

    // Stop component
    println!("{}", "Stopping component...".cyan());
    let _ = wash_cmd()
        .args(["stop", "component", component_id])
        .output();
    println!("{} Component stopped", "✓".green());

    if cleanup {
        clean_configs()?;
    }

    println!("\n{}", "Environment stopped successfully".green().bold());
    Ok(())
}

fn clean_configs() -> Result<()> {
    println!("{}", "Cleaning up persistent configurations and links...".cyan());

    // Delete httpserver-config
    let _ = wash_cmd()
        .args(["config", "del", "httpserver-config"])
        .output();

    // Delete link (format: wash link del <source-id> <wit-namespace> <wit-package>)
    let _ = wash_cmd()
        .args(["link", "del", "mcp-multi-tools", "wasi", "http"])
        .output();

    println!("{} Configs and links cleaned", "✓".green());
    Ok(())
}
