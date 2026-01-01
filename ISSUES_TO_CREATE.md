# GitHub Issues to Create - Gemini Co-CLI Improvements

This file contains all issues to be created in BDD (Business Driven Design) format.

---

## P0 - Critical Security Issues

### Issue 1: [P0] Implement SSH Server Key Verification to Prevent MITM Attacks
**Labels:** `priority: critical`, `security`, `bug`

#### User Story
As a **system administrator**
I want **SSH connections to verify server keys properly**
So that **I am protected from man-in-the-middle attacks and can trust my SSH connections**

#### Current Problem
The application currently accepts ANY SSH server key without verification (src/ssh.rs:27-30), making users vulnerable to MITM attacks where an attacker could intercept and modify SSH traffic.

#### Acceptance Criteria
**Given** I am connecting to an SSH server
**When** the server presents its public key
**Then** the application should verify the key against a known_hosts file or key fingerprint
**And** warn me if the key doesn't match or is unknown
**And** allow me to accept and save new keys explicitly

**Given** I connect to a server with a changed key
**When** the key doesn't match the saved key
**Then** the connection should be rejected
**And** I should see a clear warning about the key change

#### Technical Details
- Location: `src/ssh.rs:27-30`
- Current code always returns `Ok(true)`
- Implement proper `check_server_key` logic
- Consider storing known hosts in session state or persistent storage
- Add user interface for key acceptance/rejection
- Follow OpenSSH known_hosts format or similar

#### Security Impact
**CRITICAL** - Without this fix, users are vulnerable to MITM attacks where credentials and commands could be intercepted.

#### References
- russh documentation on key verification
- RFC 4251 (SSH Protocol Architecture)

---

### Issue 2: [P0] Add Rate Limiting to Prevent DoS Attacks
**Labels:** `priority: critical`, `security`, `enhancement`

#### User Story
As a **platform operator**
I want **rate limiting on all API endpoints and WebSocket connections**
So that **the service remains available and isn't overwhelmed by abuse or attacks**

#### Current Problem
There is no rate limiting on any API endpoints or WebSocket connections, making the application vulnerable to:
- Denial of Service (DoS) attacks
- Resource exhaustion
- Abuse from automated tools

#### Acceptance Criteria
**Given** I am making requests to API endpoints
**When** I exceed the rate limit threshold
**Then** I should receive a 429 Too Many Requests response
**And** see a clear message about when I can retry

**Given** I am establishing WebSocket connections
**When** I exceed connection limits per IP/session
**Then** new connections should be rejected
**And** existing connections should remain stable

**Given** I am sending WebSocket messages
**When** I send messages too frequently
**Then** excessive messages should be dropped or throttled
**And** I should receive a warning message

#### Technical Details
- Add rate limiting middleware to Axum router
- Protect endpoints:
  - POST /api/session/create
  - POST /api/ssh/connect
  - All WebSocket endpoints
- Consider using tower-governor or similar middleware
- Implement per-IP and per-session limits
- Add WebSocket message size limits
- Configure reasonable limits (e.g., 10 sessions/min per IP)

#### Suggested Implementation
```rust
// Use tower-governor for HTTP rate limiting
// Add custom WebSocket message rate limiting in websocket.rs
// Track message counts per session with time windows
```

#### Priority
**CRITICAL** - Required to prevent service disruption and resource exhaustion

---

### Issue 3: [P0] Add Comprehensive Input Validation and Sanitization
**Labels:** `priority: critical`, `security`, `enhancement`

#### User Story
As a **security-conscious user**
I want **all user inputs to be validated and sanitized**
So that **I am protected from injection attacks and unexpected behavior**

#### Current Problem
The application lacks input validation for:
- SSH commands (no sanitization before execution)
- Terminal resize dimensions (could crash with extreme values)
- WebSocket message content (no size limits)
- Form inputs (host, port, username, passwords)
- Session IDs and other parameters

#### Acceptance Criteria
**Given** I enter SSH connection details
**When** I provide invalid input (empty host, port out of range, etc.)
**Then** I should see clear validation errors
**And** the form should not submit

**Given** I am using the terminal
**When** terminal resize events occur
**Then** dimensions should be validated (min: 1x1, max: reasonable bounds)
**And** invalid dimensions should be ignored safely

**Given** WebSocket messages are sent
**When** message size exceeds limits
**Then** the message should be rejected
**And** connection should remain stable with error message

**Given** I enter commands or text
**When** potentially dangerous characters are present
**Then** they should be safely handled or escaped
**And** no command injection should be possible

#### Technical Details

**Backend Validation (Rust)**
- src/ssh.rs: Validate host format, port range (1-65535)
- src/websocket.rs: Add message size limits (e.g., 1MB max)
- src/gemini.rs: Validate PTY dimensions (1-10000 range)
- Validate session UUIDs properly (replace .unwrap() usage)

**Frontend Validation (JavaScript)**
- static/app.js: Add form validation before submission
- Validate host format (hostname or IP)
- Validate port is numeric and in range
- Validate username is not empty
- Add input length limits

**Command Sanitization**
- Review SSH command execution for injection risks
- Consider allowlist for dangerous characters
- Document any escape mechanisms used

#### Examples
```rust
// Port validation
if port < 1 || port > 65535 {
    return Err(SshError::InvalidPort);
}

// Dimension validation
if cols < 1 || rows < 1 || cols > 10000 || rows > 10000 {
    return Err(TerminalError::InvalidDimensions);
}
```

#### Priority
**CRITICAL** - Required to prevent security vulnerabilities and crashes

---

### Issue 4: [P0] Add Security Headers and SRI for CDN Resources
**Labels:** `priority: critical`, `security`, `enhancement`

#### User Story
As a **web application user**
I want **proper security headers and verified CDN resources**
So that **I am protected from XSS, clickjacking, and supply chain attacks**

