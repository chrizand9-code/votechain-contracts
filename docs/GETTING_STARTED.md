# Getting Started Guide for Contributors

Welcome to VoteChain! This guide will help you set up your development environment and make your first contribution.

## Video Walkthrough

Prefer to learn by watching?  The **[Video Walkthrough for First-Time Contributors](video-walkthrough.md)** is a ~10-minute screen-recording that covers the complete setup and contribution workflow — clone, install dependencies, run tests, make a small change, and run tests again.

A full written transcript is available in the same document for accessibility.

> **Video status:** 🎬 Pending recording — the transcript and placeholder are
> ready; the video link will be updated once it is uploaded.

---

## Prerequisites

Before you begin, ensure you have the following installed on your system:

### Required Software

| Tool | Minimum Version | Purpose |
|------|----------------|---------|
| **Rust** | 1.75.0+ | Core language for smart contracts |
| **Cargo** | 1.75.0+ | Rust package manager (comes with Rust) |
| **Stellar CLI** | Latest | Build and deploy Soroban contracts |
| **Git** | 2.0+ | Version control |

### Optional but Recommended

| Tool | Purpose |
|------|---------|
| **VS Code** | IDE with Rust extensions |
| **rust-analyzer** | Rust language server for IDE support |

## Environment Setup

### Step 1: Install Rust

Install Rust using rustup (the official Rust installer):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the on-screen instructions. After installation, restart your terminal and verify:

```bash
rustc --version
cargo --version
```

### Step 2: Add WebAssembly Target

Soroban contracts compile to WebAssembly. Add the wasm32 target:

```bash
rustup target add wasm32-unknown-unknown
```

### Step 3: Install Stellar CLI

The Stellar CLI is required to build and deploy Soroban contracts:

```bash
cargo install --locked stellar-cli --features opt
```

Verify the installation:

```bash
stellar --version
```

### Step 4: Clone the Repository

Clone the VoteChain repository and navigate to the project directory:

```bash
git clone https://github.com/veracindarella/votechain-contracts.git
cd votechain-contracts
```

### Step 5: Build the Project

Build both contracts (governance and token):

```bash
make build
```

This compiles the contracts to `.wasm` files in the `target/wasm32-unknown-unknown/release/` directory.

### Step 6: Run Tests

Run the full test suite to ensure everything is working:

```bash
make test
```

You should see output indicating all tests passed:

```
running 45 tests
test test_create_proposal ... ok
test test_cast_vote_and_finalise_passed ... ok
...
test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Running Tests

### Run All Tests

```bash
make test
```

### Run Tests for a Specific Contract

```bash
# Governance contract only
cargo test -p votechain-governance

# Token contract only
cargo test -p votechain-token
```

### Run a Specific Test

```bash
cargo test test_create_proposal
```

### Run Tests with Output

By default, Rust captures test output. To see println! statements:

```bash
cargo test -- --nocapture
```

### Run Tests in Verbose Mode

```bash
cargo test -- --test-threads=1 --nocapture
```

## Code Quality Checks

Before submitting a pull request, ensure your code passes all quality checks:

### Format Check

```bash
make fmt-check
```

If formatting issues are found, auto-fix them:

```bash
make fmt
```

### Linting

Run Clippy (Rust's linter) to catch common mistakes:

```bash
make lint
```

Fix any warnings or errors before committing.

## Deploying to Testnet

### Step 1: Configure Testnet

Ensure you have a Stellar testnet account with XLM for fees. You can get testnet XLM from the [Stellar Laboratory](https://laboratory.stellar.org/#account-creator?network=test).

### Step 2: Set Environment Variables

Create a `.env` file in the project root (this file is gitignored):

```bash
# Stellar testnet configuration
STELLAR_NETWORK=testnet
STELLAR_RPC_URL=https://soroban-testnet.stellar.org
STELLAR_NETWORK_PASSPHRASE="Test SDF Network ; September 2015"

