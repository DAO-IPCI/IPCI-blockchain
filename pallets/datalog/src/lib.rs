///////////////////////////////////////////////////////////////////////////////
//
//  Copyright 2018-2020 Airalab <research@aira.life>
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
//
///////////////////////////////////////////////////////////////////////////////

//! Simple Robonomics datalog runtime module. This can be compiled with `#[no_std]`, ready for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]
pub use default_weight::WeightInfo;
use frame_support::{
    codec::{Codec, Decode, Encode, EncodeLike},
    decl_error, decl_event, decl_module, decl_storage, ensure,
    sp_runtime::traits::Member,
    sp_std::prelude::*,
    traits::{Get, Time},
};
use frame_system::ensure_signed;
mod benchmarking;
mod default_weight;
/// Type synonym for timestamp data type.
pub type MomentOf<T> = <<T as Config>::Time as Time>::Moment;
/// system::AccountId type
pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

#[cfg_attr(feature = "std", derive(Debug, PartialEq))]
#[derive(Encode, Decode)]
pub struct RingBufferItem<T: Config>(
    #[codec(compact)] <<T as Config>::Time as Time>::Moment,
    <T as Config>::Record,
);

impl<T: Config> Default for RingBufferItem<T> {
    fn default() -> Self {
        Self(Default::default(), Default::default())
    }
}

#[cfg(test)]
impl<T: Config> RingBufferItem<T> {
    fn new(now: <<T as Config>::Time as Time>::Moment, record: <T as Config>::Record) -> Self {
        Self(now, record)
    }
}

impl<T: Config> RingBufferItem<T> {
    fn into(self) -> (<<T as Config>::Time as Time>::Moment, <T as Config>::Record) {
        (self.0, self.1)
    }
}

#[cfg_attr(feature = "std", derive(Debug, PartialEq))]
#[derive(Encode, Decode, Default)]
pub struct RingBufferIndex {
    #[codec(compact)]
    start: u64,
    #[codec(compact)]
    end: u64,
}

impl RingBufferIndex {
    #[inline]
    fn next(val: &mut u64, max: u64) {
        *val += 1;
        if *val == max {
            *val = 0
        }
    }

    pub fn add(&mut self, max: u64) -> u64 {
        let v = self.end;
        Self::next(&mut self.end, max);
        if self.start == self.end {
            Self::next(&mut self.start, max);
        }
        v
    }

    fn iter(&mut self, max: u64) -> RingBufferIterator<'_> {
        RingBufferIterator { inner: self, max }
    }
}

struct RingBufferIterator<'a> {
    inner: &'a mut RingBufferIndex,
    max: u64,
}

impl Iterator for RingBufferIterator<'_> {
    type Item = u64;
    fn next(&mut self) -> Option<Self::Item> {
        if self.inner.end == self.inner.start {
            None
        } else {
            let u = self.inner.start;
            RingBufferIndex::next(&mut self.inner.start, self.max);
            Some(u)
        }
    }
}

/// Datalog module main trait.
pub trait Config: frame_system::Config {
    /// Timestamp source.
    type Time: Time;
    /// Datalog record data type.
    type Record: Codec + EncodeLike + Member + Default;
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    /// log window
    type WindowSize: Get<u64>;
    /// maximum record length
    type MaximumMessageSize: Get<usize>;
    /// extrinsic weights
    type WeightInfo: WeightInfo;
}
decl_error! {
    pub enum Error for Module<T: Config> {
        /// Potentially dangerous action
        RecordTooBig,
    }
}

decl_event! {
    pub enum Event<T>
    where AccountId = <T as frame_system::Config>::AccountId,
          Moment = MomentOf<T>,
          Record = <T as Config>::Record,
    {
        /// New data added.
        NewRecord(AccountId, Moment, Record),
        /// Account datalog erased.
        Erased(AccountId),
        /// Record sender to another location.
        RecordSent(AccountId),
    }
}

