//! Memory footprint scenarios for the quant-submission substrate.
//!
//! Reports allocator heap deltas (`peak_alloc`) and process RSS deltas
//! (`memory-stats`) after growing state, pool, and execute paths.
//!
//! Run (optimized `bench` / release-class profile):
//! ```bash
//! cargo bench -p ivory-bench --bench memory
//! ```

use std::hint::black_box;

use ivory_core::{Account, Transaction};
use ivory_crypto::{keypair_from_byte, signed_tx};
use ivory_executor::{ExecutionContext, Executor};
use ivory_primitives::{Address, Bytes, SecretKey, U256};
use ivory_state::StateDB;
use ivory_txpool::{PoolConfig, TransactionPool, TxOrigin};
use memory_stats::memory_stats;
use peak_alloc::PeakAlloc;

#[global_allocator]
static PEAK: PeakAlloc = PeakAlloc;

fn addr_n(n: u64) -> Address {
    let mut bytes = [0u8; 20];
    bytes[12..20].copy_from_slice(&n.to_be_bytes());
    Address::from_bytes(bytes)
}

fn sender(seed: u8) -> (SecretKey, Address) {
    let (sk, _, addr) = keypair_from_byte(seed);
    (sk, addr)
}

fn from1() -> Address {
    sender(1).1
}

fn to_addr() -> Address {
    keypair_from_byte(2).2
}

fn tx_from_seed1(nonce: u64, data: Bytes) -> Transaction {
    let data_gas = (data.as_slice().len() as u64).saturating_mul(16);
    let (sk, _) = sender(1);
    signed_tx(
        &sk,
        Some(to_addr()),
        nonce,
        U256::from(1u64),
        21_000u64.saturating_add(data_gas),
        U256::ONE,
        data,
    )
}

fn funded(balance: u64) -> Account {
    let mut account = Account::new();
    account.balance = U256::from(balance);
    account
}

fn pool_for(max: usize) -> TransactionPool {
    TransactionPool::with_config(PoolConfig {
        max_pending: max.max(1),
        max_per_sender: max.max(1),
        min_gas: 21_000,
    })
}

fn fmt_bytes(n: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    let v = n as f64;
    if v >= MIB {
        format!("{:.2} MiB", v / MIB)
    } else if v >= KIB {
        format!("{:.2} KiB", v / KIB)
    } else {
        format!("{n} B")
    }
}

struct Sample {
    heap: usize,
    rss: Option<usize>,
}

fn sample() -> Sample {
    Sample {
        heap: PEAK.current_usage(),
        rss: memory_stats().map(|m| m.physical_mem),
    }
}

struct Row {
    name: String,
    items: usize,
    heap_delta: usize,
    rss_delta: Option<isize>,
    /// Retained so measured allocations stay live for RSS/heap sampling.
    #[allow(dead_code)]
    keep: Box<dyn std::any::Any>,
}

fn delta(before: &Sample, after: &Sample) -> (usize, Option<isize>) {
    let heap = after.heap.saturating_sub(before.heap);
    let rss = match (before.rss, after.rss) {
        (Some(b), Some(a)) => Some(a as isize - b as isize),
        _ => None,
    };
    (heap, rss)
}

fn measure_accounts(n: usize) -> Row {
    let before = sample();
    let state = StateDB::new();
    for i in 0..n as u64 {
        state.set_account(addr_n(i), funded(1_000));
    }
    black_box(state.get_account(&addr_n(0)));
    let after = sample();
    let (heap_delta, rss_delta) = delta(&before, &after);
    Row {
        name: format!("state_accounts_{n}"),
        items: n,
        heap_delta,
        rss_delta,
        keep: Box::new(state),
    }
}

fn measure_pool(n: usize, data_len: usize) -> Row {
    let before = sample();
    let pool = pool_for(n);
    let data = Bytes::from_slice(&vec![0xcd; data_len]);
    for i in 0..n as u64 {
        // Clone payload each admit so pool owns distinct Bytes buffers.
        let tx = tx_from_seed1(i, Bytes::from_slice(data.as_slice()));
        pool.add_transaction(tx, TxOrigin::Local).expect("admit");
    }
    black_box(pool.pending_count());
    let after = sample();
    let (heap_delta, rss_delta) = delta(&before, &after);
    Row {
        name: format!("pool_pending_{n}_data_{data_len}"),
        items: n,
        heap_delta,
        rss_delta,
        keep: Box::new(pool),
    }
}

