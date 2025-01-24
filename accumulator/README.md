# Nomad Accumulator

A set of accumulator-related tooling for Nomad development. This crate contains
a full incremental sparse merkle tree, as well as a lightweight tree which
stores only the leading branch. The full tree is suitable for proving, while the
light tree is suitable only for verifying proofs.

### Interface

We use constant generics for fixed-depth trees. Trees can be instantiated with
any depth, but it is NOT RECOMMENDED to create very deep trees.

We provide two trees structures:

- `LightMerkle<const N: usize>`
  - This tree stores only the leading branch, and may be used as a verifier.
  - Ingested leaves are discarded.
  - In-memory size is constant.
- `Tree<const N: usize>`
  - This tree stores all leaves and may be used as a prover
  - Ingested leaves are kept in memory.
  - In-memory size grows with each leaf.

We provide a single `Proof<const N: usize>` struct. It may be produced
by `Tree::<N>::prove` and verified with `Tree::<N>::verify` or with
`LightMerkle::<N>::verify`.

For convenient use in our own crates, we have aliased the depth 32 trees as
`NomadTree` and `NomadLightMerkle`.

```rust
use accumulator::{Tree, Proof, Merkle, MerkleProof};
use ethers::prelude::H256;

let mut tree: Tree<16> = Default::default();
tree.ingest(H256::zero());

// Error
let proof: Proof<16> = tree.prove(0).unwrap();
tree.verify(&proof).unwrap();
```

### Wasm Bindings

We also expose a WASM interface. Limitations:

- The WASM bindings expose only the proving tree, and proof structs.
- WASM bindings do not yet support const generics
- Instead we expose trees of depth 2, 4, 8, 16, and 32
  - e.g. `Tree16` is a depth 16 tree, and creates and verifies `Proof16`
- WASM-bindings are not yet published on npm
