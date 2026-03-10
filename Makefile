# Lumen Makefile 🛠️

.PHONY: dev build clean help

# Default target
help:
	@echo "Lumen Development Commands:"
	@echo "  make run    - Start the app (Hot reloading)"
	@echo "  make dev    - Alias for run"
	@echo "  make build  - Build the production binary (AppImage/Bundles)"
	@echo "  make clean  - Wipe the massive Rust target directory (Saves space! 🧹)"

# Run in development mode
run:
	npm run tauri dev

dev: run

# Build production bundle
build:
	npm run tauri build

# Surgical cleaning of the target directory
clean:
	@echo "🧹 Cleaning up Rust build artifacts..."
	cd src-tauri && cargo clean
	@echo "✨ Clean complete. Your disk says thank you."
