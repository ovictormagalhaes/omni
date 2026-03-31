use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::models::{Chain, Protocol};

/// Per-protocol+chain circuit breaker.
///
/// After `failure_threshold` consecutive failures the circuit opens and
/// the indexer is skipped for `cooldown` seconds. When the cooldown expires
/// the circuit moves to half-open: one probe request is allowed through.
/// If it succeeds the circuit closes; if it fails the cooldown resets.
#[derive(Clone)]
pub struct CircuitBreaker {
    state: Arc<RwLock<HashMap<(Protocol, Chain), CircuitState>>>,
    failure_threshold: u32,
    cooldown: Duration,
}

#[derive(Debug, Clone)]
struct CircuitState {
    consecutive_failures: u32,
    last_failure: Option<Instant>,
    status: Status,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Status {
    Closed,   // normal — requests flow through
    Open,     // tripped — requests are blocked
    HalfOpen, // probe — one request allowed
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, cooldown_secs: u64) -> Self {
        Self {
            state: Arc::new(RwLock::new(HashMap::new())),
            failure_threshold,
            cooldown: Duration::from_secs(cooldown_secs),
        }
    }

    /// Returns `true` if the indexer should be skipped (circuit is open).
    pub async fn should_skip(&self, protocol: &Protocol, chain: &Chain) -> bool {
        let key = (protocol.clone(), chain.clone());
        let mut state = self.state.write().await;
        let entry = state.entry(key).or_insert_with(|| CircuitState {
            consecutive_failures: 0,
            last_failure: None,
            status: Status::Closed,
        });

        match entry.status {
            Status::Closed => false,
            Status::Open => {
                // Check if cooldown expired → transition to half-open
                if let Some(last) = entry.last_failure {
                    if last.elapsed() >= self.cooldown {
                        entry.status = Status::HalfOpen;
                        tracing::info!(
                            "⚡ Circuit half-open for {:?}/{:?} — allowing probe request",
                            protocol, chain
                        );
                        false // allow the probe
                    } else {
                        true // still cooling down
                    }
                } else {
                    true
                }
            }
            Status::HalfOpen => false, // allow the probe
        }
    }

    /// Record a successful call — closes the circuit.
    pub async fn record_success(&self, protocol: &Protocol, chain: &Chain) {
        let key = (protocol.clone(), chain.clone());
        let mut state = self.state.write().await;
        if let Some(entry) = state.get_mut(&key) {
            if entry.status != Status::Closed {
                tracing::info!(
                    "✅ Circuit closed for {:?}/{:?} after successful probe",
                    protocol, chain
                );
            }
            entry.consecutive_failures = 0;
            entry.last_failure = None;
            entry.status = Status::Closed;
        }
    }

    /// Record a failed call — may trip the circuit.
    pub async fn record_failure(&self, protocol: &Protocol, chain: &Chain) {
        let key = (protocol.clone(), chain.clone());
        let mut state = self.state.write().await;
        let entry = state.entry(key).or_insert_with(|| CircuitState {
            consecutive_failures: 0,
            last_failure: None,
            status: Status::Closed,
        });

        entry.consecutive_failures += 1;
        entry.last_failure = Some(Instant::now());

        if entry.consecutive_failures >= self.failure_threshold {
            if entry.status != Status::Open {
                tracing::warn!(
                    "🔴 Circuit OPEN for {:?}/{:?} after {} consecutive failures — skipping for {}s",
                    protocol, chain, entry.consecutive_failures, self.cooldown.as_secs()
                );
            }
            entry.status = Status::Open;
        }
    }

    /// Snapshot of all circuit states for telemetry/dashboard.
    pub async fn snapshot(&self) -> Vec<CircuitSnapshot> {
        let state = self.state.read().await;
        state.iter().map(|((protocol, chain), s)| {
            CircuitSnapshot {
                protocol: protocol.clone(),
                chain: chain.clone(),
                consecutive_failures: s.consecutive_failures,
                status: match s.status {
                    Status::Closed => "closed".to_string(),
                    Status::Open => "open".to_string(),
                    Status::HalfOpen => "half-open".to_string(),
                },
            }
        }).collect()
    }
}

