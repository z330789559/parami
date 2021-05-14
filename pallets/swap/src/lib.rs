#![allow(clippy::unused_unit)]

use codec::{FullCodec, Codec, Encode, Decode, EncodeLike};
use sp_runtime::{
	traits::{
		Member, One, Zero, AtLeast32Bit, MaybeSerializeDeserialize, CheckedAdd,CheckedSub,Saturating,AtLeast32BitUnsigned,Bounded,
		AccountIdConversion, SaturatedConversion,StaticLookup
	},
	DispatchError, DispatchResult, RuntimeDebug,TypeId,AccountId32
};
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{
		BalanceStatus as Status, Currency as PalletCurrency, ExistenceRequirement, Get, Imbalance,
		LockableCurrency as PalletLockableCurrency, ReservableCurrency as PalletReservableCurrency, SignedImbalance,
		WithdrawReasons,
	},
	transactional,
};
use frame_system::{self as system, ensure_signed, pallet_prelude::*};
use sp_std::{
	convert::{Infallible, TryFrom, TryInto},
	marker,
	prelude::*,
	vec::Vec,
	 str,
	collections::btree_map::BTreeMap,
	 result, marker::PhantomData, ops::Div, fmt::Debug
};

use parami_traits::{
       MultiCurrency, MultiCurrencyExtended, MultiLockableCurrency,
       Balance, CurrencyId,PalletId
};

mod errors;
use errors::{ConvertError, ConvertError::*};
pub const MODULE_ID: PalletId = PalletId(*b"paraswap");

pub use swap_module::*;

#[derive(Clone, Eq, PartialEq, Encode, Decode,Default)]
pub struct Swap<AccountId, AssetId> {
	// The token being swapped.
	token_id: AssetId,
	// The "swap token" id.
	swap_token: AssetId,
	// This swap account.
	account: AccountId,
}

type BalanceOf<T> = <<T as Config>::Tokens as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;

type CurrencyIdOf<T> = <<T as Config>::Tokens as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;

type CurrencyBalance<T> = <<T as Config>::Currency as PalletCurrency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
// NOTE: The name of the pallet is provided by `construct_runtime` and is used as
// the unique identifier for the pallet's storage. It is not defined in the pallet itself.
pub mod swap_module {


    use super::*;

	// Define the generic parameter of the pallet
	// The macro parses `#[pallet::constant]` attributes and uses them to generate metadata
	// for the pallet's constants.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;


	/// The balance of an account
		type Currency:  PalletCurrency<Self::AccountId>;

		type Tokens: MultiCurrency<Self::AccountId>;

	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config>
	{
		/// Logs (CurrencyId, SwapAccount)
		SwapCreated(CurrencyIdOf<T>, T::AccountId),
		/// Logs (CurrencyId, x, x, x)
		LiquidityAdded(CurrencyIdOf<T>, T::AccountId, BalanceOf<T>, BalanceOf<T>),
	   /// Logs (CurrencyId, x, x, x)
		LiquidityRemoved(CurrencyIdOf<T>, T::AccountId, BalanceOf<T>, BalanceOf<T>),
		/// Logs (CurrencyId, buyer, currency_bought, tokens_sold, recipient)
		CurrencyPurchase(),
		/// Logs (CurrencyId, buyer, currency_sold, tokens_bought, recipient)
		TokenPurchase(),
	}

	// Define the pallet struct placeholder, various pallet function are implemented on it.
	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// The currentId of this next token.
	#[pallet::storage]
	#[pallet::getter(fn swap_count)]
	pub type SwapCount<T: Config> = StorageValue<_, CurrencyIdOf<T>, ValueQuery>;



	/// The total issuance of a token type.
	#[pallet::storage]
	#[pallet::getter(fn token_to_swap)]
	pub type TokenToSwap<T: Config> = StorageMap<_, Twox64Concat, CurrencyIdOf<T>, CurrencyIdOf<T>, ValueQuery>;


	/// The total issuance of a token type.
	#[pallet::storage]
	#[pallet::getter(fn swaps_house)]
	pub type SwapsHouse<T: Config> = StorageMap<_, Twox64Concat,  CurrencyIdOf<T>, Option<Swap<T::AccountId, CurrencyIdOf<T>>>, ValueQuery>;




