// Bitcoin Dev Kit
// Written in 2020 by Alekos Filini <alekos.filini@gmail.com>
//
// Copyright (c) 2020-2021 Bitcoin Dev Kit Developers
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::default::Default;
use std::marker::PhantomData;

use bitcoin::psbt::{self, Psbt};
use bitcoin::{OutPoint, ScriptBuf, Sequence, Transaction, absolute, script::PushBytes};

use super::coin_selection::{CoinSelectionAlgorithm, DefaultCoinSelectionAlgorithm};
use crate::{Error, Utxo, Wallet, database::BatchDatabase};
use crate::{
    TransactionDetails,
    types::{FeeRate, KeychainKind, LocalUtxo, WeightedUtxo},
};
/// Context in which the [`TxBuilder`] is valid
pub trait TxBuilderContext: std::fmt::Debug + Default + Clone {}

/// Marker type to indicate the [`TxBuilder`] is being used to create a new transaction (as opposed
/// to bumping the fee of an existing one).
#[derive(Debug, Default, Clone)]
pub struct CreateTx;
impl TxBuilderContext for CreateTx {}

/// Marker type to indicate the [`TxBuilder`] is being used to bump the fee of an existing transaction.
#[derive(Debug, Default, Clone)]
pub struct BumpFee;
impl TxBuilderContext for BumpFee {}

#[derive(Debug)]
pub struct TxBuilder<'a, D, Cs, Ctx> {
    pub(crate) wallet: &'a Wallet<D>,
    pub(crate) params: TxParams,
    pub(crate) coin_selection: Cs,
    pub(crate) phantom: PhantomData<Ctx>,
}

