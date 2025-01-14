// This file is part of Bit.Country.

// Copyright (C) 2020-2021 Bit.Country.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
    dispatch::{DispatchResultWithPostInfo, DispatchResult},
    decl_error, decl_event, decl_module, decl_storage, ensure, Parameter,
    pallet_prelude::*,
};
use frame_system::{self as system, ensure_signed};
use orml_traits::{
    account::MergeAccount,
    arithmetic::{Signed, SimpleArithmetic},
    BalanceStatus, BasicCurrency, BasicCurrencyExtended, BasicLockableCurrency, BasicReservableCurrency,
    LockIdentifier, MultiCurrency, MultiCurrencyExtended, MultiLockableCurrency, MultiReservableCurrency,
};
use primitives::{Balance, CountryId, CurrencyId, SocialTokenCurrencyId};
use sp_runtime::{
    traits::{AtLeast32Bit, One, StaticLookup, Zero, AccountIdConversion},
    DispatchError,
};
use sp_std::vec::Vec;
use frame_support::sp_runtime::ModuleId;
use bc_country::*;
use auction_manager::{SwapManager};
use frame_support::traits::{Get, Currency};
use frame_system::pallet_prelude::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// A wrapper for a token name.
pub type TokenName = Vec<u8>;

/// A wrapper for a ticker name.
pub type Ticker = Vec<u8>;

#[derive(Encode, Decode, Default, Clone, PartialEq)]
pub struct Token<Balance> {
    pub ticker: Ticker,
    pub total_supply: Balance,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use primitives::{SocialTokenCurrencyId, TokenId};
    use frame_support::sp_runtime::{SaturatedConversion, FixedPointNumber};
    use primitives::dex::Price;
    use frame_support::sp_runtime::traits::Saturating;

