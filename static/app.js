// Global state
let sessionId = null;
let geminiTerminalWs = null;
let sshTerminalWs = null;
let commandApprovalWs = null;
let geminiTerminal = null;
let sshTerminal = null;
let geminiFitAddon = null;
let sshFitAddon = null;
let pendingCommand = null;
let sshConnected = false;
let geminiConnected = false;

// SSH Context Awareness - Output Buffer
const SSH_BUFFER_MAX_ENTRIES = 100;
const SSH_BUFFER_MAX_CHARS = 50000; // Maximum total characters to keep
let sshOutputBuffer = [];
let sshContextEnabled = true; // Toggle for automatic context inclusion
let currentInputBuffer = ''; // Track current user input for context detection

// System prompt for Gemini - establishes context and rules
const SYSTEM_PROMPT = `You are an expert DevOps and Systems Administration AI assistant embedded in a web-based terminal application.

YOUR ENVIRONMENT:
1. You exist within a web browser interface on the user's local machine.
2. The user is viewing a split-screen layout:
   - LEFT PANE: Your chat interface (Gemini).
   - RIGHT PANE: A live SSH session connected to a completely separate remote server.

YOUR CAPABILITIES:
1. CONTEXT AWARENESS: You are provided with the text buffer (logs, standard output, standard error) from the user's SSH terminal. You must use this context to answer questions, diagnose errors, and summarize system state.
2. COMMAND SUGGESTION: You can suggest shell commands to the user. The user interface allows the user to execute these commands with a single click after reviewing them.

YOUR CONSTRAINTS & RULES:
1. SYSTEM SEPARATION (CRITICAL):
   - You do NOT have direct access to the remote server's filesystem or kernel. You are an observer in the browser.
   - You cannot "read a file" on the server unless the user \`cat\`s it to the terminal output first.
   - You cannot "fix" a bug on the server directly; you can only provide the command for the user to fix it.
   - If the user asks you to "save a file," clarify if they mean downloading it to their local machine (browser action) or creating a file on the remote server (requires a command like \`echo "content" > file.txt\`).

2. COMMAND SAFETY & FORMATTING:
   - When you suggest a command to be executed in the SSH session, you MUST format it inside a Markdown code block with \`\`\`bash or \`\`\`sh.
   - NEVER suggest destructive commands (like \`rm -rf /\` or formatting disks) without an extreme warning and explicit confirmation request.
   - If a command requires root privileges, verify if the user is root (look for \`#\` in the prompt) or prepend \`sudo\`.

3. RESPONSE STYLE:
   - Be concise. Terminal users value brevity.
   - Do not repeat the terminal output back to the user unless analyzing a specific line for an error.
   - Focus on "Actionable Advice."

Please confirm you understand these constraints by responding briefly.`;

// Command suggestion tracking
let suggestedCommands = []; // Track detected commands from Gemini output
let geminiOutputBuffer = ''; // Buffer Gemini output for code block detection
let systemPromptSent = false; // Track if system prompt has been sent

// Context detection patterns - keywords that suggest user is asking about SSH output
const CONTEXT_TRIGGER_PATTERNS = [
    /what (does|is) (this|that|the) (error|output|result|message)/i,
    /explain (this|that|the) (error|output|result|message)/i,
    /what('s| is) (wrong|happening|going on)/i,
    /why (did|does|is) (this|that|it)/i,
    /summarize (the )?(files|output|results|logs)/i,
    /what (files|commands|errors)/i,
    /help (me )?(understand|fix|debug)/i,
    /can you (see|read|analyze) (the|this|my)/i,
    /look at (the|this|my) (output|terminal|error|log)/i,
    /(fix|debug|solve|resolve) (this|that|the|it)/i,
    /what (happened|went wrong)/i,
    /^(this|that) (error|output|command)/i,
    /analyze (the|this|my)/i,
];

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    initializeApp();
});

// ============================================================================
// SSH Context Awareness Functions
// ============================================================================

/**
 * Add SSH output to the local buffer with timestamp
 */
