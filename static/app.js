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

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    initializeApp();
});

// Initialize the main application immediately
async function initializeApp() {
    try {
        // Create a session
        const response = await fetch('/api/session/create', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({}),
        });

        const result = await response.json();
        if (!result.success) {
            console.error('Failed to create session');
            alert('Failed to create session. Please refresh the page.');
            return;
        }

        sessionId = result.session_id;
        console.log('Session created:', sessionId);

        // Setup terminals immediately
        setupTerminals();

        // Connect Gemini WebSocket
        connectGeminiWebSocket();

        // Setup SSH form handler
        setupSSHForm();

        // Setup resizer
        setupResizer();
    } catch (error) {
        console.error('Failed to initialize app:', error);
        alert('Failed to initialize application: ' + error.message);
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

        // Handle Gemini terminal input
        geminiTerminal.onData((data) => {
            if (geminiTerminalWs && geminiTerminalWs.readyState === WebSocket.OPEN) {
                geminiTerminalWs.send(JSON.stringify({
                    type: 'input',
                    data: data,
                }));
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
    };

    geminiTerminalWs.onmessage = (event) => {
        const message = JSON.parse(event.data);
        if (message.type === 'output') {
            geminiTerminal.write(message.data);
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
        geminiTerminal.write('\x1b[33m✗ Connection closed\x1b[0m\r\n');
    };

    // Connect command approval WebSocket
    commandApprovalWs = new WebSocket(`${protocol}//${host}/ws/commands/${sessionId}`);

    commandApprovalWs.onopen = () => {
        console.log('Command approval WebSocket connected');
    };

    commandApprovalWs.onmessage = (event) => {
        const message = JSON.parse(event.data);
        if (message.type === 'command_request') {
            showCommandApproval(message.command, message.command_id);
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
        } else if (message.type === 'error') {
            sshTerminal.write(`\x1b[31mError: ${message.message}\x1b[0m\r\n`);
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
function showCommandApproval(command, commandId) {
    pendingCommand = { command, commandId };

    document.getElementById('command-preview').textContent = command;
    document.getElementById('command-approval-modal').style.display = 'flex';

    document.getElementById('approve-btn').onclick = () => approveCommand(true);
    document.getElementById('reject-btn').onclick = () => approveCommand(false);
}

function approveCommand(approved) {
    if (pendingCommand && commandApprovalWs && commandApprovalWs.readyState === WebSocket.OPEN) {
        commandApprovalWs.send(JSON.stringify({
            type: 'command_approval',
            command_id: pendingCommand.commandId,
            approved: approved,
        }));

        if (approved) {
            geminiTerminal.write(`\x1b[32m✓ Approved command: ${pendingCommand.command}\x1b[0m\r\n`);
        } else {
            geminiTerminal.write(`\x1b[33m✗ Rejected command: ${pendingCommand.command}\x1b[0m\r\n`);
        }
    }

    document.getElementById('command-approval-modal').style.display = 'none';
    pendingCommand = null;
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
