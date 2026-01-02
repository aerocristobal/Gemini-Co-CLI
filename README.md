# Gemini Co-CLI

A containerized web application that provides a Google Gemini Canvas-like interface, combining the power of Google's Gemini Pro AI with real-time SSH terminal access. Built with Rust for performance and safety.

## Features

- **Dual Terminal Interface**: Google Gemini Canvas-inspired UI with:
  - Left pane: Interactive Gemini CLI terminal
  - Right pane: Live SSH terminal connection
- **Interactive Gemini Terminal**: Run Gemini CLI directly in the browser with full terminal capabilities
- **AI-Powered Terminal Assistant**: Gemini observes SSH terminal outputs and provides contextual help
- **MCP-Based Command Execution**: Structured tool calls via Model Context Protocol (MCP) with user approval
- **Event-Driven Architecture**: Real-time approval events using broadcast channels (no polling)
- **Real-time Communication**: WebSocket connections for seamless terminal interaction
- **Containerized Deployment**: Easy deployment with Docker
- **Secure SSH Connections**: Support for both password and SSH key authentication
- **Official Gemini CLI**: Uses [@google/gemini-cli](https://github.com/google-gemini/gemini-cli) with Google OAuth

## Architecture

The application uses a **Hybrid MCP Architecture** where Gemini CLI connects to an embedded MCP server for structured command execution while maintaining interactive PTY terminals.

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Web Browser UI                                │
│  ┌─────────────────────────┬───────────────────────────────────┐   │
│  │   Gemini Terminal       │        SSH Terminal               │   │
│  │   (xterm.js)            │        (xterm.js)                 │   │
│  └─────────────────────────┴───────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │              Command Approval UI (event-driven)              │   │
│  └─────────────────────────────────────────────────────────────┘   │
└──────────┬─────────────────────┬─────────────────────┬─────────────┘
           │                     │                     │
       WS: Gemini            WS: SSH            WS: Approvals
           │                     │              (broadcast events)
           │                     │                     │
┌──────────┴─────────────────────┴─────────────────────┴─────────────┐
│                      Rust Backend (Axum)                            │
│                                                                     │
│  ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐  │
│  │   Gemini CLI    │   │   MCP Server    │   │   SSH Client    │  │
│  │   PTY Manager   │   │  (HTTP/SSE)     │   │    (russh)      │  │
│  └────────┬────────┘   └────────┬────────┘   └────────┬────────┘  │
│           │                     │                     │            │
│           │            ┌────────┴────────┐            │            │
│           │            │ ApprovalChannel │            │            │
│           │            │  (broadcast +   │            │            │
│           │            │   oneshot)      │            │            │
│           │            └────────┬────────┘            │            │
│           │                     │                     │            │
│  ┌────────┴─────────────────────┴─────────────────────┴────────┐  │
│  │                     Session State                            │  │
│  │         (MCP Service, SSH Session, Output Buffers)          │  │
│  └──────────────────────────────────────────────────────────────┘  │
└──────────┬──────────────────────────────────────────────┬──────────┘
           │                                              │
      Gemini CLI                                         SSH
      (PTY process)                                       │
           │                                        Remote Server
      Google Gemini API
```

### MCP Server Integration

The embedded MCP server provides three tools for Gemini CLI:

| Tool | Description |
|------|-------------|
| `ssh_connect` | Connect to a remote SSH server with credentials |
| `ssh_execute` | Execute a command (requires user approval) |
| `ssh_read_output` | Read recent terminal output for context |

**Key Benefits of MCP Architecture:**
- **Structured Communication**: JSON-RPC tool calls replace fragile text pattern parsing
- **Event-Driven Approval**: Broadcast channels push events instantly (no 500ms polling)
- **Type-Safe Schemas**: JsonSchema validation for all tool parameters
- **Interactive Terminals**: PTY streams remain dedicated to terminal I/O

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

### Using the Dual Terminal Interface

Once connected, you'll see two terminals side by side:

#### Left Pane: Gemini CLI Terminal

The interactive Gemini CLI runs directly in your browser:

1. **Authenticate**: On first launch, you'll see Gemini CLI's authentication menu
   - Select "Login with Google" (recommended)
   - Or use API key if already configured

2. **Chat with Gemini**: Type prompts directly into the terminal
   - Ask questions about programming, systems, etc.
   - Request help understanding SSH terminal output
   - Get recommendations for commands to run

3. **Gemini Observes SSH**: The Gemini CLI can read SSH terminal output via MCP tools
   - Use `ssh_read_output` tool to get recent terminal content
   - Get contextual explanations and suggestions

#### Right Pane: SSH Terminal

Your live SSH connection to the remote server:

1. **Run Commands**: Type commands as you would in any SSH session
2. **Full Terminal Support**: Colors, cursor control, vim, etc.
3. **Output Available to Gemini**: Terminal buffer accessible via MCP tools

#### Command Execution Flow (MCP-Based)

When Gemini needs to execute a command on the SSH terminal:

1. **Tool Call**: Gemini calls the `ssh_execute` MCP tool with the command
2. **Event Broadcast**: The approval request is broadcast to all connected clients
3. **User Review**: An approval modal appears instantly in the browser
4. **Decision**: Click "Approve" or "Reject"
5. **Execution**: Approved commands execute immediately on the SSH terminal
6. **Results**: See output in real-time; Gemini can read results via `ssh_read_output`

```
Gemini CLI                    MCP Server                 Frontend
    │                             │                          │
    ├─── ssh_execute("ls -la") ──►│                          │
    │                             ├── broadcast event ──────►│
    │                             │                          │
    │                             │◄─── user approves ───────┤
    │                             │                          │
    │◄── command executed ────────┤                          │
    │                             │                          │
    ├─── ssh_read_output() ──────►│                          │
    │◄── terminal output ─────────┤                          │
```

### Terminal Features

- **Dual Terminals**: Both Gemini and SSH terminals support full terminal emulation
- **Resizable Panes**: Drag the divider to resize left/right panes
- **Copy/Paste**: Standard terminal copy/paste operations in both terminals
- **Scrollback**: Scroll through history in both terminals
- **Event-Driven Updates**: Instant approval notifications (no polling delay)

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

### MCP Server Configuration

The MCP server is automatically available for each session at:
```
POST http://localhost:3000/mcp/{session_id}     # JSON-RPC endpoint
GET  http://localhost:3000/mcp/{session_id}/events  # SSE event stream
```

The session ID is returned when creating a session via `/api/session/create`.

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
- **Command Approval**: All SSH commands require explicit user approval via MCP flow
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
  - Ensure you approved the command in the modal
  - Check terminal pane for errors
  - Verify SSH connection is still active
  - Check browser console for WebSocket errors

### MCP Issues

- **Approval modal not appearing**:
  - Verify WebSocket connection to `/ws/commands/{session_id}`
  - Check browser console for connection errors
  - Ensure session ID is valid

- **Command approval timeout**:
  - Default timeout is 30 seconds
  - Approve or reject commands promptly
  - Check if frontend is properly connected

## Development

### Project Structure

```
gemini-co-cli/
├── src/
│   ├── main.rs           # Application entry point and routes
│   ├── state.rs          # Session state with MCP service registry
│   ├── ssh.rs            # SSH client implementation (russh)
│   ├── gemini.rs         # Gemini CLI PTY process manager
│   ├── websocket.rs      # WebSocket handlers (Gemini, SSH, Approvals)
│   └── mcp/              # MCP server implementation
│       ├── mod.rs        # Module exports
│       ├── approval.rs   # Event-driven ApprovalChannel (broadcast/oneshot)
│       ├── server.rs     # McpSshService with JSON-RPC handlers
│       ├── tools.rs      # Tool schemas (JsonSchema) and definitions
│       └── http.rs       # Axum HTTP/SSE handlers for MCP endpoints
├── static/
│   ├── index.html        # Frontend HTML (dual terminal UI)
│   ├── style.css         # Styling
│   └── app.js            # Frontend JavaScript (xterm.js, approval handling)
├── Cargo.toml            # Rust dependencies
├── Dockerfile            # Container image definition
└── docker-compose.yml    # Docker Compose configuration
```

### Key Components

| Component | Description |
|-----------|-------------|
| `ApprovalChannel` | Manages command approval flow with broadcast events and oneshot responses |
| `McpSshService` | Handles MCP JSON-RPC requests for SSH tools |
| `SshState` | Shared SSH session state with output buffer |
| `GeminiTerminal` | PTY manager for Gemini CLI process |

### Adding Features

To extend functionality:

1. **Add MCP Tools**: Define new tools in `src/mcp/tools.rs` and handlers in `src/mcp/server.rs`
2. **Customize Approval Flow**: Modify `src/mcp/approval.rs` for different approval patterns
3. **Enhanced Terminal**: Add features in `static/app.js` using xterm.js addons
4. **Additional APIs**: Create new modules in `src/` and register routes in `main.rs`

## API Endpoints

### HTTP Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/` | Serve the main HTML page |
| `POST` | `/api/session/create` | Create a new session (returns session_id and mcp_url) |
| `POST` | `/api/ssh/connect` | Establish SSH connection |
| `POST` | `/mcp/:session_id` | MCP JSON-RPC endpoint for tool calls |
| `GET` | `/mcp/:session_id/events` | SSE stream for approval events |
| `GET` | `/static/*` | Serve static files |

### WebSocket Endpoints

| Path | Description |
|------|-------------|
| `/ws/gemini-terminal/:session_id` | Gemini CLI terminal I/O |
| `/ws/ssh-terminal/:session_id` | SSH terminal I/O |
| `/ws/commands/:session_id` | Command approval events (broadcast) |

### MCP Tools (JSON-RPC)

```json
// ssh_connect
{
  "method": "tools/call",
  "params": {
    "name": "ssh_connect",
    "arguments": {
      "host": "example.com",
      "port": 22,
      "username": "user",
      "password": "pass"
    }
  }
}

// ssh_execute (requires approval)
{
  "method": "tools/call",
  "params": {
    "name": "ssh_execute",
    "arguments": {
      "command": "ls -la",
      "timeout_seconds": 30,
      "wait_for_output": true
    }
  }
}

// ssh_read_output
{
  "method": "tools/call",
  "params": {
    "name": "ssh_read_output",
    "arguments": {
      "lines": 50
    }
  }
}
```

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
- MCP protocol schemas via [schemars](https://github.com/GREsau/schemars)

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
- [Model Context Protocol](https://modelcontextprotocol.io/)
