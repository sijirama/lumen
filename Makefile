# Lumen Makefile 🛠️

.PHONY: dev build clean help

# Default target
help:
	@echo "Lumen Development Commands:"
	@echo "  make dev    - Start the development environment (Hot reloading)"
	@echo "  make build  - Build the production binary (AppImage/Bundles)"
	@echo "  make clean  - Wipe the massive Rust target directory (Saves space! 🧹)"

# Run in development mode
dev:
	npm run tauri dev

# Build production bundle
build:
	npm run tauri build

# Surgical cleaning of the target directory
clean:
	@echo "🧹 Cleaning up Rust build artifacts..."
	cd src-tauri && cargo clean
	@echo "✨ Clean complete. Your disk says thank you."
