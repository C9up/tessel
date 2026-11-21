# MLS test vectors

Official test vectors of the MLS working group (`mlswg/mls-implementations`, `test-vectors/`), as
OpenMLS ships them in its repository at tag `v0.9.2` (commit `8a8f83b4b61fc45f28facec25e914992b3465de6`,
`openmls/test_vectors/`, MIT license, Copyright (c) 2020 OpenMLS Authors).

- `crypto-basics.json`: the entry of ciphersuite 1, Tessel's suite (ADR-001).
- `messages.json`: the first 25 entries of 300.

`tests/mls_vectors.rs` checks them through public APIs only. The vectors that need OpenMLS's
internals (tree math, key schedule, secret tree, passive clients…) run in OpenMLS's own test
suite, on the very version Tessel pins (`openmls = "=0.9.2"`).

To refresh after an OpenMLS upgrade, from a checkout of the new tag:

```sh
jq 'map(select(.cipher_suite == 1))' openmls/test_vectors/crypto-basics.json > crypto-basics.json
jq '.[:25]' openmls/test_vectors/messages.json > messages.json
```