function addSshOutputToBuffer(output) {
    const entry = {
        timestamp: Date.now(),
        data: output,
        // Strip ANSI codes for analysis (keep original for display context)
        plainText: stripAnsiCodes(output),
    };

    sshOutputBuffer.push(entry);

    // Trim buffer if too many entries
    while (sshOutputBuffer.length > SSH_BUFFER_MAX_ENTRIES) {
        sshOutputBuffer.shift();
    }

    // Also trim by total character count
    let totalChars = sshOutputBuffer.reduce((sum, e) => sum + e.data.length, 0);
    while (totalChars > SSH_BUFFER_MAX_CHARS && sshOutputBuffer.length > 1) {
        const removed = sshOutputBuffer.shift();
        totalChars -= removed.data.length;
    }
}

/**
 * Strip ANSI escape codes from text
 */
function stripAnsiCodes(text) {
    // eslint-disable-next-line no-control-regex
    return text.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '')
               .replace(/\x1B\][^\x07]*\x07/g, '')  // OSC sequences
               .replace(/\r/g, '');  // Carriage returns
}

/**
 * Get recent SSH output as formatted context string
 */
function getSshContext(maxLines = 50, maxChars = 10000) {
    if (sshOutputBuffer.length === 0) {
        return null;
    }

    let context = '';
    let lineCount = 0;

    // Build context from most recent entries
    for (let i = sshOutputBuffer.length - 1; i >= 0 && lineCount < maxLines; i--) {
        const entry = sshOutputBuffer[i];
        const lines = entry.plainText.split('\n');

        for (let j = lines.length - 1; j >= 0 && lineCount < maxLines; j--) {
            const line = lines[j].trim();
            if (line) {
                context = line + '\n' + context;
                lineCount++;
            }
        }

        if (context.length > maxChars) {
            context = context.substring(context.length - maxChars);
            break;
        }
    }

    return context.trim() || null;
}

/**
 * Check if user input suggests they want SSH context
 */
function shouldIncludeSshContext(input) {
    if (!sshConnected || sshOutputBuffer.length === 0) {
        return false;
    }

    if (!sshContextEnabled) {
        return false;
    }

    const trimmedInput = input.trim();

    // Check against context trigger patterns
    for (const pattern of CONTEXT_TRIGGER_PATTERNS) {
        if (pattern.test(trimmedInput)) {
            return true;
        }
    }

    return false;
}

/**
 * Format SSH context for injection into prompt
 */
function formatSshContextForPrompt(context) {
    return `\n[SSH Terminal Context - Recent Output]\n\`\`\`\n${context}\n\`\`\`\n\n`;
}

/**
 * Clear SSH output buffer (useful for fresh start)
 */
function clearSshBuffer() {
    sshOutputBuffer = [];
    console.log('SSH output buffer cleared');
}

// ============================================================================
// Command Suggestion Detection & Execution
// ============================================================================

/**
 * Detect code blocks in Gemini output and extract commands
 */
function detectCodeBlocks(text) {
    // Match markdown code blocks: ```bash, ```sh, ```shell, or just ```
    const codeBlockRegex = /```(?:bash|sh|shell|zsh)?\s*\n([\s\S]*?)```/g;
    const commands = [];
    let match;

    while ((match = codeBlockRegex.exec(text)) !== null) {
        const command = match[1].trim();
        if (command && !commands.includes(command)) {
            commands.push(command);
        }
    }

    return commands;
}

/**
 * Add a suggested command to the panel
 */
function addSuggestedCommand(command) {
    // Avoid duplicates
    if (suggestedCommands.some(c => c.command === command)) {
        return;
    }

    const suggestion = {
        id: Date.now() + Math.random().toString(36).substr(2, 9),
        command: command,
        timestamp: Date.now(),
    };

    suggestedCommands.push(suggestion);

    // Keep only last 10 suggestions
    if (suggestedCommands.length > 10) {
        suggestedCommands.shift();
    }

    updateSuggestionsPanel();
}

/**
 * Update the suggestions panel UI
 */
