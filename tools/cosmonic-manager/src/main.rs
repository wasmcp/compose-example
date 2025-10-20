use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::fs;
use std::process::Command;
use tera::{Tera, Context as TeraContext};

#[derive(Parser)]
#[command(name = "cosmonic-manager")]
#[command(about = "Manage Cosmonic Control deployments", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Set up kind cluster and install Cosmonic Control
    Setup {
        /// Cluster name
        #[arg(long, default_value = "cosmonic-cluster")]
        cluster: String,
        /// Cosmonic license key (or set COSMONIC_LICENSE_KEY env var)
        #[arg(long)]
        license_key: String,
    },
    /// Deploy application to cluster
    Deploy {
        /// Deployment type (httptrigger or deployment)
        #[arg(short, long, default_value = "httptrigger")]
        deploy_type: String,
        /// Application version (can be overridden by --image-tag)
        #[arg(short, long, default_value = "latest")]
        version: String,
        /// Namespace
        #[arg(short, long, default_value = "default")]
        namespace: String,
        /// Application name
        #[arg(long, default_value = "mcp-multi-tools")]
        app_name: String,
        /// Full image reference (e.g., ghcr.io/user/image:tag) - overrides --image-base and --version
        #[arg(long)]
        image: Option<String>,
        /// Image base without tag (e.g., ghcr.io/user/image)
        #[arg(long, default_value = "ghcr.io/wasmcp/example-mcp")]
        image_base: String,
    },
    /// Check deployment status
    Status {
        /// Namespace
        #[arg(short, long, default_value = "default")]
        namespace: String,
        /// Application name
        #[arg(long, default_value = "mcp-multi-tools")]
        app_name: String,
    },
    /// Clean up deployment
    Clean {
        /// Namespace
        #[arg(short, long, default_value = "default")]
        namespace: String,
        /// Application name
        #[arg(long, default_value = "mcp-multi-tools")]
        app_name: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup { cluster, license_key } => setup_cluster(&cluster, &license_key)?,
        Commands::Deploy { deploy_type, version, namespace, app_name, image, image_base } => {
            deploy(&deploy_type, &version, &namespace, &app_name, image.as_deref(), &image_base)?
        }
        Commands::Status { namespace, app_name } => check_status(&namespace, &app_name)?,
        Commands::Clean { namespace, app_name } => clean(&namespace, &app_name)?,
    }

    Ok(())
}

fn kubectl_cmd() -> Command {
    Command::new("kubectl")
}

fn helm_cmd() -> Command {
    Command::new("helm")
}

fn kind_cmd() -> Command {
    Command::new("kind")
}