# Your testnet account secret key (NEVER commit this!)
STELLAR_SECRET_KEY=S...
```

### Step 3: Deploy Contracts

Deploy the token contract first, then the governance contract:

```bash
NETWORK=testnet ./scripts/deploy.sh
```

The script will:
1. Build both contracts
2. Deploy the token contract
3. Deploy the governance contract
4. Initialize both contracts
5. Output the deployed contract addresses

### Step 4: Verify Deployment

Check the deployment on [Stellar Expert](https://stellar.expert/explorer/testnet) by searching for your contract addresses.

## Docker Development

If you would rather not install Rust, the Stellar CLI, and a local Stellar node directly on your
machine, the project ships a `docker-compose.yml` that gives you a fully working development
environment — a dev container with the Rust toolchain, a local Stellar/Soroban node, the backend
API, and Redis — with a single command.

### Step 1: Install Docker

Install Docker Desktop (macOS/Windows) or Docker Engine + the Compose plugin (Linux):

- macOS/Windows: [Docker Desktop](https://www.docker.com/products/docker-desktop/)
- Linux: follow the [official install guide](https://docs.docker.com/engine/install/) and then
  install the [Compose plugin](https://docs.docker.com/compose/install/linux/)

Verify the installation:

```bash
docker --version
docker compose version
```

### Step 2: Start the Environment

From the repository root:

```bash
docker compose up -d
```

This starts four services, defined in `docker-compose.yml`:

| Service | Purpose | Port |
|---------|---------|------|
| `stellar-node` | Local Soroban-enabled Stellar node (`stellar/quickstart`) | `8000` |
| `dev` | Rust toolchain container for building/testing contracts | — |
| `backend` | Node.js API server | `3001` |
| `redis` | Cache used by the backend | `6379` |

`dev` and `backend` wait on their `depends_on` healthchecks, so `stellar-node` and `redis` will be
reported healthy before the dependent services start.

Check that everything came up correctly:

```bash
docker compose ps
```

### Step 3: Run Tests Inside the Container

Run the Rust contract test suite inside the `dev` container instead of installing the toolchain
locally:

```bash
docker compose exec dev make test
```

Other `make` targets work the same way, for example:

```bash
docker compose exec dev make fmt-check
docker compose exec dev make lint
docker compose exec dev make build
```

Follow container logs (useful for the backend or the local node) with:

```bash
docker compose logs -f backend
docker compose logs -f stellar-node
```

### Step 4: Deploy to the Local Node

Once the contracts are built inside the `dev` container, deploy them to the local `stellar-node`
service:

```bash
docker compose exec dev bash -c "NETWORK=local ./scripts/deploy.sh"
```

The `dev` service is already configured with `NETWORK=local` and depends on `stellar-node` being
healthy, so no extra network flags are needed for local experimentation.

### Step 5: Inspect the Local Stellar Node

The `stellar-node` service runs the [`stellar/quickstart`](https://github.com/stellar/quickstart)
image with `ENABLE_SOROBAN_RPC=true`, exposing the standard local-network endpoints on port `8000`:

- **Horizon UI / API root:** [http://localhost:8000/](http://localhost:8000/) — confirms the node
  is up and reports the current ledger.
- **Friendbot (fund a local test account):**

  ```bash
  curl "http://localhost:8000/friendbot?addr=<YOUR_PUBLIC_KEY>"
  ```

- **Soroban RPC endpoint:** `http://localhost:8000/soroban/rpc` — used by the Stellar CLI and
  `scripts/deploy.sh` when `NETWORK=local`.

### Docker Troubleshooting

#### Port conflicts (`port is already allocated`)

Another process is already bound to `8000`, `3001`, or `6379`. Either stop that process or remap
the port in `docker-compose.yml`, e.g.:

```yaml
backend:
  ports:
    - "3011:3001"
```

You can find what's using a port with:

```bash
lsof -i :8000
```

#### Image pull failures

If `docker compose up` fails to pull `stellar/quickstart` or `redis:7-alpine` (registry timeouts,
rate limits, or DNS issues):

```bash
# Retry the pull explicitly with more verbose output
docker compose pull stellar-node

# If you're being rate-limited by Docker Hub, log in to raise your pull limit
docker login
```

#### `dev` container can't reach `stellar-node`

The healthcheck on `stellar-node` can take up to `start_period: 30s` before the endpoint responds.
If `dev` or `backend` exit early, wait for the healthcheck to pass first:

```bash
docker compose ps stellar-node
docker compose logs stellar-node
```

#### Stale containers/volumes after a Cargo.toml change

Rebuild the `dev` image and clear the cargo registry cache volume:

```bash
docker compose build dev
docker compose down -v
docker compose up -d
```

### Step 6: Tear Down

```bash
docker compose down
```

Add `-v` to also remove the `cargo-cache` volume if you want a completely clean slate.

## Making Your First Contribution

### Step 1: Create a Branch

Create a new branch for your feature or bug fix:

```bash
git checkout -b feature/my-new-feature
```

Use descriptive branch names:
- `feature/add-delegation`
- `fix/double-vote-bug`
- `test/quorum-edge-cases`
- `docs/update-readme`