function updateSuggestionsPanel() {
    const panel = document.getElementById('suggestions-panel');
    const container = document.getElementById('suggestions-container');

    if (!panel || !container) return;

    if (suggestedCommands.length === 0) {
        panel.style.display = 'none';
        return;
    }

    panel.style.display = 'block';
    container.innerHTML = '';

    suggestedCommands.forEach((suggestion, index) => {
        const card = document.createElement('div');
        card.className = 'command-card';
        card.innerHTML = `
            <div class="command-text">
                <code>${escapeHtml(suggestion.command)}</code>
            </div>
            <div class="command-actions">
                <button class="cmd-btn cmd-run" title="Execute in SSH terminal" onclick="runCommand('${suggestion.id}')">
                    <span class="btn-icon">▶</span> Run
                </button>
                <button class="cmd-btn cmd-edit" title="Paste to SSH terminal for editing" onclick="editCommand('${suggestion.id}')">
                    <span class="btn-icon">✎</span> Edit
                </button>
                <button class="cmd-btn cmd-copy" title="Copy to clipboard" onclick="copyCommand('${suggestion.id}')">
                    <span class="btn-icon">⧉</span> Copy
                </button>
                <button class="cmd-btn cmd-dismiss" title="Dismiss" onclick="dismissCommand('${suggestion.id}')">
                    ✕
                </button>
            </div>
        `;
        container.appendChild(card);
    });
}

/**
 * Escape HTML to prevent XSS
 */
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

/**
 * Run a suggested command in the SSH terminal
 */
function runCommand(id) {
    const suggestion = suggestedCommands.find(s => s.id === id);
    if (!suggestion) return;

    if (!sshConnected || !sshTerminalWs || sshTerminalWs.readyState !== WebSocket.OPEN) {
        showSSHError('SSH not connected. Connect to SSH first to run commands.');
        return;
    }

    // Send command to SSH terminal with newline to execute
    sshTerminalWs.send(JSON.stringify({
        type: 'input',
        data: suggestion.command + '\n',
    }));

    // Show feedback in Gemini terminal
    geminiTerminal.write(`\r\n\x1b[32m✓ Executed: ${suggestion.command}\x1b[0m\r\n`);

    // Focus SSH terminal to see output
    sshTerminal.focus();

    // Remove from suggestions after running
    dismissCommand(id);
}

/**
 * Paste command to SSH terminal for editing (without executing)
 */
function editCommand(id) {
    const suggestion = suggestedCommands.find(s => s.id === id);
    if (!suggestion) return;

    if (!sshConnected || !sshTerminalWs || sshTerminalWs.readyState !== WebSocket.OPEN) {
        showSSHError('SSH not connected. Connect to SSH first.');
        return;
    }

    // Send command to SSH terminal WITHOUT newline (user can edit)
    sshTerminalWs.send(JSON.stringify({
        type: 'input',
        data: suggestion.command,
    }));

    // Show feedback
    geminiTerminal.write(`\r\n\x1b[33m✎ Pasted for editing: ${suggestion.command}\x1b[0m\r\n`);

    // Focus SSH terminal for editing
    sshTerminal.focus();
}

/**
 * Copy command to clipboard
 */
function copyCommand(id) {
    const suggestion = suggestedCommands.find(s => s.id === id);
    if (!suggestion) return;

    navigator.clipboard.writeText(suggestion.command).then(() => {
        geminiTerminal.write(`\r\n\x1b[36m⧉ Copied to clipboard: ${suggestion.command}\x1b[0m\r\n`);
    }).catch(err => {
        console.error('Failed to copy:', err);
    });
}

/**
 * Dismiss a suggested command
 */
function dismissCommand(id) {
    suggestedCommands = suggestedCommands.filter(s => s.id !== id);
    updateSuggestionsPanel();
}

/**
 * Clear all suggested commands
 */
function clearAllSuggestions() {
    suggestedCommands = [];
    updateSuggestionsPanel();
}

/**
 * Process Gemini output to detect commands
 */
function processGeminiOutput(output) {
    // Add to buffer
    geminiOutputBuffer += output;

    // Look for complete code blocks
    const commands = detectCodeBlocks(geminiOutputBuffer);

    // Add any new commands found
    commands.forEach(cmd => {
        addSuggestedCommand(cmd);
    });

    // Keep buffer manageable (last 10000 chars)
    if (geminiOutputBuffer.length > 10000) {
        geminiOutputBuffer = geminiOutputBuffer.slice(-5000);
    }
}

