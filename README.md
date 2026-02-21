# pleme-config

Configuration management library for Pleme platform

## Installation

```toml
[dependencies]
pleme-config = "0.1"
```

## Usage

```rust
use pleme_config::Config;
use serde::Deserialize;

#[derive(Deserialize)]
struct AppConfig {
    database_url: String,
    port: u16,
}

let config: AppConfig = Config::from_env()?;
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `multi-source` | Enhanced multi-source config loading |
| `validation` | Config validation with `validator` |
| `formats` | YAML, JSON, TOML file support |
| `secrets` | SOPS/age secrets management |
| `database` | Database URL parsing helpers |
| `full` | All features enabled |

Enable features in your `Cargo.toml`:

```toml
pleme-config = { version = "0.1", features = ["full"] }
```

## Development

This project uses [Nix](https://nixos.org/) for reproducible builds:

```bash
nix develop            # Dev shell with Rust toolchain
nix run .#check-all    # cargo fmt + clippy + test
nix run .#publish      # Publish to crates.io (--dry-run supported)
nix run .#regenerate   # Regenerate Cargo.nix
```

## License

MIT - see [LICENSE](LICENSE) for details.
