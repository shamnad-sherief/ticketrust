# IRCTC Automation Project

This project automates the login flow for IRCTC using Rust, `thirtyfour`, and `tokio`.

## Dependencies & Versions

To ensure compatibility and prevent dependency issues in the future, the following versions are verified to work:

- **Browser**: Waterfox 6.6.7 (or equivalent Firefox version)
- **WebDriver**: geckodriver 0.35.0 (9f0a0036bea4 2024-08-03)
- **Rust thirtyfour crate**: 0.35.0

## Setup Instructions

1. Ensure you have Rust installed.
2. Download and install `geckodriver` (ensure it's available in your system path).
3. The project expects a `waterfox` binary to be available via a symlink in the project root to avoid hardcoding personal paths. To set this up on your system, run:
   ```bash
   ln -s /path/to/your/waterfox/or/firefox/binary ./waterfox
   ```
4. Run the project:
   ```bash
   cargo run
   ```