/**
 * Send system prompt to Gemini when ready
 */
function sendSystemPrompt() {
    if (systemPromptSent) return;

    // Wait a bit for Gemini CLI to be ready
    setTimeout(() => {
        if (geminiTerminalWs && geminiTerminalWs.readyState === WebSocket.OPEN) {
            // Send the system prompt
            geminiTerminalWs.send(JSON.stringify({
                type: 'input',
                data: SYSTEM_PROMPT + '\n',
            }));
            systemPromptSent = true;
            console.log('System prompt sent to Gemini');
        }
    }, 2000); // Wait 2 seconds for CLI to initialize
}

// Initialize the main application - show auth forms first
async function initializeApp() {
    try {
        // Setup terminals (but don't connect yet)
        setupTerminals();

        // Setup Gemini auth form handler
        setupGeminiAuthForm();

        // Setup SSH form handler
        setupSSHForm();

        // Setup resizer
        setupResizer();

        // Setup SSH context toggle
        setupSshContextToggle();
    } catch (error) {
        console.error('Failed to initialize app:', error);
        alert('Failed to initialize application: ' + error.message);
    }
}

/**
 * Setup the SSH context toggle and status indicator
 */
function setupSshContextToggle() {
    const toggle = document.getElementById('ssh-context-toggle');
    const statusElement = document.getElementById('context-status');

    if (toggle) {
        toggle.addEventListener('change', (e) => {
            sshContextEnabled = e.target.checked;
            console.log('SSH context auto-injection:', sshContextEnabled ? 'enabled' : 'disabled');
            updateContextStatus();
        });
    }

    // Update status periodically
    setInterval(updateContextStatus, 2000);
    updateContextStatus();
}

/**
 * Update the SSH context status indicator
 */
function updateContextStatus() {
    const statusElement = document.getElementById('context-status');
    if (!statusElement) return;

    if (!sshConnected) {
        statusElement.textContent = '';
        statusElement.className = 'context-status';
    } else if (sshOutputBuffer.length > 0) {
        const lineCount = sshOutputBuffer.reduce((sum, e) => {
            return sum + (e.plainText.match(/\n/g) || []).length + 1;
        }, 0);
        statusElement.textContent = `${lineCount} lines`;
        statusElement.className = 'context-status available';
    } else {
        statusElement.textContent = 'empty';
        statusElement.className = 'context-status empty';
    }
}

// Setup Gemini authentication form
function setupGeminiAuthForm() {
    const form = document.getElementById('gemini-form');

    form.addEventListener('submit', async (e) => {
        e.preventDefault();

        const apiKey = document.getElementById('api-key').value.trim();

        try {
            // Create a session with optional API key
            const response = await fetch('/api/session/create', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    api_key: apiKey || null,
                }),
            });

            const result = await response.json();
            if (!result.success) {
                showGeminiError('Failed to create session');
                return;
            }

            sessionId = result.session_id;
            console.log('Session created:', sessionId);

            // Hide the auth form
            document.getElementById('gemini-auth-form').style.display = 'none';

            // Update status
            updateGeminiStatus('connecting');

            // Connect Gemini WebSocket
            connectGeminiWebSocket();

        } catch (error) {
            showGeminiError('Failed to connect: ' + error.message);
        }
    });
}

// Show Gemini error
function showGeminiError(message) {
    const errorDiv = document.getElementById('gemini-error');
    errorDiv.textContent = message;
    errorDiv.style.display = 'block';

    setTimeout(() => {
        errorDiv.style.display = 'none';
    }, 5000);
}

// Update Gemini status badge
function updateGeminiStatus(status) {
    const statusBadge = document.getElementById('gemini-status');
    if (status === 'connected') {
        statusBadge.textContent = 'Connected';
        statusBadge.className = 'status-badge connected';
        geminiConnected = true;
    } else if (status === 'connecting') {
        statusBadge.textContent = 'Connecting...';
        statusBadge.className = 'status-badge connecting';
    } else {
        statusBadge.textContent = 'Not Connected';
        statusBadge.className = 'status-badge disconnected';
        geminiConnected = false;
    }
}

