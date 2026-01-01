#!/usr/bin/env python3
"""
Gemini CLI wrapper for Gemini Co-CLI application.
Provides a simple command-line interface to Google's Gemini API using OAuth authentication.
"""

import sys
import os
import argparse
import json
from pathlib import Path

try:
    import google.generativeai as genai
    from google.auth.transport.requests import Request
    from google.oauth2.credentials import Credentials
    from google_auth_oauthlib.flow import InstalledAppFlow
except ImportError:
    print("Error: Required packages not installed. Run: pip install google-generativeai google-auth-oauthlib", file=sys.stderr)
    sys.exit(1)

# OAuth scopes for Gemini API
SCOPES = ['https://www.googleapis.com/auth/generative-language.retriever']

# Credentials storage location
CONFIG_DIR = Path.home() / '.config' / 'gemini-co-cli'
CREDS_FILE = CONFIG_DIR / 'credentials.json'
TOKEN_FILE = CONFIG_DIR / 'token.json'


def get_credentials():
    """Get or refresh OAuth credentials."""
    creds = None

    # Check if we have stored credentials
    if TOKEN_FILE.exists():
        creds = Credentials.from_authorized_user_file(str(TOKEN_FILE), SCOPES)

    # If no valid credentials, authenticate
    if not creds or not creds.valid:
        if creds and creds.expired and creds.refresh_token:
            try:
                creds.refresh(Request())
            except Exception as e:
                print(f"Error refreshing credentials: {e}", file=sys.stderr)
                print("Please run 'gemini-cli auth login' again", file=sys.stderr)
                return None
        else:
            if not CREDS_FILE.exists():
                print("Error: No credentials.json file found.", file=sys.stderr)
                print(f"Please place your OAuth client credentials at: {CREDS_FILE}", file=sys.stderr)
                print("Or use API key authentication instead.", file=sys.stderr)
                return None

            try:
                flow = InstalledAppFlow.from_client_secrets_file(str(CREDS_FILE), SCOPES)
                creds = flow.run_local_server(port=0)
            except Exception as e:
                print(f"Error during authentication: {e}", file=sys.stderr)
                return None

        # Save credentials for next time
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        with open(TOKEN_FILE, 'w') as token:
            token.write(creds.to_json())

    return creds


def auth_login():
    """Authenticate with Google account."""
    print("Authenticating with Google Gemini...")
    print(f"Note: For OAuth authentication, place your credentials.json file at: {CREDS_FILE}")
    print("Alternatively, set GOOGLE_API_KEY environment variable for API key authentication.")

    # Check for API key first
    api_key = os.environ.get('GOOGLE_API_KEY')
    if api_key:
        print("Using API key authentication from GOOGLE_API_KEY environment variable")
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        with open(CONFIG_DIR / 'api_key.txt', 'w') as f:
            f.write(api_key)
        print("✓ API key stored successfully")
        return

    # Try OAuth
    creds = get_credentials()
    if creds:
        print("✓ Authentication successful!")
    else:
        print("✗ Authentication failed", file=sys.stderr)
        sys.exit(1)


def auth_status():
    """Check authentication status."""
    # Check for API key
    api_key_file = CONFIG_DIR / 'api_key.txt'
    if api_key_file.exists():
        print("Authenticated with API key")
        return True

    # Check for OAuth token
    if TOKEN_FILE.exists():
        try:
            creds = Credentials.from_authorized_user_file(str(TOKEN_FILE), SCOPES)
            if creds and creds.valid:
                print("Authenticated with OAuth")
                return True
            elif creds and creds.expired:
                print("OAuth token expired - will refresh on next use")
                return True
        except Exception:
            pass

    print("Not authenticated", file=sys.stderr)
    return False


def chat(prompt, model='gemini-1.5-pro'):
    """Send a chat message to Gemini."""
    # Try API key first
    api_key_file = CONFIG_DIR / 'api_key.txt'
    if api_key_file.exists():
        with open(api_key_file, 'r') as f:
            api_key = f.read().strip()
        genai.configure(api_key=api_key)
    else:
        # Try OAuth
        creds = get_credentials()
        if not creds:
            print("Error: Not authenticated. Run 'gemini-cli auth login' first", file=sys.stderr)
            sys.exit(1)
        genai.configure(credentials=creds)

    try:
        model_obj = genai.GenerativeModel(model)
        response = model_obj.generate_content(prompt)
        print(response.text)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


def main():
    """Main CLI entry point."""
    parser = argparse.ArgumentParser(description='Gemini CLI wrapper')
    subparsers = parser.add_subparsers(dest='command', help='Commands')

    # Auth commands
    auth_parser = subparsers.add_parser('auth', help='Authentication commands')
    auth_subparsers = auth_parser.add_subparsers(dest='auth_command')
    auth_subparsers.add_parser('login', help='Authenticate with Google')
    auth_subparsers.add_parser('status', help='Check authentication status')

    # Chat command
    chat_parser = subparsers.add_parser('chat', help='Send a chat message')
    chat_parser.add_argument('--model', default='gemini-1.5-pro', help='Model to use')
    chat_parser.add_argument('--prompt', required=True, help='Prompt to send')

    args = parser.parse_args()

    if args.command == 'auth':
        if args.auth_command == 'login':
            auth_login()
        elif args.auth_command == 'status':
            if auth_status():
                sys.exit(0)
            else:
                sys.exit(1)
        else:
            auth_parser.print_help()
    elif args.command == 'chat':
        chat(args.prompt, args.model)
    else:
        parser.print_help()


if __name__ == '__main__':
    main()
