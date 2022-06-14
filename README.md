# Jolt network node

`jolt-node` is the software that all Jolt workers should run in order to automate smart contract executions.

## Installation

### macOS and Linux

For macOS and Linux, installing should be as easy as opening a terminal and running:

```
curl -L https://raw.githubusercontent.com/jolt-network/node/main/install.sh | sh
```

The installer will ask you for your account's password in order to be able to move the binary to `/usr/local/bin`. If you don't trust the installation script with sudo access, you can simply download the correct binary for your platform (remember to pick the `arm64` version for Mac if you have the M1 chip) from the latest release and manually put it into `/usr/local/bin`, renaming it to `jolt-node` and making it executable (`sudo chmod +x /usr/local/bin/jolt-node`). After doing so, you should see the command being picked up by whatever shell you like to use.

### Windows

Windows installation instructions will be added in the future.