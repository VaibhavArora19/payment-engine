# Payment Engine

A streaming payments engine that processes transactions from a CSV file, maintains client account state, and outputs final balances as CSV.

## Usage

```bash
cargo run -- transactions.csv > accounts.csv
```

Input is read from the file path argument. Output is written to stdout.

## Building and Testing

```bash
cargo build
cargo test
```

---

## Design Decisions

### Fixed-Point Arithmetic (`Amount` as `i64`)

All monetary values are stored as `i64` scaled by `10_000` (4 decimal places of precision). For example, `1.5000` is stored internally as `15_000`.

**Why not `f64`?**

Floating-point arithmetic is unsuitable for financial values due to rounding errors:

```
0.1 + 0.2 = 0.30000000000000004  (f64)
0.1 + 0.2 = 0.1000               (fixed-point, exact)
```

**Trade-off — reduced maximum value:**

`i64::MAX = 9_223_372_036_854_775_807`

Divided by `10_000` (our precision factor for 4 decimal points):

```
Max representable amount = 922_337_203_685_477.5807
```

This is orders of magnitude above any realistic single account balance, so the trade-off is acceptable for this use case.

All arithmetic uses `checked_add` / `checked_sub` which returns an explicit `Overflow` error rather than silently wrapping, ensuring no silent corruption of financial data.

---

### Streaming CSV Processing

The engine never loads the entire CSV into memory. Rows are read one at a time via an `Iterator` over `TransactionReader`:

```rust
for result in reader {        // one row at a time
    engine.process(raw)?;
}
```

This means a file with 4 billion rows uses the same memory as a file with 10 rows. The input file size has no impact on RAM usage.

---

### Memory Strategy for Transactions

Transaction IDs are `u32`, meaning up to **4,294,967,295** unique transactions are possible. Storing all of them naively would require:

```
4,294,967,295 entries × ~48 bytes each (16 bytes for the Transaction struct and 32 bytes of HashMap overhead) ≈ 206 GB
```

This is obviously not viable. The engine uses three strategies to keep memory bounded:

**1. Only deposits are stored**

Withdrawals can never be disputed per the spec, so they are never written to the transactions `HashMap`. This immediately halves the worst-case memory footprint.

**2. Terminal states are pruned immediately**

When a transaction is resolved or charged back, it is removed from the map (`transactions.remove(&tx_id)`). Once in a terminal state, a transaction can never be acted on again, so there is no reason to retain it.

**3. Only active surface is in memory at any point**

The `HashMap` at any moment contains only deposits that are either `Active` (could be disputed in future) or `Disputed` (currently under dispute). Everything else is gone.

**Practical memory bound on a 16 GB machine:**

```
Available RAM (after OS + accounts):  ~12 GB
Each Transaction entry in HashMap:    ~48 bytes
                                       ├─ amount (i64):       8 bytes
                                       ├─ tx_id (u32):        4 bytes
                                       ├─ client_id (u16):    2 bytes
                                       ├─ state (enum):       1 byte
                                       ├─ alignment padding:  1 byte
                                       └─ HashMap overhead:  ~32 bytes

Max concurrent active transactions:  12 GB / 48 bytes ≈ 268 million
```

In practice this is much higher because most deposits resolve or chargeback quickly, freeing memory continuously.

**Accounts are always bounded:**

Client IDs are `u16`, so at most **65,535** accounts can exist. Each account is ~24 bytes, giving a maximum of ~1.5 MB for all accounts which is negligible.

---

### Scaling Beyond RAM

If the workload requires retaining hundreds of millions of simultaneously active/disputed transactions (e.g., a system processing all 4.3 billion possible `u32` transaction IDs where nothing ever resolves), the in-memory `HashMap` would exhaust available RAM.

At that scale, the `transactions` map should be replaced with a disk-backed key-value store:

RocksDB (embedded, disk-backed) or Redis (distributed, in-memory) would be natural fits. The interface to swap either in would be a trait over the `transactions` field — the processor logic itself would not change, only the storage backend.

---

### Soft vs Hard Errors

The engine distinguishes between two classes of errors:

**Soft errors (logged, skipped):** Business rule violations that represent invalid but expected inputs — insufficient funds, locked account, unknown transaction ID for a dispute, client mismatch on a dispute. These are logged to stderr and the engine continues processing the rest of the stream.

**Hard errors (fatal):** I/O failures, CSV parse errors on a structural level. These propagate up and terminate the process. A corrupted file should not produce partial output silently.

This means a single malformed or invalid transaction never halts processing of the remaining file.

---

### Logs

Logs go to stderr so they don't pollute the CSV output. To see them:

```bash
RUST_LOG=warn cargo run -- transactions.csv > accounts.csv
```

---

## Correctness

The engine is tested at three levels:

**Unit tests — `Amount`** (`src/models/amount.rs`)
Covers parsing (integer, decimals, whitespace, overflow, negative, too many decimal places), display (4 decimal places, round-trip), arithmetic (add, subtract, overflow, insufficient funds), and comparison.

**Unit tests — `Account`** (`src/models/account.rs`)
Covers all five operations (deposit, withdraw, dispute, resolve, chargeback) with happy paths and every error path: locked account, insufficient funds, overflow.

**Unit tests — `Engine`** (`src/engine/processor.rs`)
Covers every transaction type end-to-end including all skip conditions: unknown tx, client mismatch, non-active/non-disputed state, missing amount, duplicate tx ID, ghost account prevention on withdrawal for unknown client, and the dispute state rollback on account mutation failure.

**Integration tests** (`tests/integration_test.rs`)
Runs the compiled binary as a subprocess against real CSV fixtures and asserts the exact stdout output. Covers deposit/withdrawal, insufficient funds, dispute→resolve, dispute→chargeback (with subsequent locked-account deposit ignored), multiple independent clients, and a full combined scenario.

---

## Project Structure

```
src/
  main.rs                  — CLI entry point, wires reader → engine → writer
  error.rs                 — AppError and AmountError types
  models/
    amount.rs              — Fixed-point monetary value type
    account.rs             — Per-client account state and mutations
    transaction.rs         — RawTransaction (CSV row) and Transaction (stored) types
  engine/
    processor.rs           — Core engine: processes transactions, maintains state
  io/
    reader.rs              — Streaming CSV reader (one row at a time)
    writer.rs              — CSV writer to stdout
tests/
  integration_test.rs      — End-to-end binary tests
  fixtures/                — CSV input files for integration tests
```
