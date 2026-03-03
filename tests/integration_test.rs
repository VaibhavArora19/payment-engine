use std::collections::HashMap;
use std::process::Command;

struct Account {
    available: String,
    held: String,
    total: String,
    locked: String,
}

fn run_engine(fixture: &str) -> HashMap<u16, Account> {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), fixture);

    let output = Command::new(env!("CARGO_BIN_EXE_payment-engine"))
        .arg(&path)
        .output()
        .expect("failed to run payment-engine binary");

    assert!(
        output.status.success(),
        "binary exited with error: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout is not valid utf-8");
    let mut accounts = HashMap::new();

    for line in stdout.lines().skip(1) {
        // skip header
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 5 {
            continue;
        }
        let client: u16 = cols[0]
            .trim()
            .parse()
            .expect("client id is not a valid u16");
        accounts.insert(
            client,
            Account {
                available: cols[1].trim().to_string(),
                held: cols[2].trim().to_string(),
                total: cols[3].trim().to_string(),
                locked: cols[4].trim().to_string(),
            },
        );
    }

    accounts
}

#[test]
fn deposit_and_withdrawal_updates_balance() {
    let accounts = run_engine("deposit_withdrawal.csv");
    let acc = &accounts[&1];

    assert_eq!(acc.available, "60.0000");
    assert_eq!(acc.held, "0.0000");
    assert_eq!(acc.total, "60.0000");
    assert_eq!(acc.locked, "false");
}

#[test]
fn withdrawal_insufficient_funds_leaves_balance_unchanged() {
    let accounts = run_engine("insufficient_funds.csv");
    let acc = &accounts[&1];

    assert_eq!(acc.available, "10.0000");
    assert_eq!(acc.held, "0.0000");
    assert_eq!(acc.total, "10.0000");
    assert_eq!(acc.locked, "false");
}

#[test]
fn dispute_then_resolve_restores_available() {
    let accounts = run_engine("dispute_resolve.csv");
    let acc = &accounts[&1];

    assert_eq!(acc.available, "100.0000");
    assert_eq!(acc.held, "0.0000");
    assert_eq!(acc.total, "100.0000");
    assert_eq!(acc.locked, "false");
}

#[test]
fn dispute_then_chargeback_locks_account_and_ignores_further_deposits() {
    // fixture: deposit 100, dispute, chargeback, then deposit 50 (ignored because locked)
    let accounts = run_engine("dispute_chargeback.csv");
    let acc = &accounts[&1];

    assert_eq!(acc.available, "0.0000");
    assert_eq!(acc.held, "0.0000");
    assert_eq!(acc.total, "0.0000");
    assert_eq!(acc.locked, "true");
}

#[test]
fn multiple_clients_are_independent() {
    let accounts = run_engine("multiple_clients.csv");

    let c1 = &accounts[&1];
    assert_eq!(c1.available, "70.0000");
    assert_eq!(c1.held, "0.0000");
    assert_eq!(c1.total, "70.0000");
    assert_eq!(c1.locked, "false");

    let c2 = &accounts[&2];
    assert_eq!(c2.available, "150.0000");
    assert_eq!(c2.held, "0.0000");
    assert_eq!(c2.total, "150.0000");
    assert_eq!(c2.locked, "false");
}

#[test]
fn full_scenario() {
    // client 1 -> deposit 100, withdrawal 30
    // client 2 -> deposit 50, dispute, resolve
    // client 3 -> deposit 100, dispute, chargeback
    let accounts = run_engine("full.csv");

    let c1 = &accounts[&1];
    assert_eq!(c1.available, "70.0000");
    assert_eq!(c1.held, "0.0000");
    assert_eq!(c1.total, "70.0000");
    assert_eq!(c1.locked, "false");

    let c2 = &accounts[&2];
    assert_eq!(c2.available, "50.0000");
    assert_eq!(c2.held, "0.0000");
    assert_eq!(c2.total, "50.0000");
    assert_eq!(c2.locked, "false");

    let c3 = &accounts[&3];
    assert_eq!(c3.available, "0.0000");
    assert_eq!(c3.held, "0.0000");
    assert_eq!(c3.total, "0.0000");
    assert_eq!(c3.locked, "true");
}