// Setup both xterm.js terminals
function setupTerminals() {
    try {
        console.log('Setting up terminals...');

        // Verify xterm.js is loaded
        if (typeof Terminal === 'undefined') {
            console.error('Terminal library not loaded!');
            return;
        }

        if (typeof FitAddon === 'undefined') {
            console.error('FitAddon library not loaded!');
            return;
        }

        // Setup Gemini terminal
        console.log('Creating Gemini terminal...');
        geminiTerminal = new Terminal({
            cursorBlink: true,
            fontSize: 14,
            fontFamily: 'Menlo, Monaco, "Courier New", monospace',
            theme: {
                background: '#1a1a2e',
                foreground: '#e8e8e8',
                cursor: '#4ade80',
                selection: '#ffffff33',
            },
        });

        geminiFitAddon = new FitAddon.FitAddon();
        geminiTerminal.loadAddon(geminiFitAddon);

        const geminiContainer = document.getElementById('gemini-terminal');
        if (!geminiContainer) {
            console.error('Gemini terminal container not found!');
            return;
        }

        geminiTerminal.open(geminiContainer);

        // Write initial message
        geminiTerminal.write('\x1b[36m┌─────────────────────────────────────┐\x1b[0m\r\n');
        geminiTerminal.write('\x1b[36m│  Gemini CLI Terminal                │\x1b[0m\r\n');
        geminiTerminal.write('\x1b[36m│  Initializing...                    │\x1b[0m\r\n');
        geminiTerminal.write('\x1b[36m└─────────────────────────────────────┘\x1b[0m\r\n\r\n');

        // Focus the terminal to make it interactive
        geminiTerminal.focus();

        // Fit after a short delay to ensure DOM is ready
        setTimeout(() => {
            geminiFitAddon.fit();
            console.log('Gemini terminal fitted:', geminiTerminal.cols, 'x', geminiTerminal.rows);
            geminiTerminal.focus();
        }, 100);

        // Handle Gemini terminal input with SSH context awareness
        geminiTerminal.onData((data) => {
            if (geminiTerminalWs && geminiTerminalWs.readyState === WebSocket.OPEN) {
                // Track input buffer for context detection
                if (data === '\r' || data === '\n') {
                    // User pressed Enter - check if we should inject SSH context
                    const inputToCheck = currentInputBuffer.trim();

                    if (inputToCheck && shouldIncludeSshContext(inputToCheck)) {
                        const context = getSshContext();
                        if (context) {
                            // Inject SSH context before the user's prompt
                            const contextPrefix = formatSshContextForPrompt(context);

                            // Show context injection indicator in Gemini terminal
                            geminiTerminal.write('\r\n\x1b[36m[SSH context included automatically]\x1b[0m\r\n');

                            // Send context first, then the original prompt
                            geminiTerminalWs.send(JSON.stringify({
                                type: 'input',
                                data: contextPrefix + currentInputBuffer + '\r',
                            }));

                            // Clear input buffer after sending
                            currentInputBuffer = '';
                            return; // Don't send the Enter separately
                        }
                    }

                    // Clear input buffer on Enter (context wasn't needed)
                    currentInputBuffer = '';

                    // Send the Enter normally
                    geminiTerminalWs.send(JSON.stringify({
                        type: 'input',
                        data: data,
                    }));
                } else if (data === '\x7f' || data === '\b') {
                    // Backspace - remove last character from buffer
                    currentInputBuffer = currentInputBuffer.slice(0, -1);
                    geminiTerminalWs.send(JSON.stringify({
                        type: 'input',
                        data: data,
                    }));
                } else if (data === '\x03') {
                    // Ctrl+C - clear buffer
                    currentInputBuffer = '';
                    geminiTerminalWs.send(JSON.stringify({
                        type: 'input',
                        data: data,
                    }));
                } else {
                    // Regular character - add to buffer
                    currentInputBuffer += data;
                    geminiTerminalWs.send(JSON.stringify({
                        type: 'input',
                        data: data,
                    }));
                }
            }
        });

        // Focus Gemini terminal when clicked
        geminiContainer.addEventListener('click', () => {
            geminiTerminal.focus();
        });

        // Setup SSH terminal (initially hidden behind form)
        console.log('Creating SSH terminal...');
        sshTerminal = new Terminal({
            cursorBlink: true,
            fontSize: 14,
            fontFamily: 'Menlo, Monaco, "Courier New", monospace',
            theme: {
                background: '#0f0f0f',
                foreground: '#e8e8e8',
                cursor: '#4ade80',
                selection: '#ffffff33',
            },
        });

        sshFitAddon = new FitAddon.FitAddon();
        sshTerminal.loadAddon(sshFitAddon);

        const sshContainer = document.getElementById('ssh-terminal');
        if (!sshContainer) {
            console.error('SSH terminal container not found!');
            return;
        }

        sshTerminal.open(sshContainer);

        // Fit after a short delay to ensure DOM is ready
        setTimeout(() => {
            sshFitAddon.fit();
            console.log('SSH terminal fitted:', sshTerminal.cols, 'x', sshTerminal.rows);
        }, 100);

        // Handle SSH terminal input
        sshTerminal.onData((data) => {
            if (sshTerminalWs && sshTerminalWs.readyState === WebSocket.OPEN) {
                sshTerminalWs.send(JSON.stringify({
                    type: 'input',
                    data: data,
                }));
            }
        });

        // Focus SSH terminal when clicked
        sshContainer.addEventListener('click', () => {
            if (sshConnected) {
                sshTerminal.focus();
            }
        });

        // Handle window resize for both terminals
        window.addEventListener('resize', () => {
            console.log('Window resized, refitting terminals...');
            if (geminiFitAddon) {
                geminiFitAddon.fit();
            }
            if (sshFitAddon) {
                sshFitAddon.fit();
            }

            if (geminiTerminalWs && geminiTerminalWs.readyState === WebSocket.OPEN) {
                geminiTerminalWs.send(JSON.stringify({
                    type: 'resize',
                    width: geminiTerminal.cols,
                    height: geminiTerminal.rows,
                }));
            }

            if (sshTerminalWs && sshTerminalWs.readyState === WebSocket.OPEN) {
                sshTerminalWs.send(JSON.stringify({
                    type: 'resize',
                    width: sshTerminal.cols,
                    height: sshTerminal.rows,
                }));
            }
        });

        console.log('Terminals setup complete');
    } catch (error) {
        console.error('Error setting up terminals:', error);
        alert('Failed to initialize terminals: ' + error.message);
    }
}

