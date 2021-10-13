# Contribution Guide

Our goal is to make contributing to the Novi projects easy and transparent.

## Contributing to Novi's LibraBFT-Simulator

To contribute, ensure that you have the latest version of the codebase. To clone the repository, run the following:
```bash
$ git clone https://github.com/novifinancial/librabft_simulator.git
$ cd librabft_simulator
$ cargo build --all --all-targets
$ cargo test
```

## Coding Guidelines for Rust code

For detailed guidance on how to contribute to the Rust code in this repository refer to
[Coding Guidelines](https://developers.libra.org/docs/coding-guidelines).

## Pull Requests

Please refer to the documentation to determine the status of each project (e.g. actively
developed vs. archived) before submitting a pull request.

To submit your pull request:

1. Fork Novi's `librabft_simulator` repository and create your branch from `main`.
2. If you have added code that should be tested, add unit tests.
3. If you have made changes to APIs, update the relevant documentation, and build and test the developer site.
4. Verify and ensure that the test suite passes.
5. Make sure your code passes both linters.
6. Complete the Contributor License Agreement (CLA), if you haven't already done so.
7. Submit your pull request.

## Contributor License Agreement ("CLA")

In order to accept your pull request, we need you to submit a CLA. You only need to do
this once to work on any of Facebook's open source projects.

Complete your CLA here: <https://code.facebook.com/cla>

## Code of Conduct

Please refer to the [Code of Conduct](https://github.com/libra/libra/blob/main/CODE_OF_CONDUCT.md) for guidelines on interacting with the community.

## Issues

We use GitHub issues to track public bugs. Please ensure your description is
clear and has sufficient instructions to be able to reproduce the issue.
