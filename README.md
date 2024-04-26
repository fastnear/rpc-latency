# RPC Latency tester server

Runs a list of tests in a loop, measuring the time it takes to complete each test.
Reports metrics in Prometheus format.

## Usage

See `demo_config.json` for an example configuration file.

Create `.env` file with the following content:

```
RUST_LOG="rpc=debug,actix_server=info,actix_web=info"
RPC_CONFIG_PATH="demo_config.json"
PORT=3005
```

Run the server:

```bash
cargo run --release
```

Query the metrics:

```bash
curl localhost:3005/metrics
```