#### Current Problem
The application is missing critical security headers and CDN resources lack Subresource Integrity (SRI) hashes:
- No Content Security Policy (CSP)
- No X-Frame-Options
- No X-Content-Type-Options
- No HSTS headers
- CDN resources (xterm.js) can be tampered with

#### Acceptance Criteria
**Given** I access the application
**When** the page loads
**Then** all security headers should be present in the response
**And** they should be properly configured

**Given** external scripts load from CDN
**When** the browser fetches them
**Then** SRI hashes should be verified
**And** scripts should fail to load if hash doesn't match

**Given** I try to embed the app in an iframe
**When** X-Frame-Options is set
**Then** the embedding should be blocked (unless explicitly allowed)

#### Technical Details

**Security Headers to Add (src/main.rs)**
```rust
use tower_http::set_header::SetResponseHeaderLayer;

// Add middleware:
// - Content-Security-Policy
// - X-Frame-Options: DENY
// - X-Content-Type-Options: nosniff
// - Referrer-Policy: strict-origin-when-cross-origin
// - Permissions-Policy
```

**SRI Hashes (static/index.html:83-87)**
Generate and add integrity attributes:
```html
<script
  src="https://cdn.jsdelivr.net/npm/xterm@5.3.0/lib/xterm.min.js"
  integrity="sha384-[HASH_HERE]"
  crossorigin="anonymous"></script>
```

**CSP Configuration**
Suggest starting with:
```
Content-Security-Policy:
  default-src 'self';
  script-src 'self' https://cdn.jsdelivr.net;
  style-src 'self' https://cdn.jsdelivr.net 'unsafe-inline';
  connect-src 'self' ws://localhost:3000 wss://localhost:3000;
  img-src 'self' data:;
```

**HSTS Configuration**
```
Strict-Transport-Security: max-age=31536000; includeSubDomains
```

#### Implementation Steps
1. Add tower-http middleware for security headers
2. Generate SRI hashes for all CDN resources
3. Test CSP doesn't break functionality
4. Document header configuration
5. Consider HTTPS-only mode for production

#### Priority
**CRITICAL** - Required to protect against common web vulnerabilities