/// The parameters for transaction creation sans coin selection algorithm.
//TODO: TxParams should eventually be exposed publicly.
#[derive(Default, Debug, Clone)]
pub(crate) struct TxParams {
    pub(crate) recipients: Vec<(ScriptBuf, u64)>,
    pub(crate) drain_wallet: bool,
    pub(crate) drain_to: Option<ScriptBuf>,
    pub(crate) fee_policy: Option<FeePolicy>,
    pub(crate) internal_policy_path: Option<BTreeMap<String, Vec<usize>>>,
    pub(crate) external_policy_path: Option<BTreeMap<String, Vec<usize>>>,
    pub(crate) utxos: Vec<WeightedUtxo>,
    pub(crate) unspendable: HashSet<OutPoint>,
    pub(crate) manually_selected_only: bool,
    pub(crate) sighash: Option<psbt::PsbtSighashType>,
    pub(crate) ordering: TxOrdering,
    pub(crate) locktime: Option<absolute::LockTime>,
    pub(crate) rbf: Option<RbfValue>,
    pub(crate) version: Option<Version>,
    pub(crate) change_policy: ChangeSpendPolicy,
    pub(crate) only_witness_utxo: bool,
    pub(crate) add_global_xpubs: bool,
    pub(crate) include_output_redeem_witness_script: bool,
    pub(crate) bumping_fee: Option<PreviousFee>,
    pub(crate) current_height: Option<absolute::LockTime>,
    pub(crate) allow_dust: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PreviousFee {
    pub absolute: u64,
    pub rate: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum FeePolicy {
    FeeRate(FeeRate),
    FeeAmount(u64),
}

impl std::default::Default for FeePolicy {
    fn default() -> Self {
        FeePolicy::FeeRate(FeeRate::default_min_relay_fee())
    }
}

impl<'a, Cs: Clone, Ctx, D> Clone for TxBuilder<'a, D, Cs, Ctx> {
    fn clone(&self) -> Self {
        TxBuilder {
            wallet: self.wallet,
            params: self.params.clone(),
            coin_selection: self.coin_selection.clone(),
            phantom: PhantomData,
        }
    }
}

// methods supported by both contexts, for any CoinSelectionAlgorithm
impl<'a, D: BatchDatabase, Cs: CoinSelectionAlgorithm<D>, Ctx: TxBuilderContext>
    TxBuilder<'a, D, Cs, Ctx>
{
    /// Set a custom fee rate
    pub fn fee_rate(&mut self, fee_rate: FeeRate) -> &mut Self {
        self.params.fee_policy = Some(FeePolicy::FeeRate(fee_rate));
        self
    }

    /// Set an absolute fee
    pub fn fee_absolute(&mut self, fee_amount: u64) -> &mut Self {
        self.params.fee_policy = Some(FeePolicy::FeeAmount(fee_amount));
        self
    }

    pub fn policy_path(
        &mut self,
        policy_path: BTreeMap<String, Vec<usize>>,
        keychain: KeychainKind,
    ) -> &mut Self {
        let to_update = match keychain {
            KeychainKind::Internal => &mut self.params.internal_policy_path,
            KeychainKind::External => &mut self.params.external_policy_path,
        };

        *to_update = Some(policy_path);
        self
    }

    /// Add the list of outpoints to the internal list of UTXOs that **must** be spent.
    ///
    /// If an error occurs while adding any of the UTXOs then none of them are added and the error is returned.
    ///
    /// These have priority over the "unspendable" utxos, meaning that if a utxo is present both in
    /// the "utxos" and the "unspendable" list, it will be spent.
    pub fn add_utxos(&mut self, outpoints: &[OutPoint]) -> Result<&mut Self, Error> {
        let utxos = outpoints
            .iter()
            .map(|outpoint| self.wallet.get_utxo(*outpoint)?.ok_or(Error::UnknownUtxo))
            .collect::<Result<Vec<_>, _>>()?;

        for utxo in utxos {
            let descriptor = self.wallet.get_descriptor_for_keychain(utxo.keychain);
            #[allow(deprecated)]
            let satisfaction_weight = descriptor.max_weight_to_satisfy().unwrap();
            self.params.utxos.push(WeightedUtxo {
                satisfaction_weight,
                utxo: Utxo::Local(utxo),
            });
        }

        Ok(self)
    }

    /// Add a utxo to the internal list of utxos that **must** be spent
    ///
    /// These have priority over the "unspendable" utxos, meaning that if a utxo is present both in
    /// the "utxos" and the "unspendable" list, it will be spent.
    pub fn add_utxo(&mut self, outpoint: OutPoint) -> Result<&mut Self, Error> {
        self.add_utxos(&[outpoint])
    }

    /// Add a foreign UTXO i.e. a UTXO not owned by this wallet.
    ///
    /// At a minimum to add a foreign UTXO we need:
    ///
    /// 1. `outpoint`: To add it to the raw transaction.
    /// 2. `psbt_input`: To know the value.
    /// 3. `satisfaction_weight`: To know how much weight/vbytes the input will add to the transaction for fee calculation.
    ///
    /// There are several security concerns about adding foreign UTXOs that application
    /// developers should consider. First, how do you know the value of the input is correct? If a
    /// `non_witness_utxo` is provided in the `psbt_input` then this method implicitly verifies the
    /// value by checking it against the transaction. If only a `witness_utxo` is provided then this
    /// method doesn't verify the value but just takes it as a given -- it is up to you to check
    /// that whoever sent you the `input_psbt` was not lying!
    ///
    /// Secondly, you must somehow provide `satisfaction_weight` of the input. Depending on your
    /// application it may be important that this be known precisely. If not, a malicious
    /// counterparty may fool you into putting in a value that is too low, giving the transaction a
    /// lower than expected feerate. They could also fool you into putting a value that is too high
    /// causing you to pay a fee that is too high. The party who is broadcasting the transaction can
    /// of course check the real input weight matches the expected weight prior to broadcasting.
    ///
    /// To guarantee the `satisfaction_weight` is correct, you can require the party providing the
    /// `psbt_input` provide a miniscript descriptor for the input so you can check it against the
    /// `script_pubkey` and then ask it for the [`max_weight_to_satisfy`].
    ///
    /// This is an **EXPERIMENTAL** feature, API and other major changes are expected.
    ///
    /// # Errors
    ///
    /// This method returns errors in the following circumstances:
    ///
    /// 1. The `psbt_input` does not contain a `witness_utxo` or `non_witness_utxo`.
    /// 2. The data in `non_witness_utxo` does not match what is in `outpoint`.
    ///
    /// Note unless you set [`only_witness_utxo`] any non-taproot `psbt_input` you pass to this
    /// method must have `non_witness_utxo` set otherwise you will get an error when [`finish`]
    /// is called.
    ///
    /// [`only_witness_utxo`]: Self::only_witness_utxo
    /// [`finish`]: Self::finish
    /// [`max_weight_to_satisfy`]: miniscript::Descriptor::max_weight_to_satisfy
    pub fn add_foreign_utxo(
        &mut self,
        outpoint: OutPoint,
        psbt_input: psbt::Input,
        satisfaction_weight: usize,
    ) -> Result<&mut Self, Error> {
        if psbt_input.witness_utxo.is_none() {
            match psbt_input.non_witness_utxo.as_ref() {
                Some(tx) => {
                    if tx.txid() != outpoint.txid {
                        return Err(Error::Generic(
                            "Foreign utxo outpoint does not match PSBT input".into(),
                        ));
                    }
                    if tx.output.len() <= outpoint.vout as usize {
                        return Err(Error::InvalidOutpoint(outpoint));
                    }
                }
                None => {
                    return Err(Error::Generic(
                        "Foreign utxo missing witness_utxo or non_witness_utxo".into(),
                    ));
                }
            }
        }

        self.params.utxos.push(WeightedUtxo {
            satisfaction_weight,
            utxo: Utxo::Foreign {
                outpoint,
                psbt_input: Box::new(psbt_input),
            },
        });

        Ok(self)
    }

    /// Only spend utxos added by [`add_utxo`].
    ///
    /// The wallet will **not** add additional utxos to the transaction even if they are needed to
    /// make the transaction valid.
    ///
    /// [`add_utxo`]: Self::add_utxo
    pub fn manually_selected_only(&mut self) -> &mut Self {
        self.params.manually_selected_only = true;
        self
    }

    /// Replace the internal list of unspendable utxos with a new list
    ///
    /// It's important to note that the "must-be-spent" utxos added with [`TxBuilder::add_utxo`]
    /// have priority over these. See the docs of the two linked methods for more details.
    pub fn unspendable(&mut self, unspendable: Vec<OutPoint>) -> &mut Self {
        self.params.unspendable = unspendable.into_iter().collect();
        self
    }

    /// Add a utxo to the internal list of unspendable utxos
    ///
    /// It's important to note that the "must-be-spent" utxos added with [`TxBuilder::add_utxo`]
    /// have priority over this. See the docs of the two linked methods for more details.
    pub fn add_unspendable(&mut self, unspendable: OutPoint) -> &mut Self {
        self.params.unspendable.insert(unspendable);
        self
    }

    /// Sign with a specific sig hash
    ///
    /// **Use this option very carefully**
    pub fn sighash(&mut self, sighash: psbt::PsbtSighashType) -> &mut Self {
        self.params.sighash = Some(sighash);
        self
    }

    /// Choose the ordering for inputs and outputs of the transaction
    pub fn ordering(&mut self, ordering: TxOrdering) -> &mut Self {
        self.params.ordering = ordering;
        self
    }

    /// Use a specific nLockTime while creating the transaction
    ///
    /// This can cause conflicts if the wallet's descriptors contain an "after" (OP_CLTV) operator.
    pub fn nlocktime(&mut self, locktime: absolute::LockTime) -> &mut Self {
        self.params.locktime = Some(locktime);
        self
    }

    /// Build a transaction with a specific version
    ///
    /// The `version` should always be greater than `0` and greater than `1` if the wallet's
    /// descriptors contain an "older" (OP_CSV) operator.
    pub fn version(&mut self, version: i32) -> &mut Self {
        self.params.version = Some(Version(version));
        self
    }

    /// Do not spend change outputs
    ///
    /// This effectively adds all the change outputs to the "unspendable" list. See
    /// [`TxBuilder::unspendable`].
    pub fn do_not_spend_change(&mut self) -> &mut Self {
        self.params.change_policy = ChangeSpendPolicy::ChangeForbidden;
        self
    }

    /// Only spend change outputs
    ///
    /// This effectively adds all the non-change outputs to the "unspendable" list. See
    /// [`TxBuilder::unspendable`].
    pub fn only_spend_change(&mut self) -> &mut Self {
        self.params.change_policy = ChangeSpendPolicy::OnlyChange;
        self
    }

    /// Set a specific [`ChangeSpendPolicy`]. See [`TxBuilder::do_not_spend_change`] and
    /// [`TxBuilder::only_spend_change`] for some shortcuts.
    pub fn change_policy(&mut self, change_policy: ChangeSpendPolicy) -> &mut Self {
        self.params.change_policy = change_policy;
        self
    }

    /// Only Fill-in the [`psbt::Input::witness_utxo`](bitcoin::psbt::Input::witness_utxo) field when spending from
    /// SegWit descriptors.
    ///
    /// This reduces the size of the PSBT, but some signers might reject them due to the lack of
    /// the `non_witness_utxo`.
    pub fn only_witness_utxo(&mut self) -> &mut Self {
        self.params.only_witness_utxo = true;
        self
    }

    /// Fill-in the [`psbt::Output::redeem_script`](bitcoin::psbt::Output::redeem_script) and
    /// [`psbt::Output::witness_script`](bitcoin::psbt::Output::witness_script) fields.
    ///
    /// This is useful for signers which always require it, like ColdCard hardware wallets.
    pub fn include_output_redeem_witness_script(&mut self) -> &mut Self {
        self.params.include_output_redeem_witness_script = true;
        self
    }

    /// Fill-in the `PSBT_GLOBAL_XPUB` field with the extended keys contained in both the external
    /// and internal descriptors
    ///
    /// This is useful for offline signers that take part to a multisig. Some hardware wallets like
    /// BitBox and ColdCard are known to require this.
    pub fn add_global_xpubs(&mut self) -> &mut Self {
        self.params.add_global_xpubs = true;
        self
    }

    /// Spend all the available inputs. This respects filters like [`TxBuilder::unspendable`] and the change policy.
    pub fn drain_wallet(&mut self) -> &mut Self {
        self.params.drain_wallet = true;
        self
    }

    /// Choose the coin selection algorithm
    ///
    /// Overrides the [`DefaultCoinSelectionAlgorithm`](super::coin_selection::DefaultCoinSelectionAlgorithm).
    ///
    /// Note that this function consumes the builder and returns it so it is usually best to put this as the first call on the builder.
    pub fn coin_selection<P: CoinSelectionAlgorithm<D>>(
        self,
        coin_selection: P,
    ) -> TxBuilder<'a, D, P, Ctx> {
        TxBuilder {
            wallet: self.wallet,
            params: self.params,
            coin_selection,
            phantom: PhantomData,
        }
    }

    /// Finish building the transaction.
    ///
    /// Returns the [`BIP174`] "PSBT" and summary details about the transaction.
    ///
    /// [`BIP174`]: https://github.com/bitcoin/bips/blob/master/bip-0174.mediawiki
    pub fn finish(self) -> Result<(Psbt, TransactionDetails), Error> {
        self.wallet.create_tx(self.coin_selection, self.params)
    }

    /// Enable signaling RBF
    ///
    /// This will use the default nSequence value of `0xFFFFFFFD`.
    pub fn enable_rbf(&mut self) -> &mut Self {
        self.params.rbf = Some(RbfValue::Default);
        self
    }

    /// Enable signaling RBF with a specific nSequence value
    ///
    /// This can cause conflicts if the wallet's descriptors contain an "older" (OP_CSV) operator
    /// and the given `nsequence` is lower than the CSV value.
    ///
    /// If the `nsequence` is higher than `0xFFFFFFFD` an error will be thrown, since it would not
    /// be a valid nSequence to signal RBF.
    pub fn enable_rbf_with_sequence(&mut self, nsequence: Sequence) -> &mut Self {
        self.params.rbf = Some(RbfValue::Value(nsequence));
        self
    }

    /// Set the current blockchain height.
    ///
    /// This will be used to:
    /// 1. Set the nLockTime for preventing fee sniping.
    /// **Note**: This will be ignored if you manually specify a nlocktime using [`TxBuilder::nlocktime`].
    /// 2. Decide whether coinbase outputs are mature or not. If the coinbase outputs are not
    ///    mature at `current_height`, we ignore them in the coin selection.
    ///    If you want to create a transaction that spends immature coinbase inputs, manually
    ///    add them using [`TxBuilder::add_utxos`].
    ///
    /// In both cases, if you don't provide a current height, we use the last sync height.
    pub fn current_height(&mut self, height: u32) -> &mut Self {
        self.params.current_height =
            Some(absolute::LockTime::from_height(height).expect("Invalid height"));
        self
    }

    /// Set whether or not the dust limit is checked.
    ///
    /// **Note**: by avoiding a dust limit check you may end up with a transaction that is non-standard.
    pub fn allow_dust(&mut self, allow_dust: bool) -> &mut Self {
        self.params.allow_dust = allow_dust;
        self
    }
}

impl<'a, D: BatchDatabase, Cs: CoinSelectionAlgorithm<D>> TxBuilder<'a, D, Cs, CreateTx> {
    /// Replace the recipients already added with a new list
    pub fn set_recipients(&mut self, recipients: Vec<(ScriptBuf, u64)>) -> &mut Self {
        self.params.recipients = recipients;
        self
    }

