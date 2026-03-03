use crate::error::AppError;
use crate::models::account::Account;
use std::io::Write;

pub fn write_accounts<W: Write>(
    writer: W,
    accounts: impl Iterator<Item = Account>,
) -> Result<(), AppError> {
    let mut wtr = csv::Writer::from_writer(writer);

    // write header manually for clarity
    wtr.write_record(["client", "available", "held", "total", "locked"])?;

    for account in accounts {
        wtr.write_record(&[
            account.client.to_string(),
            account.available.to_string(),
            account.held.to_string(),
            account.total()?.to_string(),
            account.locked.to_string(),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}