#### References
- [OWASP Secure Headers Project](https://owasp.org/www-project-secure-headers/)
- [MDN: CSP](https://developer.mozilla.org/en-US/docs/Web/HTTP/CSP)
- [SRI Hash Generator](https://www.srihash.org/)

---

### Issue 5: [P0] Run Docker Container as Non-Root User
**Labels:** `priority: critical`, `security`, `docker`

#### User Story
As a **security engineer**
I want **the Docker container to run as a non-root user**
So that **potential container breakouts have limited impact on the host system**

#### Current Problem
The Dockerfile doesn't create or switch to a non-root user, meaning the application runs as root inside the container. This violates security best practices and increases risk if the container is compromised.

#### Acceptance Criteria
**Given** the Docker container starts
**When** I check the running process
**Then** it should be running as a non-root user
**And** the user should have minimal privileges

**Given** the application needs to bind to port 3000
**When** running as non-root
**Then** it should still function correctly
**And** all file permissions should be properly configured

**Given** volumes are mounted
**When** the container writes to them
**Then** file ownership should be appropriate
**And** no permission errors should occur

#### Technical Details

**Dockerfile Changes**
```dockerfile
# After installing dependencies, before CMD
# Create non-root user
RUN groupadd -r gemini && useradd -r -g gemini gemini

# Set ownership of application files
RUN chown -R gemini:gemini /app

# Switch to non-root user
USER gemini

# CMD remains the same
CMD ["./gemini-co-cli"]
```

**Verify Implementation**
```bash
# Check process user
docker exec <container> ps aux | grep gemini-co-cli

# Should NOT show root
```

**Volume Permissions**
Update docker-compose.yml if needed to handle volume permissions:
```yaml
volumes:
  gemini-config:
    driver: local
    driver_opts:
      type: none
      o: bind,uid=1000,gid=1000
```

#### Testing Checklist
- [ ] Container starts successfully
- [ ] Application binds to port 3000
- [ ] WebSocket connections work
- [ ] Gemini CLI can write to config volume
- [ ] No permission errors in logs
- [ ] ps shows non-root user

#### Priority
**CRITICAL** - Required for production security compliance

#### References
- [Docker Security Best Practices](https://docs.docker.com/develop/security-best-practices/)
- [CIS Docker Benchmark](https://www.cisecurity.org/benchmark/docker)

---

### Issue 6: [P0] Implement Session Authentication and Authorization
**Labels:** `priority: critical`, `security`, `enhancement`

#### User Story
As a **platform user**
I want **secure authentication for my sessions**
So that **only I can access my terminals and SSH connections, not anyone who knows my session ID**

#### Current Problem
- No authentication/authorization - anyone with a session_id can access it
- Session IDs are just UUIDs without cryptographic security
- No CSRF protection on API endpoints
- No user identity verification
- No IP binding or session hijacking prevention

#### Acceptance Criteria
**Given** I create a new session
**When** the session is created
**Then** I should receive a secure session token
**And** the token should be cryptographically signed
**And** it should expire after a reasonable time

**Given** I try to access someone else's session
**When** I provide their session ID
**Then** access should be denied
**And** I should see an authentication error

**Given** I make API requests
**When** CSRF protection is enabled
**Then** requests without proper tokens should be rejected
**And** legitimate requests should work seamlessly

**Given** my session is idle
**When** the timeout period expires
**Then** the session should be invalidated
**And** I should need to reconnect

#### Technical Details

**Implement Session Authentication**
```rust
// Add to Cargo.toml
tower-sessions = "0.10"
tower-sessions-redis-store = "0.10" // or memory store

// In src/state.rs
pub struct Session {
    pub id: Uuid,
    pub user_id: String,  // Add user identification
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub ip_address: IpAddr,  // Track IP for validation
    // ... existing fields
}
```

**Add CSRF Protection**
- Use tower-csrf or similar middleware
- Add CSRF tokens to forms
- Validate tokens on state-changing requests

**Session Validation Middleware**
```rust
async fn validate_session(
    session_id: &Uuid,
    ip: IpAddr,
    state: &AppState
) -> Result<Session, AuthError> {
    // Verify session exists
    // Check IP matches
    // Check not expired
    // Update last_activity
}
```

**Session Timeout**
- Add configurable timeout (e.g., 30 minutes)
- Auto-cleanup expired sessions
- Notify frontend when session expires

#### Implementation Phases
1. **Phase 1**: Add session expiration and cleanup
2. **Phase 2**: Add IP binding and validation
3. **Phase 3**: Implement proper authentication tokens
4. **Phase 4**: Add CSRF protection
5. **Phase 5**: (Optional) Add user accounts and login

#### Security Considerations
- Use secure random tokens (not just UUIDs)
- Store session secrets securely
- Implement rate limiting on auth endpoints
- Log authentication failures
- Consider using JWT or similar signed tokens

#### Priority
**CRITICAL** - Anyone with a session ID can hijack the session

#### References
- [OWASP Session Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)
- [tower-sessions documentation](https://docs.rs/tower-sessions/)

---

## P1 - High Priority (Stability) Issues

### Issue 7: [P1] Replace .unwrap() and .expect() with Proper Error Handling
**Labels:** `priority: high`, `bug`, `stability`

#### User Story
As a **user of the application**
I want **graceful error handling instead of crashes**
So that **temporary issues don't crash the entire application and I get helpful error messages**

#### Current Problem
The codebase uses `.unwrap()` and `.expect()` in many places that could fail, causing panics and crashing the application instead of gracefully handling errors.

**Locations:**
- main.rs:52-53 - TCP listener binding and server startup
- websocket.rs:51,176,221,228,389,476 - JSON serialization
- websocket.rs:495 - UUID parsing
- gemini.rs:78,84 - PTY operations

#### Acceptance Criteria
**Given** an error occurs during operation
**When** the error is encountered
**Then** the application should handle it gracefully
**And** log the error with proper context
**And** return an appropriate error response
**And** the application should NOT panic or crash

**Given** I am using the WebSocket connection
**When** JSON serialization fails
**Then** I should receive an error message
**And** the WebSocket should remain connected (if possible)

#### Technical Details

**Replace patterns like:**
```rust
// BEFORE
let uuid = Uuid::parse_str(&session_id).unwrap();
serde_json::to_string(&msg).unwrap()

// AFTER
let uuid = Uuid::parse_str(&session_id)
    .map_err(|e| anyhow::anyhow!("Invalid session ID: {}", e))?;
serde_json::to_string(&msg)
    .map_err(|e| anyhow::anyhow!("Failed to serialize message: {}", e))?;
```

**Add proper error types:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum WebSocketError {
    #[error("Invalid session ID: {0}")]
    InvalidSessionId(#[from] uuid::Error),

    #[error("Serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),

    // ... more error types
}
```

**Handle gracefully:**
- Log errors with tracing::error!
- Send error messages to clients when appropriate
- Keep connections alive when possible
- Provide helpful error messages

#### Files to Update
- [ ] src/main.rs - Replace unwrap on server startup
- [ ] src/websocket.rs - All unwrap() and expect() calls
- [ ] src/gemini.rs - All expect() calls
- [ ] src/ssh.rs - Any unwrap() calls

#### Testing
- Test error scenarios (invalid UUIDs, serialization failures)
- Verify application doesn't crash
- Verify error messages are helpful

#### Priority
**HIGH** - Crashes affect all users and make debugging difficult

---

### Issue 8: [P1] Implement Session Cleanup and Garbage Collection
**Labels:** `priority: high`, `bug`, `memory-leak`

#### User Story
As a **platform operator**
I want **unused sessions to be automatically cleaned up**
So that **memory doesn't grow unbounded and server resources are properly managed**

#### Current Problem
Sessions are created but never removed from memory. Over time, this leads to:
- Memory leaks (sessions accumulate forever)
- Resource exhaustion (file handles, processes)
- Security concerns (abandoned sessions remain accessible)

#### Acceptance Criteria
**Given** I create sessions
**When** sessions are idle for the configured timeout
**Then** they should be automatically cleaned up
**And** all associated resources should be freed

**Given** a session is cleaned up
**When** someone tries to access it
**Then** they should receive a "session not found" error
**And** be prompted to create a new session

**Given** I disconnect from a session
**When** I explicitly close it
**Then** cleanup should happen immediately
**And** resources should be freed

#### Technical Details

**Add Session Metadata**
```rust
// src/state.rs
pub struct Session {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub timeout_minutes: u64,
    // ... existing fields
}
```

**Implement Cleanup Task**
```rust
// Spawn background task in main.rs
tokio::spawn(session_cleanup_task(app_state.clone()));

async fn session_cleanup_task(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;

        let now = Utc::now();
        let mut sessions = state.sessions.write().await;

        sessions.retain(|id, session| {
            let idle_time = now - session.last_activity;
            let should_keep = idle_time.num_minutes() < session.timeout_minutes;

            if !should_keep {
                tracing::info!("Cleaning up expired session: {}", id);
                // Cleanup SSH, Gemini processes, etc.
            }

            should_keep
        });
    }
}
```

**Cleanup Actions**
When removing a session, ensure:
- [ ] Close SSH connection
- [ ] Kill Gemini CLI process
- [ ] Close all WebSocket connections
- [ ] Clear pending commands
- [ ] Remove from sessions HashMap

**Configuration**
- Add SESSION_TIMEOUT_MINUTES env var (default: 30)
- Add SESSION_CLEANUP_INTERVAL_SECONDS (default: 60)
- Document in .env.example

#### Testing
- Create session, wait for timeout, verify cleanup
- Verify resources are freed (no zombie processes)
- Verify accessing expired session returns error
- Load test with many sessions

#### Priority
**HIGH** - Memory leak affects long-running instances

---

### Issue 9: [P1] Add WebSocket Reconnection Logic
**Labels:** `priority: high`, `enhancement`, `ux`

#### User Story
As a **user working with remote terminals**
I want **WebSocket connections to automatically reconnect**
So that **temporary network issues don't permanently disconnect my session**

#### Current Problem
When network connectivity is lost or interrupted:
- WebSocket connections close permanently
- User must refresh the page to reconnect
- No indication of connection status
- Work in progress may be lost

#### Acceptance Criteria
**Given** my WebSocket connection drops
**When** network connectivity returns
**Then** the application should automatically attempt to reconnect
**And** restore my session state
**And** show me the reconnection status

**Given** reconnection attempts fail
**When** maximum retries are exceeded
**Then** I should see a clear error message
**And** be offered the option to manually reconnect

**Given** I am reconnecting
**When** the reconnection is in progress
**Then** I should see a loading indicator
**And** know which connection is reconnecting

#### Technical Details

**Frontend Changes (static/app.js)**
```javascript
class WebSocketManager {
    constructor(url, onMessage) {
        this.url = url;
        this.onMessage = onMessage;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 5;
        this.reconnectDelay = 1000; // Start with 1s
        this.connect();
    }

    connect() {
        this.ws = new WebSocket(this.url);

        this.ws.onclose = () => {
            if (this.reconnectAttempts < this.maxReconnectAttempts) {
                this.reconnect();
            } else {
                this.showReconnectError();
            }
        };

        // ... other handlers
    }

    reconnect() {
        this.reconnectAttempts++;
        const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);

        showReconnecting();
        setTimeout(() => this.connect(), delay);
    }
}
```

**UI Indicators**
Add connection status indicators:
```html
<!-- Connection status banner -->
<div id="connection-status" class="hidden">
    <span class="status-icon"></span>
    <span class="status-text"></span>
</div>
```

**Backend Support**
- Ensure sessions persist during brief disconnections
- Allow reconnecting to existing session
- Send session state on reconnection (e.g., buffered output)

**Exponential Backoff**
- Attempt 1: 1s delay
- Attempt 2: 2s delay
- Attempt 3: 4s delay
- Attempt 4: 8s delay
- Attempt 5: 16s delay
- After 5 attempts: Show error, allow manual retry

#### Testing
- Simulate network interruption (disconnect WiFi)
- Verify automatic reconnection
- Test max retries behavior
- Verify session state is preserved

#### Priority
**HIGH** - Poor UX and data loss potential

---

### Issue 10: [P1] Fix Command Monitoring Performance Issue
**Labels:** `priority: high`, `bug`, `performance`

#### User Story
As a **system operator**
I want **efficient command approval monitoring**
So that **the system doesn't waste resources and responds quickly**

#### Current Problem
The command approval WebSocket handler (src/websocket.rs:466-481) has O(N²) complexity:
- Polls every 500ms
- Re-sends ALL unapproved commands every iteration
- No tracking of what's already been sent to the client
- Creates unnecessary network traffic and CPU usage

#### Acceptance Criteria
**Given** there are pending commands
**When** the monitoring loop runs
**Then** only NEW commands should be sent to the client
**And** already-sent commands should not be resent

**Given** a command is approved
**When** the command is executed
**Then** it should be immediately removed from monitoring
**And** not sent again in future iterations

**Given** the system is under load
**When** many commands are pending
**Then** performance should not degrade significantly
**And** response time should remain acceptable

#### Technical Details

**Current Problem Code:**
```rust
// src/websocket.rs:466-481
loop {
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Gets ALL pending commands every time
    let pending = session.pending_commands.lock().unwrap().clone();

    // Sends ALL of them, even if already sent!
    for cmd in pending {
        if !cmd.approved {
            ws_tx.send(Message::Text(...)).await; // Re-sends every time!
        }
    }
}
```

**Proposed Solution:**
```rust
// Track what's been sent
let mut sent_command_ids = HashSet::new();

loop {
    tokio::time::sleep(Duration::from_millis(500)).await;

    let pending = session.pending_commands.lock().unwrap();

    for cmd in pending.iter() {
        if !cmd.approved && !sent_command_ids.contains(&cmd.id) {
            // Only send NEW commands
            ws_tx.send(Message::Text(...)).await;
            sent_command_ids.insert(cmd.id);
        }
    }

    // Clean up sent_command_ids for removed commands
    pending.iter().map(|c| c.id).collect::<HashSet<_>>()
        .difference(&sent_command_ids)
        .for_each(|id| { sent_command_ids.remove(id); });
}
```

**Better Approach - Event-Driven:**
```rust
// Instead of polling, use tokio::sync::watch channel
// Notify when new command added
// Only send when notified
```

**Alternative - Use Channels:**
- Add a channel to Session for command notifications
- Send to channel when new command added
- Listen on channel instead of polling
- Much more efficient

#### Performance Impact
- Current: O(N²) where N = number of pending commands
- Fixed: O(N) with tracking, or O(1) with event-driven
- Reduces network traffic significantly
- Reduces CPU usage

#### Files to Update
- src/websocket.rs:466-481 (command_approval_ws_handler)
- src/state.rs (possibly add notification channel)

#### Testing
- Test with multiple pending commands
- Verify commands sent only once
- Measure performance improvement
- Verify commands still work correctly

#### Priority
**HIGH** - Affects performance and scalability

---

### Issue 11: [P1] Add Health Checks and Monitoring Endpoints
**Labels:** `priority: high`, `enhancement`, `ops`

#### User Story
As a **DevOps engineer**
I want **health check endpoints and basic metrics**
So that **I can monitor the application and integrate with orchestration tools**

#### Current Problem
No way to:
- Check if the application is healthy
- Monitor active sessions
- Track resource usage
- Integrate with Kubernetes/Docker health checks
- Alert on issues

#### Acceptance Criteria
**Given** I query the health endpoint
**When** the application is healthy
**Then** I should receive a 200 OK response
**And** see basic health information

**Given** I query the metrics endpoint
**When** the application is running
**Then** I should see metrics like active sessions, uptime, etc.

**Given** I configure Docker/Kubernetes
**When** using health checks
**Then** the orchestrator should correctly detect unhealthy state

#### Technical Details

**Add Health Check Endpoint**
```rust
// src/main.rs
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
        uptime_seconds: /* calculate uptime */,
        version: env!("CARGO_PKG_VERSION"),
    })
}

// Add route
.route("/health", get(health_check))
```

**Add Metrics Endpoint**
```rust
async fn metrics(State(state): State<Arc<AppState>>) -> Json<MetricsResponse> {
    let sessions = state.sessions.read().await;

    Json(MetricsResponse {
        active_sessions: sessions.len(),
        total_sessions_created: state.total_sessions_created.load(Ordering::Relaxed),
        uptime_seconds: /* uptime */,
        // Add more metrics as needed
    })
}

.route("/metrics", get(metrics))
```

**Update Dockerfile**
```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:3000/health || exit 1
```

**Update docker-compose.yml**
```yaml
healthcheck:
  test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:3000/health"]
  interval: 30s
  timeout: 3s
  retries: 3
  start_period: 5s
```

**Metrics to Track**
- Active sessions count
- Total sessions created (lifetime)
- Active WebSocket connections
- Failed SSH connections
- Application uptime
- Memory usage (optional)

**Optional - Prometheus Integration**
Consider adding prometheus-compatible /metrics endpoint:
```rust
// Add to Cargo.toml
prometheus = "0.13"
```

#### Implementation Steps
1. Add health check endpoint
2. Add metrics endpoint
3. Update Dockerfile with HEALTHCHECK
4. Update docker-compose.yml
5. Document endpoints in README
6. (Optional) Add Prometheus metrics

#### Testing
- Test health endpoint returns 200
- Test metrics accuracy
- Test Docker health check works
- Verify unhealthy state detected

#### Priority
**HIGH** - Required for production deployments

---

## P2 - Medium Priority (Quality) Issues

### Issue 12: [P2] Add Integration Tests for Core Workflows
**Labels:** `priority: medium`, `testing`, `quality`

#### User Story
As a **developer**
I want **comprehensive integration tests**
So that **I can confidently make changes without breaking existing functionality**

#### Current Problem
The codebase has minimal testing:
- Only 1 unit test (gemini.rs:115-130)
- ZERO integration tests for WebSockets
- ZERO tests for SSH connections
- ZERO tests for session management
- ZERO API endpoint tests
- No CI/CD pipeline

#### Acceptance Criteria
**Given** I run the test suite
**When** all tests execute
**Then** I should have confidence the core workflows work
**And** see coverage for critical paths

**Given** I make code changes
**When** I run tests
**Then** breaking changes should be caught
**And** I should know what broke

#### Technical Details

**Test Framework Setup**
```rust
// Cargo.toml
[dev-dependencies]
tokio-test = "0.4"
axum-test = "14"
mockall = "0.12"
```

**Test Categories to Add**

**1. Session Management Tests**
```rust
#[tokio::test]
async fn test_create_session() {
    // Test POST /api/session/create
    // Verify session created
    // Verify UUID returned
}

#[tokio::test]
async fn test_session_cleanup() {
    // Create session
    // Wait for timeout
    // Verify cleaned up
}
```

**2. WebSocket Tests**
```rust
#[tokio::test]
async fn test_gemini_websocket_connection() {
    // Create session
    // Connect to WebSocket
    // Send message
    // Verify response
}

#[tokio::test]
async fn test_command_approval_flow() {
    // Simulate Gemini sending EXECUTE command
    // Verify command sent to approval WS
    // Approve command
    // Verify execution on SSH
}
```

**3. SSH Connection Tests (with mock)**
```rust
#[tokio::test]
async fn test_ssh_connect_success() {
    // Mock SSH server
    // Test connection
    // Verify success
}

#[tokio::test]
async fn test_ssh_connect_failure() {
    // Test with invalid credentials
    // Verify error handling
}
```

**4. Error Handling Tests**
```rust
#[tokio::test]
async fn test_invalid_session_id() {
    // Try to access non-existent session
    // Verify error response
}
```

**Test Utilities**
Create test helpers:
```rust
// tests/common/mod.rs
pub async fn create_test_app() -> Router { /* ... */ }
pub async fn create_test_session(app: &Router) -> Uuid { /* ... */ }
pub fn mock_ssh_server() -> MockSshServer { /* ... */ }
```

#### Test Coverage Goals
- Session management: 80%+
- WebSocket handlers: 70%+
- SSH client: 60%+
- Error handling: 80%+

#### CI/CD Integration
Add GitHub Actions workflow:
```yaml
name: Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - run: cargo test --all
```

#### Priority
**MEDIUM** - Important for long-term maintainability

---

### Issue 13: [P2] Reduce Code Duplication Across Modules
**Labels:** `priority: medium`, `refactor`, `quality`

#### User Story
As a **developer**
I want **shared code to be reused rather than duplicated**
So that **bug fixes and improvements only need to be made once**

#### Current Problem
Significant code duplication exists:
- WebSocket session validation repeated 3+ times (websocket.rs:126-132, 342-348, 446-452)
- Terminal initialization duplicated in frontend (Gemini vs SSH setup)
- Error handling patterns repeated throughout
- WebSocket connection setup duplicated

#### Acceptance Criteria
**Given** I fix a bug in shared logic
**When** I make the fix
**Then** it should apply everywhere the logic is used
**And** I shouldn't need to update multiple copies

**Given** I add a new feature to shared code
**When** I implement it
**Then** all users of that code should benefit
**And** behavior should be consistent

#### Technical Details

**1. Extract Session Validation**
```rust
// src/websocket.rs - Create shared function
async fn validate_session_id(
    session_id: &str,
    state: &Arc<AppState>
) -> Result<Arc<RwLock<Session>>, String> {
    let uuid = Uuid::parse_str(session_id)
        .map_err(|e| format!("Invalid session ID: {}", e))?;

    let sessions = state.sessions.read().await;
    sessions.get(&uuid)
        .cloned()
        .ok_or_else(|| "Session not found".to_string())
}

// Use in all WebSocket handlers
let session = validate_session_id(&session_id, &state).await?;
```

**2. Extract WebSocket Error Sending**
```rust
async fn send_error(ws_tx: &WebSocketSender, error: &str) -> Result<()> {
    let error_msg = json!({
        "type": "error",
        "message": error
    });
    ws_tx.send(Message::Text(serde_json::to_string(&error_msg)?)).await?;
    Ok(())
}
```

**3. Frontend Terminal Setup**
```javascript
// static/app.js - Extract terminal factory
function createTerminal(containerId, options = {}) {
    const terminal = new Terminal({
        cursorBlink: true,
        fontSize: 14,
        fontFamily: 'Menlo, Monaco, "Courier New", monospace',
        theme: {
            background: '#1e1e1e',
            foreground: '#ffffff',
        },
        ...options
    });

    const fitAddon = new FitAddon.FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(document.getElementById(containerId));
    fitAddon.fit();

    return { terminal, fitAddon };
}

// Usage
const gemini = createTerminal('gemini-terminal');
const ssh = createTerminal('ssh-terminal');
```

**4. Extract WebSocket Manager**
```javascript
class WebSocketConnection {
    constructor(url, terminal) {
        this.url = url;
        this.terminal = terminal;
        this.connect();
    }

    connect() { /* shared logic */ }
    send(data) { /* shared logic */ }
    close() { /* shared logic */ }
}
```

#### Files to Refactor
- [ ] src/websocket.rs - Extract validation, error sending
- [ ] static/app.js - Extract terminal setup
- [ ] static/app.js - Extract WebSocket management
- [ ] src/*.rs - Standardize error handling patterns

#### Metrics
- Before: ~200 lines duplicated
- After: <50 lines duplicated
- Code reduction: 20-30%

#### Testing
- All existing tests should pass
- Add tests for extracted functions
- Verify behavior unchanged

#### Priority
**MEDIUM** - Improves maintainability

---

### Issue 14: [P2] Add Comprehensive Logging and Error Messages
**Labels:** `priority: medium`, `enhancement`, `ops`

#### User Story
As a **developer debugging issues**
I want **comprehensive logging with context**
So that **I can quickly identify and fix problems in production**

#### Current Problem
- Minimal structured logging
- Generic error messages don't help troubleshooting
- No request/response logging
- No correlation IDs across operations
- Difficult to trace issues through the system

#### Acceptance Criteria
**Given** an error occurs
**When** I check the logs
**Then** I should see detailed context about what happened
**And** be able to trace the operation across components

**Given** I'm debugging a user issue
**When** I search logs by session ID
**Then** I should see all operations for that session
**And** understand the sequence of events

#### Technical Details

**Add Request Logging Middleware**
```rust
// src/main.rs
use tower_http::trace::{TraceLayer, DefaultMakeSpan, DefaultOnResponse};

let app = Router::new()
    // ... routes ...
    .layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().include_headers(true))
            .on_response(DefaultOnResponse::new().include_headers(true))
    );
