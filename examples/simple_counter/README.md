# ic-dev-kit-rs Simple Counter Example

A minimal, working example demonstrating the core features of ic-dev-kit-rs:
- **Authentication** - Protected admin functions
- **Telemetry** - Automatic logging with Canistergeek
- **Storage** - Persistent stable storage
- **Simple counter** - Demonstrates all features together

## Endpoints Reference

### Counter
| Method | Function | Auth |
|--------|----------|------|
| `increment()` | Increment counter, returns new value | Public |
| `get_counter()` | Get current value | Public |
| `reset_counter()` | Reset to 0 | Admin |

### Storage
| Method | Function | Auth |
|--------|----------|------|
| `store_message(key, message)` | Store a text message | Admin |
| `get_message(key)` | Retrieve a message | Public |
| `list_storage_keys()` | List known storage keys | Public |

### Status
| Method | Function | Auth |
|--------|----------|------|
| `canister_status()` | Get counter, admin count, caller | Public |

### Auth
| Method | Function | Auth |
|--------|----------|------|
| `authorize_principal(principal)` | Add admin | Admin |
| `deauthorize_principal(principal)` | Remove admin | Admin |
| `get_authorized_principals()` | List all admins | Admin |
| `check_principal_authorized(principal)` | Check if principal is admin | Admin |
| `get_authorized_count()` | Get admin count | Admin |

### Telemetry (Canistergeek)
| Method | Function | Auth |
|--------|----------|------|
| `getCanistergeekInformation(request)` | Get metrics/status | Monitor |
| `updateCanistergeekInformation(request)` | Update/collect metrics | Monitor |
| `getCanisterLog(request)` | Get log messages | Monitor |
| `authorize_monitoring(principal)` | Add monitoring principal | Admin |
| `deauthorize_monitoring(principal)` | Remove monitoring principal | Admin |
| `get_monitoring_principals()` | List monitoring principals | Monitor |

## Quick Start

```bash
# Start dfx
dfx start --clean --background

# Deploy
dfx deploy example_canister

# Test counter
dfx canister call example_canister increment
dfx canister call example_canister get_counter
```

## Example Usage

### Counter Operations
```bash
dfx canister call example_canister increment
# (1 : nat64)

dfx canister call example_canister increment
# (2 : nat64)

dfx canister call example_canister get_counter
# (2 : nat64)

dfx canister call example_canister reset_counter
# ("Counter reset to 0")
```

### Storage Operations
```bash
dfx canister call example_canister store_message '("hello", "Hello World!")'
# ("Stored message under key: hello")

dfx canister call example_canister get_message '("hello")'
# (opt "Hello World!")
```

### Status
```bash
dfx canister call example_canister canister_status
# ("Counter: 2\nAdmins: 1\nCaller: xxxxx-xxxxx-xxxxx-xxxxx-xxx")
```

### Auth Operations
```bash
# List admins
dfx canister call example_canister get_authorized_principals

# Add admin
dfx canister call example_canister authorize_principal '(principal "aaaaa-aa")'

# Remove admin
dfx canister call example_canister deauthorize_principal '(principal "aaaaa-aa")'

# Check if principal is admin
dfx canister call example_canister check_principal_authorized '(principal "aaaaa-aa")'

# Get admin count
dfx canister call example_canister get_authorized_count
```

### Telemetry Operations
```bash
# Get Canistergeek information
dfx canister call example_canister getCanistergeekInformation '(record {
    status = null;
    metrics = opt variant { normal };
    logs = null;
    version = true
})'

# Get logs
dfx canister call example_canister getCanisterLog '(variant { getLatestMessages = record { count = 10 : nat32 } })'

# Monitoring principals
dfx canister call example_canister get_monitoring_principals
dfx canister call example_canister authorize_monitoring '(principal "aaaaa-aa")'
dfx canister call example_canister deauthorize_monitoring '(principal "aaaaa-aa")'
```

## Testing Persistence (Upgrade)

```bash
# Increment counter
dfx canister call example_canister increment
dfx canister call example_canister increment
dfx canister call example_canister get_counter
# (2 : nat64)

# Upgrade canister
dfx deploy example_canister --upgrade-unchanged

# Counter value persists!
dfx canister call example_canister get_counter
# (2 : nat64)
```

## Project Structure

```
examples/simple_counter/
├── dfx.json
├── Cargo.toml
├── src/
│   └── example_canister/
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs
└── README.md
```

## How It Works

### When you call `increment()`:
1. **Telemetry**: `collect_metrics()` tracks the call
2. **Counter**: Value incremented in thread-local storage
3. **Logging**: Increment logged via `log_info()`
4. **Storage**: New value saved to stable storage
5. **Response**: New counter value returned

### On Upgrade:
1. **Pre-upgrade**: Auth and telemetry state saved to stable storage
2. **Post-upgrade**: State restored from stable storage

## Troubleshooting

### "Unauthorized" error
```bash
# Get your principal
dfx identity get-principal

# Verify you're an admin
dfx canister call example_canister get_authorized_principals
```

### Build errors with ic-dev-kit-rs
Ensure `Cargo.toml` has correct path and features:
```toml
ic-dev-kit-rs = { path = "../../", features = ["telemetry", "storage"] }
```

## Resources

- [Internet Computer Docs](https://internetcomputer.org/docs)
- [Canistergeek](https://github.com/usergeek/canistergeek-ic-rust)