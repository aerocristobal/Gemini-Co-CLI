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

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    setupSSHForm();
    setupResizer();
});

// Setup SSH connection form
function setupSSHForm() {
    const form = document.getElementById('ssh-form');
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
            // First, create a session
            const sessionResponse = await fetch('/api/session/create', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({}),
            });

            const sessionResult = await sessionResponse.json();
            if (!sessionResult.success) {
                showError('Failed to create session');
                return;
            }

            sessionId = sessionResult.session_id;

            // Then connect SSH
            const sshResponse = await fetch('/api/ssh/connect', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(formData),
            });

            const sshResult = await sshResponse.json();

            if (sshResult.success) {
                // Use the session ID from SSH connection if different
                sessionId = sshResult.session_id;
                hideConnectionModal();
                initializeApp();
            } else {
                showError(sshResult.error || 'SSH connection failed');
            }
        } catch (error) {
            showError('Failed to connect: ' + error.message);
        }
    });
}

function showError(message) {
    const errorDiv = document.getElementById('connection-error');
    errorDiv.textContent = message;
    errorDiv.style.display = 'block';
}

function hideConnectionModal() {
    document.getElementById('connection-modal').style.display = 'none';
    document.getElementById('app-container').style.display = 'block';
}

// Initialize the main application
function initializeApp() {
    setupTerminals();
    setupDisconnect();
    connectWebSockets();
}

// Setup both xterm.js terminals
function setupTerminals() {
    // Setup Gemini terminal
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
    geminiTerminal.open(document.getElementById('gemini-terminal'));
    geminiFitAddon.fit();

    // Handle Gemini terminal input
    geminiTerminal.onData((data) => {
        if (geminiTerminalWs && geminiTerminalWs.readyState === WebSocket.OPEN) {
            geminiTerminalWs.send(JSON.stringify({
                type: 'input',
                data: data,
            }));
        }
    });

    // Setup SSH terminal
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
    sshTerminal.open(document.getElementById('ssh-terminal'));
    sshFitAddon.fit();

    // Handle SSH terminal input
    sshTerminal.onData((data) => {
        if (sshTerminalWs && sshTerminalWs.readyState === WebSocket.OPEN) {
            sshTerminalWs.send(JSON.stringify({
                type: 'input',
                data: data,
            }));
        }
    });

    // Handle window resize for both terminals
    window.addEventListener('resize', () => {
        geminiFitAddon.fit();
        sshFitAddon.fit();

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
}

// Setup WebSocket connections
function connectWebSockets() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;

    // Connect Gemini terminal WebSocket
    geminiTerminalWs = new WebSocket(`${protocol}//${host}/ws/gemini-terminal/${sessionId}`);

    geminiTerminalWs.onopen = () => {
        console.log('Gemini terminal WebSocket connected');
        geminiTerminal.write('\x1b[32m✓ Gemini CLI connected\x1b[0m\r\n');
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

    // Connect SSH terminal WebSocket
    sshTerminalWs = new WebSocket(`${protocol}//${host}/ws/ssh-terminal/${sessionId}`);

    sshTerminalWs.onopen = () => {
        console.log('SSH terminal WebSocket connected');
        sshTerminal.write('\x1b[32m✓ SSH terminal connected\x1b[0m\r\n');
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
        sshTerminal.write('\x1b[33m✗ Connection closed\x1b[0m\r\n');
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

// Setup disconnect functionality
function setupDisconnect() {
    document.getElementById('disconnect-btn').addEventListener('click', () => {
        if (geminiTerminalWs) geminiTerminalWs.close();
        if (sshTerminalWs) sshTerminalWs.close();
        if (commandApprovalWs) commandApprovalWs.close();
        if (geminiTerminal) geminiTerminal.dispose();
        if (sshTerminal) sshTerminal.dispose();

        document.getElementById('app-container').style.display = 'none';
        document.getElementById('connection-modal').style.display = 'flex';

        // Reset form
        document.getElementById('ssh-form').reset();
        document.getElementById('connection-error').style.display = 'none';
    });
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
