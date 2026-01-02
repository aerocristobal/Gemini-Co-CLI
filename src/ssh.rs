use anyhow::{Context, Result};
use russh::client::{self, Handle};
use russh::keys::{decode_secret_key, PrivateKeyWithHashAlg};
use russh::*;
use std::io::Cursor;
use std::sync::Arc;

pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub private_key: Option<String>,
}

pub struct SshSession {
    #[allow(dead_code)]
    handle: Handle<Client>,
    channel: Channel<client::Msg>,
}

struct Client;

impl client::Handler for Client {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> impl std::future::Future<Output = Result<bool, Self::Error>> + Send {
        // In production, you should verify the server key properly
        async { Ok(true) }
    }
}

impl SshSession {
    pub async fn connect(config: SshConfig) -> Result<Self> {
        let client_config = client::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(300)),
            ..<_>::default()
        };

        let client_config = Arc::new(client_config);
        let mut session = client::connect(
            client_config,
            (config.host.as_str(), config.port),
            Client,
        )
        .await
        .context("Failed to connect to SSH server")?;

        // Authenticate
        let auth_result = if let Some(password) = config.password {
            session
                .authenticate_password(config.username.clone(), password)
                .await
                .context("Failed to authenticate with password")?
        } else if let Some(key_data) = config.private_key {
            let key = decode_secret_key(&key_data, None)
                .context("Failed to decode private key")?;
            let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key), None);
            session
                .authenticate_publickey(config.username.clone(), key_with_alg)
                .await
                .context("Failed to authenticate with public key")?
        } else {
            return Err(anyhow::anyhow!("No authentication method provided"));
        };

        if !auth_result.success() {
            return Err(anyhow::anyhow!("Authentication failed"));
        }

        // Open a channel with PTY
        let channel = session
            .channel_open_session()
            .await
            .context("Failed to open channel")?;

        channel
            .request_pty(
                false,
                "xterm",
                80,
                24,
                0,
                0,
                &[], //pty modes
            )
            .await
            .context("Failed to request PTY")?;

        channel
            .request_shell(false)
            .await
            .context("Failed to request shell")?;

        Ok(Self {
            handle: session,
            channel,
        })
    }

    /// Send raw input to the SSH terminal (for user keystrokes)
    pub async fn send_input(&mut self, data: String) -> Result<()> {
        self.channel
            .data(Cursor::new(data.into_bytes()))
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send input"))?;
        Ok(())
    }

    /// Execute a complete command (adds newline automatically)
    pub async fn execute_command(&mut self, command: String) -> Result<()> {
        let cmd_with_newline = format!("{}\n", command);
        self.channel
            .data(Cursor::new(cmd_with_newline.into_bytes()))
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send command"))?;
        Ok(())
    }

    pub async fn read_output(&mut self) -> Result<Option<String>> {
        // Check if there's any message from the channel
        if let Some(msg) = self.channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    let output = String::from_utf8_lossy(&data).to_string();
                    Ok(Some(output))
                }
                ChannelMsg::ExtendedData { data, ext: _ } => {
                    let output = String::from_utf8_lossy(&data).to_string();
                    Ok(Some(output))
                }
                ChannelMsg::Eof => Ok(None),
                ChannelMsg::ExitStatus { exit_status } => {
                    tracing::info!("Command exited with status: {}", exit_status);
                    Ok(None)
                }
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub async fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.channel
            .window_change(width, height, 0, 0)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to resize terminal"))?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn close(self) -> Result<()> {
        self.channel
            .eof()
            .await
            .map_err(|_| anyhow::anyhow!("Failed to close channel"))?;
        self.handle
            .disconnect(Disconnect::ByApplication, "", "English")
            .await
            .context("Failed to disconnect")?;
        Ok(())
    }
}
