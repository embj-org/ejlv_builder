# EJLV Builder

EJLV Builder is the tool that builds and deploys hardware performance tests for [LVGL](https://github.com/lvgl/lvgl.git) across different embedded hardware platforms.
It integrates the [EJ Builder SDK](https://crates.io/crates/ej-builder-sdk) for automated embedded testing.

## Installation

### From Git

```bash
cargo install --git https://github.com/AndreCostaaa/ejlv_builder
```

### From Source

```bash
git clone https://github.com/AndreCostaaa/ejlv_builder.git
cd ejlv_builder
cargo install --path .
```

## Usage

The builder is typically invoked by EJB, but can be used directly:

```bash
# When called by EJ, receives these arguments:
# ejkmer-builder <action> <config_path> <board_name> <board_config_name> <socket_path>

# The application automatically determines whether to build or run based on the action parameter
```

## Comparison with Shell Scripts

This Rust-based builder provides several advantages over simple shell scripts:

- Cleans up remote processes when jobs are cancelled  
- Clear error messages and proper exit codes  
- You get to write code in Rust

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

For questions about this builder or the EJ framework:

- Check the [EJ Documentation](https://embj-org.github.io/ej/)
- Visit the [EJ GitHub Repository](https://github.com/embj-org/ej)
- Review the [Builder SDK Documentation](https://crates.io/crates/ej-builder-sdk)