	// Implement the pallet hooks.
	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			unimplemented!();
		}

		// can implement also: on_finalize, on_runtime_upgrade, offchain_worker, ...
		// see `Hooks` trait
	}

	// Declare Call struct and implement dispatchables.
	//
	// WARNING: Each parameter used in functions must implement: Clone, Debug, Eq, PartialEq,
	// Codec.
	//
	// The macro parses `#[pallet::compact]` attributes on function arguments and implements
	// the `Call` encoding/decoding accordingly.
	#[pallet::call]
	impl<T: Config> Pallet<T> {

		#[pallet::weight(0)]
		pub fn create_swap(origin:OriginFor<T>,
			token_id: CurrencyIdOf<T>,
			symbol: Vec<u8>,
			big_decimal: u32
		) -> DispatchResultWithPostInfo
		{
			let sender = ensure_signed(origin)?;
			ensure!(!TokenToSwap::<T>::contains_key(token_id), Error::<T>::SwapAlreadyExists);

			let swap_id = Self::swap_count();
			let next_id = swap_id.checked_add(&One::one())
				.ok_or("Overflow")?;

			let swap_token_id  = T::Tokens::create_token(&sender, Zero::zero(),&symbol,big_decimal).map_err(|_|Error::<T>::SwapAlreadyExists)?;

			let account: T::AccountId = MODULE_ID.into_sub_account(swap_token_id);

			let new_swap = Swap {
				token_id: token_id,
				swap_token: swap_token_id,
				account: account.clone(),
			};

			<TokenToSwap<T>>::insert(token_id, swap_id);
			<SwapsHouse<T>>::insert(swap_id, Some(new_swap));
			<SwapCount<T>>::put(next_id);

			Self::deposit_event(Event::SwapCreated(swap_id, account));

			Ok(().into())
		}

		#[pallet::weight(0)]
        pub fn add_liquidity(origin:OriginFor<T>,
			swap_id: CurrencyIdOf<T>,				// ID of swap to access.
			currency_amount: BalanceOf<T>,  // Amount of base currency to lock.
            min_liquidity: BalanceOf<T>,	// Min amount of swap shares to create.
			max_tokens: BalanceOf<T>,	// Max amount of tokens to input.
            deadline: T::BlockNumber,		// When to invalidate the transaction.
        ) -> DispatchResultWithPostInfo
        {
			// Deadline is to prevent front-running (more of a problem on Ethereum).
			let now = frame_system::Module::<T>::block_number();
			ensure!(deadline > now, Error::<T>::Deadline);

			let who = ensure_signed(origin.clone())?;

			ensure!(max_tokens > Zero::zero(), Error::<T>::ZeroTokens);
			ensure!(currency_amount > Zero::zero(), Error::<T>::ZeroAmount);

			if let Some(swap) = Self::swaps_house(swap_id) {
				let total_liquidity = T::Tokens::total_issuance(swap.swap_token.clone());

				if total_liquidity > Zero::zero() {
					ensure!(min_liquidity > Zero::zero(), Error::<T>::RequestedZeroLiquidity);
					let swap_balance = Self::unconvert(Self::get_swap_balance(&swap));
					let token_reserve = Self::get_token_reserve(&swap);
					let token_amount = currency_amount * token_reserve / swap_balance;
					let liquidity_minted = currency_amount * total_liquidity / swap_balance;

					ensure!(max_tokens >= token_amount, Error::<T>::TooManyTokens);
					ensure!(liquidity_minted >= min_liquidity, Error::<T>::TooLowLiquidity);

					T::Currency::transfer(&who, &swap.account, Self::convert(currency_amount), ExistenceRequirement::KeepAlive)?;
					T::Tokens::deposit(swap.swap_token, &who, liquidity_minted)?;
					T::Tokens::transfer(swap.token_id, &who, &swap.account, token_amount)?;
					Self::deposit_event(Event::LiquidityAdded(swap_id, who.clone(), currency_amount.clone(), token_amount));
				} else {
					// Fresh swap with no liquidity ~
					let token_amount = max_tokens;
					let this = swap.account.clone();
					T::Currency::transfer(&who, &swap.account, Self::convert(currency_amount), ExistenceRequirement::KeepAlive)?;
					let initial_liquidity = T::Currency::free_balance(&this).saturated_into::<u64>();
					T::Tokens::deposit(swap.swap_token.clone(), &who, initial_liquidity.into())?;
					T::Tokens::transfer(swap.token_id, &who, &this, token_amount)?;
					Self::deposit_event(Event::LiquidityAdded(swap_id, who, currency_amount, token_amount));
				}

				Ok(().into())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}

		#[pallet::weight(0)]
		pub fn remove_liquidity(origin:OriginFor<T>,
			swap_id: CurrencyIdOf<T>,
			shares_to_burn: BalanceOf<T>,
			min_currency: BalanceOf<T>,		// Minimum currency to withdraw.
			min_tokens: BalanceOf<T>,	// Minimum tokens to withdraw.
			deadline: T::BlockNumber,
		) -> DispatchResultWithPostInfo
		{
			let now = frame_system::Module::<T>::block_number();
			ensure!(deadline > now, Error::<T>::Deadline);

			let who = ensure_signed(origin.clone())?;

			ensure!(shares_to_burn > Zero::zero(), Error::<T>::BurnZeroShares);

			if let Some(swap) = Self::swaps_house(swap_id) {
				let total_liquidity = T::Tokens::total_issuance(swap.swap_token);

				ensure!(total_liquidity > Zero::zero(), Error::<T>::NoLiquidity);

				let token_reserve = Self::get_token_reserve(&swap);
				let swap_balance = Self::get_swap_balance(&swap);
				let currency_amount = shares_to_burn.clone() * Self::unconvert(swap_balance) / total_liquidity.clone();
				let token_amount = shares_to_burn.clone() * token_reserve / total_liquidity.clone();

				ensure!(currency_amount >= min_currency, Error::<T>::NotEnoughCurrency);
				ensure!(token_amount >= min_tokens, Error::<T>::NotEnoughTokens);

				T::Tokens::withdraw(swap.swap_token, &who, shares_to_burn)?;

				T::Currency::transfer(&swap.account, &who, Self::convert(currency_amount), ExistenceRequirement::AllowDeath)?;
				// Need to ensure this happens.
				T::Tokens::transfer(swap.token_id, &swap.account, &who, token_amount.clone())?;
				Self::deposit_event(Event::LiquidityRemoved(swap_id, who, currency_amount, token_amount));

				Ok(().into())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}

		/// Converts currency to tokens.
		///
		/// User specifies the exact amount of currency to spend and the minimum
		/// tokens to be returned.
		#[pallet::weight(0)]
		pub fn currency_to_tokens_input(origin:OriginFor<T>,
			swap_id: CurrencyIdOf<T>,
			currency: BalanceOf<T>,
			min_tokens: BalanceOf<T>,
			deadline: T::BlockNumber,
			recipient: T::AccountId,
		) ->  DispatchResultWithPostInfo
		{
			let now = frame_system::Module::<T>::block_number();
			ensure!(deadline > now, Error::<T>::Deadline);

			let buyer = ensure_signed(origin)?;

			ensure!(currency > Zero::zero(), Error::<T>::NoCurrencySwapped);
			ensure!(min_tokens > Zero::zero(), Error::<T>::NoTokensSwapped);

			if let Some(swap) = Self::swaps_house(swap_id) {
				let token_reserve = Self::get_token_reserve(&swap);
				let swap_balance = Self::get_swap_balance(&swap);
				let tokens_bought = Self::get_input_price(currency, Self::unconvert(swap_balance), token_reserve);
				ensure!(tokens_bought >= min_tokens, Error::<T>::NotEnoughTokens);
				T::Currency::transfer(&buyer, &swap.account, Self::convert(currency), ExistenceRequirement::KeepAlive)?;
				T::Tokens::transfer(swap.token_id, &swap.account, &recipient, tokens_bought)?;

				Self::deposit_event(Event::TokenPurchase());

				Ok(().into())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}

		/// Converts currency to tokens.
		///
		/// User specifies the maximum currency to spend and the exact amount of
		/// tokens to be returned.
		#[pallet::weight(0)]
		pub fn currency_to_tokens_output(origin: OriginFor<T>,
			swap_id: CurrencyIdOf<T>,
			tokens_bought: BalanceOf<T>,
			max_currency: BalanceOf<T>,
			deadline: T::BlockNumber,
			recipient: T::AccountId,
		) ->  DispatchResultWithPostInfo
		{
			let now = frame_system::Module::<T>::block_number();
			ensure!(deadline >= now, Error::<T>::Deadline);

			let buyer = ensure_signed(origin)?;

			ensure!(tokens_bought > Zero::zero(), Error::<T>::NoTokensSwapped);
			ensure!(max_currency > Zero::zero(), Error::<T>::NoCurrencySwapped);

			if let Some(swap) = Self::swaps_house(swap_id) {
				let token_reserve = Self::get_token_reserve(&swap);
				let swap_balance = Self::get_swap_balance(&swap);
				let currency_sold = Self::get_output_price(tokens_bought, Self::unconvert(swap_balance), token_reserve);

				ensure!(currency_sold <= max_currency, Error::<T>::TooExpensiveCurrency);

				T::Currency::transfer(&buyer, &swap.account, Self::convert(currency_sold), ExistenceRequirement::KeepAlive)?;
				T::Tokens::transfer(swap.token_id, &swap.account, &recipient, tokens_bought)?;

				Self::deposit_event(Event::TokenPurchase());

				Ok(().into())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}

		/// Converts tokens to currency.
		///
		/// The user specifies exact amount of tokens sold and minimum amount of
		/// currency that is returned.
		#[pallet::weight(0)]
		pub fn tokens_to_currency_input(origin:OriginFor<T>,
			swap_id: CurrencyIdOf<T>,
			tokens_sold: BalanceOf<T>,
			min_currency: BalanceOf<T>,
			deadline: T:: BlockNumber,
			recipient: T::AccountId,
		) ->  DispatchResultWithPostInfo
		{
			let now = frame_system::Module::<T>::block_number();
			ensure!(deadline >= now, Error::<T>::Deadline);

			let buyer = ensure_signed(origin)?;

			ensure!(tokens_sold > Zero::zero(), Error::<T>::NoTokensSwapped);
			ensure!(min_currency > Zero::zero(), Error::<T>::NoCurrencySwapped);

			if let Some(swap) = Self::swaps_house(swap_id) {
				let token_reserve = Self::get_token_reserve(&swap);
				let swap_balance = Self::get_swap_balance(&swap);
				let currency_bought = Self::get_input_price(tokens_sold, token_reserve, Self::unconvert(swap_balance));

				ensure!(currency_bought >= min_currency, Error::<T>::NotEnoughCurrency);

				T::Currency::transfer(&swap.account, &recipient, Self::convert(currency_bought), ExistenceRequirement::AllowDeath)?;
				T::Tokens::transfer(swap.token_id, &buyer, &swap.account, tokens_sold)?;

				Self::deposit_event(Event::CurrencyPurchase());

				Ok(().into())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}

		/// Converts tokens to currency.
		///
		/// The user specifies the maximum tokens to swap and the exact
		/// currency to be returned.
		#[pallet::weight(0)]
		pub fn tokens_to_currency_output(origin:OriginFor<T>,
			swap_id:  CurrencyIdOf<T>,
			currency_bought: BalanceOf<T>,
			max_tokens: BalanceOf<T>,
			deadline: T::BlockNumber,
			recipient: T::AccountId,
		) ->  DispatchResultWithPostInfo
		{
			let now = frame_system::Module::<T>::block_number();
			ensure!(deadline >= now, Error::<T>::Deadline);

			let buyer = ensure_signed(origin)?;

			ensure!(max_tokens > Zero::zero(), Error::<T>::NoTokensSwapped);
			ensure!(currency_bought > Zero::zero(), Error::<T>::NoCurrencySwapped);

			if let Some(swap) = Self::swaps_house(swap_id) {
				let token_reserve = Self::get_token_reserve(&swap);
				let swap_balance = Self::get_swap_balance(&swap);
				let tokens_sold = Self::get_output_price(currency_bought, token_reserve, Self::unconvert(swap_balance));

				ensure!(max_tokens >= tokens_sold, Error::<T>::TooExpensiveTokens);

				T::Currency::transfer(&swap.account, &buyer, Self::convert(currency_bought), ExistenceRequirement::AllowDeath)?;
				T::Tokens::transfer(swap.token_id, &recipient, &swap.account, tokens_sold)?;

				Self::deposit_event(Event::CurrencyPurchase());

				Ok(().into())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}

	}



	impl<T:Config> Pallet<T>{
		pub fn get_currency_to_token_input_price(swap: &Swap<T::AccountId, CurrencyIdOf<T>>, currency_sold: BalanceOf<T>)
		-> BalanceOf<T>
	{
		if currency_sold == Zero::zero() { return Zero::zero(); }

		let token_reserve = Self::get_token_reserve(swap);
		let swap_balance = Self::get_swap_balance(swap);
		Self::get_input_price(currency_sold, Self::unconvert(swap_balance), token_reserve)
	}

	// pub fn get_currency_to_token_output_price(swap: &Swap<T::AccountId, CurrencyIdOf<T>>, tokens_bought: T::TokenBalance)
	// 	-> T::TokenBalance
	// {

	// }

	// pub fn get_token_to_currency_input_price(swap: &Swap<T::AccountId, CurrencyIdOf<T>>, tokens_sold: T::TokenBalance)
	// 	-> T::TokenBalance
	// {

	// }

	// pub fn get_token_to_currency_output_price(swap: &Swap<T::AccountId, CurrencyIdOf<T>>, currency_bought: BalanceOf<T>)
	// 	-> T::TokenBalance
	// {

	// }

	fn get_output_price(
		output_amount: BalanceOf<T>,
		input_reserve: BalanceOf<T>,
		output_reserve: BalanceOf<T>,
	) -> BalanceOf<T>
	{

		let numerator = input_reserve * output_amount * Self::convert_balance(1000u128);
		let denominator = (output_reserve - output_amount) * Self::convert_balance(1000u128);
		numerator / denominator + Self::convert_balance(1u128)
	}

	fn get_input_price(
		input_amount: BalanceOf<T>,
		input_reserve: BalanceOf<T>,
		output_reserve: BalanceOf<T>,
	) -> BalanceOf<T>
	{
		let input_amount_with_fee = input_amount * Self::convert_balance(1000u128);
		let numerator = input_amount_with_fee * output_reserve;
		let denominator = (input_reserve * Self::convert_balance(1000u128)) + input_amount_with_fee;
		numerator / denominator
	}

	fn convert(balance_of: BalanceOf<T>) -> CurrencyBalance<T> {
		let m = balance_of.saturated_into::<u128>();
		m.saturated_into()
	}
	fn convert_balance(balance: u128)->BalanceOf<T>{
		balance.saturated_into()
	}

	fn unconvert(token_balance: CurrencyBalance<T>) -> BalanceOf<T> {
		let m = token_balance.saturated_into::<u128>();
		m.saturated_into()
	}

	fn get_token_reserve(swap: &Swap<T::AccountId, CurrencyIdOf<T>>) -> BalanceOf<T> {
		T::Tokens::total_balance(swap.token_id, &swap.account)
	}

	fn get_swap_balance(swap: &Swap<T::AccountId, CurrencyIdOf<T>>) -> CurrencyBalance<T> {
		T::Currency::free_balance(&swap.account)
	 }
	}

	// Declare the pallet `Error` enum (this is optional).
	// The macro generates error metadata using the doc comment on each variant.
	#[pallet::error]
	pub enum Error<T> {
		/// doc comment put into metadata
	/// Deadline hit.
		Deadline,
		/// Zero tokens supplied.
		ZeroTokens,
		/// Zero reserve supplied.
		ZeroAmount,
		/// No Swap exists at this Id.
		NoSwapExists,
		/// A Swap already exists for a particular AssetId.
		SwapAlreadyExists,
		/// Requested zero liquidity.
		RequestedZeroLiquidity,
		/// Would add too many tokens to liquidity.
		TooManyTokens,
		/// Not enough liquidity created.
		TooLowLiquidity,
		/// No currency is being swapped.
		NoCurrencySwapped,
		/// No tokens are being swapped.
		NoTokensSwapped,
		/// Trying to burn zero shares.
		BurnZeroShares,
		/// No liquidity in the swap.
		NoLiquidity,
		/// Not enough currency will be returned.
		NotEnoughCurrency,
		/// Not enough tokens will be returned.
		NotEnoughTokens,
		/// Swap would cost too much in currency.
		TooExpensiveCurrency,
		/// Swap would cost too much in tokens.
		TooExpensiveTokens,
	}

	// Declare pallet Event enum (this is optional).
	//
	// WARNING: Each type used in variants must implement: Clone, Debug, Eq, PartialEq, Codec.
	//
	// The macro generates event metadata, and derive Clone, Debug, Eq, PartialEq and Codec
	// #[pallet::event]
	// // Additional argument to specify the metadata to use for given type.
	// #[pallet::metadata(T::AccountId= "AccountId")]
	// // Generate a funciton on Pallet to deposit an event.
	// #[pallet::generate_deposit(pub(super) fn deposit_event)]
	// pub enum Event<T: Config> {
	// 	/// Logs (CurrencyId, SwapAccount)
	// 	SwapCreated(CurrencyIdOf<T>, T::AccountId),
	// 	/// Logs (CurrencyId, x, x, x)
	// 	LiquidityAdded(CurrencyIdOf<T>,  T::AccountId, BalanceOf<T>,  BalanceOf<T> ),
	// 	/// Logs (CurrencyId, x, x, x)
	// 	LiquidityRemoved(CurrencyIdOf<T>,  T::AccountId, BalanceOf<T>,  BalanceOf<T>),
	// 	/// Logs (CurrencyId, buyer, currency_bought, tokens_sold, recipient)
	// 	CurrencyPurchase(),
	// 	/// Logs (CurrencyId, buyer, currency_sold, tokens_bought, recipient)
	// 	TokenPurchase(),
	// }


	// Declare the genesis config (optional).
	//
	// The macro accepts either a struct or an enum; it checks that generics are consistent.
	//
	// Type must implement the `Default` trait.
	// #[pallet::genesis_config]
	// #[derive(Default)]
	// pub struct GenesisConfig {
	// 	_myfield: u32,
	// }

	// Declare genesis builder. (This is need only if GenesisConfig is declared)
	// #[pallet::genesis_build]
	// impl<T: Config> GenesisBuild<T> for GenesisConfig {
	// 	fn build(&self) {}
	// }

	// Declare a pallet origin (this is optional).
	//
	// The macro accept type alias or struct or enum, it checks generics are consistent.
	// #[pallet::origin]
	// pub struct Origin<T>(PhantomData<T>);


	// Declare validate_unsigned implementation (this is optional).
	// #[pallet::validate_unsigned]
	// impl<T: Config> ValidateUnsigned for Pallet<T> {
	// 	type Call = Call<T>;
	// 	fn validate_unsigned(
	// 			source: TransactionSource,
	// 		call: &Self::Call
	// 	) -> TransactionValidity {
	// 		Err(TransactionValidityError::Invalid(InvalidTransaction::Call))
	// 	}
	// }

	// Declare inherent provider for pallet (this is optional).
	// #[pallet::inherent]
	// impl<T: Config> ProvideInherent for Pallet<T> {
	// 	type Call = Call<T>;
	// 	type Error = InherentError;
	//
	// 	const INHERENT_IDENTIFIER: InherentIdentifier = INHERENT_IDENTIFIER;
	//
	// 	fn create_inherent(_data: &InherentData) -> Option<Self::Call> {
	// 		unimplemented!();
	// 	}
	// }

	// Regular rust code needed for implementing ProvideInherent trait

	// #[derive(codec::Encode, sp_runtime::RuntimeDebug)]
	// #[cfg_attr(feature = "std", derive(codec::Decode))]
	// pub enum InherentError {
	// }
	//
	// impl sp_inherents::IsFatalError for InherentError {
	// 	fn is_fatal_error(&self) -> bool {
	// 		unimplemented!();
	// 	}
	// }
	//
	// pub const INHERENT_IDENTIFIER: sp_inherents::InherentIdentifier = *b"testpall";

	}




