# Smartloop Command Line Interface

Smartloop CLI is designed to use with studio desktop and interacting with local service to manage projects, documents, skills and connections

## Install

macOS and Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/smartloop-ai/smartloop-cli/main/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/smartloop-ai/smartloop-cli/main/install.ps1 | iex
```

The binary lands in `~/.smartloop/bin` (`%USERPROFILE%\.smartloop\bin` on
Windows). Set `SMARTLOOP_CLI_INSTALL_DIR` to install elsewhere, or
`SMARTLOOP_CLI_VERSION` to pin a specific release.

Prebuilt binaries are published for Linux (x86_64, aarch64 — statically linked
against musl), macOS (Apple Silicon and Intel) and Windows (x86_64).

### From source

Requires Rust (2024 edition):

```sh
cargo install --path .
```

## Usage

List projects:

```sh
smartloop project list
```

Output is rendered as a table showing each project's ID, name, and whether it is a system project.

Create a project from a blank template:

```sh
smartloop project create --name my-project
smartloop project create --name my-project --description "Research notes"
```

A blank project starts with no skills; the service seeds it with the workspace
defaults. Everything the project stores — skills, documents, its index — lives
under the service's own project directory, so there is no working directory to
choose.

Import a project from an archive produced by an earlier export:

```sh
smartloop project create --import my-project.zip
smartloop project create --import my-project.zip --name restored-project
```

`--name` is optional here — pass it to rename the imported project. The import
gets a fresh project ID, and MCP OAuth credentials are stripped from the
archive on the way in.

Delete a project:

```sh
smartloop project delete --id <project-id>
```

## Configuration

The CLI connects to the Smartloop API at `http://localhost:38540` by default.
Point it elsewhere with `SMARTLOOP_API_URL`:

```sh
SMARTLOOP_API_URL=http://localhost:9000 smartloop project list
```

## Releases

Releases are cut from the `Release` workflow — pick a `patch`, `minor` or
`major` bump and it updates `Cargo.toml`, tags the commit, builds every target
and publishes the archives together with a `SHA256SUMS` file that the
installers verify against.
