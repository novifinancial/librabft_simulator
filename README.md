[![Build Status](https://github.com/novifinancial/librabft_simulator/workflows/Rust/badge.svg)](https://github.com/novifinancial/librabft_simulator/actions?query=workflow%3ARust)
[![License](https://img.shields.io/badge/license-Apache-green.svg)](LICENSE)

# Discrete-Event Simulation for BFT Consensus Protocols

The code in this repository is experimental and meant to explore the simulation and the specification of BFT consensus protocols such as LibraBFT / DiemBFT.

**Note:** On December 1, 2020, the Libra Association was renamed to Diem Association. In this repository, we use LibraBFT to denote early versions of the consensus protocol now
used in the [Diem blockchain](https://github.com/diem/diem).

## The LibraBFT Simulator

In relation to the [version 2](https://diem-developers-components.netlify.app/papers/diem-consensus-state-machine-replication-in-the-diem-blockchain/2019-10-24.pdf) of the
LibraBFT report, we are providing a minimal, reference implementation of the protocol LibraBFTv2 in a discrete-event simulated environment.

Usage:
```
RUST_LOG=warn cargo run --feature simulator --bin librabft_simulator
```

This simulator is provided for research-purpose only and is not meant to be used in production. It will continue to evolve along with the LibraBFT whitepaper.

Example output:
```
    Finished dev [unoptimized + debuginfo] target(s) in 0.17s
     Running target/debug/librabft_simulator
[2019-09-26T16:21:49Z WARN  librabft_simulator::record_store] Creating new record store for epoch: EpochId(0), initial_hash: QuorumCertificateHash(0), initial_state: State(13646096770106105413), configuration: EpochConfiguration { voting_rights: {Author(0): 1, Author(1): 1, Author(2): 1}, total_votes: 3 }
[2019-09-26T16:21:49Z WARN  librabft_simulator::record_store] Creating new record store for epoch: EpochId(0), initial_hash: QuorumCertificateHash(0), initial_state: State(13646096770106105413), configuration: EpochConfiguration { voting_rights: {Author(0): 1, Author(1): 1, Author(2): 1}, total_votes: 3 }
[2019-09-26T16:21:49Z WARN  librabft_simulator::record_store] Creating new record store for epoch: EpochId(0), initial_hash: QuorumCertificateHash(0), initial_state: State(13646096770106105413), configuration: EpochConfiguration { voting_rights: {Author(0): 1, Author(1): 1, Author(2): 1}, total_votes: 3 }
[2019-09-26T16:21:49Z WARN  librabft_simulator] Commands executed per node: [
    29,
    29,
    29,
]
```

## Contributing

Read our [Contributing guide](https://developers.diem.org/docs/community/contributing).

## License

The content of this repository is licensed as [Apache 2.0](LICENSE)