fn measure_execute_block(n: usize) -> Row {
    let before = sample();
    let state = StateDB::new();
    let balance = (n as u64).saturating_mul(50_000).saturating_add(1_000_000);
    state.set_account(from1(), funded(balance));
    let exec = Executor::new(state.clone());
    let mut ctx = ExecutionContext::new(1, 0);
    for i in 0..n as u64 {
        let tx = tx_from_seed1(i, Bytes::new());
        exec.execute_transaction(&tx, &mut ctx).expect("execute");
    }
    black_box(ctx.gas_used);
    let after = sample();
    let (heap_delta, rss_delta) = delta(&before, &after);
    Row {
        name: format!("execute_block_{n}"),
        items: n,
        heap_delta,
        rss_delta,
        keep: Box::new((state, exec)),
    }
}

fn measure_pipeline(n: usize) -> Row {
    let before = sample();
    let state = StateDB::new();
    let balance = (n as u64).saturating_mul(50_000).saturating_add(1_000_000);
    state.set_account(from1(), funded(balance));
    let pool = pool_for(n);
    for i in 0..n as u64 {
        pool.add_transaction(tx_from_seed1(i, Bytes::new()), TxOrigin::Local)
            .expect("admit");
    }
    let mut pending = pool.get_pending(n);
    pending.sort_by_key(|t| t.nonce);
    let exec = Executor::new(state.clone());
    let mut ctx = ExecutionContext::new(1, 0);
    for tx in &pending {
        exec.execute_transaction(tx, &mut ctx).expect("execute");
        pool.remove(&tx.hash());
    }
    black_box((pool.pending_count(), ctx.gas_used));
    let after = sample();
    let (heap_delta, rss_delta) = delta(&before, &after);
    Row {
        name: format!("pipeline_{n}"),
        items: n,
        heap_delta,
        rss_delta,
        keep: Box::new((state, pool, exec)),
    }
}

fn print_table(rows: &[Row]) {
    println!();
    println!(
        "{:<36} {:>8} {:>12} {:>12} {:>14} {:>12}",
        "scenario", "items", "heap_delta", "heap/item", "rss_delta", "rss/item"
    );
    println!("{}", "-".repeat(100));
    for row in rows {
        let per = if row.items == 0 {
            0
        } else {
            row.heap_delta / row.items
        };
        let rss = row
            .rss_delta
            .map(|d| {
                if d >= 0 {
                    fmt_bytes(d as usize)
                } else {
                    format!("-{}", fmt_bytes((-d) as usize))
                }
            })
            .unwrap_or_else(|| "n/a".into());
        let rss_per = row
            .rss_delta
            .filter(|d| *d > 0 && row.items > 0)
            .map(|d| fmt_bytes((d as usize) / row.items))
            .unwrap_or_else(|| "—".into());
        println!(
            "{:<36} {:>8} {:>12} {:>12} {:>14} {:>12}",
            row.name,
            row.items,
            fmt_bytes(row.heap_delta),
            fmt_bytes(per),
            rss,
            rss_per
        );
    }
    println!();
}

fn main() {
    // Touch allocator / RSS so the first delta is not dominated by lazy init.
    let _warmup = vec![0u8; 64 * 1024];
    black_box(&_warmup);
    drop(_warmup);

    let baseline = sample();
    println!("ivory-bench memory");
    println!(
        "profile: bench/release-class | heap_now={} | rss_now={} | peak_so_far={}",
        fmt_bytes(baseline.heap),
        baseline.rss.map(fmt_bytes).unwrap_or_else(|| "n/a".into()),
        fmt_bytes(PEAK.peak_usage())
    );
    println!("heap = peak_alloc current_usage delta; rss = process physical_mem delta");
    println!("Objects are retained until the end of each scenario so deltas stay live.");

    let mut rows = Vec::new();
    for n in [1_000usize, 10_000, 100_000] {
        rows.push(measure_accounts(n));
    }
    for n in [1_000usize, 4_096] {
        rows.push(measure_pool(n, 0));
    }
    rows.push(measure_pool(256, 256));
    rows.push(measure_pool(256, 4096));
    for n in [256usize, 1_024] {
        rows.push(measure_execute_block(n));
    }
    for n in [256usize, 1_024] {
        rows.push(measure_pipeline(n));
    }

    print_table(&rows);
    println!(
        "process peak heap (entire run): {}",
        fmt_bytes(PEAK.peak_usage())
    );
    // Keep rows alive until after peak print.
    black_box(&rows);
}
