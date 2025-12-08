# Malachite

Malachite is a service for automated provisioning and lifecycle management of EVM Subchains on the Velas ecosystem. It listens for Subchain registrations in the on-chain Subchain Registry, prepares chain state, provisions a bridge VM, configures DNS, and marks the chain as active.

## Goals

- Automate new Subchain deployment end-to-end (on-chain state + infrastructure).
- Ensure deployments are reproducible and observable.
- Minimize manual steps required for onboarding new subchains.

## High-level architecture

- Event listener: watches SubchainRegistry events and reacts to new registrations
- Metadata & compilation: fetch and compile contract/alloc metadata (metadata module)
- Key management: create, store, forget Subchain owner keypairs
- On-chain actions: create/fund subchain state and owner accounts via Velas RPC
- Infrastructure provisioning: deploy VM on OpenStack and bootstrap containerized bridge via Docker (openstack, bootstrapper, docker)
- DNS: register subdomain records in Cloudflare
- Notifications: Telegram alerts for important events/errors (alert)
- Glue/context: the Context module wires clients/configs together for all components

## Primary program flow (summary)

1. Listen to SubchainRegistered events from SubchainRegistry.
2. Fetch subchain entry details and metadata URL.
3. Extract and compile metadata (alloc, contracts).
4. Create a Subchain Owner keypair and fund it via Velas RPC.
5. Create and fund the Subchain EVM state account on-chain.
6. Deploy an OpenStack VM and register a Cloudflare DNS record pointing to its IP.
7. Ping and wait for SSH, then bootstrap the instance (Docker Compose artifacts).
8. Send Telegram alerts on failures/successes.