```

**Add Correlation IDs**
```rust
// Generate correlation ID per request
use uuid::Uuid;

#[derive(Clone)]
struct CorrelationId(String);

// Middleware to add correlation ID
// Include in all log messages
```

**Structured Logging Examples**
```rust
// src/websocket.rs
tracing::info!(
    session_id = %session.id,
    correlation_id = %correlation_id,
    "WebSocket connection established"
);

tracing::error!(
    session_id = %session.id,
    error = %e,
    "Failed to send WebSocket message"
);

// src/ssh.rs
tracing::info!(
    host = %config.host,
    port = config.port,
    user = %config.username,
    "Attempting SSH connection"
);

tracing::warn!(
    host = %config.host,
    attempt = retry_count,
    "SSH connection failed, retrying"
);
```

**Log Levels**
- ERROR: Failures that need immediate attention
- WARN: Issues that recovered but might indicate problems
- INFO: Key operations (connections, commands)
- DEBUG: Detailed flow for debugging
- TRACE: Very verbose internal details

**Better Error Messages**
```rust
// Instead of:
"Connection failed"

// Provide:
"SSH connection to {}:{} failed for user {}: {} (attempt {}/{})",
    host, port, username, error, attempt, max_attempts
```

**Add Log Configuration**
```rust
// src/main.rs
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

