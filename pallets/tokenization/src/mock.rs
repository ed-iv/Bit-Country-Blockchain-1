use crate::{Module, Config};
use crate as tokenization;
use super::*;
use frame_support::{
    construct_runtime, parameter_types, ord_parameter_types, weights::Weight,
    impl_outer_event, impl_outer_origin, impl_outer_dispatch, traits::EnsureOrigin,
};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::{IdentityLookup, AccountIdConversion}, ModuleId, Perbill};
use primitives::{CurrencyId, Amount, SocialTokenCurrencyId};
use frame_system::{EnsureSignedBy, EnsureRoot};
use frame_support::pallet_prelude::{MaybeSerializeDeserialize, Hooks, GenesisBuild};
use frame_support::sp_runtime::traits::AtLeast32Bit;
use orml_traits::parameter_type_with_key;
use primitives::SocialTokenCurrencyId::SocialToken;

pub type AccountId = u128;
pub type AuctionId = u64;
pub type Balance = u128;
pub type CountryId = u64;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 4;
pub const BOB: AccountId = 5;
pub const COUNTRY_ID: CountryId = 1;
pub const COUNTRY_ID_NOT_EXIST: CountryId = 1;
pub const NUUM: CurrencyId = 0;
pub const COUNTRY_FUND: SocialTokenCurrencyId = SocialTokenCurrencyId::SocialToken(1);


// Configure a mock runtime to test the pallet.

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Config for Runtime {
    type Origin = Origin;
    type Index = u64;
    type BlockNumber = BlockNumber;
    type Call = Call;
    type Hash = H256;
    type Hashing = ::sp_runtime::traits::BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type BlockWeights = ();
    type BlockLength = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type DbWeight = ();
    type BaseCallFilter = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type MaxLocks = ();
    type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: SocialTokenCurrencyId| -> Balance {
		Default::default()
	};
}

parameter_types! {
    pub const BitCountryTreasuryModuleId: ModuleId = ModuleId(*b"bit/trsy");
    pub TreasuryModuleAccount: AccountId = BitCountryTreasuryModuleId::get().into_account();
    pub const CountryFundModuleId: ModuleId = ModuleId(*b"bit/fund");
}

impl orml_tokens::Config for Runtime {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = SocialTokenCurrencyId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type OnDust = orml_tokens::TransferDust<Runtime, TreasuryModuleAccount>;
}

pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

pub struct CountryInfoSource {}

impl BCCountry<AccountId> for CountryInfoSource {
    fn check_ownership(who: &AccountId, country_id: &CountryId) -> bool {
        match *who {
            ALICE => true,
            _ => false,
        }
    }

    fn get_country(country_id: CountryId) -> Option<Country<AccountId>> {
        None
    }

    fn get_country_token(country_id: CountryId) -> Option<SocialTokenCurrencyId> {
        None
    }

    fn update_country_token(country_id: u64, currency_id: SocialTokenCurrencyId) -> Result<(), DispatchError> {
        Ok(())
    }
}

pub struct DEXManager {}

impl SwapManager<AccountId, SocialTokenCurrencyId, Balance> for DEXManager {
    fn add_liquidity(who: &AccountId, token_id_a: SocialTokenCurrencyId, token_id_b: SocialTokenCurrencyId, max_amount_a: Balance, max_amount_b: Balance) -> DispatchResult {
        Ok(())
    }
}

impl Config for Runtime {
    type Event = Event;
    type TokenId = u64;
    type CountryCurrency = Currencies;
    type SocialTokenTreasury = CountryFundModuleId;
    type CountryInfoSource = CountryInfoSource;
    type LiquidityPoolManager = DEXManager;
}

parameter_types! {
	pub const GetNativeCurrencyId: SocialTokenCurrencyId = SocialTokenCurrencyId::NativeToken(0);
}

impl orml_currencies::Config for Runtime {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = AdaptedBasicCurrency;
    type GetNativeCurrencyId = GetNativeCurrencyId;
    type WeightInfo = ();
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Module, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},        
        Currencies: orml_currencies::{ Module, Storage, Call, Event<T>},
        Tokens: orml_tokens::{ Module, Storage, Call, Event<T>},
        TokenizationModule: tokenization:: {Module, Call, Storage, Event<T>},
	}
);

pub struct ExtBuilder;

impl Default for ExtBuilder {
    fn default() -> Self {
        ExtBuilder
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![(ALICE, 100000)],
        }
            .assimilate_storage(&mut t)
            .unwrap();

        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| System::set_block_number(1));
        ext
    }
}

pub fn last_event() -> Event {
    frame_system::Module::<Runtime>::events()
        .pop()
        .expect("Event expected")
        .event
}