decl_storage! {
    trait Store for Module<T: Config> as Datalog {
        /// Time tagged data of given account (old values).
        Datalog get(fn datalog): map hasher(blake2_128_concat)
                                 T::AccountId => Vec<(MomentOf<T>, T::Record)>;
        /// Ringbuffer start/end pointers
        DatalogIndex get(fn datalogidx): map hasher(twox_64_concat)
                                 T::AccountId => RingBufferIndex;
        /// Ringbuffer items
        DatalogItem get(fn datalogitem): map hasher(twox_64_concat)
                                 (T::AccountId, u64) => RingBufferItem::<T>;
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        /// Store new data into blockchain.
        #[weight = <T as Config>::WeightInfo::record()]
        fn record(origin, record: T::Record) {
            ensure!(record.size_hint() <= T::MaximumMessageSize::get(), Error::<T>::RecordTooBig );
            let sender = ensure_signed(origin)?;

            // remove previous version
            Datalog::<T>::remove(&sender);
            let now = T::Time::now();
            let item = RingBufferItem( now, record );

            DatalogIndex::<T>::mutate(&sender, |idx|{
                let window_size = T::WindowSize::get();
                let end = idx.add( window_size );

                DatalogItem::<T>::insert( (&sender, end), &item )
            });

            let ( now, record ) = item.into();
            Self::deposit_event(RawEvent::NewRecord( sender, now, record ));
        }

        /// Clear account`s datalog.
        #[weight = <T as Config>::WeightInfo::erase(T::WindowSize::get())]
        fn erase(origin) {
            let sender = ensure_signed(origin)?;
            Datalog::<T>::remove(&sender);

            let mut idx = DatalogIndex::<T>::take(&sender);
            let window_size = T::WindowSize::get();
            for start in idx.iter(window_size){
                DatalogItem::<T>::remove( (&sender, start) )
            }

            Self::deposit_event(RawEvent::Erased(sender));
        }
    }
}

impl<T: Config> Module<T> {
    pub fn data(account: &T::AccountId) -> Vec<RingBufferItem<T>> {
        let mut idx = DatalogIndex::<T>::get(&account);
        let window_size = T::WindowSize::get();

        idx.iter(window_size)
            .map(|i| DatalogItem::<T>::get((&account, i)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::from_over_into)]
    use crate::{self as datalog, *};

    use base58::FromBase58;
    use frame_support::sp_runtime::{
        testing::Header, traits::BlakeTwo256, traits::IdentityLookup, DispatchError,
    };
    use frame_support::{assert_err, assert_ok, parameter_types};
    use node_primitives::Moment;
    use sp_core::H256;

    type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
    type RuntimeError = Error<Runtime>;
    type Block = frame_system::mocking::MockBlock<Runtime>;
    type Item = RingBufferItem<Runtime>;

    frame_support::construct_runtime!(
        pub enum Runtime where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system::{Module, Call, Config, Storage, Event<T>},
            Timestamp: pallet_timestamp::{Module, Storage},
            Datalog: datalog::{Module, Call, Storage, Event<T>},
        }
    );

    parameter_types! {
        pub const BlockHashCount: u64 = 2400;
    }

    impl frame_system::Config for Runtime {
        type Origin = Origin;
        type Index = u64;
        type BlockNumber = u64;
        type Call = Call;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type Event = Event;
        type BlockHashCount = BlockHashCount;
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = ();
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type DbWeight = ();
        type BaseCallFilter = ();
        type SystemWeightInfo = ();
        type BlockWeights = ();
        type BlockLength = ();
        type SS58Prefix = ();
    }

    parameter_types! {
        pub const MinimumPeriod: Moment = 5;
    }

    impl pallet_timestamp::Config for Runtime {
        type Moment = Moment;
        type OnTimestampSet = ();
        type MinimumPeriod = ();
        type WeightInfo = ();
    }

    const WINDOW: u64 = 20;
    parameter_types! {
        pub const WindowSize: u64 = WINDOW;
        pub const MaximumMessageSize: usize = 512;
    }

    impl Config for Runtime {
        type Time = Timestamp;
        type Record = Vec<u8>;
        type Event = Event;
        type WindowSize = WindowSize;
        type MaximumMessageSize = MaximumMessageSize;
        type WeightInfo = ();
    }

