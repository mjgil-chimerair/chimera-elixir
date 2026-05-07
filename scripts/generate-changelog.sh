#!/bin/bash
# Script to generate changelog from git commits with contributor credits

set -euo pipefail

# Configuration
REPO_ROOT=$(git rev-parse --show-toplevel)
CHANGELOG_FILE="$REPO_ROOT/CHANGELOG.md"
TEMP_FILE="/tmp/changelog_temp.md"

# Get latest tag
LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
PREVIOUS_TAG=$(git tag --sort=-version:refname | grep -v "$LATEST_TAG" | head -1 || echo "")

echo "Generating changelog from $PREVIOUS_TAG to $LATEST_TAG"

# Start with header
cat > "$TEMP_FILE" << EOF
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
EOF

# If we have a previous tag, generate changelog for changes since then
if [ -n "$PREVIOUS_TAG" ] && [ "$PREVIOUS_TAG" != "" ]; then
    echo "## [$LATEST_TAG] - $(date +'%Y-%m-%d')" >> "$TEMP_FILE"
    
    # Get commits between tags
    git log "$PREVIOUS_TAG..HEAD" --pretty=format:"- %s" --no-merges | while read -r commit; do
        # Categorize commits
        if echo "$commit" | grep -qi "^feat\|^feature"; then
            echo "### Added" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^feat\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^fix\|^bug"; then
            echo "### Fixed" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^fix\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^docs\|^doc"; then
            echo "### Documentation" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^docs\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^refactor"; then
            echo "### Changed" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^refactor\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^perf\|^performance"; then
            echo "### Performance" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^perf\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^test"; then
            echo "### Testing" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^test\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^chore\|^build\|^ci"; then
            echo "### Maintenance" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^chore\?: //i' >> "$TEMP_FILE"
        else
            echo "### Other" >> "$TEMP_FILE"
            echo "$commit" >> "$TEMP_FILE"
        fi
        echo "" >> "$TEMP_FILE"
    done
    
    # Add contributor credits
    echo "## Contributors" >> "$TEMP_FILE"
    echo "" >> "$TEMP_FILE"
    git log "$PREVIOUS_TAG..HEAD" --format='%aN <%aE>' | sort -u | while read -r contributor; do
        echo "- $contributor" >> "$TEMP_FILE"
    done
    echo "" >> "$TEMP_FILE"
else
    # No previous tag, generate initial changelog
    echo "## [$LATEST_TAG] - Initial release" >> "$TEMP_FILE"
    git log --pretty=format:"- %s" --no-merges | while read -r commit; do
        if echo "$commit" | grep -qi "^feat\|^feature"; then
            echo "### Added" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^feat\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^fix\|^bug"; then
            echo "### Fixed" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^fix\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^docs\|^doc"; then
            echo "### Documentation" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^docs\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^refactor"; then
            echo "### Changed" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^refactor\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^perf\|^performance"; then
            echo "### Performance" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^perf\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^test"; then
            echo "### Testing" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^test\?: //i' >> "$TEMP_FILE"
        elif echo "$commit" | grep -qi "^chore\|^build\|^ci"; then
            echo "### Maintenance" >> "$TEMP_FILE"
            echo "$commit" | sed 's/^chore\?: //i' >> "$TEMP_FILE"
        else
            echo "### Other" >> "$TEMP_FILE"
            echo "$commit" >> "$TEMP_FILE"
        fi
        echo "" >> "$TEMP_FILE"
    done
    
    # Add contributor credits for initial release
    echo "## Contributors" >> "$TEMP_FILE"
    echo "" >> "$TEMP_FILE"
    git log --format='%aN <%aE>' | sort -u | while read -r contributor; do
        echo "- $contributor" >> "$TEMP_FILE"
    done
    echo "" >> "$TEMP_FILE"
fi

# If CHANGELOG exists, append to it
if [ -f "$CHANGELOG_FILE" ]; then
    # Extract existing content (skip first header if present)
    tail -n +3 "$CHANGELOG_FILE" >> "$TEMP_FILE"
fi

# Move temp file to changelog
mv "$TEMP_FILE" "$CHANGELOG_FILE"

echo "Changelog generated at $CHANGELOG_FILE"