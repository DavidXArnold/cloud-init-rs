#!/usr/bin/env bash
# Generate changelog from git tags for CHANGELOG.md and debian/changelog
#
# Usage: ./scripts/generate-changelog.sh [--version VERSION]
#
# Outputs:
#   CHANGELOG.md        - Markdown format for GitHub releases and documentation
#   debian/changelog    - Debian package changelog format (for cargo-deb)

set -euo pipefail

PACKAGE_NAME="cloud-init-rs"
MAINTAINER_NAME="cloud-init-rs contributors"
MAINTAINER_EMAIL="contributors@cloud-init-rs.dev"

# Parse arguments
VERSION=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --version)
            VERSION="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

# Determine version: argument > latest tag > Cargo.toml
if [ -z "$VERSION" ]; then
    LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || true)
    if [ -n "$LATEST_TAG" ]; then
        VERSION="${LATEST_TAG#v}"
    else
        VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
    fi
fi

# Return all version tags sorted from oldest to newest
get_tags() {
    git tag --sort=version:refname 2>/dev/null | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+' || true
}

# Get commit subjects between two refs (start exclusive, end inclusive).
# When $1 is empty, returns all commits up to $2.
# Uses tformat (terminator) to ensure each line ends with a newline,
# so that "while read" correctly captures the final entry.
get_commits() {
    local from="$1"
    local to="$2"
    if [ -z "$from" ]; then
        git log --pretty=tformat:"%s" "$to" 2>/dev/null || true
    else
        git log --pretty=tformat:"%s" "${from}..${to}" 2>/dev/null || true
    fi
}

# ISO-8601 date (YYYY-MM-DD) for a tag or ref
get_tag_date() {
    local tag="$1"
    git log -1 --format="%ai" "$tag" 2>/dev/null | cut -d' ' -f1 || date +%Y-%m-%d
}

# RFC 2822 date required by the Debian changelog format
get_tag_rfc2822() {
    local tag="$1"
    git log -1 --format="%aD" "$tag" 2>/dev/null || date -R
}

# ---------------------------------------------------------------------------
# Generate CHANGELOG.md (Keep-a-Changelog / Markdown format)
# ---------------------------------------------------------------------------
generate_markdown() {
    local output="${1:-CHANGELOG.md}"
    local -a tag_array
    mapfile -t tag_array < <(get_tags)

    {
        echo "# Changelog"
        echo ""
        echo "All notable changes to this project will be documented in this file."
        echo ""
        echo "The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),"
        echo "and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)."
        echo ""

        if [ ${#tag_array[@]} -eq 0 ]; then
            # No tags yet – show all commits under the current version
            local date
            date=$(date +%Y-%m-%d)
            echo "## [${VERSION}] - ${date}"
            echo ""
            while IFS= read -r commit; do
                [ -n "$commit" ] && echo "- ${commit}"
            done < <(git log --pretty=format:"%s" HEAD 2>/dev/null || true)
            echo ""
        else
            # Iterate newest-first (standard Keep-a-Changelog order)
            for ((i=${#tag_array[@]}-1; i>=0; i--)); do
                local tag="${tag_array[$i]}"
                local version="${tag#v}"
                local date
                date=$(get_tag_date "$tag")
                local prev_tag=""
                [ "$i" -gt 0 ] && prev_tag="${tag_array[$((i-1))]}"

                echo "## [${version}] - ${date}"
                echo ""
                while IFS= read -r commit; do
                    [ -n "$commit" ] && echo "- ${commit}"
                done < <(get_commits "$prev_tag" "$tag")
                echo ""
            done
        fi
    } > "$output"

    echo "Generated $output" >&2
}

# ---------------------------------------------------------------------------
# Generate debian/changelog (dpkg-parsechangelog format)
# ---------------------------------------------------------------------------
generate_debian_changelog() {
    local output="${1:-debian/changelog}"
    local -a tag_array
    mapfile -t tag_array < <(get_tags)

    {
        if [ ${#tag_array[@]} -eq 0 ]; then
            # No tags yet – create a single entry for the current version
            local date
            date=$(date -R)
            echo "${PACKAGE_NAME} (${VERSION}) unstable; urgency=low"
            echo ""
            local has_commits=false
            while IFS= read -r commit; do
                if [ -n "$commit" ]; then
                    echo "  * ${commit}"
                    has_commits=true
                fi
            done < <(git log --pretty=format:"%s" HEAD 2>/dev/null || true)
            $has_commits || echo "  * Initial release"
            echo ""
            echo " -- ${MAINTAINER_NAME} <${MAINTAINER_EMAIL}>  ${date}"
            echo ""
        else
            # Debian format requires newest entry first
            for ((i=${#tag_array[@]}-1; i>=0; i--)); do
                local tag="${tag_array[$i]}"
                local version="${tag#v}"
                local date
                date=$(get_tag_rfc2822 "$tag")
                local prev_tag=""
                [ "$i" -gt 0 ] && prev_tag="${tag_array[$((i-1))]}"

                echo "${PACKAGE_NAME} (${version}) unstable; urgency=low"
                echo ""
                local has_commits=false
                while IFS= read -r commit; do
                    if [ -n "$commit" ]; then
                        echo "  * ${commit}"
                        has_commits=true
                    fi
                done < <(get_commits "$prev_tag" "$tag")
                $has_commits || echo "  * Release ${version}"
                echo ""
                echo " -- ${MAINTAINER_NAME} <${MAINTAINER_EMAIL}>  ${date}"
                echo ""
            done
        fi
    } > "$output"

    echo "Generated $output" >&2
}

generate_markdown "CHANGELOG.md"
mkdir -p debian
generate_debian_changelog "debian/changelog"
