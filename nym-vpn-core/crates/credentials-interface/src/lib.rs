// Copyright 2024 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use thiserror::Error;

pub use nym_coconut::{
    hash_to_scalar,
    Attribute, 
PublicAttribute,
PrivateAttribute,
    Signature,
};

pub(crate) use nym_coconut::{
VerifyCredentialRequest,
};

pub(crate) const VOUCHER_INFO_TYPE: &str = "BandwidthVoucher";
pub(crate) const FREE_PASS_INFO_TYPE: &str = "FreeBandwidthPass";

#[derive(Debug, Error)]
#[error("{0} is not a valid credential type")]
pub struct UnknownCredentialType(String);

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CredentialType {
    Voucher,
    FreePass,
}

impl FromStr for CredentialType {
    type Err = UnknownCredentialType;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == VOUCHER_INFO_TYPE {
            Ok(CredentialType::Voucher)
        } else if s == FREE_PASS_INFO_TYPE {
            Ok(CredentialType::FreePass)
        } else {
            Err(UnknownCredentialType(s.to_string()))
        }
    }
}

impl Display for CredentialType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialType::Voucher => VOUCHER_INFO_TYPE.fmt(f),
            CredentialType::FreePass => FREE_PASS_INFO_TYPE.fmt(f),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub(crate) struct CredentialSpendingData {
    pub(crate) embedded_private_attributes: usize,

    pub(crate) verify_credential_request: VerifyCredentialRequest,

    pub(crate) public_attributes_plain: Vec<String>,

    pub(crate) typ: CredentialType,

    /// The (DKG) epoch id under which the credential has been issued so that the verifier could use correct verification key for validation.
    pub(crate) epoch_id: u64,
}
