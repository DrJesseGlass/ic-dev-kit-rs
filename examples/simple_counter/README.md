# ic-dev-kit-rs Simple Counter Example

A minimal, working example demonstrating the core features of ic-dev-kit-rs:
- **Authentication** - Protected admin functions
- **Telemetry** - Automatic logging with Canistergeek
- **Storage** - Persistent stable storage
- **Simple counter** - Demonstrates all features together

## Features Demonstrated

### 1. Counter (Main Feature)
- `increment()` - Increment counter and log to telemetry
- `get_counter()` - Get current value
- `reset_counter()` - Reset to 0 (admin only)

### 2. Storage
- `store_message(key, message)` - Store a text message (admin only)
- `get_message(key)` - Retrieve a message
- Counter value persists across upgrades

### 3. Authentication
- `whoami()` - Get your principal (admin only)
- `get_admins()` - List all authorized principals (admin only)
- `add_admin(principal)` - Add a new admin (admin only)
- Deployer is automatically added as admin

### 4. Telemetry
- All operations are logged
- Metrics tracked automatically
- View logs in Canistergeek dashboard

## Quick Start

### 1. Setup Project
```bash
cd ~/Documents/jeshli/solo/ic-dev-kit-rs

# Create example directory
mkdir -p examples/simple_counter
cd examples/simple_counter

# Run setup script (or follow manual steps below)
chmod +x ../../setup_example.sh
../../setup_example.sh
```

### 2. Manual Setup (Alternative)
```bash
# Create directory structure
mkdir -p examples/simple_counter/src/example_canister/src
cd examples/simple_counter

# Copy files
cp /path/to/dfx.json .
cp /path/to/Cargo_workspace.toml ./Cargo.toml
cp /path/to/Cargo_canister.toml ./src/example_canister/Cargo.toml
cp /path/to/simple_example_canister.rs ./src/example_canister/src/lib.rs

# Adjust the ic-dev-kit-rs path in src/example_canister/Cargo.toml
# Change: path = "../../../../ic-dev-kit-rs"
# To match your actual ic-dev-kit-rs location
```

### 3. Deploy
```bash
# Start dfx (if not already running)
dfx start --clean --background

# Deploy the canister
dfx deploy

# You should see:
# ✓ Canister created with ID: xxx
# ✓ Canister installed
```

### 4. Test
```bash
# Run automated tests
chmod +x test_example.sh
./test_example.sh

# Or test manually:
dfx canister call example_canister increment
# Output: (1 : nat64)

dfx canister call example_canister increment
# Output: (2 : nat64)

dfx canister call example_canister get_counter
# Output: (2 : nat64)
```

## Example Usage

### Increment Counter
```bash
$ dfx canister call example_canister increment
(1 : nat64)

$ dfx canister call example_canister increment
(2 : nat64)

$ dfx canister call example_canister get_counter
(2 : nat64)
```

### Store and Retrieve Messages
```bash
$ dfx canister call example_canister store_message '("hello", "Hello World!")'
("Stored message under key: hello")

$ dfx canister call example_canister get_message '("hello")'
(opt "Hello World!")
```

### Check Status
```bash
$ dfx canister call example_canister canister_status
(
  "Counter: 2
   Admins: 1
   Caller: xxxxx-xxxxx-xxxxx-xxxxx-xxx"
)
```

### View Logs (Telemetry)
```bash
# Every increment is logged!
$ dfx canister call example_canister get_canister_log_query '(record { count = 10 })'
```

### Reset Counter (Admin Only)
```bash
$ dfx canister call example_canister reset_counter
("Counter reset to 0")

$ dfx canister call example_canister get_counter
(0 : nat64)
```

## Testing Telemetry

The canister automatically logs every operation:
- Counter increments
- Message storage
- Admin additions
- Counter resets

View the logs using Canistergeek dashboard or query directly:

