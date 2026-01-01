# Gemini Co-CLI

A containerized web application that provides a Google Gemini Canvas-like interface, combining the power of Google's Gemini Pro AI with real-time SSH terminal access. Built with Rust for performance and safety.

## Features

- **Split-Pane Interface**: Google Gemini Canvas-inspired UI with:
  - Left pane: Gemini Pro AI chat interface
  - Right pane: Live SSH terminal connection
- **AI-Powered Terminal Assistant**: Gemini can observe terminal outputs and provide contextual help
- **Command Execution with Approval**: Gemini can suggest and execute terminal commands with user permission
- **Real-time Communication**: WebSocket-based bidirectional communication between Gemini and the terminal
- **Containerized Deployment**: Easy deployment with Docker
- **Secure SSH Connections**: Support for both password and SSH key authentication
- **Flexible Authentication**: Support for both API key and OAuth authentication

## Architecture

```
┌─────────────────────────────────────────┐
│           Web Browser UI                 │
│  ┌──────────────┬───────────────────┐   │
│  │   Gemini     │     Terminal      │   │
│  │   Chat       │     (xterm.js)    │   │
│  └──────────────┴───────────────────┘   │
└─────────────┬──────────────┬────────────┘
              │              │
         WebSocket      WebSocket
              │              │
┌─────────────┴──────────────┴────────────┐
│          Rust Backend (Axum)            │
│  ┌────────────┐      ┌────────────┐    │
│  │  Gemini    │      │    SSH     │    │
│  │   CLI      │      │   Client   │    │
│  │  Wrapper   │      │  (russh)   │    │
│  └────────────┘      └────────────┘    │
└─────────────┬──────────────┬────────────┘
              │              │
         Gemini CLI          SSH
              │              │
     Google Gemini API   Remote Server
```

## Prerequisites

- Docker and Docker Compose (for containerized deployment)
- OR Rust 1.83+ (for local development)
- Google Gemini API key OR Google account for OAuth (see setup below)
- SSH access to a remote server

## Quick Start with Docker

### Option 1: API Key Authentication (Recommended)

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/gemini-co-cli.git
   cd gemini-co-cli
   ```

2. Get your Gemini API key from [Google AI Studio](https://makersuite.google.com/app/apikey)

3. Build the Docker image:
   ```bash
   docker-compose build
   ```

4. Authenticate with your API key (one-time setup):
   ```bash
   docker-compose run -e GOOGLE_API_KEY=your_api_key_here gemini-co-cli gemini auth login
   ```
   Your API key will be stored in a Docker volume for future use.

5. Start the application:
   ```bash
   docker-compose up
   ```

6. Open your browser and navigate to:
   ```
   http://localhost:3000
   ```

7. Connect to your SSH server using the connection form.

### Option 2: OAuth Authentication (Advanced)

1-3. Same as above

4. Set up OAuth credentials:
   - Go to [Google Cloud Console](https://console.cloud.google.com/)
   - Create OAuth 2.0 credentials for a Desktop application
   - Download the credentials.json file
   - The container will guide you through the OAuth flow

5. Authenticate (one-time setup):
   ```bash
   docker-compose run gemini-co-cli gemini auth login
   ```
   Follow the browser prompts to authenticate with your Google account.

6-7. Same as Option 1

## Local Development

1. Install Rust (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Install Python dependencies:
   ```bash
   pip install google-generativeai google-auth-oauthlib
   ```

3. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/gemini-co-cli.git
   cd gemini-co-cli
   ```

4. Authenticate with Gemini (one-time setup):

   **Option A - API Key (Recommended):**
   ```bash
   export GOOGLE_API_KEY=your_api_key_here
   python3 scripts/gemini-cli.py auth login
   ```

   **Option B - OAuth (Advanced):**
   - Set up OAuth credentials as described above
   - Place credentials.json in `~/.config/gemini-co-cli/`
   - Run:
   ```bash
   python3 scripts/gemini-cli.py auth login
   ```

5. Run the application:
   ```bash
   cargo run --release
   ```

6. Open http://localhost:3000 in your browser.

## Usage

### Connecting to SSH

1. When you first open the application, you'll see a connection form.
2. Fill in your SSH server details:
   - **Host**: Your server's hostname or IP address
   - **Port**: SSH port (usually 22)
   - **Username**: Your SSH username
   - **Password**: Your password (if using password authentication)
   - **Private Key**: Your SSH private key (if using key-based authentication)
3. Click "Connect"

### Using Gemini Assistant

Once connected, you can:

1. **Ask Questions**: Type questions in the Gemini chat pane
   - Example: "What files are in my current directory?"
   - Example: "How do I check disk usage?"

2. **Get Contextual Help**: Gemini automatically observes terminal output
   - Run a command in the terminal
   - Ask Gemini to explain the output
   - Get suggestions for next steps

3. **Execute Commands**: Gemini can suggest commands
   - Gemini will format suggested commands as: `EXECUTE: <command>`
   - You'll see an approval dialog
   - Approve or reject the command
   - Approved commands execute automatically in the terminal

