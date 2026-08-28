use clap::{Parser, Subcommand};
use prettytable::{Table, Row, Cell, Attr, color};

const PROJECTS_URL: &str = "http://localhost:38540/v1/projects";

#[derive(Parser)]
#[command(
    name = "smartloop", 
    version="1.0", 
    about="Smartloop Command Line Interface"
)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new project
    Project {
        /// Name of the project
        #[command(subcommand)]
        command: ProjectCommands,
    },
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// Create a new project with a name
    List,
}

fn print_projects(projects: &Vec<serde_json::Value>) {

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

fn main() {
    let args = Args::parse();
    match args.command {
        Commands::Project { command } => {
            match command {
                ProjectCommands::List => {
                    let response = reqwest::blocking::get(PROJECTS_URL)
                    .expect("Failed to list projects");

                    if response.status().is_success() {
                        let data: serde_json::Value = response
                            .json()
                            .expect("Failed to parse response as JSON");

                        print_projects(&data["projects"].as_array().expect("Expected projects to be an array"));
                    } else {
                        panic!("Failed to list projects: {}", response.status());
                    }
                }
            }
        }
    }
}

