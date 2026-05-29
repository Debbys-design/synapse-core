//! # Pool Manager with Optimized Connection Pooling
//!
//! Provides efficient connection pooling with failover, metrics, and health checks.
//!
//! ## Features
//!
//! - **Dual-pool architecture**: Primary for writes, replica(s) for reads
//! - **Failover detection**: Monitors replica health and falls back to primary
//! - **Warm-up**: Eagerly establishes connections at startup
//! - **Metrics**: Tracks pool usage, contention, and failovers
//! - **Dynamic routing**: Automatically routes to replica for read queries when healthy
//!
//! ## Configuration
//!
//! - `max_connections`: Per-pool limit (tuned per workload: 10-50)
//! - `min_connections`: Pre-warmed connections (reduces startup latency)
//! - `idle_timeout`: Auto-close idle connections to free resources
//! - `acquire_timeout`: Max time to wait for available connection

use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use std::time::Duration;

#[derive(Clone)]
pub struct PoolManager {
    primary: PgPool,
    replica: Option<PgPool>,
    failover_state: Arc<RwLock<FailoverState>>,
    metrics: Arc<PoolMetrics>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FailoverState {
    primary_healthy: bool,
    replica_healthy: bool,
}

/// Metrics for connection pool monitoring.
#[derive(Debug, Default)]
pub struct PoolMetrics {
    /// Total read operations routed to replica
    pub read_to_replica: AtomicU64,
    /// Total read operations routed to primary (replica unavailable)
    pub read_to_primary_fallback: AtomicU64,
    /// Total replica health check failures
    pub replica_health_failures: AtomicU64,
    /// Total successful failovers from replica to primary
    pub failovers_executed: AtomicU64,
}

impl PoolManager {
    /// Create a new pool manager with optional replica.
    ///
    /// # Performance Tuning
    ///
    /// Pool sizes depend on workload:
    /// - API layer (high concurrency): 20-30 connections
    /// - Background workers (steady load): 10-15 connections
    /// - Long-running tasks: 5-10 connections
    ///
    /// # Examples
    /// ```ignore
    /// let manager = PoolManager::new(
    ///     "postgresql://user:pass@primary:5432/db",
    ///     Some("postgresql://user:pass@replica:5432/db"),
    /// ).await?;
    /// ```
    pub async fn new(primary_url: &str, replica_url: Option<&str>) -> Result<Self, sqlx::Error> {
        let primary = PgPoolOptions::new()
            .max_connections(20) // Optimized for API workload
            .min_connections(5)
            .idle_timeout(Duration::from_secs(300))
            .acquire_timeout(Duration::from_secs(30))
            .connect(primary_url)
            .await?;

        let replica = if let Some(url) = replica_url {
            Some(
                PgPoolOptions::new()
                    .max_connections(30) // Replica can handle more concurrent reads
                    .min_connections(5)
                    .idle_timeout(Duration::from_secs(300))
                    .acquire_timeout(Duration::from_secs(30))
                    .connect(url)
                    .await?,
            )
        } else {
            None
        };

        Ok(Self {
            primary,
            replica,
            failover_state: Arc::new(RwLock::new(FailoverState {
                primary_healthy: true,
                replica_healthy: true,
            })),
            metrics: Arc::new(PoolMetrics::default()),
        })
    }

    /// Create with custom connection limits for specific workloads.
    pub async fn with_config(
        primary_url: &str,
        replica_url: Option<&str>,
        primary_max_conn: u32,
        replica_max_conn: u32,
        min_connections: u32,
        idle_timeout_secs: u64,
    ) -> Result<Self, sqlx::Error> {
        let primary = PgPoolOptions::new()
            .max_connections(primary_max_conn)
            .min_connections(min_connections)
            .idle_timeout(Duration::from_secs(idle_timeout_secs))
            .acquire_timeout(Duration::from_secs(30))
            .connect(primary_url)
            .await?;

        let replica = if let Some(url) = replica_url {
            Some(
                PgPoolOptions::new()
                    .max_connections(replica_max_conn)
                    .min_connections(min_connections)
                    .idle_timeout(Duration::from_secs(idle_timeout_secs))
                    .acquire_timeout(Duration::from_secs(30))
                    .connect(url)
                    .await?,
            )
        } else {
            None
        };

        Ok(Self {
            primary,
            replica,
            failover_state: Arc::new(RwLock::new(FailoverState {
                primary_healthy: true,
                replica_healthy: true,
            })),
            metrics: Arc::new(PoolMetrics::default()),
        })
    }