// Connect Gemini WebSocket
function connectGeminiWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;

    geminiTerminalWs = new WebSocket(`${protocol}//${host}/ws/gemini-terminal/${sessionId}`);

    geminiTerminalWs.onopen = () => {
        console.log('Gemini terminal WebSocket connected');
        updateGeminiStatus('connected');
        geminiTerminal.write('\x1b[32m✓ Connected to Gemini CLI\x1b[0m\r\n\r\n');

        // Send initial terminal size
        if (geminiTerminal.cols && geminiTerminal.rows) {
            geminiTerminalWs.send(JSON.stringify({
                type: 'resize',
                width: geminiTerminal.cols,
                height: geminiTerminal.rows,
            }));
            console.log('Sent initial Gemini terminal size:', geminiTerminal.cols, 'x', geminiTerminal.rows);
        }

        // Send system prompt to establish context
        sendSystemPrompt();
    };

    geminiTerminalWs.onmessage = (event) => {
        const message = JSON.parse(event.data);
        if (message.type === 'output') {
            geminiTerminal.write(message.data);
            // Process output for command detection
            processGeminiOutput(message.data);
        } else if (message.type === 'error') {
            geminiTerminal.write(`\x1b[31mError: ${message.message}\x1b[0m\r\n`);
        }
    };

    geminiTerminalWs.onerror = (error) => {
        console.error('Gemini terminal WebSocket error:', error);
        geminiTerminal.write('\x1b[31m✗ Connection error\x1b[0m\r\n');
    };

    geminiTerminalWs.onclose = () => {
        console.log('Gemini terminal WebSocket closed');
        updateGeminiStatus('disconnected');
        geminiTerminal.write('\x1b[33m✗ Connection closed\x1b[0m\r\n');
        // Show auth form again for reconnection
        document.getElementById('gemini-auth-form').style.display = 'flex';
        // Reset state for reconnection
        systemPromptSent = false;
        geminiOutputBuffer = '';
    };

    // Connect command approval WebSocket
    commandApprovalWs = new WebSocket(`${protocol}//${host}/ws/commands/${sessionId}`);

    commandApprovalWs.onopen = () => {
        console.log('Command approval WebSocket connected');
    };

    commandApprovalWs.onmessage = (event) => {
        const message = JSON.parse(event.data);
        if (message.type === 'command_requested') {
            // New MCP-based approval request
            showCommandApproval(message.command, message.approval_id);
        } else if (message.type === 'command_approved') {
            // Command was approved (confirmation from another client or timeout)
            console.log('Command approved:', message.approval_id);
            hideCommandApprovalModal();
        } else if (message.type === 'command_rejected') {
            // Command was rejected
            console.log('Command rejected:', message.approval_id);
            hideCommandApprovalModal();
        }
    };

    commandApprovalWs.onerror = (error) => {
        console.error('Command approval WebSocket error:', error);
    };

    commandApprovalWs.onclose = () => {
        console.log('Command approval WebSocket closed');
    };
}

