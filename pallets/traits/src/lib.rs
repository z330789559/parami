#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use impl_trait_for_tuples::impl_for_tuples;
use sp_runtime::{traits::AccountIdConversion, DispatchResult, RuntimeDebug, TypeId};
use sp_std::{
    cmp::{Eq, PartialEq},
    prelude::Vec,
};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub use currency::{
    Balance, BalanceStatus, BasicCurrency, BasicCurrencyExtended, BasicLockableCurrency,
    BasicReservableCurrency, CurrencyId, LockIdentifier, MultiCurrency, MultiCurrencyExtended,
    MultiLockableCurrency, MultiReservableCurrency, OnDust,
};

pub use get_by_key::*;

pub mod arithmetic;
pub mod currency;
pub mod get_by_key;

/// A pallet identifier. These are per pallet and should be stored in a registry somewhere.
#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode)]
pub struct PalletId(pub [u8; 8]);

impl TypeId for PalletId {
    const TYPE_ID: [u8; 4] = *b"modl";
}
/// Input that adds infinite number of zero after wrapped input.
// pub struct TrailingZeroInput<'a>(&'a [u8]);
//
// impl<'a> TrailingZeroInput<'a> {
// 	/// Create a new instance from the given byte array.
// 	pub fn new(data: &'a [u8]) -> Self {
// 		Self(data)
// 	}
// }
//
// impl<'a> codec::Input for TrailingZeroInput<'a> {
// 	fn remaining_len(&mut self) -> Result<Option<usize>, codec::Error> {
// 		Ok(None)
// 	}
//
// 	fn read(&mut self, into: &mut [u8]) -> Result<(), codec::Error> {
// 		let len_from_inner = into.len().min(self.0.len());
// 		into[..len_from_inner].copy_from_slice(&self.0[..len_from_inner]);
// 		for i in &mut into[len_from_inner..] {
// 			*i = 0;
// 		}
// 		self.0 = &self.0[len_from_inner..];
//
// 		Ok(())
// 	}
// }

/// Format is TYPE_ID ++ encode(parachain ID) ++ 00.... where 00... is indefinite trailing zeroes to
/// fill AccountId.
// impl<T: Encode + Decode + Default> AccountIdConversion<T> for PalletId {
// 	fn into_sub_account<S: Encode>(&self, sub: S) -> T {
// 		(<PalletId as LocalTypeId >::TYPE_ID, self, sub).using_encoded(|b|
// 			T::decode(&mut TrailingZeroInput(b))
// 		).unwrap_or_default()
// 	}
//
// 	fn try_from_sub_account<S: Decode>(x: &T) -> Option<(Self, S)> {
// 		x.using_encoded(|d| {
// 			if &d[0..4] != <PalletId as LocalTypeId >::TYPE_ID { return None }
// 			let mut cursor = &d[4..];
// 			let result = Decode::decode(&mut cursor).ok()?;
// 			if cursor.iter().all(|x| *x == 0) {
// 				Some(result)
// 			} else {
// 				None
// 			}
// 		})
// 	}
// }

#[derive(Eq, PartialEq, Clone, Copy, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TokenError {
    SymbolDuplicateError,
    Other(
        #[codec(skip)]
        #[cfg_attr(feature = "std", serde(skip_deserializing))]
        &'static str,
    ),
}

impl From<&'static str> for TokenError {
    fn from(err: &'static str) -> TokenError {
        TokenError::Other(err)
    }
}

impl From<TokenError> for &'static str {
    fn from(err: TokenError) -> &'static str {
        match err {
            TokenError::Other(msg) => msg,
            TokenError::SymbolDuplicateError => "SymbolDuplicate",
        }
    }
}

pub type TokenResult<T> = Result<T, TokenError>;

/// New data handler
#[impl_trait_for_tuples::impl_for_tuples(30)]
pub trait OnNewData<AccountId, Key, Value> {
    /// New data is available
    fn on_new_data(who: &AccountId, key: &Key, value: &Value);
}

/// Combine data provided by operators
pub trait CombineData<Key, TimestampedValue> {
    /// Combine data provided by operators
    fn combine_data(
        key: &Key,
        values: Vec<TimestampedValue>,
        prev_value: Option<TimestampedValue>,
    ) -> Option<TimestampedValue>;
}

/// Indicate if should change a value
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub enum Change<Value> {
    /// No change.
    NoChange,
    /// Changed to new value.
    NewValue(Value),
}

#[derive(Encode, Decode, RuntimeDebug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TimestampedValue<Value: Ord + PartialOrd, Moment> {
    pub value: Value,
    pub timestamp: Moment,
}

#[impl_for_tuples(30)]
pub trait Happened<T> {
    fn happened(t: &T);
}

pub trait Handler<T> {
    fn handle(t: &T) -> DispatchResult;
}

#[impl_for_tuples(30)]
impl<T> Handler<T> for Tuple {
    fn handle(t: &T) -> DispatchResult {
        for_tuples!( #( Tuple::handle(t); )* );
        Ok(())
    }
}

pub trait Contains<T> {
    fn contains(t: &T) -> bool;
}