fn setup_cluster(cluster_name: &str, license_key: &str) -> Result<()> {
    println!("{}", format!("Setting up cluster: {}", cluster_name).cyan());

    // Check if cluster exists
    let check_cluster = kind_cmd()
        .args(["get", "clusters"])
        .output()
        .context("Failed to check clusters")?;

    let cluster_exists = String::from_utf8_lossy(&check_cluster.stdout)
        .contains(cluster_name);

    if !cluster_exists {
        println!("{}", "Creating kind cluster...".cyan());

        // Create kind config
        let kind_config = format!(r#"kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
  kubeadmConfigPatches:
  - |
    kind: InitConfiguration
    nodeRegistration:
      kubeletExtraArgs:
        node-labels: "ingress-ready=true"
  extraPortMappings:
  - containerPort: 30950
    hostPort: 30950
    protocol: TCP
  - containerPort: 80
    hostPort: 80
    protocol: TCP
  - containerPort: 443
    hostPort: 443
    protocol: TCP
containerdConfigPatches:
- |-
  [plugins."io.containerd.grpc.v1.cri".registry.mirrors."localhost:5001"]
    endpoint = ["http://registry:5000"]
"#);

        fs::write("/tmp/kind-config.yaml", kind_config)
            .context("Failed to write kind config")?;

        let create = kind_cmd()
            .args(["create", "cluster", "--name", cluster_name, "--config", "/tmp/kind-config.yaml"])
            .output()
            .context("Failed to create cluster")?;

        if !create.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to create cluster: {}",
                String::from_utf8_lossy(&create.stderr)
            ));
        }

        println!("{} Cluster created", "✓".green());

        // Create local registry
        println!("{}", "Setting up local registry...".cyan());
        let registry_running = Command::new("docker")
            .args(["ps", "--filter", "name=kind-registry", "--format", "{{.Names}}"])
            .output()
            .context("Failed to check registry")?;

        if !String::from_utf8_lossy(&registry_running.stdout).contains("kind-registry") {
            let registry = Command::new("docker")
                .args([
                    "run", "-d", "--restart=always",
                    "-p", "5001:5000",
                    "--network=bridge",
                    "--name", "kind-registry",
                    "registry:2"
                ])
                .output()
                .context("Failed to start registry")?;

            if !registry.status.success() {
                println!("{} Registry may already exist", "⚠".yellow());
            }

            // Connect registry to kind network
            let _ = Command::new("docker")
                .args(["network", "connect", "kind", "kind-registry"])
                .output();
        }

        println!("{} Registry ready", "✓".green());
    } else {
        println!("{} Cluster already exists", "✓".green());
    }

    // Install Cosmonic Control
    println!("{}", "Installing Cosmonic Control...".cyan());

    let namespace = "cosmonic-system";

    // Create namespace
    kubectl_cmd()
        .args(["create", "namespace", namespace, "--dry-run=client", "-o", "yaml"])
        .output()
        .and_then(|output| {
            kubectl_cmd()
                .args(["apply", "-f", "-"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(mut stdin) = child.stdin.take() {
                        stdin.write_all(&output.stdout)?;
                    }
                    child.wait_with_output()
                })
        })
        .context("Failed to create namespace")?;

    // Check if Cosmonic Control is already installed
    let check_cosmonic = helm_cmd()
        .args(["list", "-n", namespace, "--output", "json"])
        .output()
        .context("Failed to check existing helm releases")?;

    let cosmonic_exists = if check_cosmonic.status.success() {
        String::from_utf8_lossy(&check_cosmonic.stdout).contains("cosmonic-control")
    } else {
        false
    };

    if cosmonic_exists {
        println!("{} Cosmonic Control already installed", "✓".green());
    } else {
        // Install Cosmonic Control with helm
        let install = helm_cmd()
            .args([
                "install", "cosmonic-control",
                "oci://ghcr.io/cosmonic/cosmonic-control",
                "--version", "0.3.0",
                "--namespace", namespace,
                "--set", &format!("cosmonicLicenseKey={}", license_key),
                "--set", "envoy.service.type=NodePort",
                "--set", "envoy.service.httpNodePort=30950",
                "--wait",
                "--timeout", "5m"
            ])
            .output()
            .context("Failed to install Cosmonic Control")?;

        if !install.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to install Cosmonic Control: {}",
                String::from_utf8_lossy(&install.stderr)
            ));
        }
        println!("{} Cosmonic Control installed", "✓".green());
    }

    // Wait for CRDs
    println!("{}", "Waiting for CRDs...".cyan());
    std::thread::sleep(std::time::Duration::from_secs(5));

    // Check if HostGroup is already installed
    let check_hostgroup = helm_cmd()
        .args(["list", "-n", namespace, "--output", "json"])
        .output()
        .context("Failed to check existing helm releases")?;

    let hostgroup_exists = if check_hostgroup.status.success() {
        String::from_utf8_lossy(&check_hostgroup.stdout).contains("hostgroup")
    } else {
        false
    };

    if hostgroup_exists {
        println!("{} HostGroup already installed", "✓".green());
    } else {
        // Install HostGroup
        println!("{}", "Installing HostGroup...".cyan());
        let hostgroup = helm_cmd()
            .args([
                "install", "hostgroup",
                "oci://ghcr.io/cosmonic/cosmonic-control-hostgroup",
                "--version", "0.3.0",
                "--namespace", namespace,
                "--wait",
                "--timeout", "1m"
            ])
            .output()
            .context("Failed to install HostGroup")?;

        if !hostgroup.status.success() {
            println!("{} HostGroup installation may have issues", "⚠".yellow());
        } else {
            println!("{} HostGroup installed", "✓".green());
        }
    }

    println!("\n{}", "Setup complete!".green().bold());
    Ok(())
}