```bash
dfx canister call example_canister get_canistergeek_information '(record {
    status = null;
    metrics = opt variant { normal };
    logs = opt record { count = 20 };
    version = true
})'
```

## Testing Persistence (Upgrade)

The counter persists across upgrades:

```bash
# Increment counter
dfx canister call example_canister increment
dfx canister call example_canister increment
dfx canister call example_canister get_counter
# Output: (2 : nat64)

# Upgrade canister
dfx deploy example_canister --upgrade-unchanged

# Counter value persists!
dfx canister call example_canister get_counter
# Output: (2 : nat64)
```

## Project Structure

```
examples/simple_counter/
├── dfx.json                          # DFX configuration
├── Cargo.toml                        # Workspace config
├── src/
│   └── example_canister/
│       ├── Cargo.toml               # Canister dependencies
│       ├── example_canister.did     # Candid interface
│       └── src/
│           └── lib.rs               # Canister code
├── test_example.sh                  # Test script
└── README.md                        # This file
```

## What's Happening Under the Hood

### When you call `increment()`:

1. **Telemetry**: `collect_metrics()` tracks the call
2. **Counter**: Value incremented in thread-local storage
3. **Logging**: Increment logged: `"Counter incremented to X"`
4. **Storage**: New value saved to stable storage
5. **Response**: New counter value returned

### On Upgrade:

1. **Pre-upgrade**: 
   - Auth state saved to stable storage
   - Counter already saved (done on each increment)

2. **Post-upgrade**:
   - Auth state restored
   - Counter value restored from stable storage
   - Telemetry reinitialized

## Key Concepts Demonstrated

### 1. Thread-Local Storage
```rust
thread_local! {
    static COUNTER: RefCell<u64> = RefCell::new(0);
}
```
Fast, in-memory storage for the counter value.

### 2. Stable Storage
```rust
STORAGE.with(|s| {
    ic_dev_kit_rs::storage::save_bytes(s, "counter", value.to_le_bytes());
});
```
Persistent storage that survives upgrades.

### 3. Authorization Guards
```rust
#[ic_cdk::update(guard = "ic_dev_kit_rs::auth::is_authorized")]
fn reset_counter() -> String { ... }
```
Only authorized principals can call this function.

### 4. Automatic Telemetry
```rust
ic_dev_kit_rs::telemetry::collect_metrics();
ic_dev_kit_rs::telemetry::log_info("Counter incremented");
```
Every operation is logged and tracked.

## Troubleshooting

### Build Errors

**Problem**: `ic-dev-kit-rs` not found  
**Solution**: Adjust the path in `src/example_canister/Cargo.toml`

**Problem**: Feature errors  
**Solution**: Make sure `features = ["telemetry", "storage"]` is set

### Deployment Errors

**Problem**: Candid interface mismatch  
**Solution**: Run `dfx generate` to regenerate interfaces

**Problem**: Memory issues  
**Solution**: The example uses minimal memory, should not occur

### Runtime Errors

**Problem**: "Unauthorized" when calling protected functions  
**Solution**: Make sure you're calling as the deployer, or add your principal

```bash
# Get your principal
dfx identity get-principal

# Add yourself as admin (call as deployer)
dfx canister call example_canister add_admin '(principal "YOUR-PRINCIPAL")'
```

## Next Steps

Once you understand this example:

1. **Explore more features**: Check out the full ic-dev-kit-rs documentation
2. **Build your own**: Use this as a template for your canister
3. **Add HTTP endpoints**: See the full example_canister.rs for HTTP support
4. **Add file uploads**: Use the large_objects module for chunked uploads
5. **Add LLM support**: Check out the Qwen3 example for ML models

## Resources

- [ic-dev-kit-rs GitHub](https://github.com/DrJesseGlass/ic-dev-kit-rs)
- [Internet Computer Docs](https://internetcomputer.org/docs)
- [Canistergeek](https://github.com/usergeek/canistergeek-ic-rust)

## License

This example is part of ic-dev-kit-rs and uses the same license.