    /// Add a recipient to the internal list
    pub fn add_recipient(&mut self, script_pubkey: ScriptBuf, amount: u64) -> &mut Self {
        self.params.recipients.push((script_pubkey, amount));
        self
    }

    /// Add data as an output, using OP_RETURN
    pub fn add_data<T: AsRef<PushBytes>>(&mut self, data: &T) -> &mut Self {
        let script = ScriptBuf::new_op_return(data);
        self.add_recipient(script, 0u64);
        self
    }

    pub fn drain_to(&mut self, script_pubkey: ScriptBuf) -> &mut Self {
        self.params.drain_to = Some(script_pubkey);
        self
    }
}

// methods supported only by bump_fee
impl<'a, D: BatchDatabase> TxBuilder<'a, D, DefaultCoinSelectionAlgorithm, BumpFee> {
    /// Explicitly tells the wallet that it is allowed to reduce the amount of the output matching this
    /// `script_pubkey` in order to bump the transaction fee. Without specifying this the wallet
    /// will attempt to find a change output to shrink instead.
    ///
    /// **Note** that the output may shrink to below the dust limit and therefore be removed. If it is
    /// preserved then it is currently not guaranteed to be in the same position as it was
    /// originally.
    ///
    /// Returns an `Err` if `script_pubkey` can't be found among the recipients of the
    /// transaction we are bumping.
    pub fn allow_shrinking(&mut self, script_pubkey: ScriptBuf) -> Result<&mut Self, Error> {
        match self
            .params
            .recipients
            .iter()
            .position(|(recipient_script, _)| *recipient_script == script_pubkey)
        {
            Some(position) => {
                self.params.recipients.remove(position);
                self.params.drain_to = Some(script_pubkey);
                Ok(self)
            }
            None => Err(Error::Generic(format!(
                "{} was not in the original transaction",
                script_pubkey
            ))),
        }
    }
}

/// Ordering of the transaction's inputs and outputs
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Copy)]
pub enum TxOrdering {
    /// Randomized (default)
    Shuffle,
    /// Unchanged
    Untouched,
    /// BIP69 / Lexicographic
    Bip69Lexicographic,
}

