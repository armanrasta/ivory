//! Process-local Prometheus text metrics (no scrape auth).

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// In-process counters, gauges, and a produce-time histogram.
#[derive(Debug)]
pub struct IvoryMetrics {
    rpc_requests: Mutex<BTreeMap<String, u64>>,
    rpc_errors: Mutex<BTreeMap<(String, i64), u64>>,
    head_number: AtomicU64,
    head_timestamp: AtomicU64,
    txpool_pending: AtomicU64,
    blocks_imported: AtomicU64,
    blocks_produced: AtomicU64,
    p2p_peers: AtomicU64,
    p2p_messages: Mutex<BTreeMap<String, u64>>,
    produce_buckets: Mutex<Vec<u64>>,
    produce_sum: Mutex<f64>,
    produce_count: AtomicU64,
}

const PRODUCE_BOUNDS: &[f64] = &[
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
];

impl Default for IvoryMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl IvoryMetrics {
    /// Empty metric set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rpc_requests: Mutex::new(BTreeMap::new()),
            rpc_errors: Mutex::new(BTreeMap::new()),
            head_number: AtomicU64::new(0),
            head_timestamp: AtomicU64::new(0),
            txpool_pending: AtomicU64::new(0),
            blocks_imported: AtomicU64::new(0),
            blocks_produced: AtomicU64::new(0),
            p2p_peers: AtomicU64::new(0),
            p2p_messages: Mutex::new(BTreeMap::new()),
            produce_buckets: Mutex::new(vec![0; PRODUCE_BOUNDS.len() + 1]),
            produce_sum: Mutex::new(0.0),
            produce_count: AtomicU64::new(0),
        }
    }

    /// `ivory_rpc_requests_total`.
    pub fn rpc_request(&self, method: &str) {
        *self
            .rpc_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(method.to_string())
            .or_insert(0) += 1;
    }

    /// `ivory_rpc_errors_total`.
    pub fn rpc_error(&self, method: &str, code: i64) {
        *self
            .rpc_errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry((method.to_string(), code))
            .or_insert(0) += 1;
    }

    /// Head gauges.
    pub fn set_head(&self, number: u64, timestamp: u64) {
        self.head_number.store(number, Ordering::Relaxed);
        self.head_timestamp.store(timestamp, Ordering::Relaxed);
    }

    /// Mempool gauge.
    pub fn set_txpool_pending(&self, n: u64) {
        self.txpool_pending.store(n, Ordering::Relaxed);
    }

    /// P2P peer gauge.
    pub fn set_p2p_peers(&self, n: u64) {
        self.p2p_peers.store(n, Ordering::Relaxed);
    }

    /// Imported-block counter.
    pub fn inc_blocks_imported(&self) {
        self.blocks_imported.fetch_add(1, Ordering::Relaxed);
    }

    /// Produced-block counter.
    pub fn inc_blocks_produced(&self) {
        self.blocks_produced.fetch_add(1, Ordering::Relaxed);
    }

    /// `ivory_p2p_messages_total`.
    pub fn p2p_message(&self, kind: &str) {
        *self
            .p2p_messages
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(kind.to_string())
            .or_insert(0) += 1;
    }

    /// Observe block production latency in seconds.
    pub fn observe_produce_seconds(&self, seconds: f64) {
        let mut buckets = self
            .produce_buckets
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut placed = false;
        for (i, bound) in PRODUCE_BOUNDS.iter().enumerate() {
            if seconds <= *bound {
                buckets[i] += 1;
                placed = true;
                break;
            }
        }
        if !placed {
            let last = buckets.len() - 1;
            buckets[last] += 1;
        }
        *self.produce_sum.lock().unwrap_or_else(|e| e.into_inner()) += seconds;
        self.produce_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Prometheus text exposition.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = String::new();
        out.push_str("# HELP ivory_rpc_requests_total JSON-RPC requests\n");
        out.push_str("# TYPE ivory_rpc_requests_total counter\n");
        for (method, n) in self
            .rpc_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            out.push_str(&format!(
                "ivory_rpc_requests_total{{method=\"{}\"}} {n}\n",
                escape(method)
            ));
        }
        out.push_str("# HELP ivory_rpc_errors_total JSON-RPC errors\n");
        out.push_str("# TYPE ivory_rpc_errors_total counter\n");
        for ((method, code), n) in self
            .rpc_errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            out.push_str(&format!(
                "ivory_rpc_errors_total{{method=\"{}\",code=\"{code}\"}} {n}\n",
                escape(method)
            ));
        }
        out.push_str("# HELP ivory_head_number Canonical head height\n");
        out.push_str("# TYPE ivory_head_number gauge\n");
        out.push_str(&format!(
            "ivory_head_number {}\n",
            self.head_number.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP ivory_head_timestamp Canonical head timestamp\n");
        out.push_str("# TYPE ivory_head_timestamp gauge\n");
        out.push_str(&format!(
            "ivory_head_timestamp {}\n",
            self.head_timestamp.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP ivory_txpool_pending Pending transactions\n");
        out.push_str("# TYPE ivory_txpool_pending gauge\n");
        out.push_str(&format!(
            "ivory_txpool_pending {}\n",
            self.txpool_pending.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP ivory_blocks_imported_total Imported blocks\n");
        out.push_str("# TYPE ivory_blocks_imported_total counter\n");
        out.push_str(&format!(
            "ivory_blocks_imported_total {}\n",
            self.blocks_imported.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP ivory_blocks_produced_total Produced blocks\n");
        out.push_str("# TYPE ivory_blocks_produced_total counter\n");
        out.push_str(&format!(
            "ivory_blocks_produced_total {}\n",
            self.blocks_produced.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP ivory_p2p_peers Connected peers\n");
        out.push_str("# TYPE ivory_p2p_peers gauge\n");
        out.push_str(&format!(
            "ivory_p2p_peers {}\n",
            self.p2p_peers.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP ivory_p2p_messages_total Gossip messages\n");
        out.push_str("# TYPE ivory_p2p_messages_total counter\n");
        for (kind, n) in self
            .p2p_messages
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            out.push_str(&format!(
                "ivory_p2p_messages_total{{kind=\"{}\"}} {n}\n",
                escape(kind)
            ));
        }
        out.push_str("# HELP ivory_block_produce_seconds Block production latency\n");
        out.push_str("# TYPE ivory_block_produce_seconds histogram\n");
        let buckets = self
            .produce_buckets
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut acc = 0u64;
        for (i, bound) in PRODUCE_BOUNDS.iter().enumerate() {
            acc += buckets[i];
            out.push_str(&format!(
                "ivory_block_produce_seconds_bucket{{le=\"{bound}\"}} {acc}\n"
            ));
        }
        acc += buckets[PRODUCE_BOUNDS.len()];
        out.push_str(&format!(
            "ivory_block_produce_seconds_bucket{{le=\"+Inf\"}} {acc}\n"
        ));
        out.push_str(&format!(
            "ivory_block_produce_seconds_sum {}\n",
            *self.produce_sum.lock().unwrap_or_else(|e| e.into_inner())
        ));
        out.push_str(&format!(
            "ivory_block_produce_seconds_count {}\n",
            self.produce_count.load(Ordering::Relaxed)
        ));
        out
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
