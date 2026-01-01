#!/usr/bin/env python3
"""
Script to create GitHub issues from the ISSUES_TO_CREATE.md file.
Requires: pip install PyGithub
Usage: GITHUB_TOKEN=your_token python3 create_issues.py
"""

import os
import sys
import re

try:
    from github import Github
except ImportError:
    print("Error: PyGithub not installed. Run: pip install PyGithub")
    sys.exit(1)

def parse_issues_file(filename):
    """Parse the ISSUES_TO_CREATE.md file and extract issues."""
    with open(filename, 'r') as f:
        content = f.read()

    # Split by issue sections
    issue_pattern = r'### Issue \d+: (.+?)\n\*\*Labels:\*\* (.+?)\n\n#### User Story\n(.*?)(?=###|$)'
    matches = re.finditer(issue_pattern, content, re.DOTALL)

    issues = []
    for match in matches:
        title = match.group(1).strip()
        labels_str = match.group(2).strip()
        body_content = match.group(3).strip()

        # Parse labels
        labels = [label.strip().strip('`') for label in labels_str.split(',')]

        # Reconstruct the full body with User Story header
        body = f"## User Story\n{body_content}"

        issues.append({
            'title': title,
            'labels': labels,
            'body': body
        })

    return issues

def create_github_issues(repo_name, issues):
    """Create issues in the GitHub repository."""
    token = os.environ.get('GITHUB_TOKEN')
    if not token:
        print("Error: GITHUB_TOKEN environment variable not set")
        print("Usage: GITHUB_TOKEN=your_token python3 create_issues.py")
        sys.exit(1)

    try:
        g = Github(token)
        repo = g.get_repo(repo_name)

        print(f"Creating {len(issues)} issues in {repo_name}...")

        for i, issue_data in enumerate(issues, 1):
            try:
                issue = repo.create_issue(
                    title=issue_data['title'],
                    body=issue_data['body'],
                    labels=issue_data['labels']
                )
                print(f"✓ Created issue #{issue.number}: {issue_data['title']}")
            except Exception as e:
                print(f"✗ Failed to create issue: {issue_data['title']}")
                print(f"  Error: {e}")

        print(f"\n✓ Successfully created issues in {repo_name}")

    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

if __name__ == '__main__':
    issues = parse_issues_file('ISSUES_TO_CREATE.md')
    print(f"Found {len(issues)} issues to create")

    create_github_issues('aerocristobal/Gemini-Co-CLI', issues)