impl Default for TxOrdering {
    fn default() -> Self {
        TxOrdering::Shuffle
    }
}

impl TxOrdering {
    /// Sort transaction inputs and outputs by [`TxOrdering`] variant
    pub fn sort_tx(&self, tx: &mut Transaction) {
        match self {
            TxOrdering::Untouched => {}
            TxOrdering::Shuffle => {
                #[cfg(test)]
                use rand::SeedableRng;
                use rand::seq::SliceRandom;

                #[cfg(not(test))]
                let mut rng = rand::thread_rng();
                #[cfg(test)]
                let mut rng = rand::rngs::StdRng::seed_from_u64(12345);

                tx.output.shuffle(&mut rng);
            }
            TxOrdering::Bip69Lexicographic => {
                tx.input.sort_unstable_by_key(|txin| {
                    (txin.previous_output.txid, txin.previous_output.vout)
                });
                tx.output
                    .sort_unstable_by_key(|txout| (txout.value, txout.script_pubkey.clone()));
            }
        }
    }
}

/// Transaction version
///
/// Has a default value of `1`
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Copy)]
pub(crate) struct Version(pub(crate) i32);

impl Default for Version {
    fn default() -> Self {
        Version(1)
    }
}

/// RBF nSequence value
///
/// Has a default value of `0xFFFFFFFD`
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Copy)]
pub(crate) enum RbfValue {
    Default,
    Value(Sequence),
}

