use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed = 0,
    Degraded = 1,
    Open = 2,
    HalfOpen = 3,
}

impl From<u8> for CircuitState {
    fn from(val: u8) -> Self {
        match val {
            0 => CircuitState::Closed,
            1 => CircuitState::Degraded,
            2 => CircuitState::Open,
            3 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }
}

pub struct CircuitBreaker {
    state: AtomicU8,
    failure_count: AtomicU64,
    failure_threshold: u64,
    degradation_threshold: u64,
    reset_timeout: Duration,
    last_state_change: RwLock<Instant>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u64, degradation_threshold: u64, reset_timeout: Duration) -> Self {
        Self {
            state: AtomicU8::new(CircuitState::Closed as u8),
            failure_count: AtomicU64::new(0),
            failure_threshold,
            degradation_threshold,
            reset_timeout,
            last_state_change: RwLock::new(Instant::now()),
        }
    }

    pub fn state(&self) -> CircuitState {
        let current_u8 = self.state.load(Ordering::Acquire);
        let current_state = CircuitState::from(current_u8);

        if current_state == CircuitState::Open {
            if let Ok(guard) = self.last_state_change.read() {
                if guard.elapsed() >= self.reset_timeout {
                    drop(guard);
                    if let Ok(mut write_guard) = self.last_state_change.write() {
                        self.state.store(CircuitState::HalfOpen as u8, Ordering::Release);
                        *write_guard = Instant::now();
                        return CircuitState::HalfOpen;
                    }
                }
            }
        }

        current_state
    }

    pub fn can_execute(&self) -> bool {
        match self.state() {
            CircuitState::Closed | CircuitState::Degraded | CircuitState::HalfOpen => true,
            CircuitState::Open => false,
        }
    }

    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        let current = self.state();
        if current != CircuitState::Closed {
            self.state.store(CircuitState::Closed as u8, Ordering::Release);
            if let Ok(mut guard) = self.last_state_change.write() {
                *guard = Instant::now();
            }
        }
    }

    pub fn record_failure(&self) {
        let fails = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;

        if fails >= self.failure_threshold {
            self.state.store(CircuitState::Open as u8, Ordering::Release);
            if let Ok(mut guard) = self.last_state_change.write() {
                *guard = Instant::now();
            }
        } else if fails >= self.degradation_threshold {
            self.state.store(CircuitState::Degraded as u8, Ordering::Release);
            if let Ok(mut guard) = self.last_state_change.write() {
                *guard = Instant::now();
            }
        }
    }
}
