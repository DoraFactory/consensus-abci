#!/bin/bash

# build the repo
cargo build --release

# mkdir log && cd log && touch log.server && touch log.client && cd ..

# start abci server(counter app)
nohup ../target/release/counter

# start abci client(consensus)
nohup ../target/release/consensus-node run




