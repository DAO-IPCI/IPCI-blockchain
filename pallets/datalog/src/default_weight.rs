use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

pub trait WeightInfo {
    fn record() -> Weight;
    fn erase(win: u64) -> Weight;
}

impl WeightInfo for () {
    fn record() -> Weight {
        (500_000 as Weight)
            .saturating_add(DbWeight::get().reads(2 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }

    fn erase(win: u64) -> Weight {
        (100_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1+win as Weight))
    }
}