fn deploy(deploy_type: &str, version: &str, namespace: &str, app_name: &str, image_override: Option<&str>, image_base: &str) -> Result<()> {
    println!("{}", format!("Deploying {} as {}", app_name, deploy_type).cyan());

    // Verify prerequisites
    println!("{}", "Checking prerequisites...".cyan());

    // Check if kubectl can connect to cluster
    let cluster_check = kubectl_cmd()
        .args(["cluster-info"])
        .output()
        .context("Failed to check cluster")?;

    let need_setup = !cluster_check.status.success();

    // Check if Cosmonic Control is installed
    let cosmonic_check = kubectl_cmd()
        .args(["get", "crd", "httptriggers.control.cosmonic.io"])
        .output();

    let cosmonic_installed = cosmonic_check
        .map(|o| o.status.success())
        .unwrap_or(false);

    let need_cosmonic = !cosmonic_installed && deploy_type == "httptrigger";

    if need_setup || need_cosmonic {
        println!("{}", "Prerequisites not met, running setup...".yellow());

        // Get license key from environment
        let license_key = std::env::var("COSMONIC_LICENSE_KEY")
            .context("COSMONIC_LICENSE_KEY environment variable not set. Please set it or run setup manually.")?;

        let cluster_name = std::env::var("CLUSTER_NAME").unwrap_or_else(|_| "cosmonic-cluster".to_string());

        setup_cluster(&cluster_name, &license_key)?;
    } else {
        println!("{} Prerequisites verified", "✓".green());
    }

    // Determine final image reference
    let image = if let Some(img) = image_override {
        img.to_string()
    } else {
        format!("{}:{}", image_base, version)
    };

    // Ensure namespace exists (suppress warning for default namespace)
    if namespace != "default" {
        kubectl_cmd()
            .args(["create", "namespace", namespace, "--dry-run=client", "-o", "yaml"])
            .output()
            .and_then(|output| {
                kubectl_cmd()
                    .args(["apply", "-f", "-"])
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                    .and_then(|mut child| {
                        use std::io::Write;
                        if let Some(mut stdin) = child.stdin.take() {
                            stdin.write_all(&output.stdout)?;
                        }
                        child.wait_with_output()
                    })
            })
            .context("Failed to create namespace")?;
    }

    // Render manifest from template
    let project_root = std::env::current_dir()
        .context("Failed to get current directory")?;
    let templates_dir = project_root.join("manifests/templates");
    let output_dir = project_root.join("manifests");

    fs::create_dir_all(&output_dir)
        .context("Failed to create manifests directory")?;

    let tera = Tera::new(&format!("{}/*.yaml.tpl", templates_dir.display()))
        .context("Failed to initialize template engine")?;

    let mut context = TeraContext::new();
    context.insert("app_name", app_name);
    context.insert("namespace", namespace);
    context.insert("version", version);
    context.insert("image", &image);

    let template_name = if deploy_type == "httptrigger" {
        "httptrigger.yaml.tpl"
    } else {
        "deployment.yaml.tpl"
    };

    let rendered = tera.render(template_name, &context)
        .context("Failed to render template")?;

    let output_file = output_dir.join(if deploy_type == "httptrigger" {
        "httptrigger.yaml"
    } else {
        "deployment.yaml"
    });

    fs::write(&output_file, rendered)
        .context("Failed to write manifest")?;

    println!("{} Manifest generated: {}", "✓".green(), output_file.display());

    // Apply manifest
    let apply = kubectl_cmd()
        .args(["apply", "-f", output_file.to_str().unwrap()])
        .output()
        .context("Failed to apply manifest")?;

    if !apply.status.success() {
        return Err(anyhow::anyhow!(
            "Failed to apply manifest: {}",
            String::from_utf8_lossy(&apply.stderr)
        ));
    }

    println!("{} Manifest applied", "✓".green());

    // Wait for deployment
    if deploy_type == "httptrigger" {
        println!("{}", "Waiting for HTTPTrigger...".cyan());
        std::thread::sleep(std::time::Duration::from_secs(5));
    } else {
        println!("{}", "Waiting for Deployment...".cyan());
        let _ = kubectl_cmd()
            .args(["rollout", "status", &format!("deployment/{}", app_name), "-n", namespace, "--timeout=60s"])
            .output();
    }

    println!("\n{}", "Deployment complete!".green().bold());

    // Get endpoint information
    println!("\n{}", "=== Access Information ===".cyan());

    // Get Cosmonic ingress NodePort
    let nodeport_check = kubectl_cmd()
        .args([
            "get", "svc", "ingress",
            "-n", "cosmonic-system",
            "-o", "jsonpath={.spec.ports[?(@.port==80)].nodePort}"
        ])
        .output();

    if let Ok(output) = nodeport_check {
        if output.status.success() {
            let nodeport = String::from_utf8_lossy(&output.stdout);
            if !nodeport.is_empty() {
                println!("\n{}", "MCP Server Endpoint:".green());
                println!("  http://localhost:{}/mcp", nodeport);
                println!("\n{}", "Test with curl:".yellow());
                println!("  curl -X POST http://localhost:{}/mcp \\", nodeport);
                println!("    -H 'Content-Type: application/json' \\");
                println!("    -d '{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{{}},\"clientInfo\":{{\"name\":\"test\",\"version\":\"1.0\"}}}}}}'");
            }
        }
    }

    // Show internal service endpoint
    println!("\n{}", "Internal Service:".cyan());
    println!("  {}.{}.svc.cluster.local", app_name, namespace);

    // Show port-forward option
    println!("\n{}", "Alternative (port-forward):".yellow());
    println!("  kubectl port-forward svc/{} 8080:80 -n {}", app_name, namespace);
    println!("  Then visit: http://localhost:8080");

    Ok(())
}