### Terminal Features

- **Full Terminal Emulation**: Supports colors, cursor movement, etc.
- **Resizable Panes**: Drag the divider to resize panes
- **Copy/Paste**: Standard terminal copy/paste operations
- **Scrollback**: Scroll through terminal history

## Configuration

### Environment Variables

- `RUST_LOG`: Logging level (optional, default: `gemini_co_cli=debug,tower_http=debug`)
- `GOOGLE_API_KEY`: Your Google Gemini API key (optional, for API key authentication)

### Gemini Authentication

The application provides two authentication methods:

**1. API Key (Recommended)**
- Get a key from [Google AI Studio](https://makersuite.google.com/app/apikey)
- Set `GOOGLE_API_KEY` environment variable
- Run `gemini auth login` to store the key
- Simpler setup, good for development and personal use

**2. OAuth (Advanced)**
- Set up OAuth 2.0 credentials in Google Cloud Console
- Authenticate with your Google account
- Supports automatic token refresh
- Better for production or shared environments

**Storage:**
- Docker: Credentials stored in `gemini-config` volume
- Local: Credentials stored in `~/.config/gemini-co-cli/`

### Port Configuration

By default, the application runs on port 3000. To change this:

1. **Docker**: Edit `docker-compose.yml`:
   ```yaml
   ports:
     - "8080:3000"  # Change 8080 to your desired port
   ```

2. **Local Development**: Edit `src/main.rs`:
   ```rust
   let addr = SocketAddr::from(([0, 0, 0, 0], 8080)); // Change port here
   ```

## Security Considerations

- **SSH Credentials**: Credentials are only stored in memory during active sessions
- **Gemini Authentication**: OAuth credentials managed securely by Google Gemini CLI
- **API Keys**: If using API key auth, keep your key secure and never commit to version control
- **Command Approval**: Always review commands before approval
- **HTTPS**: For production, use a reverse proxy (nginx, traefik) with SSL/TLS
- **Server Key Verification**: Currently accepts all server keys (modify `src/ssh.rs` for stricter verification)
- **Docker Volumes**: Gemini credentials stored in Docker volume - backup if needed

## Troubleshooting

### Connection Issues

- **Can't connect to SSH server**:
  - Verify server address and port
  - Check firewall rules
  - Ensure SSH is running on the remote server

- **Authentication fails**:
  - Verify username and password/key
  - Check private key format (OpenSSH format)
  - Ensure user has SSH access permissions

### Gemini Issues

- **Gemini not responding**:
  - Verify authentication: `gemini auth status`
  - Docker: `docker-compose run gemini-co-cli gemini auth status`
  - Local: `python3 scripts/gemini-cli.py auth status`
  - Check if GOOGLE_API_KEY is set (for API key auth)
  - Review application logs for errors
  - Ensure Python packages are installed: `pip list | grep google-generativeai`

- **Commands not executing**:
  - Ensure you approved the command
  - Check terminal pane for errors
  - Verify SSH connection is still active

## Development

### Project Structure

```
gemini-co-cli/
├── src/
│   ├── main.rs           # Application entry point
│   ├── state.rs          # Session and state management
│   ├── ssh.rs            # SSH client implementation
│   ├── gemini.rs         # Gemini CLI wrapper
│   └── websocket.rs      # WebSocket handlers
├── scripts/
│   └── gemini-cli.py     # Python CLI wrapper for Gemini API
├── static/
│   ├── index.html        # Frontend HTML
│   ├── style.css         # Styling
│   └── app.js            # Frontend JavaScript
├── Cargo.toml            # Rust dependencies
├── Dockerfile            # Container image definition
└── docker-compose.yml    # Docker Compose configuration
```

### Adding Features

To extend functionality:

1. **New Gemini Commands**: Modify `scripts/gemini-cli.py` to add new CLI commands
2. **Enhanced Terminal**: Add features in `static/app.js` using xterm.js addons
3. **Additional APIs**: Create new modules in `src/` and register routes in `main.rs`

## API Endpoints

- `GET /` - Serve the main HTML page
- `POST /api/ssh/connect` - Establish SSH connection
- `GET /ws/terminal/:session_id` - WebSocket for terminal communication
- `GET /ws/gemini/:session_id` - WebSocket for Gemini communication
- `GET /static/*` - Serve static files

## License

MIT License - see LICENSE file for details

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Submit a pull request

## Acknowledgments

- Built with [Axum](https://github.com/tokio-rs/axum) web framework
- Terminal powered by [xterm.js](https://xtermjs.org/)
- SSH via [russh](https://github.com/warp-tech/russh)
- AI by [Google Gemini](https://deepmind.google/technologies/gemini/)
- Python SDK: [google-generativeai](https://github.com/google/generative-ai-python)

## Support

For issues and questions:
- Open an issue on GitHub
- Check existing issues for solutions
- Review the troubleshooting section above
