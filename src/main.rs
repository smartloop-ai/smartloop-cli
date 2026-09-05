use std::process::exit;

use clap::{Parser, Subcommand};
use prettytable::{Attr, Cell, Row, Table, color};
use reqwest::blocking::{Client, Response, multipart};

const DEFAULT_API_URL: &str = "http://localhost:38540";

#[derive(Parser)]
#[command(
    name = "smartloop", 
    version, 
    about="Smartloop Command Line Interface"
)]

struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage projects
    Project {
        /// Name of the project
        #[command(subcommand)]
        command: ProjectCommands,
    },
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// List all projects
    List,
    /// Create a project from a blank template, or from an exported archive
    Create {
        /// Name of the project; optional with --import, where it renames the imported project
        #[arg(long)]
        name: Option<String>,
        /// Description of the project
        #[arg(long, conflicts_with = "import")]
        description: Option<String>,
        /// Path to a zip archive produced by an earlier project export
        #[arg(long, value_name = "ZIP")]
        import: Option<String>,
    },
    /// Delete a project by ID
    Delete {
        /// ID of the project to delete
        #[arg(long)]
        id: String,
    },
}

/// Base URL of the Smartloop API, overridable for non-default installs.
fn projects_url() -> String {
    let base = std::env::var("SMARTLOOP_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string());
    format!("{}/v1/projects", base.trim_end_matches('/'))
}

/// Print an error and stop; used instead of panicking so failures read as
/// CLI output rather than a Rust backtrace.
fn fail(message: String) -> ! {
    eprintln!("Error: {}", message);
    exit(1);
}

/// Turn a non-2xx response into a message, preferring FastAPI's `detail` field
/// over the bare status code.
fn error_message(action: &str, response: Response) -> String {
    let status = response.status();
    let detail = response
        .json::<serde_json::Value>()
        .ok()
        .and_then(|body| body["detail"].as_str().map(str::to_string));

    match detail {
        Some(detail) => format!("Failed to {}: {} ({})", action, detail, status),
        None => format!("Failed to {}: {}", action, status),
    }
}

fn print_projects(projects: &[serde_json::Value]) {

    let mut table = Table::new();

    table.add_row(Row::new(vec![
        Cell::new("ID"),
        Cell::new("Name"),
        Cell::new("System"),
    ]));

    for project in projects {
        let id = project["id"].as_str().unwrap_or_default();
        let name = project["name"].as_str().unwrap_or_default();
        let system = if project["system"].as_bool().unwrap_or_default() {
            Cell::new("true").with_style(Attr::ForegroundColor(color::MAGENTA))
        } else {
            Cell::new("false")
        };
        table.add_row(Row::new(vec![
            Cell::new(id),
            Cell::new(name),
            system,
        ]));
    }

    table.printstd();

}

fn list_projects(client: &Client) {
    let response = client
        .get(projects_url())
        .send()
        .unwrap_or_else(|e| fail(format!("Failed to list projects: {}", e)));

    if !response.status().is_success() {
        fail(error_message("list projects", response));
    }

    let data: serde_json::Value = response
        .json()
        .unwrap_or_else(|e| fail(format!("Failed to parse response as JSON: {}", e)));

    let projects = data["projects"]
        .as_array()
        .unwrap_or_else(|| fail("Expected projects to be an array".to_string()));

    print_projects(projects);
}

/// Create an empty project — the blank template is a project with no skills,
/// which the API seeds with the workspace defaults.
fn create_project(client: &Client, name: String, description: Option<String>) {
    let mut body = serde_json::json!({
        "name": name,
        "system": false,
        "skills": [],
    });

    if let Some(description) = description {
        body["description"] = serde_json::Value::String(description);
    }

    let response = client
        .post(projects_url())
        .json(&body)
        .send()
        .unwrap_or_else(|e| fail(format!("Failed to create project: {}", e)));

    if !response.status().is_success() {
        fail(error_message("create project", response));
    }

    report_created(response, "Project created");
}

/// Create a project from a zip archive produced by an earlier export.
fn import_project(client: &Client, path: String, name: Option<String>) {
    let mut form = multipart::Form::new()
        .file("file", &path)
        .unwrap_or_else(|e| fail(format!("Failed to read {}: {}", path, e)));

    if let Some(name) = name {
        form = form.text("name", name);
    }

    let response = client
        .post(format!("{}/import", projects_url()))
        .multipart(form)
        .send()
        .unwrap_or_else(|e| fail(format!("Failed to import project: {}", e)));

    if !response.status().is_success() {
        fail(error_message("import project", response));
    }

    report_created(response, "Project imported");
}

/// Print the confirmation line and a one-row table for a newly created project.
fn report_created(response: Response, action: &str) {
    let project: serde_json::Value = response
        .json()
        .unwrap_or_else(|e| fail(format!("Failed to parse response as JSON: {}", e)));

    println!("{} successfully", action);
    print_projects(&[project]);
}

fn delete_project(client: &Client, id: String) {
    let response = client
        .delete(format!("{}/{}", projects_url(), id))
        .send()
        .unwrap_or_else(|e| fail(format!("Failed to delete project: {}", e)));

    if !response.status().is_success() {
        fail(error_message("delete project", response));
    }

    println!("Project deleted successfully");
}

fn main() {
    let args = Args::parse();
    let client = Client::new();

    match args.command {
        Commands::Project { command } => {
            match command {
                ProjectCommands::List => list_projects(&client),
                ProjectCommands::Create { name, description, import } => {
                    match import {
                        Some(path) => import_project(&client, path, name),
                        None => match name {
                            Some(name) => create_project(&client, name, description),
                            None => fail(
                                "--name is required when creating a project without --import".to_string(),
                            ),
                        },
                    }
                }
                ProjectCommands::Delete { id } => delete_project(&client, id),
            }
        }
    }
}