tracing_subscriber::registry()
    .with(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "gemini_co_cli=debug,tower_http=debug".into())
    )
    .with(tracing_subscriber::fmt::layer().json()) // JSON for production
    .init();
```

#### Implementation Checklist
- [ ] Add TraceLayer middleware
- [ ] Add correlation IDs
- [ ] Update all tracing calls with context
- [ ] Improve error messages
- [ ] Configure JSON logging for production
- [ ] Document logging configuration
- [ ] Add log sampling for high-volume events

#### Priority
**MEDIUM** - Critical for production operations

---

### Issue 15: [P2] Add API Documentation
**Labels:** `priority: medium`, `documentation`, `dx`

#### User Story
As a **developer integrating with the API**
I want **clear API documentation**
So that **I understand request/response formats and can integrate successfully**

#### Current Problem
- No API documentation
- Request/response schemas not documented
- WebSocket message formats not specified
- No examples of API usage
- Developers must read source code

#### Acceptance Criteria
**Given** I want to integrate with the API
**When** I read the documentation
**Then** I should understand all endpoints
**And** know the request/response formats
**And** see examples of usage

#### Technical Details

**Create API Documentation File**
```markdown
# API Documentation

## REST Endpoints

### POST /api/session/create
Creates a new terminal session.

**Request:** None

