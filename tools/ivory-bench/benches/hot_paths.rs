//! Hot-path Criterion benches for the quant-submission engine substrate.
//!
//! Run: `cargo bench -p ivory-bench`
//! Quick smoke: `cargo bench -p ivory-bench -- --quick`

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use ivory_core::{Account, Transaction};
use ivory_crypto::{keypair_from_byte, keypair_from_seed, signed_tx};
use ivory_executor::{ExecutionContext, Executor};
use ivory_primitives::{Address, Bytes, SecretKey, U256};
use ivory_state::StateDB;
use ivory_txpool::{PoolConfig, TransactionPool, TxOrigin};

fn sender(seed: u8) -> (SecretKey, Address) {
    let (sk, _, addr) = keypair_from_byte(seed);
    (sk, addr)
}

fn to_addr() -> Address {
    keypair_from_byte(2).2
}

fn addr(byte: u8) -> Address {
    Address::from_bytes([byte; 20])
}

fn transfer(sk: &SecretKey, to: Address, nonce: u64, data: Bytes) -> Transaction {
    let data_gas = (data.as_slice().len() as u64).saturating_mul(16);
    signed_tx(
        sk,
        Some(to),
        nonce,
        U256::from(1u64),
        21_000u64.saturating_add(data_gas),
        U256::ONE,
        data,
    )
}

fn tx_from_seed1(nonce: u64, data: Bytes) -> Transaction {
    let (sk, _) = sender(1);
    transfer(&sk, to_addr(), nonce, data)
}

fn from1() -> Address {
    sender(1).1
}

fn funded(balance: u64) -> Account {
    let mut account = Account::new();
    account.balance = U256::from(balance);
    account
}

fn pool_for_batch(max: usize) -> TransactionPool {
    TransactionPool::with_config(PoolConfig {
        max_pending: max.max(1),
        max_per_sender: max.max(1),
        min_gas: 21_000,
    })
}

fn bench_tx_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("tx_hash");
    for data_len in [0usize, 256, 4096] {
        let data = Bytes::from_slice(&vec![0xab; data_len]);
        let tx = tx_from_seed1(0, data);
        group.throughput(Throughput::Bytes(data_len as u64));
        group.bench_function(format!("data_{data_len}"), |b| {
            b.iter(|| tx.hash());
        });
    }
    group.finish();
}

fn bench_state(c: &mut Criterion) {
    let state = StateDB::new();
    let a = addr(1);
    state.set_account(a, funded(1_000_000));

    let mut group = c.benchmark_group("state");
    group.bench_function("get_account", |b| {
        b.iter(|| state.get_account(&a));
    });
    group.bench_function("set_account", |b| {
        b.iter(|| state.set_account(a, funded(1_000_000)));
    });
    group.finish();
}