fn check_status(namespace: &str, app_name: &str) -> Result<()> {
    println!("{}", "Checking deployment status...".cyan());

    // Check for HTTPTriggers
    let httptrigger = kubectl_cmd()
        .args(["get", "httptrigger", app_name, "-n", namespace])
        .output()
        .context("Failed to check httptrigger")?;

    if httptrigger.status.success() {
        println!("\n{}", "HTTPTrigger:".cyan());
        println!("{}", String::from_utf8_lossy(&httptrigger.stdout));
    }

    // Check deployments
    let deployment = kubectl_cmd()
        .args(["get", "deployment", app_name, "-n", namespace])
        .output()
        .context("Failed to check deployment")?;

    if deployment.status.success() {
        println!("\n{}", "Deployment:".cyan());
        println!("{}", String::from_utf8_lossy(&deployment.stdout));
    }

    // Check pods
    let pods = kubectl_cmd()
        .args(["get", "pods", "-l", &format!("app={}", app_name), "-n", namespace])
        .output()
        .context("Failed to check pods")?;

    if pods.status.success() {
        println!("\n{}", "Pods:".cyan());
        println!("{}", String::from_utf8_lossy(&pods.stdout));
    }

    // Check services
    let svc = kubectl_cmd()
        .args(["get", "svc", "-l", &format!("app={}", app_name), "-n", namespace])
        .output()
        .context("Failed to check services")?;

    if svc.status.success() {
        println!("\n{}", "Services:".cyan());
        println!("{}", String::from_utf8_lossy(&svc.stdout));
    }

    Ok(())
}

fn clean(namespace: &str, app_name: &str) -> Result<()> {
    println!("{}", format!("Cleaning up deployment: {}", app_name).cyan());

    // Delete HTTPTrigger
    let _ = kubectl_cmd()
        .args(["delete", "httptrigger", app_name, "-n", namespace])
        .output();

    // Delete Deployment
    let _ = kubectl_cmd()
        .args(["delete", "deployment", app_name, "-n", namespace])
        .output();

    // Delete Service
    let _ = kubectl_cmd()
        .args(["delete", "service", app_name, "-n", namespace])
        .output();

    // Delete Ingress
    let _ = kubectl_cmd()
        .args(["delete", "ingress", app_name, "-n", namespace])
        .output();

    println!("{} Cleanup complete", "✓".green());
    Ok(())
}
