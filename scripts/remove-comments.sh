#!/bin/bash
# Script to remove comment blocks from Rust and TypeScript/JavaScript files
# Usage: ./scripts/remove-comments.sh [directory]

set -e

DIRECTORY="${1:-.}"

echo "Removing comments from files in: $DIRECTORY"

# Function to remove comments from Rust files
remove_rust_comments() {
    local file="$1"
    echo "Processing Rust file: $file"

    # Use sed to remove:
    # - Single line comments: // ...
    # - Doc comments: /// ... and //! ...
    # - Multi-line comments: /* ... */
    # - Doc block comments: /** ... */

    # Create temp file
    local temp_file=$(mktemp)

    # Remove single-line comments (// but preserve URLs with //)
    # Remove doc comments (/// and //!)
    # Keep lines that are not just comments
    sed -E '
        # Remove /// and //! doc comments (entire line if only comment)
        /^[[:space:]]*\/\/[\!/]/d

        # Remove // comments at end of lines (but not in strings or URLs)
        s/[[:space:]]*\/\/[^"'\'']*$//

        # Remove standalone // comment lines
        /^[[:space:]]*\/\/[^\/]/d
    ' "$file" > "$temp_file"

    # Remove multi-line comments /* */ and /** */
    perl -0777 -pe 's/\/\*\*?[\s\S]*?\*\///g' "$temp_file" > "${temp_file}.2"
    mv "${temp_file}.2" "$temp_file"

    # Remove empty lines that resulted from comment removal (consecutive empty lines -> single)
    cat -s "$temp_file" > "$file"
    rm -f "$temp_file"
}

# Function to remove comments from TypeScript/JavaScript files
remove_ts_comments() {
    local file="$1"
    echo "Processing TS/JS file: $file"

    local temp_file=$(mktemp)

    # Remove single-line comments
    sed -E '
        # Remove // comments (entire line if only comment)
        /^[[:space:]]*\/\/[^\/]/d

        # Remove // comments at end of lines
        s/[[:space:]]*\/\/[^"'\'']*$//
    ' "$file" > "$temp_file"

    # Remove multi-line comments /* */ and /** */
    perl -0777 -pe 's/\/\*\*?[\s\S]*?\*\///g' "$temp_file" > "${temp_file}.2"
    mv "${temp_file}.2" "$temp_file"

    # Remove empty lines that resulted from comment removal
    cat -s "$temp_file" > "$file"
    rm -f "$temp_file"
}

# Function to remove comments from SQL files
remove_sql_comments() {
    local file="$1"
    echo "Processing SQL file: $file"

    local temp_file=$(mktemp)

    # Remove -- comments
    sed -E '
        # Remove -- comments (entire line if only comment)
        /^[[:space:]]*--/d

        # Remove -- comments at end of lines
        s/[[:space:]]*--.*$//
    ' "$file" > "$temp_file"

    # Remove empty lines
    cat -s "$temp_file" > "$file"
    rm -f "$temp_file"
}

# Process Rust files
echo "=== Processing Rust files ==="
find "$DIRECTORY" -type f -name "*.rs" ! -path "*/target/*" | while read -r file; do
    remove_rust_comments "$file"
done

# Process TypeScript files
echo "=== Processing TypeScript files ==="
find "$DIRECTORY" -type f -name "*.ts" ! -path "*/node_modules/*" ! -path "*/.svelte-kit/*" ! -path "*/build/*" | while read -r file; do
    remove_ts_comments "$file"
done

# Process Svelte files (contains TypeScript)
echo "=== Processing Svelte files ==="
find "$DIRECTORY" -type f -name "*.svelte" ! -path "*/node_modules/*" ! -path "*/.svelte-kit/*" ! -path "*/build/*" | while read -r file; do
    remove_ts_comments "$file"
done

# Process SQL migration files (optional - uncomment if needed)
# echo "=== Processing SQL files ==="
# find "$DIRECTORY" -type f -name "*.sql" | while read -r file; do
#     remove_sql_comments "$file"
# done

echo "Done! Comments have been removed."
echo "Note: Please review the changes and test the code before committing."
