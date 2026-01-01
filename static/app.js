// Global state
let sessionId = null;
let terminalWs = null;
let geminiWs = null;
let terminal = null;
let fitAddon = null;
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
            const response = await fetch('/api/ssh/connect', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(formData),
            });

            const result = await response.json();

            if (result.success) {
                sessionId = result.session_id;
                hideConnectionModal();
                initializeApp();
            } else {
                showError(result.error || 'Connection failed');
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
    setupTerminal();
    setupGeminiChat();
    setupDisconnect();
    connectWebSockets();
}

// Setup xterm.js terminal
function setupTerminal() {
    terminal = new Terminal({
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

    fitAddon = new FitAddon.FitAddon();
    terminal.loadAddon(fitAddon);

    terminal.open(document.getElementById('terminal'));
    fitAddon.fit();

    // Handle terminal input
    terminal.onData((data) => {
        if (terminalWs && terminalWs.readyState === WebSocket.OPEN) {
            terminalWs.send(JSON.stringify({
                type: 'input',
                data: data,
            }));
        }
    });

    // Handle terminal resize
    window.addEventListener('resize', () => {
        fitAddon.fit();
        if (terminalWs && terminalWs.readyState === WebSocket.OPEN) {
            terminalWs.send(JSON.stringify({
                type: 'resize',
                width: terminal.cols,
                height: terminal.rows,
            }));
        }
    });
}

// Setup Gemini chat interface
function setupGeminiChat() {
    const sendBtn = document.getElementById('send-btn');
    const input = document.getElementById('gemini-input');

    sendBtn.addEventListener('click', sendGeminiMessage);
    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            sendGeminiMessage();
        }
    });
}

function sendGeminiMessage() {
    const input = document.getElementById('gemini-input');
    const message = input.value.trim();

    if (!message) return;

    // Add user message to chat
    addChatMessage('user', message);
    input.value = '';

    // Send to Gemini via WebSocket
    if (geminiWs && geminiWs.readyState === WebSocket.OPEN) {
        geminiWs.send(JSON.stringify({
            type: 'user_message',
            content: message,
        }));
    }
}

function addChatMessage(role, content) {
    const chatContainer = document.getElementById('gemini-chat');
    const messageDiv = document.createElement('div');
    messageDiv.className = `message ${role}`;
    messageDiv.textContent = content;
    chatContainer.appendChild(messageDiv);
    chatContainer.scrollTop = chatContainer.scrollHeight;
}

// Setup WebSocket connections
function connectWebSockets() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;

    // Connect terminal WebSocket
    terminalWs = new WebSocket(`${protocol}//${host}/ws/terminal/${sessionId}`);

    terminalWs.onopen = () => {
        console.log('Terminal WebSocket connected');
    };

    terminalWs.onmessage = (event) => {
        const message = JSON.parse(event.data);
        if (message.type === 'output') {
            terminal.write(message.data);
        } else if (message.type === 'error') {
            terminal.write(`\r\n\x1b[31mError: ${message.message}\x1b[0m\r\n`);
        }
    };

    terminalWs.onerror = (error) => {
        console.error('Terminal WebSocket error:', error);
        addChatMessage('system', 'Terminal connection error');
    };

    terminalWs.onclose = () => {
        console.log('Terminal WebSocket closed');
        addChatMessage('system', 'Terminal connection closed');
    };

    // Connect Gemini WebSocket
    geminiWs = new WebSocket(`${protocol}//${host}/ws/gemini/${sessionId}`);

    geminiWs.onopen = () => {
        console.log('Gemini WebSocket connected');
        addChatMessage('system', '✨ Gemini is ready! Ask me anything about your terminal.');
    };

    geminiWs.onmessage = (event) => {
        const message = JSON.parse(event.data);
        handleGeminiMessage(message);
    };

    geminiWs.onerror = (error) => {
        console.error('Gemini WebSocket error:', error);
        addChatMessage('system', 'Gemini connection error');
    };

    geminiWs.onclose = () => {
        console.log('Gemini WebSocket closed');
        addChatMessage('system', 'Gemini connection closed');
    };
}

function handleGeminiMessage(message) {
    switch (message.type) {
        case 'gemini_response':
            addChatMessage('gemini', message.content);
            break;

        case 'command_request':
            showCommandApproval(message.command, message.command_id);
            break;

        case 'error':
            addChatMessage('system', `Error: ${message.message}`);
            break;

        case 'command_executed':
            addChatMessage('system', `✓ Command executed: ${message.command}`);
            break;
    }
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
    if (pendingCommand && geminiWs && geminiWs.readyState === WebSocket.OPEN) {
        geminiWs.send(JSON.stringify({
            type: 'command_approval',
            command_id: pendingCommand.commandId,
            approved: approved,
        }));

        if (approved) {
            addChatMessage('system', `✓ Approved command: ${pendingCommand.command}`);
        } else {
            addChatMessage('system', `✗ Rejected command: ${pendingCommand.command}`);
        }
    }

    document.getElementById('command-approval-modal').style.display = 'none';
    pendingCommand = null;
}

// Setup disconnect functionality
function setupDisconnect() {
    document.getElementById('disconnect-btn').addEventListener('click', () => {
        if (terminalWs) terminalWs.close();
        if (geminiWs) geminiWs.close();
        if (terminal) terminal.dispose();

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

            // Resize terminal
            if (fitAddon) {
                setTimeout(() => fitAddon.fit(), 0);
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
