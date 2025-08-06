use alloy::network::EthereumWallet;
use alloy::providers::fillers::{
    BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, WalletFiller,
};
use alloy::providers::{Identity, RootProvider};
use alloy::sol;

pub type SubchainRegistryImpl = SubchainRegistry::SubchainRegistryInstance<
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

// https://github.com/askucher/velas-subchain-manager/blob/067923b91b1c20c2c40d95dae555df663c158264/contracts/src/SubchainRegistry.sol
sol! {
    #[sol(rpc)]
    contract SubchainRegistry {

        #[derive(Debug, PartialEq, Eq)]
        enum Status {
            Pending, // Newly registered, awaiting activation
            Active, // Active and operational
            Suspended, // Temporarily suspended
            Deleted // Deleted or deactivated
        }

        // Emitted when a new subchain is registered
        event SubchainRegistered(uint256 indexed index, address indexed owner);
        // Emitted when a subchain's status changes
        event StatusChanged(uint256 indexed index, Status newStatus);
        // Emitted when a monthly payment is made to extend active period
        event MonthlyPayment(uint256 indexed index, uint256 newActiveTill);

        /// @notice Set the status of a subchain
        /// @param index The index of the subchain
        /// @param newStatus The new status to set
        function setStatus(uint256 index, Status newStatus);

        // --- Read helpers ---
        /// @notice Get the total number of registered subchains
        /// @return The count of subchains
        function totalSubchains() external view returns (uint256);

        /// @notice Get details of a subchain by index
        /// @param index The index of the subchain
        /// @return name The name of the subchain
        /// @return domain The domain of the subchain
        /// @return symbol The symbol of the subchain
        /// @return metadataUrl The metadata URL of the subchain
        /// @return chainId The chain ID of the subchain
        /// @return owner The owner address of the subchain
        /// @return status The current status of the subchain
        /// @return registrationTime The registration timestamp
        /// @return activeTill The timestamp until which the subchain is active
        function getSubchain(uint256 index)
            external
            view
            returns (
                string memory name,
                string memory domain,
                string memory symbol,
                string memory metadataUrl,
                uint256 chainId,
                address owner,
                Status status,
                uint256 registrationTime,
                uint256 activeTill
            );
    }
}
