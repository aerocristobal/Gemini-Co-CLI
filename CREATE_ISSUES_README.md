# How to Create GitHub Issues

This directory contains 20 comprehensive GitHub issues in BDD (Business Driven Design) format for improving the Gemini-Co-CLI codebase.

## Files

- **ISSUES_TO_CREATE.md** - All 20 issues with full details in BDD format
- **create_issues.py** - Python script to create issues automatically
- **create_issues.sh** - Bash script template

## Issues Summary

### P0 - Critical Security (6 issues)
1. Implement SSH Server Key Verification to Prevent MITM Attacks
2. Add Rate Limiting to Prevent DoS Attacks
3. Add Comprehensive Input Validation and Sanitization
4. Add Security Headers and SRI for CDN Resources
5. Run Docker Container as Non-Root User
6. Implement Session Authentication and Authorization

### P1 - High Priority Stability (5 issues)
7. Replace .unwrap() and .expect() with Proper Error Handling
8. Implement Session Cleanup and Garbage Collection
9. Add WebSocket Reconnection Logic
10. Fix Command Monitoring Performance Issue
11. Add Health Checks and Monitoring Endpoints

### P2 - Medium Priority Quality (4 issues)
12. Add Integration Tests for Core Workflows
13. Reduce Code Duplication Across Modules
14. Add Comprehensive Logging and Error Messages
15. Add API Documentation

### P3 - Lower Priority Enhancements (5 issues)
16. Add Session Persistence Across Restarts
17. Add Keyboard Shortcuts and Improved UX
18. Add Command History and Management
19. Add Accessibility Improvements
20. Add SSH Connection Features

## Method 1: Using GitHub CLI (Recommended)

### Prerequisites
```bash
# Install GitHub CLI
# macOS
brew install gh

# Linux
sudo apt install gh

# Windows
winget install GitHub.cli
```

### Authenticate
```bash
gh auth login
```

### Create All Issues
```bash
# Navigate to the repository
cd /path/to/Gemini-Co-CLI

# Use the markdown file to create issues
# You'll need to create them individually or use the Python script
```

## Method 2: Using Python Script (Automated)

### Prerequisites
```bash
pip install PyGithub
```

### Set GitHub Token
```bash
# Create a personal access token at:
# https://github.com/settings/tokens
# Required scopes: repo, write:issues

export GITHUB_TOKEN=your_github_token_here
```

### Run the Script
```bash
python3 create_issues.py
```

This will create all 20 issues automatically with proper labels and formatting.

## Method 3: Manual Creation (Most Control)

### Steps for Each Issue

1. Go to https://github.com/aerocristobal/Gemini-Co-CLI/issues/new
2. Copy the title from `ISSUES_TO_CREATE.md`
3. Copy the body content (everything under the issue heading)
4. Add the labels specified in the issue
5. Click "Submit new issue"
6. Repeat for all 20 issues

### Tips
- Create P0 (Critical) issues first
- Use the exact labels specified for consistent tracking
- Reference the original analysis when needed

## Method 4: Using GitHub Web Interface with Templates

You can create issue templates in `.github/ISSUE_TEMPLATE/` to streamline creation:

```bash
mkdir -p .github/ISSUE_TEMPLATE
# Copy issue content into template files
```

## Issue Format (BDD)

Each issue follows Business Driven Design format:

```markdown
## User Story
As a [type of user]
I want [goal/desire]
So that [benefit/value]

## Current Problem
[Description of the issue]

## Acceptance Criteria
**Given** [context]
**When** [action]
**Then** [expected outcome]
**And** [additional expectations]

## Technical Details
[Implementation guidance]

## Priority
[Priority level and justification]
```

## Verifying Issues Were Created

```bash
# List all issues
gh issue list

# List by label
gh issue list --label "priority: critical"
gh issue list --label "security"

# View specific issue
gh issue view 1
```

## Bulk Operations

If you need to update multiple issues:

```bash
# Add label to multiple issues
gh issue edit 1,2,3 --add-label "needs-review"

# Assign issues
gh issue edit 1 --assignee @me

# Close issues
gh issue close 1
```

## Next Steps

After creating the issues:

1. **Prioritize**: Review and adjust priorities if needed
2. **Assign**: Assign issues to team members
3. **Milestones**: Create milestones for each priority level
4. **Projects**: Add issues to a GitHub Project board
5. **Dependencies**: Link related issues
6. **Discussion**: Add comments with additional context

## Project Board Setup

Consider creating a project board:

```bash
# Create project
gh project create --title "Gemini-Co-CLI Improvements" --body "Tracking improvements from code analysis"

# Add issues to project
gh project item-add <PROJECT_ID> --issue 1
```

## Issue Labels Recommended

Create these labels in your repository:

- `priority: critical` (red) - Must fix immediately
- `priority: high` (orange) - Fix soon
- `priority: medium` (yellow) - Fix when possible
- `priority: low` (green) - Nice to have
- `security` (red) - Security vulnerability
- `bug` (red) - Something isn't working
- `enhancement` (blue) - New feature or improvement
- `quality` (purple) - Code quality improvement
- `documentation` (blue) - Documentation updates
- `testing` (green) - Test coverage
- `ux` (pink) - User experience
- `performance` (yellow) - Performance improvement

## Questions or Issues?

If you encounter problems creating issues:

1. Check GitHub permissions (need write access)
2. Verify authentication token has correct scopes
3. Check rate limits: https://api.github.com/rate_limit
4. Review GitHub API documentation

## Contributing

Once issues are created, contributors can:

1. Comment on issues to ask questions
2. Submit PRs referencing issue numbers
3. Update acceptance criteria as needed
4. Link related issues and PRs

---

**Total Issues to Create: 20**
- P0 Critical: 6
- P1 High: 5
- P2 Medium: 4
- P3 Low: 5
