<a href="https://calibra.com/">
	<img width="200" src=".assets/calibra.png" alt="Calibra Logo" />
</a>

<hr/>

[![License](https://img.shields.io/badge/license-Apache-green.svg)](LICENSE.md)

This repository is dedicated to sharing research material and scientific contributions by Calibra researchers towards Libra Core development.

## Projects

### The LibraBFT simulator

In relation to the [version 2](https://developers.libra.org/docs/assets/papers/libra-consensus-state-machine-replication-in-the-libra-blockchain/2019-10-24.pdf) of the LibraBFT report, we are providing a minimal, reference implementation of the protocol LibraBFTv2 in a discrete-event simulated environment.

Usage:
```
RUST_LOG=warn cargo run --bin librabft_simulator
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

### Libra Technical Papers
* [The Libra Blockchain](https://developers.libra.org/docs/the-libra-blockchain-paper)
* [Move: A Language With Programmable Resources](https://developers.libra.org/docs/move-paper)
* [State Machine Replication in the Libra Blockchain](https://developers.libra.org/docs/state-machine-replication-paper)

## Contributing

Read our [Contributing guide](https://developers.libra.org/docs/community/contributing).

## Libra Community

Join us on the [Libra Discourse](https://community.libra.org)

Get the latest updates to our project by signing up to our [newsletter](https://developers.libra.org/newsletter_form).

## License

The content of this repository is licensed as [Apache 2.0](https://github.com/calibra/research/blob/master/LICENSE)
