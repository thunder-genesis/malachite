use alloy::network::EthereumWallet;
use alloy::providers::fillers::{
    BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, WalletFiller,
};
use alloy::providers::{Identity, RootProvider};
use alloy::sol;

pub type SubchainDBImpl = SubchainDB::SubchainDBInstance<
    FillProvider<
        JoinFill<
            JoinFill<
                Identity,
                JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
            >,
            WalletFiller<EthereumWallet>,
        >,
        RootProvider,
    >,
>;

sol! {
    // SPDX-License-Identifier: MIT
    pragma solidity ^0.8.0;

    #[sol(rpc)]
    contract SubchainDB {
        enum SubchainStatus {
            // Subchain does not exist, default value
            None,
            // The service has paid the fee and deployed the subchain
            SubchainDeployed,
            // The service has deployed the subchain bridge
            Running,
            // The bridge has been stopped
            Stopped
        }

        // ---PUBLIC INTERFACE BEGIN---
        function getSubchainStatus(uint64 key) public view returns (SubchainStatus) {
            return subchainStatus[key];
        }

        function setSubchainStatus(uint64 key, SubchainStatus status) public {
            subchainStatus[key] = status;
        }

        function getExpiryTimestamp(uint64 key) public view returns (uint256) {
            return activeUntil[key];
        }

        function getDomainName(uint64 key) public view returns (string memory) {
            return domainName[key];
        }

        function setDomainName(uint64 key, string memory name) public {
            domainName[key] = name;
        }
        // ---PUBLIC INTERFACE END---

        // SubchainID => SubchainStatus
        mapping(uint64 => SubchainStatus) private subchainStatus;

        // SubchainID => Unix Timestamp
        mapping(uint64 => uint256) private activeUntil;

        // SubchainID => Domain Name
        mapping(uint64 => string) private domainName;
    }
}

// struct SubchainState {
//     payments: Vec<Payment>,
//     balance: USDT,
// }

// BTreeMap<ChainID, SubchainState>
// BTreeMap<DomainName, ChainID>