### Step 2: Make Your Changes

Edit the relevant files. Common areas:

| Area | Files |
|------|-------|
| Governance logic | `contracts/governance/src/lib.rs` |
| Token logic | `contracts/token/src/lib.rs` |
| Events | `contracts/*/src/events.rs` |
| Tests | `contracts/*/src/test.rs` |
| Documentation | `README.md`, `docs/` |

### Step 3: Write Tests

Every new function or bug fix should include tests. Add tests to the appropriate `test.rs` file:

```rust
#[test]
fn test_my_new_feature() {
    let t = setup_env();
    // Your test code here
    assert_eq!(expected, actual);
}
```

### Step 4: Run Quality Checks

Before committing, ensure all checks pass:

```bash
make test
make fmt
make lint
```

### Step 5: Commit Your Changes

Follow [Conventional Commits](https://www.conventionalcommits.org/) format:

```bash
git add .
git commit -m "feat: add delegation support to governance contract"
```

Commit message prefixes:
- `feat:` - New feature
- `fix:` - Bug fix
- `test:` - Adding or updating tests
- `docs:` - Documentation changes
- `refactor:` - Code refactoring
- `chore:` - Maintenance tasks

### Step 6: Push and Create a Pull Request

Push your branch to GitHub:

```bash
git push origin feature/my-new-feature
```

Then create a pull request on GitHub with:
- Clear title describing the change
- Description of what was changed and why
- Reference to any related issues (e.g., "Closes #42")
- Screenshots or examples if applicable

## Common Troubleshooting

### Issue: `cargo test` fails with "could not compile"

**Solution:** Ensure you're using Rust 1.75.0 or later:

```bash
rustup update
```

### Issue: `stellar: command not found`

**Solution:** Reinstall the Stellar CLI:

```bash
cargo install --locked stellar-cli --features opt
```

Ensure `~/.cargo/bin` is in your PATH.

### Issue: `wasm32-unknown-unknown` target not found

**Solution:** Add the WebAssembly target:

```bash
rustup target add wasm32-unknown-unknown
```

### Issue: Tests fail with "already initialized" error

**Solution:** This is expected behavior for re-initialization tests. If other tests fail, ensure you're running the latest code:

```bash
git pull origin main
cargo clean
make build
make test
```

### Issue: Deployment fails with "insufficient balance"

**Solution:** Ensure your testnet account has XLM. Get testnet XLM from:
- [Stellar Laboratory](https://laboratory.stellar.org/#account-creator?network=test)
- [Friendbot](https://friendbot.stellar.org/)

### Issue: `make build` fails with linker errors

**Solution:** Ensure you have the correct Rust toolchain:

```bash
rustup default stable
rustup target add wasm32-unknown-unknown
```

### Issue: Clippy warnings about unused imports

**Solution:** Remove unused imports or allow them temporarily:

```rust
#[allow(unused_imports)]
use soroban_sdk::...;
```

### Issue: Format check fails

**Solution:** Auto-format your code:

```bash
make fmt
```

## Project Structure

Understanding the project layout:

```
votechain-contracts/
├── contracts/
│   ├── governance/          # Governance contract
│   │   ├── src/
│   │   │   ├── lib.rs       # Main contract logic
│   │   │   ├── storage.rs   # Storage helpers
│   │   │   ├── events.rs    # Event emissions
│   │   │   ├── types.rs     # Type definitions
│   │   │   └── test.rs      # Unit tests
│   │   └── Cargo.toml       # Contract dependencies
│   └── token/               # Token contract (similar structure)
├── docs/                    # Documentation
│   ├── adr/                 # Architecture Decision Records
│   ├── examples/            # Usage examples
│   └── security/            # Security documentation
├── scripts/                 # Deployment scripts
├── config/                  # Network configurations
├── Cargo.toml               # Workspace configuration
├── Makefile                 # Build commands
└── README.md                # Project overview
```

## Development Workflow

1. **Pick an issue** from the GitHub issues page or create a new one
2. **Create a branch** with a descriptive name
3. **Write code** following the project's style and conventions
4. **Write tests** for your changes
5. **Run quality checks** (test, fmt, lint)
6. **Commit** with a conventional commit message
7. **Push** and create a pull request
8. **Respond to feedback** from maintainers
9. **Merge** once approved

## Coding Standards

- **No `std`:** All contracts use `#![no_std]`
- **Error handling:** Use `Result<T, ContractError>` for fallible operations
- **Events:** Emit events for all state-changing operations
- **Documentation:** Add `///` doc comments to all public functions
- **Testing:** Every function needs at least one test
- **No floating-point:** Use `i128` for all numeric values
- **Formatting:** Run `make fmt` before committing
- **Linting:** Fix all Clippy warnings

## Getting Help

- **GitHub Issues:** [github.com/veracindarella/votechain-contracts/issues](https://github.com/veracindarella/votechain-contracts/issues)
- **Stellar Discord:** [discord.gg/stellar](https://discord.gg/stellar) - #soroban channel
- **Stellar Docs:** [developers.stellar.org/docs/smart-contracts](https://developers.stellar.org/docs/smart-contracts)
- **Soroban Examples:** [github.com/stellar/soroban-examples](https://github.com/stellar/soroban-examples)

## Next Steps

Now that you're set up:

1. Read the [CONTRIBUTING.md](../CONTRIBUTING.md) for contribution guidelines
2. Review the [Architecture Decision Records](adr/README.md) to understand design choices
3. Check the [FAQ](faq.md) for common questions
4. Browse [open issues](https://github.com/veracindarella/votechain-contracts/issues) to find something to work on
5. Join the Stellar Discord to connect with the community

Happy coding! 🚀

## Health Endpoints

The VoteChain backend exposes two HTTP endpoints for liveness and readiness probing. These are available at the root path (not under `/api`) so that load balancers and container orchestrators (Docker, Kubernetes, etc.) can reach them without API credentials or rate-limit concerns.

### GET /health — Liveness Probe

Confirms the Node.js process is alive and able to handle requests.

**Always returns `200 OK`** as long as the process is running.

```bash
curl http://localhost:3001/health
```

**Response:**
```json
{
  "status": "ok",
  "uptime": 42.3,
  "version": "1.0.0"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `status` | `string` | Always `"ok"` |
| `uptime` | `number` | Seconds since the process started (`process.uptime()`) |
| `version` | `string` | Backend version from `package.json` |

---

### GET /ready — Readiness Probe

Confirms all external dependencies (currently Redis) are healthy before accepting traffic.

**Returns `200 OK`** when Redis is connected:

```bash
curl http://localhost:3001/ready
```

```json
{ "status": "ready" }
```

**Returns `503 Service Unavailable`** when Redis is not connected:

```json
{
  "status": "unavailable",
  "checks": { "redis": false }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `status` | `string` | `"ready"` or `"unavailable"` |
| `checks.redis` | `boolean` | `true` when the Redis connection is open |

---

### Docker Compose Healthcheck

The `backend` service in `docker-compose.yml` is configured with a healthcheck that polls `/health` every 30 seconds:

```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:3001/health"]
  interval: 30s
  timeout: 10s
  retries: 3
```

This means other services that use `depends_on: backend` with a `condition: service_healthy` will wait until the backend reports healthy before starting.

---

## Troubleshooting

Common failure modes encountered during local, Docker, and CI setup. Each entry includes the exact error message snippet for searchability. See also the [FAQ](faq.md).

### Local Environment

1. **Wrong Rust version**
   - Error: `error: package requires rustc 1.75.0 or newer`
   - Fix: `rustup update stable && rustup default stable`

2. **WASM target missing**
   - Error: `error[E0463]: can't find crate for core (target: wasm32-unknown-unknown)`
   - Fix: `rustup target add wasm32-unknown-unknown`

3. **Stellar CLI version mismatch**
   - Error: `error: unknown command 'contract'` or `stellar: command not found`
   - Fix: `cargo install --locked stellar-cli --features opt`

4. **Cargo lock mismatch**
   - Error: `error: the lock file ... needs to be updated`
   - Fix: `cargo update` then re-run tests.

5. **Permission denied on cargo bin**
   - Error: `Permission denied (os error 13)`
   - Fix: `chmod +x ~/.cargo/bin/*` and add to PATH.

### Docker

6. **Docker networking issues**
   - Error: `Cannot connect to the Docker daemon`
   - Fix: Start Docker service: `sudo systemctl start docker`.

7. **Container cannot reach RPC**
   - Error: `connection refused` to Stellar RPC
   - Fix: Use host networking or configure proxy URL in `.env`.

8. **Volume mount permission errors**
   - Error: `Permission denied` on mounted volumes
   - Fix: `chown -R $USER:$USER .` or run with `--user`.

### CI

9. **CI cache miss / slow builds**
   - Error: `cache not found` in workflow logs
   - Fix: Re-run workflow; ensure `Cargo.lock` committed.

10. **Missing secrets in CI**
    - Error: `secret not found` / empty env var
    - Fix: Add required secrets in repo settings.