    fn new_test_ext() -> frame_support::sp_io::TestExternalities {
        let storage = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();
        storage.into()
    }

    #[test]
    fn test_ringbuffer_index() {
        let mut idx: RingBufferIndex = Default::default();
        assert!(idx.start == idx.end);
        assert!(idx.start == 0);

        let i = idx.add(20);
        assert!(i == 0);
        assert!(idx.end == 1);
    }

    #[test]
    fn test_store_data() {
        new_test_ext().execute_with(|| {
            let sender = 1;
            let record = b"datalog".to_vec();
            assert_ok!(Datalog::record(Origin::signed(sender), record.clone()));
            assert_eq!(Datalog::data(&sender), vec![Item::new(0, record)]);
        })
    }

    #[test]
    fn test_recycle_data() {
        new_test_ext().execute_with(|| {
            let sender = 1;

            for i in 0..(WINDOW + 10) {
                assert_ok!(Datalog::record(
                    Origin::signed(sender),
                    i.to_be_bytes().to_vec()
                ));
            }

            let data: Vec<_> = (11..(WINDOW + 10))
                .map(|i| Item::new(0, i.to_be_bytes().to_vec()))
                .collect();

            assert_eq!(Datalog::data(&sender), data);
            assert_eq!(
                Datalog::datalogidx(&sender),
                RingBufferIndex { start: 11, end: 10 }
            );
        })
    }

    #[test]
    fn test_erase_data() {
        new_test_ext().execute_with(|| {
            let sender = 1;
            let record = b"datalog".to_vec();
            assert_ok!(Datalog::record(Origin::signed(sender), record.clone()));
            // old log should be empty
            assert_eq!(Datalog::datalog(sender), vec![]);
            assert_eq!(Datalog::data(&sender), vec![Item::new(0, record)]);
            assert_eq!(
                Datalog::datalogidx(&sender),
                RingBufferIndex { start: 0, end: 1 }
            );

            assert_ok!(Datalog::erase(Origin::signed(sender)));
            assert_eq!(Datalog::data(&sender), vec![]);

            assert_eq!(
                Datalog::datalogidx(&sender),
                RingBufferIndex { start: 0, end: 0 }
            );
        })
    }

    #[test]
    fn test_bad_origin() {
        new_test_ext().execute_with(|| {
            assert_err!(
                Datalog::record(Origin::none(), vec![]),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn test_big_record() {
        new_test_ext().execute_with(|| {
            assert_err!(
                Datalog::record(Origin::none(), vec![0; 600]),
                RuntimeError::RecordTooBig
            );
        })
    }

    fn hash2vec(ss58hash: &str) -> Vec<u8> {
        ss58hash.from_base58().unwrap()
    }

    #[test]
    fn test_store_ipfs_hashes() {
        new_test_ext().execute_with(|| {
            let sender = 1;
            let record = hash2vec("QmWboFP8XeBtFMbNYK3Ne8Z3gKFBSR5iQzkKgeNgQz3dz4");

            assert_ok!(Datalog::record(Origin::signed(sender), record.clone()));
            assert_eq!(Datalog::data(&sender), vec![Item::new(0, record.clone())]);

            let record2 = hash2vec("zdj7WWYAEceQ6ncfPZeRFjozov4dC7FaxU7SuMwzW4VuYBDta");

            Timestamp::set_timestamp(100);
            assert_ok!(Datalog::record(Origin::signed(sender), record2.clone()));
            assert_eq!(
                Datalog::data(&sender),
                vec![
                    Item::new(0, record.clone()),
                    Item::new(100, record2.clone()),
                ]
            );
            let record3 = hash2vec("QmWboFP8XeBtFMbNYK3Ne8Z3gKFBSR5iQzkKgeNgQz3dz2");

            Timestamp::set_timestamp(200);
            assert_ok!(Datalog::record(Origin::signed(sender), record3.clone()));
            assert_eq!(
                Datalog::data(&sender),
                vec![
                    Item::new(0, record),
                    Item::new(100, record2),
                    Item::new(200, record3),
                ]
            );
        })
    }
}