**Response:**
\`\`\`json
{
  "session_id": "550e8400-e29b-41d4-a716-446655440000"
}
\`\`\`

**Status Codes:**
- 200: Success
- 500: Server error

**Example:**
\`\`\`bash
curl -X POST http://localhost:3000/api/session/create
\`\`\`

### POST /api/ssh/connect
Establishes SSH connection for a session.

**Request:**
\`\`\`json
{
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "host": "example.com",
  "port": 22,
  "username": "user",
  "password": "optional",
  "private_key": "optional"
}
\`\`\`

**Response:**
\`\`\`json
{
  "success": true
}
\`\`\`

... (more endpoints)

## WebSocket Endpoints

### WS /ws/gemini-terminal/:session_id
Bidirectional Gemini CLI terminal stream.

**Message Types:**

*Client → Server:*
\`\`\`json
{
  "type": "input",
  "data": "user input text"
}
\`\`\`

\`\`\`json
{
  "type": "resize",
  "cols": 80,
  "rows": 24
}
\`\`\`

*Server → Client:*
\`\`\`json
{
  "type": "output",
  "data": "terminal output"
}
\`\`\`

... (more WebSocket docs)
```

**Consider Using OpenAPI/Swagger**
```rust
// Add to Cargo.toml
utoipa = "4"
utoipa-swagger-ui = "6"

// Add OpenAPI annotations
#[utoipa::path(
    post,
    path = "/api/session/create",
    responses(
        (status = 200, description = "Session created", body = SessionResponse)
    )
)]
async fn create_session() -> Json<SessionResponse> {
    // ...
}
```

