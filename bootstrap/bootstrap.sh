#!/usr/bin/env bash

# -e: Exit immediately if a command exits with non-zero status
# -u: Treat unset variables as an error
# -o pipefail: Ensure pipeline errors are captured
set -euo pipefail

sudo apt update
sudo apt upgrade -y
sudo apt install -y docker.io docker-compose
mkdir bridge
mv keypair.json bridge/
sudo docker-compose up -d
shred -u bridge/keypair.json
rm -rf bridge
# TODO: put additional admin SSH pubkeys