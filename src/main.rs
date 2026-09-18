use std::io::{IsTerminal, Write};
use std::process::exit;

use async_openai::{Client as OpenAIClient, config::OpenAIConfig};
use clap::{Parser, Subcommand};
use futures_util::StreamExt;
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
    /// Stream an interactive chat with the local agent
    Run {
        /// First message to send; the conversation continues interactively
        #[arg(value_name = "PROMPT")]
        prompt: Option<String>,
        /// Project ID to use; defaults to the server's current project
        #[arg(long, short)]
        project: Option<String>,
        /// Model to request; defaults to sl-mini when omitted
        #[arg(long, short)]
        model: Option<String>,
        /// Session ID to resume; a fresh one is created when omitted
        #[arg(long, short)]
        session: Option<String>,
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
fn api_url() -> String {
    let base = std::env::var("SMARTLOOP_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string());
    format!("{}/v1", base.trim_end_matches('/'))
}

fn projects_url() -> String {
    format!("{}/projects", api_url())
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

/// ANSI color for a `chat.status` step name, grouped by what the step is
/// doing rather than its exact label (the server's step vocabulary isn't a
/// fixed contract, so unrecognized steps still get a sensible default).
fn step_color(step: &str) -> &'static str {
    match step {
        "tools" | "web_search" | "explore" => "\x1b[36m", // cyan
        "plan" => "\x1b[35m",                             // magenta
        "model" => "\x1b[33m",                            // yellow
        "preparing" | "streaming" => "\x1b[32m",          // green
        "error" => "\x1b[31m",                            // red
        _ => "\x1b[34m",                                  // blue
    }
}

/// Print a `[step] message` progress line on stderr, coloring the step tag
/// when stderr is a terminal and leaving plain text otherwise (piped output,
/// redirected logs).
fn print_status(step: &str, message: &str) {
    if std::io::stderr().is_terminal() {
        eprintln!(
            "\x1b[2m[\x1b[0m{}{}\x1b[0m\x1b[2m]\x1b[0m {}",
            step_color(step),
            step,
            message
        );
    } else {
        eprintln!("[{}] {}", step, message);
    }
}

/// Build the async SSE client used for chat streaming. Uses `async-openai`'s
/// `eventsource_stream`-based SSE parser (the same approach real OpenAI SDKs
/// use) instead of a hand-rolled line reader over a raw socket.
fn openai_client() -> OpenAIClient<OpenAIConfig> {
    OpenAIClient::with_config(
        OpenAIConfig::new()
            .with_api_base(api_url())
            .with_api_key("not-needed"),
    )
}

/// Stream one chat turn from the service's SSE endpoint, printing content the
/// moment it arrives. Progress/status events are shown on stderr so they do
/// not corrupt the answer. Returns an error message when the stream drops
/// mid-way instead of exiting, so an interactive session can keep going after
/// a server hiccup. Prints a token/throughput summary on stderr at [DONE].
///
/// Long-running tool steps can leave the connection idle long enough for the
/// server (or a proxy in front of it) to drop it, which surfaces as a stream
/// error before any content has streamed. Retry once in that case since a
/// fresh connection usually succeeds.
async fn run_turn(
    client: &OpenAIClient<OpenAIConfig>,
    message: &str,
    project_id: &Option<String>,
    model: &Option<String>,
    session_id: &str,
) -> Result<(), String> {
    match run_turn_once(client, message, project_id, model, session_id).await {
        Ok(_) => Ok(()),
        Err((e, tokens)) if tokens == 0 => {
            print_status("error", &format!("connection dropped, retrying: {}", e));
            run_turn_once(client, message, project_id, model, session_id)
                .await
                .map_err(|(e, _)| e)
        }
        Err((e, _)) => Err(e),
    }
}

async fn run_turn_once(
    client: &OpenAIClient<OpenAIConfig>,
    message: &str,
    project_id: &Option<String>,
    model: &Option<String>,
    session_id: &str,
) -> Result<(), (String, u64)> {
    let mut body = serde_json::json!({
        "model": model.as_deref().unwrap_or("sl-mini"),
        "messages": [{"role": "user", "content": message}],
        "session_id": session_id,
        "stream": true,
    });

    if let Some(project_id) = project_id {
        body["project_id"] = serde_json::Value::String(project_id.clone());
    }

    let started = std::time::Instant::now();
    let mut stream = client
        .chat()
        .create_stream_byot::<serde_json::Value, serde_json::Value>(body)
        .await
        .map_err(|e| (format!("Failed to run: {}", e), 0))?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut tokens: u64 = 0;

    while let Some(event) = stream.next().await {
        let event = event.map_err(|e| (format!("Failed to read stream: {}", e), tokens))?;

        match event["object"].as_str() {
            Some("chat.completion.chunk") => {
                if let Some(content) = event["choices"][0]["delta"]["content"].as_str() {
                    tokens += 1;
                    let _ = out.write_all(content.as_bytes());
                    let _ = out.flush();
                }
                if event["choices"][0]["finish_reason"].is_string() {
                    break;
                }
            }
            Some("chat.status") => match event["message"].as_str() {
                Some(message) if !message.trim().is_empty() => print_status(
                    event["step"].as_str().unwrap_or_default(),
                    message.trim(),
                ),
                _ => {}
            },
            _ => {}
        }
    }

    println!();
    let elapsed = started.elapsed().as_secs_f64();
    let tokens_per_sec = if elapsed > 0.0 && tokens > 0 {
        tokens as f64 / elapsed
    } else {
        0.0
    };
    print_status(
        "stats",
        &format!("{} tokens, {:.1} tok/s, {:.0}s", tokens, tokens_per_sec, elapsed),
    );

    Ok(())
}

/// Interactive chat with the local agent: turn a reply, then keep reading new
/// prompts from stdin until EOF, `/quit`, or `exit`. One `session_id` underpins
/// the whole conversation, so the service keeps the context across turns.
async fn run_chat(
    client: &OpenAIClient<OpenAIConfig>,
    first_prompt: Option<String>,
    project_id: Option<String>,
    model: Option<String>,
    session: Option<String>,
) {
    let session_id = session.unwrap_or_else(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        format!(
            "cli-{}-{}",
            nanos,
            std::process::id()
        )
    });
    eprintln!("session: {}", session_id);

    let stdin = std::io::stdin();

    if let Some(prompt) = first_prompt.filter(|p| !p.trim().is_empty()) {
        println!("> {}", prompt);
        if let Err(e) = run_turn(client, &prompt, &project_id, &model, &session_id).await {
            if std::io::stdin().is_terminal() {
                eprintln!("{}", e);
            } else {
                fail(e);
            }
        }
    }

    loop {
        print!("> ");
        let _ = std::io::stdout().flush();

        let mut input = String::new();
        if stdin
            .read_line(&mut input)
            .unwrap_or_else(|e| fail(format!("Failed to read input: {}", e)))
            == 0
        {
            break;
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        if matches!(input, "/quit" | "/exit" | "/q" | "exit" | "Exit") {
            break;
        }

        if let Err(e) = run_turn(client, input, &project_id, &model, &session_id).await {
            if std::io::stdin().is_terminal() {
                eprintln!("{}", e);
            } else {
                fail(e);
            }
        }
    }
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Project { command } => {
            let client = Client::new();
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
        Commands::Run { prompt, project, model, session } => {
            let runtime = tokio::runtime::Runtime::new()
                .unwrap_or_else(|e| fail(format!("Failed to start async runtime: {}", e)));
            let client = openai_client();
            runtime.block_on(run_chat(&client, prompt, project, model, session));
        }
    }
}