**Documentation Sections**
1. Overview
2. Authentication (when implemented)
3. REST Endpoints
4. WebSocket Endpoints
5. Message Formats
6. Error Codes
7. Examples
8. Rate Limiting
9. Best Practices

#### Deliverables
- [ ] API.md documentation file
- [ ] OpenAPI/Swagger spec (optional)
- [ ] Interactive API explorer (optional)
- [ ] Link from README
- [ ] Code examples

#### Priority
**MEDIUM** - Important for developer experience

---

## P3 - Lower Priority (Enhancement) Issues

### Issue 16: [P3] Add Session Persistence Across Restarts
**Labels:** `priority: low`, `enhancement`, `feature`

#### User Story
As a **user with long-running sessions**
I want **my sessions to survive server restarts**
So that **I don't lose my work during deployments or maintenance**

#### Current Problem
All sessions are stored in memory only:
- Server restart loses all sessions
- Users must reconnect after deployments
- No way to recover session state
- SSH connections lost

#### Acceptance Criteria
**Given** I have an active session
**When** the server restarts
**Then** my session should be restored
**And** I should be able to reconnect to it

**Given** the server starts up
**When** it initializes
**Then** it should load previously saved sessions
**And** attempt to restore connections

#### Technical Details

**Storage Options**
1. Redis for distributed deployments
2. SQLite for single-instance
3. PostgreSQL for production

**Session State to Persist**
```rust
#[derive(Serialize, Deserialize)]
struct PersistedSession {
    id: Uuid,
    created_at: DateTime<Utc>,
    last_activity: DateTime<Utc>,
    ssh_config: Option<SshConfig>,
    gemini_state: Option<GeminiState>,
    // Don't persist: passwords, WebSocket connections
}
```

**Implementation**
```rust
trait SessionStore {
    async fn save_session(&self, session: &Session) -> Result<()>;
    async fn load_session(&self, id: Uuid) -> Result<Option<Session>>;
    async fn delete_session(&self, id: Uuid) -> Result<()>;
    async fn list_sessions(&self) -> Result<Vec<Uuid>>;
}

// Implement for Redis or SQLite
```

**Limitations**
- Cannot persist WebSocket connections (clients must reconnect)
- SSH connections may need re-authentication
- Gemini CLI process must be restarted
- Only metadata persisted, not active state

#### Priority
**LOW** - Nice to have, not critical

---

### Issue 17: [P3] Add Keyboard Shortcuts and Improved UX
**Labels:** `priority: low`, `enhancement`, `ux`

#### User Story
As a **power user**
I want **keyboard shortcuts and better UX**
So that **I can work more efficiently**

#### Current Problem
- No keyboard shortcuts
- No loading indicators
- Must use mouse for all interactions
- No visual feedback for operations
- Password field doesn't toggle visibility

#### Acceptance Criteria
**Given** I use keyboard shortcuts
**When** I press the shortcut
**Then** the corresponding action should execute

**Given** an operation is in progress
**When** I'm waiting
**Then** I should see a loading indicator

#### Features to Add

**1. Keyboard Shortcuts**
- `Ctrl+D` - Disconnect
- `Ctrl+R` - Reconnect
- `Escape` - Close modal
- `Enter` - Submit form/approve command
- `Ctrl+L` - Clear terminal
- `Ctrl+=/-` - Adjust font size

