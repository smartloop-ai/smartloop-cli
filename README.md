# smartloop-cli

Smartloop Command Line Interface.

## Requirements

- Rust (2024 edition)

## Install

```sh
cargo install --path .
```

## Usage

List projects:

```sh
smartloop project list
```

Output is rendered as a table showing each project's ID, name, and whether it is a system project.

## Configuration

The CLI connects to the Smartloop API at `http://localhost:38540/v1/projects`.