fn bench_pool_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool_add");

    for batch in [64usize, 256, 1024] {
        group.throughput(Throughput::Elements(batch as u64));
        group.bench_function(format!("single_sender_{batch}"), |b| {
            b.iter_batched(
                || {
                    let pool = pool_for_batch(batch);
                    let txs: Vec<_> = (0..batch)
                        .map(|n| tx_from_seed1(n as u64, Bytes::new()))
                        .collect();
                    (pool, txs)
                },
                |(pool, txs)| {
                    for tx in txs {
                        pool.add_transaction(tx, TxOrigin::Local).expect("admit");
                    }
                    pool.pending_count()
                },
                BatchSize::SmallInput,
            );
        });

        group.bench_function(format!("multi_sender_{batch}"), |b| {
            b.iter_batched(
                || {
                    let pool = pool_for_batch(batch);
                    let txs: Vec<_> = (0..batch)
                        .map(|n| {
                            let mut seed = [0u8; 32];
                            seed[..8].copy_from_slice(&(n as u64).to_le_bytes());
                            let (sk, _, _) = keypair_from_seed(seed);
                            transfer(&sk, to_addr(), 0, Bytes::new())
                        })
                        .collect();
                    (pool, txs)
                },
                |(pool, txs)| {
                    for tx in txs {
                        pool.add_transaction(tx, TxOrigin::Local).expect("admit");
                    }
                    pool.pending_count()
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_pool_get_pending(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool_get_pending");
    for size in [64usize, 256, 1024] {
        let pool = pool_for_batch(size);
        for n in 0..size {
            pool.add_transaction(tx_from_seed1(n as u64, Bytes::new()), TxOrigin::Local)
                .expect("admit");
        }
        group.throughput(Throughput::Elements(size as u64));
        group.bench_function(format!("take_{size}"), |b| {
            b.iter(|| pool.get_pending(size));
        });
    }
    group.finish();
}

fn bench_execute_transfer(c: &mut Criterion) {
    let mut group = c.benchmark_group("execute_transfer");

    group.bench_function("single", |b| {
        b.iter_batched(
            || {
                let state = StateDB::new();
                state.set_account(from1(), funded(10_000_000));
                let exec = Executor::new(state);
                let tx = tx_from_seed1(0, Bytes::new());
                (exec, tx)
            },
            |(exec, tx)| {
                let mut ctx = ExecutionContext::new(1, 0);
                exec.execute_transaction(&tx, &mut ctx).expect("execute")
            },
            BatchSize::SmallInput,
        );
    });

    for batch in [64usize, 256, 1024] {
        group.throughput(Throughput::Elements(batch as u64));
        group.bench_function(format!("block_{batch}"), |b| {
            b.iter_batched(
                || {
                    let state = StateDB::new();
                    // value + gas_price*gas per tx ≈ 1 + 21_000
                    let balance = (batch as u64)
                        .saturating_mul(50_000)
                        .saturating_add(1_000_000);
                    state.set_account(from1(), funded(balance));
                    let exec = Executor::new(state);
                    let txs: Vec<_> = (0..batch)
                        .map(|n| tx_from_seed1(n as u64, Bytes::new()))
                        .collect();
                    (exec, txs)
                },
                |(exec, txs)| {
                    let mut ctx = ExecutionContext::new(1, 0);
                    for tx in &txs {
                        exec.execute_transaction(tx, &mut ctx).expect("execute");
                    }
                    ctx.gas_used
                },
                BatchSize::SmallInput,
            );
        });
    }

    // Quant-sized calldata: engine path with non-empty data (intrinsic gas only today).
    for data_len in [256usize, 4096] {
        group.throughput(Throughput::Bytes(data_len as u64));
        group.bench_function(format!("quant_data_{data_len}"), |b| {
            b.iter_batched(
                || {
                    let state = StateDB::new();
                    state.set_account(from1(), funded(10_000_000));
                    let exec = Executor::new(state);
                    let data = Bytes::from_slice(&vec![0xcd; data_len]);
                    let tx = tx_from_seed1(0, data);
                    (exec, tx)
                },
                |(exec, tx)| {
                    let mut ctx = ExecutionContext::new(1, 0);
                    exec.execute_transaction(&tx, &mut ctx).expect("execute")
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_pool_to_execute(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool_to_execute");
    for batch in [64usize, 256] {
        group.throughput(Throughput::Elements(batch as u64));
        group.bench_function(format!("pipeline_{batch}"), |b| {
            b.iter_batched(
                || {
                    let state = StateDB::new();
                    let balance = (batch as u64)
                        .saturating_mul(50_000)
                        .saturating_add(1_000_000);
                    state.set_account(from1(), funded(balance));
                    let pool = pool_for_batch(batch);
                    for n in 0..batch {
                        pool.add_transaction(
                            tx_from_seed1(n as u64, Bytes::new()),
                            TxOrigin::Local,
                        )
                        .expect("admit");
                    }
                    (state, pool)
                },
                |(state, pool)| {
                    let pending = pool.get_pending(batch);
                    let mut ordered = pending;
                    ordered.sort_by_key(|t| t.nonce);
                    let exec = Executor::new(state);
                    let mut ctx = ExecutionContext::new(1, 0);
                    for tx in &ordered {
                        exec.execute_transaction(tx, &mut ctx).expect("execute");
                        pool.remove(&tx.hash());
                    }
                    (pool.pending_count(), ctx.gas_used)
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_tx_hash,
    bench_state,
    bench_pool_add,
    bench_pool_get_pending,
    bench_execute_transfer,
    bench_pool_to_execute,
);
criterion_main!(benches);
