#!/bin/bash

# Script to check for untranslated strings in Svelte files

echo "=== Checking for untranslated strings in Svelte files ==="
echo ""

cd "$(dirname "$0")/../src/routes"

echo "1. Files with 'Loading' text (excluding variables):"
echo "----------------------------------------"
grep -r "Loading" --include="*.svelte" | grep -v "actionLoading\|loadingStore\|let loading\|<!-- Loading" | grep "text-dark\|>Loading"
echo ""

echo "2. Files missing i18n import:"
echo "----------------------------------------"
for file in $(find . -name "*.svelte" -type f); do
    if ! grep -q "import.*{.*_.*}.*from.*\$lib/i18n" "$file"; then
        # Check if file has user-facing text
        if grep -q "text-white\|text-dark\|btn-primary\|<h1\|<h2\|<button" "$file"; then
            echo "$file"
        fi
    fi
done
echo ""

echo "3. Common untranslated button text:"
echo "----------------------------------------"
grep -rn ">Create<\|>Delete<\|>Edit<\|>Save<\|>Cancel<\|>Confirm<" --include="*.svelte" | head -20
echo ""

echo "4. Files with hard-coded English sentences:"
echo "----------------------------------------"
grep -rn "\"[A-Z][a-z]* [a-z]* [a-z]*\|'[A-Z][a-z]* [a-z]* [a-z]*" --include="*.svelte" | grep -v "import\|export\|class=" | head -20
echo ""

echo "=== Summary ==="
echo "Total .svelte files:"
find . -name "*.svelte" | wc -l

echo "Files with i18n import:"
grep -rl "import.*{.*_.*}.*from.*\$lib/i18n" --include="*.svelte" | wc -l

echo ""
echo "Done!"
