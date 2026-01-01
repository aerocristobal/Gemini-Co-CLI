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
- **Official Gemini CLI**: Uses [@google/gemini-cli](https://github.com/google-gemini/gemini-cli) with Google OAuth

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
│  │  Official  │      │    SSH     │    │
│  │  Gemini    │      │   Client   │    │
│  │    CLI     │      │  (russh)   │    │
│  └────────────┘      └────────────┘    │
└─────────────┬──────────────┬────────────┘
              │              │
         Gemini CLI          SSH
              │              │
     Google Gemini API   Remote Server
```

## Prerequisites

- Docker and Docker Compose (for containerized deployment)
- OR Rust 1.83+ and Node.js 20+ (for local development)
- Google account for OAuth authentication OR Gemini API key
- SSH access to a remote server

## Quick Start with Docker

### Option 1: API Key Authentication (Simplest)

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/gemini-co-cli.git
   cd gemini-co-cli
   ```

2. Get your Gemini API key from [Google AI Studio](https://aistudio.google.com/apikey)

3. Set the API key in docker-compose.yml:
   ```yaml
   environment:
     - GEMINI_API_KEY=your_api_key_here
   ```

4. Build and start:
   ```bash
   docker-compose up --build
   ```

5. Open your browser: **http://localhost:3000**

### Option 2: Login with Google (Recommended)

This uses the official Gemini CLI's OAuth authentication flow.

1. Clone and build:
   ```bash
   git clone https://github.com/yourusername/gemini-co-cli.git
   cd gemini-co-cli
   docker-compose build
   ```

2. Authenticate interactively (one-time setup):
   ```bash
   docker-compose run gemini-co-cli gemini
   ```

   When the Gemini CLI starts:
   - Select **"Login with Google"** from the menu
   - A browser window will open for Google authentication
   - Follow the prompts to grant access
   - Your credentials will be cached in a Docker volume

3. Start the application:
   ```bash
   docker-compose up
   ```

4. Open your browser: **http://localhost:3000**

Your authentication persists across container restarts via the `gemini-config` volume.

## Local Development

1. Install dependencies:
   ```bash
   # Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Node.js 20+ (for Gemini CLI)
   # macOS: brew install node@20
   # Ubuntu: curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
   #         sudo apt-get install -y nodejs

   # Official Gemini CLI
   npm install -g @google/gemini-cli
   ```

2. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/gemini-co-cli.git
   cd gemini-co-cli
   ```

3. Authenticate with Gemini (one-time setup):

   **Option A - API Key:**
   ```bash
   export GEMINI_API_KEY=your_api_key_here
   cargo run --release
   ```

   **Option B - Login with Google:**
   ```bash
   gemini  # Run the CLI interactively
   # Select "Login with Google" and follow browser prompts
   # Press Ctrl+C after authentication
   cargo run --release
   ```

4. Open http://localhost:3000 in your browser

## Usage

### Connecting to SSH

1. When you first open the application, you'll see a connection form
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
- `GEMINI_API_KEY`: Your Google Gemini API key (optional, for API key authentication)

### Gemini Authentication

The application uses the official [@google/gemini-cli](https://github.com/google-gemini/gemini-cli) which supports two authentication methods:

**1. Login with Google (Recommended)**
- Interactive OAuth flow via browser
- 60 requests/min, 1,000 requests/day (free tier)
- Credentials cached locally
- Best for personal use and development

**2. API Key**
- Set `GEMINI_API_KEY` environment variable
- Get key from [Google AI Studio](https://aistudio.google.com/apikey)
- Simpler setup, but requires managing API keys

**Storage:**
- Docker: Credentials stored in `gemini-config` volume at `/root/.config/@google/generative-ai-cli`
- Local: Stored in `~/.config/@google/generative-ai-cli/`

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
- **Gemini Authentication**: OAuth credentials managed securely by official Gemini CLI
- **API Keys**: If using API key auth, keep your key secure and never commit to version control
- **Command Approval**: Always review commands before approval
- **HTTPS**: For production, use a reverse proxy (nginx, traefik) with SSL/TLS
- **Server Key Verification**: Currently accepts all server keys (modify `src/ssh.rs:27` for stricter verification)
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
  - **Docker**: Ensure you authenticated: `docker-compose run gemini-co-cli gemini`
  - **Local**: Ensure you authenticated: `gemini` (select "Login with Google")
  - Check if `GEMINI_API_KEY` is set (for API key auth)
  - Verify Gemini CLI is installed: `gemini --version`
  - Review application logs for errors

- **"Authentication required" error**:
  - Run the interactive authentication:
    - Docker: `docker-compose run gemini-co-cli gemini`
    - Local: `gemini`
  - Select "Login with Google" and complete the browser flow
  - Or set `GEMINI_API_KEY` environment variable

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

1. **Customize Gemini Behavior**: Modify prompts in `src/gemini.rs:38-52`
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
- Official [Gemini CLI](https://github.com/google-gemini/gemini-cli) by Google

## Support

For issues and questions:
- Open an issue on GitHub
- Check existing issues for solutions
- Review the troubleshooting section above
- Gemini CLI docs: https://geminicli.com/

---

**Sources:**
- [Gemini CLI authentication setup](https://geminicli.com/docs/get-started/authentication/)
- [GitHub - google-gemini/gemini-cli](https://github.com/google-gemini/gemini-cli)
- [Gemini CLI Authentication Guide](https://github.com/google-gemini/gemini-cli/blob/main/docs/get-started/authentication.md)