**2. Loading Indicators**
- Show spinner during SSH connection
- Show progress during reconnection
- Indicate when Gemini is thinking

**3. Form Improvements**
- Toggle password visibility (eye icon)
- Form validation with inline errors
- Auto-focus appropriate fields
- Enter key submits form

**4. Visual Feedback**
- Connection status indicator
- Command execution indicator
- Success/error notifications (toast)
- Terminal focus indicator

**5. Other UX**
- Auto-focus terminal after connect
- Remember last connection details
- Export terminal history
- Copy/paste improvements

#### Priority
**LOW** - Quality of life improvements

---

### Issue 18: [P3] Add Command History and Management
**Labels:** `priority: low`, `enhancement`, `feature`

#### User Story
As a **user approving commands**
I want **to see command history and manage pending commands**
So that **I can track what's been executed and cancel unwanted commands**

#### Current Problem
- No command history
- Can't cancel pending commands
- Commands wait indefinitely
- No record of approved vs rejected
- No ability to re-run previous commands

#### Acceptance Criteria
**Given** commands have been executed
**When** I view command history
**Then** I should see all commands with their status

**Given** a command is pending
**When** I want to cancel it
**Then** I should be able to remove it from the queue

**Given** a command succeeded previously
**When** I want to run it again
**Then** I should be able to select and re-run it

#### Features to Add

**1. Command History UI**
```html
<div id="command-history">
    <h3>Command History</h3>
    <div class="command-item">
        <span class="command-text">ls -la</span>
        <span class="command-status approved">Approved</span>
        <span class="command-time">2 minutes ago</span>
    </div>
</div>
```

**2. Backend Support**
```rust
pub struct CommandHistory {
    pub id: Uuid,
    pub command: String,
    pub timestamp: DateTime<Utc>,
    pub status: CommandStatus,
    pub result: Option<String>,
}

enum CommandStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
    Failed,
}
```

**3. Features**
- View all commands (pending, approved, rejected)
- Cancel pending commands
- Re-run previous commands
- Search command history
- Export history
- Clear history

#### Priority
**LOW** - Nice to have feature

---

### Issue 19: [P3] Add Accessibility Improvements
**Labels:** `priority: low`, `enhancement`, `a11y`

#### User Story
As a **user with accessibility needs**
I want **the application to be fully accessible**
So that **I can use it effectively with assistive technologies**

#### Current Problem
- No ARIA labels for screen readers
- No keyboard-only navigation support
- Color-only status indicators
- No font size controls
- Poor contrast in some areas
- No focus indicators

#### Acceptance Criteria
**Given** I use a screen reader
**When** I navigate the application
**Then** all elements should be properly announced
**And** I should understand the current state

**Given** I use only keyboard
**When** I navigate the application
**Then** I should be able to access all features
**And** see clear focus indicators

**Given** I am colorblind
**When** I view status indicators
**Then** I should be able to distinguish states without color

#### Improvements to Add

**1. ARIA Labels**
```html
<div id="gemini-terminal"
     role="region"
     aria-label="Gemini AI Terminal"
     aria-live="polite">
</div>

<button aria-label="Connect to SSH server">Connect</button>
<input aria-label="SSH hostname or IP address" />
```

**2. Keyboard Navigation**
- Tab order logical
- All interactive elements reachable
- Skip links for terminal regions
- Focus traps for modals
- Escape closes modals

**3. Visual Improvements**
- Status icons + text (not just color)
- High contrast mode
- Font size controls
- Clear focus indicators
- Proper heading hierarchy

**4. Screen Reader Support**
- Announce connection state changes
- Announce command requests
- Live region for terminal output
- Descriptive button labels

#### Testing
- Test with NVDA/JAWS screen readers
- Test keyboard-only navigation
- Run accessibility audit (axe, Lighthouse)
- Test with high contrast mode

#### Priority
**LOW** - Important for inclusivity, but not blocking

---

### Issue 20: [P3] Add SSH Connection Features
**Labels:** `priority: low`, `enhancement`, `ssh`

#### User Story
As a **system administrator**
I want **advanced SSH features**
So that **I can connect to more complex environments**

#### Current Problem
- No SSH key passphrase support
- No SSH agent forwarding
- No jump host/bastion support
- No connection keepalive
- No known_hosts file support

#### Acceptance Criteria
**Given** my SSH key has a passphrase
**When** I connect
**Then** I should be prompted for the passphrase
**And** connection should succeed

**Given** I need to connect through a jump host
**When** I configure the connection
**Then** I should be able to specify proxy settings
**And** connect through the bastion

#### Features to Add

**1. Key Passphrase Support**
```rust
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub private_key: Option<String>,
    pub key_passphrase: Option<String>, // Add this
}
```

**2. Jump Host Support**
```rust
pub struct SshConfig {
    // ... existing fields
    pub jump_host: Option<Box<SshConfig>>,
}
```

**3. Keepalive**
```rust
// Send keepalive packets every 30s
session.channel_open_session().await?;
// Configure TCP keepalive
```

**4. Known Hosts**
- Load from ~/.ssh/known_hosts
- Verify server keys
- Update known_hosts on accept

**5. SSH Agent**
- Connect to SSH agent socket
- Use agent keys for authentication

#### Priority
**LOW** - Advanced features for power users

---

## Summary

**Total Issues: 20**
- P0 (Critical): 6 issues - Security vulnerabilities
- P1 (High): 5 issues - Stability and reliability
- P2 (Medium): 4 issues - Quality and maintainability
- P3 (Low): 5 issues - Enhancements and UX

All issues follow BDD format with:
- User stories (As a... I want... So that...)
- Acceptance criteria (Given/When/Then)
- Technical details
- Priority justification
