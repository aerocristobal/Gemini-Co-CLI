//! Event-driven approval channel for SSH command execution.
//!
//! This module replaces the polling-based command approval with
//! an event-driven system using broadcast and oneshot channels.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot, Mutex};
use uuid::Uuid;

/// Event sent to frontends when approval status changes.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ApprovalEvent {
    /// A command is awaiting approval.
    CommandRequested {
        approval_id: String,
        command: String,
    },
    /// A command was approved.
    CommandApproved { approval_id: String },
    /// A command was rejected.
    CommandRejected { approval_id: String },
}

/// Pending approval entry.
struct PendingApproval {
    response_tx: oneshot::Sender<bool>,
    command: String,
}

/// Channel for managing command approval flow.
///
/// This provides event-driven approval instead of polling:
/// - MCP tools call `request_approval()` which blocks until user decides
/// - WebSocket handlers subscribe to events via `subscribe()`
/// - Frontend sends decisions via `submit_decision()`
pub struct ApprovalChannel {
    /// Pending approvals awaiting user response.
    pending: Arc<Mutex<HashMap<Uuid, PendingApproval>>>,

    /// Broadcast channel for approval events to all connected frontends.
    event_tx: broadcast::Sender<ApprovalEvent>,
}

impl ApprovalChannel {
    /// Create a new approval channel.
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(64);
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
        }
    }

    /// Subscribe to approval events.
    ///
    /// Called by WebSocket handlers to receive events for the frontend.
    pub fn subscribe(&self) -> broadcast::Receiver<ApprovalEvent> {
        self.event_tx.subscribe()
    }

    /// Request approval for a command.
    ///
    /// This broadcasts the request to all connected frontends and returns
    /// a future that resolves when the user approves or rejects.
    ///
    /// Returns the approval ID and a receiver for the decision.
    pub async fn request_approval(&self, command: String) -> (Uuid, oneshot::Receiver<bool>) {
        let id = Uuid::new_v4();
        let (tx, rx) = oneshot::channel();

        // Store the pending approval
        {
            let mut pending = self.pending.lock().await;
            pending.insert(
                id,
                PendingApproval {
                    response_tx: tx,
                    command: command.clone(),
                },
            );
        }

        // Broadcast the request to all frontends
        let _ = self.event_tx.send(ApprovalEvent::CommandRequested {
            approval_id: id.to_string(),
            command,
        });

        (id, rx)
    }

    /// Submit an approval decision.
    ///
    /// Called by the WebSocket handler when the user approves or rejects.
    /// Returns true if the decision was delivered, false if approval ID not found.
    pub async fn submit_decision(&self, approval_id: Uuid, approved: bool) -> bool {
        let pending_approval = {
            let mut pending = self.pending.lock().await;
            pending.remove(&approval_id)
        };

        if let Some(approval) = pending_approval {
            // Broadcast the decision to all frontends
            let event = if approved {
                ApprovalEvent::CommandApproved {
                    approval_id: approval_id.to_string(),
                }
            } else {
                ApprovalEvent::CommandRejected {
                    approval_id: approval_id.to_string(),
                }
            };
            let _ = self.event_tx.send(event);

            // Send to the waiting MCP tool
            approval.response_tx.send(approved).is_ok()
        } else {
            false
        }
    }

    /// Wait for approval with timeout.
    ///
    /// Convenience method that handles the common case of waiting for approval.
    pub async fn wait_for_approval(
        &self,
        command: String,
        timeout: Duration,
    ) -> Result<bool, ApprovalError> {
        let (id, rx) = self.request_approval(command).await;

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(approved)) => Ok(approved),
            Ok(Err(_)) => {
                // Channel was dropped (shouldn't happen normally)
                self.cleanup_pending(id).await;
                Err(ApprovalError::ChannelClosed)
            }
            Err(_) => {
                // Timeout
                self.cleanup_pending(id).await;
                Err(ApprovalError::Timeout)
            }
        }
    }

    /// Clean up a pending approval (e.g., on timeout).
    async fn cleanup_pending(&self, id: Uuid) {
        let mut pending = self.pending.lock().await;
        pending.remove(&id);
    }

    /// Get the number of pending approvals.
    pub async fn pending_count(&self) -> usize {
        self.pending.lock().await.len()
    }
}

impl Default for ApprovalChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during approval.
#[derive(Debug, Clone)]
pub enum ApprovalError {
    /// The approval request timed out.
    Timeout,
    /// The approval channel was closed unexpectedly.
    ChannelClosed,
}

impl std::fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalError::Timeout => write!(f, "Approval request timed out"),
            ApprovalError::ChannelClosed => write!(f, "Approval channel closed unexpectedly"),
        }
    }
}

impl std::error::Error for ApprovalError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_approval_flow() {
        let channel = ApprovalChannel::new();
        let mut subscriber = channel.subscribe();

        // Request approval
        let (id, rx) = channel.request_approval("ls -la".to_string()).await;

        // Check event was broadcast
        let event = subscriber.try_recv().unwrap();
        match event {
            ApprovalEvent::CommandRequested {
                approval_id,
                command,
            } => {
                assert_eq!(approval_id, id.to_string());
                assert_eq!(command, "ls -la");
            }
            _ => panic!("Expected CommandRequested event"),
        }

        // Submit approval
        let delivered = channel.submit_decision(id, true).await;
        assert!(delivered);

        // Check the receiver got the decision
        assert_eq!(rx.await.unwrap(), true);
    }

    #[tokio::test]
    async fn test_rejection_flow() {
        let channel = ApprovalChannel::new();

        let (id, rx) = channel.request_approval("rm -rf /".to_string()).await;

        channel.submit_decision(id, false).await;

        assert_eq!(rx.await.unwrap(), false);
    }
}
