use csv::{Reader, ReaderBuilder, Trim};
use std::io::Read;

use crate::{error::AppError, models::transaction::RawTransaction};

pub struct TransactionReader<R: Read> {
    inner: Reader<R>,
}

impl<R: Read> TransactionReader<R> {
    pub fn new(reader: R) -> Self {
        let inner = ReaderBuilder::new()
            .trim(Trim::All)
            .flexible(true)
            .from_reader(reader);

        Self { inner }
    }
}

impl<R: Read> Iterator for TransactionReader<R> {
    type Item = Result<RawTransaction, AppError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut record = csv::StringRecord::new();

        match self.inner.read_record(&mut record) {
            Ok(true) => {
                let raw: Result<RawTransaction, _> =
                    record.deserialize(Some(self.inner.headers().unwrap()));

                Some(raw.map_err(AppError::Csv))
            }
            Ok(false) => None, // EOF
            Err(e) => Some(Err(AppError::Csv(e))),
        }
    }
}
