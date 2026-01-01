#!/bin/bash
# Script to create GitHub issues using the GitHub API
# Usage: ./create_issues.sh

REPO_OWNER="aerocristobal"
REPO_NAME="Gemini-Co-CLI"
API_BASE="https://api.github.com"

# Check for GitHub token
if [ -z "$GITHUB_TOKEN" ]; then
    echo "Error: GITHUB_TOKEN environment variable not set"
    echo "Please set it: export GITHUB_TOKEN=your_token"
    exit 1
fi

# Function to create an issue
create_issue() {
    local title="$1"
    local body="$2"
    local labels="$3"

    echo "Creating issue: $title"

    response=$(curl -s -X POST \
        -H "Authorization: Bearer $GITHUB_TOKEN" \
        -H "Accept: application/vnd.github+json" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        "$API_BASE/repos/$REPO_OWNER/$REPO_NAME/issues" \
        -d @- <<EOF
{
  "title": "$title",
  "body": $(echo "$body" | jq -Rs .),
  "labels": [$labels]
}
EOF
)

    issue_number=$(echo "$response" | jq -r '.number // empty')

    if [ -n "$issue_number" ]; then
        echo "✓ Created issue #$issue_number"
        return 0
    else
        echo "✗ Failed to create issue"
        echo "Response: $response"
        return 1
    fi
}

# Create all issues
echo "Creating GitHub issues for Gemini-Co-CLI improvements..."
echo "Repository: $REPO_OWNER/$REPO_NAME"
echo ""

# This script would be too long with all 20 issues inline
# Instead, let's use Python or read from the markdown file
# For now, let's create a few example issues

echo "Note: This script requires the issues to be defined."
echo "Please use the Python script (create_issues.py) instead, or"
echo "manually create issues from ISSUES_TO_CREATE.md"