/// Serializable snapshot for monitoring.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CircuitSnapshot {
    pub protocol: Protocol,
    pub chain: Chain,
    pub consecutive_failures: u32,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_circuit_is_closed() {
        let cb = CircuitBreaker::new(3, 60);
        // A brand new protocol+chain should not be skipped
        assert!(!cb.should_skip(&Protocol::Aave, &Chain::Ethereum).await);
    }

    #[tokio::test]
    async fn test_failures_below_threshold_stay_closed() {
        let cb = CircuitBreaker::new(3, 60);
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        // 2 failures < threshold 3, still closed
        assert!(!cb.should_skip(&Protocol::Aave, &Chain::Ethereum).await);
    }

    #[tokio::test]
    async fn test_failures_at_threshold_opens_circuit() {
        let cb = CircuitBreaker::new(3, 60);
        for _ in 0..3 {
            cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        }
        // 3 failures = threshold, circuit should be open
        assert!(cb.should_skip(&Protocol::Aave, &Chain::Ethereum).await);
    }

    #[tokio::test]
    async fn test_success_resets_failures() {
        let cb = CircuitBreaker::new(3, 60);
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        cb.record_success(&Protocol::Aave, &Chain::Ethereum).await;
        // After success, counter resets. 1 more failure should not trip.
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        assert!(!cb.should_skip(&Protocol::Aave, &Chain::Ethereum).await);
    }

    #[tokio::test]
    async fn test_different_protocols_independent() {
        let cb = CircuitBreaker::new(2, 60);
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        // Aave is open, but Morpho should still be closed
        assert!(cb.should_skip(&Protocol::Aave, &Chain::Ethereum).await);
        assert!(!cb.should_skip(&Protocol::Morpho, &Chain::Ethereum).await);
    }

    #[tokio::test]
    async fn test_same_protocol_different_chains_independent() {
        let cb = CircuitBreaker::new(2, 60);
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        // Aave/Ethereum is open, Aave/Arbitrum should be closed
        assert!(cb.should_skip(&Protocol::Aave, &Chain::Ethereum).await);
        assert!(!cb.should_skip(&Protocol::Aave, &Chain::Arbitrum).await);
    }

    #[tokio::test]
    async fn test_half_open_after_cooldown() {
        // Use very short cooldown (0 seconds) so we can test transition
        let cb = CircuitBreaker::new(2, 0);
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        // Cooldown is 0s, should immediately transition to half-open
        assert!(!cb.should_skip(&Protocol::Aave, &Chain::Ethereum).await);
    }

    #[tokio::test]
    async fn test_half_open_success_closes_circuit() {
        let cb = CircuitBreaker::new(2, 0);
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        // Transition to half-open
        cb.should_skip(&Protocol::Aave, &Chain::Ethereum).await;
        // Record success in half-open → closes
        cb.record_success(&Protocol::Aave, &Chain::Ethereum).await;
        assert!(!cb.should_skip(&Protocol::Aave, &Chain::Ethereum).await);
    }

    #[tokio::test]
    async fn test_half_open_failure_reopens_circuit() {
        let cb = CircuitBreaker::new(2, 0);
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        // Transition to half-open
        cb.should_skip(&Protocol::Aave, &Chain::Ethereum).await;
        // Probe fails
        cb.record_failure(&Protocol::Aave, &Chain::Ethereum).await;
        // With cooldown=0, will transition to half-open again immediately
        // but the consecutive_failures should have incremented
        let snap = cb.snapshot().await;
        let entry = snap.iter().find(|s| s.protocol == Protocol::Aave).unwrap();
        assert_eq!(entry.consecutive_failures, 3);
    }

    #[tokio::test]
    async fn test_snapshot_empty_initially() {
        let cb = CircuitBreaker::new(3, 60);
        let snap = cb.snapshot().await;
        assert!(snap.is_empty());
    }

    #[tokio::test]
    async fn test_snapshot_reports_status() {
        let cb = CircuitBreaker::new(2, 60);
        cb.record_failure(&Protocol::Morpho, &Chain::Base).await;
        cb.record_failure(&Protocol::Morpho, &Chain::Base).await;

        let snap = cb.snapshot().await;
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].status, "open");
        assert_eq!(snap[0].consecutive_failures, 2);
    }
}