impl RbfValue {
    pub(crate) fn get_value(&self) -> Sequence {
        match self {
            RbfValue::Default => Sequence::ENABLE_RBF_NO_LOCKTIME,
            RbfValue::Value(v) => *v,
        }
    }
}

/// Policy regarding the use of change outputs when creating a transaction
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Copy)]
pub enum ChangeSpendPolicy {
    /// Use both change and non-change outputs (default)
    ChangeAllowed,
    /// Only use change outputs (see [`TxBuilder::only_spend_change`])
    OnlyChange,
    /// Only use non-change outputs (see [`TxBuilder::do_not_spend_change`])
    ChangeForbidden,
}

impl Default for ChangeSpendPolicy {
    fn default() -> Self {
        ChangeSpendPolicy::ChangeAllowed
    }
}

impl ChangeSpendPolicy {
    pub(crate) fn is_satisfied_by(&self, utxo: &LocalUtxo) -> bool {
        match self {
            ChangeSpendPolicy::ChangeAllowed => true,
            ChangeSpendPolicy::OnlyChange => utxo.keychain == KeychainKind::Internal,
            ChangeSpendPolicy::ChangeForbidden => utxo.keychain == KeychainKind::External,
        }
    }
}

#[cfg(test)]
mod test {
    const ORDERING_TEST_TX: &str = "0200000003c26f3eb7932f7acddc5ddd26602b77e7516079b03090a16e2c2f54\
                                    85d1fd600f0100000000ffffffffc26f3eb7932f7acddc5ddd26602b77e75160\
                                    79b03090a16e2c2f5485d1fd600f0000000000ffffffff571fb3e02278217852\
                                    dd5d299947e2b7354a639adc32ec1fa7b82cfb5dec530e0500000000ffffffff\
                                    03e80300000000000002aaeee80300000000000001aa200300000000000001ff\
                                    00000000";
    macro_rules! ordering_test_tx {
        () => {
            deserialize::<bitcoin::Transaction>(&Vec::<u8>::from_hex(ORDERING_TEST_TX).unwrap())
                .unwrap()
        };
    }