// Setup SSH connection form
function setupSSHForm() {
    const form = document.getElementById('ssh-form');
    const disconnectBtn = document.getElementById('disconnect-ssh-btn');

    form.addEventListener('submit', async (e) => {
        e.preventDefault();

        const formData = {
            host: document.getElementById('host').value,
            port: parseInt(document.getElementById('port').value),
            username: document.getElementById('username').value,
            password: document.getElementById('password').value || null,
            private_key: document.getElementById('private-key').value || null,
        };

        try {
            // Connect SSH
            const sshResponse = await fetch('/api/ssh/connect', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(formData),
            });

            const sshResult = await sshResponse.json();

            if (sshResult.success) {
                // SSH connected successfully
                sessionId = sshResult.session_id;
                sshConnected = true;

                // Hide the form
                document.getElementById('ssh-connection-form').style.display = 'none';

                // Update status
                updateSSHStatus('connected');

                // Connect SSH WebSocket
                connectSSHWebSocket();

                // Show disconnect button
                disconnectBtn.style.display = 'block';

                // Focus SSH terminal
                sshTerminal.focus();
            } else {
                showSSHError(sshResult.error || 'SSH connection failed');
            }
        } catch (error) {
            showSSHError('Failed to connect: ' + error.message);
        }
    });

    disconnectBtn.addEventListener('click', () => {
        disconnectSSH();
    });
}

// Connect SSH WebSocket
function connectSSHWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;

    sshTerminalWs = new WebSocket(`${protocol}//${host}/ws/ssh-terminal/${sessionId}`);

    sshTerminalWs.onopen = () => {
        console.log('SSH terminal WebSocket connected');
        sshTerminal.write('\x1b[32m✓ SSH connection established\x1b[0m\r\n\r\n');

        // Send initial terminal size
        if (sshTerminal.cols && sshTerminal.rows) {
            sshTerminalWs.send(JSON.stringify({
                type: 'resize',
                width: sshTerminal.cols,
                height: sshTerminal.rows,
            }));
            console.log('Sent initial SSH terminal size:', sshTerminal.cols, 'x', sshTerminal.rows);
        }
    };

    sshTerminalWs.onmessage = (event) => {
        const message = JSON.parse(event.data);
        if (message.type === 'output') {
            sshTerminal.write(message.data);
            // Add to SSH context buffer for Gemini awareness
            addSshOutputToBuffer(message.data);
        } else if (message.type === 'error') {
            sshTerminal.write(`\x1b[31mError: ${message.message}\x1b[0m\r\n`);
            // Also capture errors for context
            addSshOutputToBuffer(`Error: ${message.message}`);
        }
    };

    sshTerminalWs.onerror = (error) => {
        console.error('SSH terminal WebSocket error:', error);
        sshTerminal.write('\x1b[31m✗ Connection error\x1b[0m\r\n');
    };

    sshTerminalWs.onclose = () => {
        console.log('SSH terminal WebSocket closed');
        sshTerminal.write('\x1b[33m✗ SSH connection closed\x1b[0m\r\n\r\n');
        sshConnected = false;
        updateSSHStatus('disconnected');
        // Show the form again
        document.getElementById('ssh-connection-form').style.display = 'flex';
        document.getElementById('disconnect-ssh-btn').style.display = 'none';
    };
}

