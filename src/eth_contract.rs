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
    pragma solidity =0.7.6;

    #[sol(rpc)]
    contract SubchainDB {

        #[derive(Debug, PartialEq, Eq)]
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
        function getSubchainStatus(uint64 chainId) public view returns (SubchainStatus) {
            return subchainStatus[chainId];
        }

        function setSubchainStatus(uint64 chainId, SubchainStatus status) public {
            subchainStatus[chainId] = status;
        }

        function getExpiryTimestamp(uint64 chainId) public view returns (uint256) {
            return activeUntil[chainId];
        }

        function setExpiryTimestamp(uint64 chainId, uint64 activeUntil) public {
            subchainStatus[chainId] = status;
        }

        function getDomainName(uint64 chainId) public view returns (string memory) {
            return domainName[chainId];
        }

        function setDomainName(uint64 chainId, string memory name) public {
            domainName[chainId] = name;
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