    use bitcoin::consensus::deserialize;
    use bitcoin::hashes::hex::FromHex;
    use bitcoin::{Amount, TxOut};
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_output_ordering_default_shuffle() {
        assert_eq!(TxOrdering::default(), TxOrdering::Shuffle);
    }

    #[test]
    fn test_output_ordering_untouched() {
        let original_tx = ordering_test_tx!();
        let mut tx = original_tx.clone();

        TxOrdering::Untouched.sort_tx(&mut tx);

        assert_eq!(original_tx, tx);
    }

    #[test]
    fn test_output_ordering_shuffle() {
        let original_tx = ordering_test_tx!();
        let mut tx = original_tx.clone();

        TxOrdering::Shuffle.sort_tx(&mut tx);

        assert_eq!(original_tx.input, tx.input);
        assert_ne!(original_tx.output, tx.output);
    }

    #[test]
    fn test_output_ordering_bip69() {
        let original_tx = ordering_test_tx!();
        let mut tx = original_tx;

        TxOrdering::Bip69Lexicographic.sort_tx(&mut tx);

        assert_eq!(
            tx.input[0].previous_output,
            bitcoin::OutPoint::from_str(
                "0e53ec5dfb2cb8a71fec32dc9a634a35b7e24799295ddd5278217822e0b31f57:5"
            )
            .unwrap()
        );
        assert_eq!(
            tx.input[1].previous_output,
            bitcoin::OutPoint::from_str(
                "0f60fdd185542f2c6ea19030b0796051e7772b6026dd5ddccd7a2f93b73e6fc2:0"
            )
            .unwrap()
        );
        assert_eq!(
            tx.input[2].previous_output,
            bitcoin::OutPoint::from_str(
                "0f60fdd185542f2c6ea19030b0796051e7772b6026dd5ddccd7a2f93b73e6fc2:1"
            )
            .unwrap()
        );

        assert_eq!(tx.output[0].value, Amount::from_sat(800));
        assert_eq!(tx.output[1].script_pubkey, ScriptBuf::from(vec![0xAA]));
        assert_eq!(
            tx.output[2].script_pubkey,
            ScriptBuf::from(vec![0xAA, 0xEE])
        );
    }