    #[pallet::pallet]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// The arithmetic type of asset identifier.
        type TokenId: Parameter + AtLeast32Bit + Default + Copy;
        type CountryCurrency: MultiCurrencyExtended<
            Self::AccountId,
            CurrencyId=SocialTokenCurrencyId,
            Balance=Balance,
        >;
        type SocialTokenTreasury: Get<ModuleId>;
        type CountryInfoSource: BCCountry<Self::AccountId>;
        type LiquidityPoolManager: SwapManager<Self::AccountId, SocialTokenCurrencyId, Balance>;
    }

    #[pallet::storage]
    #[pallet::getter(fn next_token_id)]
    /// The next asset identifier up for grabs.
    pub(super) type NextTokenId<T: Config> = StorageValue<_, TokenId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn token_details)]
    /// Details of the token corresponding to the token id.
    /// (hash) -> Token details [returns Token struct]
    pub(super) type SocialTokens<T: Config> =
    StorageMap<_, Blake2_128Concat, SocialTokenCurrencyId, Token<Balance>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn get_country_treasury)]
    /// Details of the token corresponding to the token id.
    /// (hash) -> Token details [returns Token struct]
    pub(super) type CountryTreasury<T: Config> =
    StorageMap<_, Blake2_128Concat, CountryId, CountryFund<T::AccountId, Balance>, OptionQuery>;

    #[pallet::error]
    pub enum Error<T> {
        /// Transfer amount should be non-zero
        AmountZero,
        /// Account balance must be greater than or equal to the transfer amount
        BalanceLow,
        /// Balance should be non-zero
        BalanceZero,
        ///Insufficient balance
        InsufficientBalance,
        /// No permission to issue token
        NoPermissionTokenIssuance,
        /// Country Currency already issued for this bitcountry
        SocialTokenAlreadyIssued,
        /// No available next token id
        NoAvailableTokenId,
        /// Country Fund Not Available
        CountryFundIsNotAvailable,
        /// Initial Social Token Supply is too low
        InitialSocialTokenSupplyIsTooLow,
        /// Failed on updating social token for this bitcountry
        FailedOnUpdateingSocialToken,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Issue a new class of fungible assets for bitcountry. There are, and will only ever be, `total`
        /// such assets and they'll all belong to the `origin` initially. It will have an
        /// identifier `TokenId` instance: this will be specified in the `Issued` event.
        #[pallet::weight(10_000)]
        pub fn mint_token(
            origin: OriginFor<T>,
            ticker: Ticker,
            country_id: CountryId,
            total_supply: Balance,
            initial_lp: (u32, u32),
            initial_backing: Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!(
                T::CountryInfoSource::check_ownership(&who, &country_id), 
                Error::<T>::NoPermissionTokenIssuance
            );
            ensure!(
                !CountryTreasury::<T>::contains_key(&country_id), 
                Error::<T>::SocialTokenAlreadyIssued
            );

            let initial_pool_numerator = total_supply.saturating_mul(initial_lp.0.saturated_into());
            let initial_pool_supply = initial_pool_numerator.checked_div(initial_lp.1.saturated_into()).unwrap_or(0);
            debug::info!("initial_pool_supply: {})", initial_pool_supply);
            let initial_supply_ratio = Price::checked_from_rational(initial_pool_supply, total_supply).unwrap_or_default();
            let supply_percent: u128 = initial_supply_ratio.saturating_mul_int(100.saturated_into());
            debug::info!("supply_percent: {})", supply_percent);
            ensure!(
                supply_percent > 0u128 && supply_percent >= 20u128,
                Error::<T>::InitialSocialTokenSupplyIsTooLow
            );

            let owner_supply = total_supply.saturating_sub(initial_pool_supply);
            debug::info!("owner_supply: {})", owner_supply);
            //Generate new TokenId
            let currency_id = NextTokenId::<T>::mutate(|id| -> Result<SocialTokenCurrencyId, DispatchError>{
                let current_id = *id;
                if current_id == 0 {
                    *id = 2;
                    Ok(SocialTokenCurrencyId::SocialToken(One::one()))
                } else {
                    *id = id.checked_add(One::one())
                        .ok_or(Error::<T>::NoAvailableTokenId)?;
                    Ok(SocialTokenCurrencyId::SocialToken(current_id))
                }
            })?;
            let fund_id: T::AccountId = T::SocialTokenTreasury::get().into_sub_account(country_id);

            //Country treasury
            let country_fund = CountryFund {
                vault: fund_id,
                value: total_supply,
                backing: initial_backing,
                currency_id: currency_id,
            };

            let token_info = Token {
                ticker,
                total_supply,
            };

            //Update currency id in BC
            T::CountryInfoSource::update_country_token(country_id.clone(), currency_id.clone())?;

            //Store social token info
            SocialTokens::<T>::insert(currency_id, token_info);

            CountryTreasury::<T>::insert(country_id, country_fund);
            T::CountryCurrency::deposit(currency_id, &who, total_supply)?;
            //Social currency should deposit to DEX pool instead, by calling provide LP function in DEX traits.
            T::LiquidityPoolManager::add_liquidity(&who, SocialTokenCurrencyId::NativeToken(0), currency_id, initial_backing, initial_pool_supply)?;
            let fund_address = Self::get_country_fund_id(country_id);
            Self::deposit_event(Event::<T>::SocialTokenIssued(currency_id.clone(), who, fund_address, total_supply, country_id));

            Ok(().into())
        }

        #[pallet::weight(10_000)]
        pub fn transfer(
            origin: OriginFor<T>,
            dest: <T::Lookup as StaticLookup>::Source,
            currency_id: SocialTokenCurrencyId,
            // #[compact] amount: Balance
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let from = ensure_signed(origin)?;
            let to = T::Lookup::lookup(dest)?;
            Self::transfer_from(currency_id, &from, &to, amount)?;

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    #[pallet::metadata(
    < T as frame_system::Config >::AccountId = "AccountId",
    Balance = "Balance",
    CurrencyId = "CurrencyId"
    )]
    pub enum Event<T: Config> {
        /// Some assets were issued. \[asset_id, owner, total_supply\]
        SocialTokenIssued(SocialTokenCurrencyId, T::AccountId, T::AccountId, u128, u64),
        /// Some assets were transferred. \[asset_id, from, to, amount\]
        SocialTokenTransferred(SocialTokenCurrencyId, T::AccountId, T::AccountId, Balance),
        /// Some assets were destroyed. \[asset_id, owner, balance\]
        SocialTokenDestroyed(SocialTokenCurrencyId, T::AccountId, Balance),
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}
}

impl<T: Config> Module<T> {
    fn transfer_from(
        currency_id: SocialTokenCurrencyId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        if amount.is_zero() || from == to {
            return Ok(());
        }

        T::CountryCurrency::transfer(currency_id, from, to, amount)?;

        Self::deposit_event(Event::<T>::SocialTokenTransferred(
            currency_id,
            from.clone(),
            to.clone(),
            amount,
        ));
        Ok(())
    }

    pub fn get_total_issuance(country_id: CountryId) -> Result<Balance, DispatchError> {
        let country_fund =
            CountryTreasury::<T>::get(country_id).ok_or(Error::<T>::CountryFundIsNotAvailable)?;
        let total_issuance = T::CountryCurrency::total_issuance(country_fund.currency_id);

        Ok(total_issuance)
    }

    pub fn get_country_fund_id(country_id: CountryId) -> T::AccountId {
        match CountryTreasury::<T>::get(country_id) {
            Some(fund) => fund.vault,
            _ => Default::default()
        }
    }
}