    /// Get reference to primary pool (for write operations).
    pub fn primary(&self) -> &PgPool {
        &self.primary
    }

    /// Get reference to replica pool if available (for read operations).
    pub fn replica(&self) -> Option<&PgPool> {
        self.replica.as_ref()
    }

    /// Get pool metrics for monitoring and alerting.
    pub fn metrics(&self) -> &PoolMetrics {
        &self.metrics
    }

    /// Determine which pool to use for read queries with automatic failover.
    ///
    /// Returns tuple of (pool_reference, was_replica_used).
    /// If replica is unavailable or unhealthy, falls back to primary.
    pub async fn read_pool(&self) -> (&PgPool, bool) {
        let state = self.failover_state.read().await;

        if let Some(replica) = &self.replica {
            if state.replica_healthy {
                self.metrics.read_to_replica.fetch_add(1, Ordering::Relaxed);
                tracing::debug!("Routing read query to replica database");
                return (replica, true);
            }
        }

        self.metrics.read_to_primary_fallback.fetch_add(1, Ordering::Relaxed);
        (&self.primary, false)
    }

    /// Get read pool with simplified API.
    pub async fn get_read_pool(&self) -> &PgPool {
        self.read_pool().await.0
    }

    /// Get write pool (always primary for consistency).
    pub async fn get_write_pool(&self) -> &PgPool {
        &self.primary
    }

    /// Check replica health and update failover state.
    ///
    /// Should be called periodically (e.g., every 30 seconds) to detect replica failures.
    pub async fn check_replica_health(&self) -> Result<(), sqlx::Error> {
        let is_healthy = if let Some(replica) = &self.replica {
            match sqlx::query("SELECT 1").fetch_one(replica).await {
                Ok(_) => {
                    tracing::debug!("Replica health check passed");
                    true
                }
                Err(e) => {
                    tracing::warn!("Replica health check failed: {}", e);
                    self.metrics.replica_health_failures.fetch_add(1, Ordering::Relaxed);
                    false
                }
            }
        } else {
            true
        };

        let mut state = self.failover_state.write().await;
        if state.replica_healthy && !is_healthy {
            tracing::error!("Replica failover triggered");
            self.metrics.failovers_executed.fetch_add(1, Ordering::Relaxed);
        }
        state.replica_healthy = is_healthy;
        Ok(())
    }

    /// Get pool statistics for monitoring dashboards.
    pub async fn pool_stats(&self) -> PoolStats {
        let primary_size = self.primary.size();
        let primary_idle = self.primary.num_idle();

        let (replica_size, replica_idle) = if let Some(replica) = &self.replica {
            (replica.size(), replica.num_idle() as u32)
        } else {
            (0, 0)
        };

        PoolStats {
            primary_active: primary_size - primary_idle as u32,
            primary_idle,
            primary_size,
            replica_active: if replica_size > replica_idle {
                replica_size - replica_idle
            } else {
                0
            },
            replica_idle,
            replica_size,
            total_reads_to_replica: self.metrics.read_to_replica.load(Ordering::Relaxed),
            total_failovers: self.metrics.failovers_executed.load(Ordering::Relaxed),
        }
    }
}

/// Connection pool statistics for monitoring.
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub primary_active: u32,
    pub primary_idle: u32,
    pub primary_size: u32,
    pub replica_active: u32,
    pub replica_idle: u32,
    pub replica_size: u32,
    pub total_reads_to_replica: u64,
    pub total_failovers: u64,
}