    fn get_test_utxos() -> Vec<LocalUtxo> {
        use bitcoin::hashes::Hash;

        vec![
            LocalUtxo {
                outpoint: OutPoint {
                    txid: bitcoin::Txid::from_slice(&[0; 32]).unwrap(),
                    vout: 0,
                },
                txout: TxOut::NULL,
                keychain: KeychainKind::External,
                is_spent: false,
            },
            LocalUtxo {
                outpoint: OutPoint {
                    txid: bitcoin::Txid::from_slice(&[0; 32]).unwrap(),
                    vout: 1,
                },
                txout: TxOut::NULL,
                keychain: KeychainKind::Internal,
                is_spent: false,
            },
        ]
    }

    #[test]
    fn test_change_spend_policy_default() {
        let change_spend_policy = ChangeSpendPolicy::default();
        let filtered = get_test_utxos()
            .into_iter()
            .filter(|u| change_spend_policy.is_satisfied_by(u))
            .count();

        assert_eq!(filtered, 2);
    }

    #[test]
    fn test_change_spend_policy_no_internal() {
        let change_spend_policy = ChangeSpendPolicy::ChangeForbidden;
        let filtered = get_test_utxos()
            .into_iter()
            .filter(|u| change_spend_policy.is_satisfied_by(u))
            .collect::<Vec<_>>();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].keychain, KeychainKind::External);
    }

    #[test]
    fn test_change_spend_policy_only_internal() {
        let change_spend_policy = ChangeSpendPolicy::OnlyChange;
        let filtered = get_test_utxos()
            .into_iter()
            .filter(|u| change_spend_policy.is_satisfied_by(u))
            .collect::<Vec<_>>();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].keychain, KeychainKind::Internal);
    }

    #[test]
    fn test_default_tx_version_1() {
        let version = Version::default();
        assert_eq!(version.0, 1);
    }
}