// Disconnect SSH
function disconnectSSH() {
    if (sshTerminalWs) {
        sshTerminalWs.close();
    }
    sshConnected = false;
    sshTerminal.clear();
    updateSSHStatus('disconnected');
    document.getElementById('ssh-connection-form').style.display = 'flex';
    document.getElementById('disconnect-ssh-btn').style.display = 'none';
}

// Update SSH status badge
function updateSSHStatus(status) {
    const statusBadge = document.getElementById('ssh-status');
    if (status === 'connected') {
        statusBadge.textContent = 'SSH: Connected';
        statusBadge.className = 'status-badge connected';
    } else {
        statusBadge.textContent = 'SSH: Disconnected';
        statusBadge.className = 'status-badge disconnected';
    }
}

// Show SSH error
function showSSHError(message) {
    const errorDiv = document.getElementById('ssh-error');
    errorDiv.textContent = message;
    errorDiv.style.display = 'block';

    // Hide error after 5 seconds
    setTimeout(() => {
        errorDiv.style.display = 'none';
    }, 5000);
}

// Command approval system
function showCommandApproval(command, approvalId) {
    pendingCommand = { command, approvalId };

    document.getElementById('command-preview').textContent = command;
    document.getElementById('command-approval-modal').style.display = 'flex';

    document.getElementById('approve-btn').onclick = () => approveCommand(true);
    document.getElementById('reject-btn').onclick = () => approveCommand(false);
}

function hideCommandApprovalModal() {
    document.getElementById('command-approval-modal').style.display = 'none';
    pendingCommand = null;
}

function approveCommand(approved) {
    if (pendingCommand && commandApprovalWs && commandApprovalWs.readyState === WebSocket.OPEN) {
        // Send decision using new MCP-based message format
        commandApprovalWs.send(JSON.stringify({
            type: 'command_decision',
            approval_id: pendingCommand.approvalId,
            approved: approved,
        }));

        if (approved) {
            geminiTerminal.write(`\x1b[32m✓ Approved command: ${pendingCommand.command}\x1b[0m\r\n`);
        } else {
            geminiTerminal.write(`\x1b[33m✗ Rejected command: ${pendingCommand.command}\x1b[0m\r\n`);
        }
    }

    hideCommandApprovalModal();
}

// Setup pane resizer
function setupResizer() {
    const resizer = document.querySelector('.resizer');
    const leftPane = document.querySelector('.gemini-pane');
    const rightPane = document.querySelector('.terminal-pane');

    let isResizing = false;

    resizer.addEventListener('mousedown', (e) => {
        isResizing = true;
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
    });

    document.addEventListener('mousemove', (e) => {
        if (!isResizing) return;

        const containerWidth = document.querySelector('.split-container').offsetWidth;
        const leftWidth = (e.clientX / containerWidth) * 100;

        if (leftWidth > 20 && leftWidth < 80) {
            leftPane.style.flex = `0 0 ${leftWidth}%`;
            rightPane.style.flex = `0 0 ${100 - leftWidth}%`;

            // Resize both terminals
            if (geminiFitAddon) {
                setTimeout(() => geminiFitAddon.fit(), 0);
            }
            if (sshFitAddon) {
                setTimeout(() => sshFitAddon.fit(), 0);
            }
        }
    });

    document.addEventListener('mouseup', () => {
        if (isResizing) {
            isResizing = false;
            document.body.style.cursor = '';
            document.body.style.userSelect = '';
        }
    });
}
