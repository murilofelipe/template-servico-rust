#!/usr/bin/env bash

# Create hooks directory if it doesn't exist
mkdir -p .git/hooks

# Copy the pre-push script to git hooks
cp scripts/pre-push .git/hooks/pre-push

# Make it executable
chmod +x .git/hooks/pre-push

echo "✅ Git hooks configurados com sucesso!"
