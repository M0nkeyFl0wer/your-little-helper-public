# Claudia-Lite Build Commands Reference

## Quick Build Commands

### Build for Release (optimized)
```bash
cd /home/flower/Downloads/claudia-lite
cargo build --release
```
This compiles with optimizations. The binary will be at: `target/release/app`

### Build for Development (faster compile, slower run)
```bash
cd /home/flower/Downloads/claudia-lite
cargo build
```
Binary at: `target/debug/app`

### Run the App
```bash
./target/release/app
```

### Clean and Rebuild
```bash
cd /home/flower/Downloads/claudia-lite
cargo clean && cargo build --release
```
Use when dependencies change or something seems corrupted.

### Check for Errors Without Building
```bash
cargo check
```
Faster than full build - just checks for compile errors.

### Run Tests
```bash
cargo test
```

## Understanding the Build

**What is Cargo?**
Cargo is Rust's package manager and build system. It:
- Downloads dependencies from crates.io (like npm for Node.js)
- Compiles your code
- Links everything together

**Key Files:**
- `Cargo.toml` - Project configuration (like package.json)
- `Cargo.lock` - Locked dependency versions (like package-lock.json)

## Dependencies Added for Editor Feature

In `crates/app/Cargo.toml`:
- `comrak = "0.28"` - Markdown to HTML conversion
- `rfd = "0.14"` - Native file dialogs (open/save)

## Troubleshooting

**"error: linker not found"**
```bash
sudo apt install build-essential
```

**Missing OpenGL/graphics libraries on Linux:**
```bash
sudo apt install libxkbcommon-dev libwayland-dev
```

**"Permission denied" when running:**
```bash
chmod +x ./target/release/app
```
