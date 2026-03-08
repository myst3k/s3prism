# S3Prism — Multi-Region Erasure-Coded S3 Gateway

> *One beam in, many regions out. Recombine from anywhere.*

*Date: 2026-03-06*
*Language: Rust*

## Overview

An open-source, Rust-based S3-compatible gateway that distributes data across multiple S3-compatible storage regions. The user provides their storage credentials, picks a redundancy level, and selects regions (or lets the gateway auto-pick based on geography). The gateway presents a single S3 endpoint with virtual buckets backed by erasure-coded data distributed across multiple storage sites.

Uses Reed-Solomon erasure coding to distribute object data across sites, and the Rob Pike fan-out pattern to achieve lowest-latency reads and writes. Simulates availability zone redundancy — a single namespace with cross-region durability, automatic failover, and best-latency reads.

Standalone binary — just needs S3-compatible storage credentials to run. No internal APIs, no special access. Standard S3 protocol only.

## Getting Started

### Install and Launch

```
$ s3prism serve
  S3Prism v0.1.0 starting...
  Management UI: https://localhost:9090
  S3 endpoint:   https://localhost:8443 (not configured yet)

  Open the management UI to complete setup.
```

On first launch, S3Prism starts in **unconfigured mode** — the management web UI is available, but the S3 endpoint returns 503 until setup is complete. All configuration happens through the UI.

### First-Time Setup (Web UI)

**Step 1: Credentials**
- Enter storage access key and secret key
- Gateway validates credentials against the storage endpoint (test API call)
- Optionally add multiple credential sets for different regions

**Step 2: Region Benchmarking**
- Click "Run Benchmark" to test all available storage regions
- Benchmark runs real transfers (configurable test object sizes: 1KB, 1MB, 10MB, 100MB)
- Results show per-region:
  - Upload throughput (MB/s) — small and large objects
  - Download throughput (MB/s) — small and large objects
  - Latency: connect, first-byte, full transfer
  - Jitter (latency variance)
  - Packet loss (via retry rate)
- Results displayed as a ranked table + bar charts
- Benchmark can be re-run anytime from the Sites page
- Historical benchmark results are stored for trend analysis

**Step 3: Select Regions**
- Pick which regions to use from the benchmark results
- UI highlights recommended regions based on benchmark scores
- Minimum: 3 regions (for 2+1 scheme)
- Drag to reorder priority (primary region first)

**Step 4: Redundancy Scheme**
- Choose from presets or custom:
  - Standard (2+1) — 1.5x storage, survives 1 region loss
  - Enhanced (2+2) — 2x storage, survives 2 region losses
  - Maximum (3+2) — 1.67x storage, survives 2 region losses, needs 5 regions
  - Custom — pick k and m values
- UI shows storage cost multiplier and failure tolerance for each option
- Validates that enough regions are selected for the chosen scheme

**Step 5: Review and Activate**
- Summary of all settings
- Gateway creates backend buckets at each selected region
- Verifies write access to all regions
- Activates the S3 endpoint
- Shows connection instructions for S3 clients

### After Setup

```
S3Prism configured and running.

  S3 endpoint:   https://localhost:8443
  Management UI: https://localhost:9090
  Regions:       us-east-1, us-east-2, us-west-1
  Scheme:        2+1 (Reed-Solomon)
  Status:        All regions healthy

  Connect with any S3 client:
    aws s3 --endpoint-url https://localhost:8443 mb s3://my-data
    aws s3 --endpoint-url https://localhost:8443 cp file.txt s3://my-data/
```

### Region Benchmark Detail

The benchmark is not a simple ping — it tests real-world S3 performance:

```
Benchmark: us-east-1
  Latency:
    Connect:     12ms (avg)    14ms (P95)    18ms (P99)
    First byte:  24ms (avg)    31ms (P95)    45ms (P99)

  Upload throughput:
    1KB objects:   842 ops/sec    0.8 MB/s
    1MB objects:   124 ops/sec  124.0 MB/s
    10MB objects:   18 ops/sec  180.0 MB/s
    100MB objects:   2 ops/sec  210.0 MB/s

  Download throughput:
    1KB objects:  1204 ops/sec    1.2 MB/s
    1MB objects:   156 ops/sec  156.0 MB/s
    10MB objects:   22 ops/sec  220.0 MB/s
    100MB objects:   3 ops/sec  280.0 MB/s

  Reliability:
    Success rate: 100.0%
    Retries:      0
    Errors:       0

  Score: 94/100 (Excellent)
```

The benchmark creates temporary test objects, transfers them, and cleans up. Results are stored in the internal database and visible in the Analytics page for historical comparison.

### What the User Gets

- **Single endpoint** — one URL, works with any S3 client (aws-cli, boto3, rclone, etc.)
- **Virtual buckets** — look like normal S3 buckets, but data is erasure-coded across regions
- **Automatic failover** — if a storage region goes down, reads reconstruct from remaining chunks
- **Best-latency reads** — fan-out to all regions, return from whichever k respond first
- **Configurable redundancy** — choose how many region failures to survive, and how data is distributed
- **Multiple storage modes** — full replicas (plain S3 objects, no lock-in) or erasure coded (storage efficient, requires gateway or recovery tool to reconstruct)
- **Full web UI** — configure, monitor, and manage everything from the browser

## Core Concepts

### Write Path (PUT)

```
Client PUT /my-object (100MB)
    |
    v
Gateway receives full object
    |
    v
Reed-Solomon encode: split into k data chunks + m parity chunks
    |
    +---> tokio::spawn ---> us-east-1  (chunk 0, 50MB)  * first!
    +---> tokio::spawn ---> us-east-2  (chunk 1, 50MB)  * second! --> return 200 OK
    +---> tokio::spawn ---> us-west-1  (parity,  50MB)  ... (background, still writing)
    |
    v
Update metadata store: object -> chunk locations
Background task confirms all chunks landed
```

- Fan out chunk uploads to all sites concurrently
- Return success after k chunks are confirmed (minimum needed to reconstruct)
- Remaining chunks complete in background
- If a background chunk fails, queue it for retry in the reconciler

### Read Path (GET)

```
Client GET /my-object
    |
    v
Gateway looks up metadata: chunks at us-east-1, us-east-2, us-west-1
    |
    +---> tokio::spawn ---> us-east-1  (chunk 0)  * first!
    +---> tokio::spawn ---> us-east-2  (chunk 1)  * second! --> reconstruct, stream to client
    +---> tokio::spawn ---> us-west-1  (parity)   ... cancel, not needed
```

- Fan out GET requests for all chunks concurrently
- As soon as any k chunks arrive, reconstruct the original object
- Cancel remaining in-flight requests
- Client gets latency of the k-th fastest site, not the slowest

### Delete Path (DELETE)

```
Client DELETE /my-object
    |
    v
Gateway looks up metadata: chunks at us-east-1, us-east-2, us-west-1
    |
    +---> tokio::spawn ---> DELETE us-east-1/chunk-0
    +---> tokio::spawn ---> DELETE us-east-2/chunk-1
    +---> tokio::spawn ---> DELETE us-west-1/parity
    |
    v
Return 204 after all succeed (or queue failures for retry)
Remove from metadata store
```

## Storage Modes

S3Prism supports multiple storage modes. These can be configured globally, per-bucket, or even per-object via custom headers.

### Mode 1: Full Replica

```
Object: "report.pdf" (10MB)
    --> us-east-1: "report.pdf"     (10MB, complete copy)
    --> us-east-2: "report.pdf"     (10MB, complete copy)
    --> us-west-1: "report.pdf"     (10MB, complete copy)
```

- Every object is stored as a complete, standard S3 object at every site
- **No lock-in** — data is directly readable from any storage site without S3Prism
- Storage cost: Nx (3 sites = 3x)
- Best for: compliance workloads, Object Lock, when recoverability without S3Prism matters
- Read path: fan-out GET to all sites, return first response (no reconstruction needed)
- Write path: fan-out PUT to all sites, return after quorum

### Mode 2: Erasure Coded

```
Object: "backup.tar" (100MB)
    --> us-east-1: "backup.tar.chunk-0"   (50MB, data shard)
    --> us-east-2: "backup.tar.chunk-1"   (50MB, data shard)
    --> us-west-1: "backup.tar.parity-0"  (50MB, parity shard)
```

- Object is split into k data + m parity shards via Reed-Solomon
- **Requires S3Prism (or recovery tool) to reconstruct** — individual chunks are not usable alone
- Storage cost: (k+m)/k (2+1 = 1.5x, 3+2 = 1.67x)
- Best for: large data sets where storage efficiency matters
- Read path: fan-out, reconstruct from first k chunks received
- Write path: fan-out, return after k chunks confirmed

### Mode 3: Hybrid (Default)

- **Small objects (< threshold)**: Full Replica — the overhead of erasure coding isn't worth it, and small objects benefit from direct readability
- **Large objects (>= threshold)**: Erasure Coded — storage savings scale with object size
- **Threshold**: Configurable (default: 1MB)
- **Multipart uploads**: Each part is independently distributed (natural chunk boundaries)

### Erasure Coding Schemes

| Sites | Scheme | Storage Cost | Survive | Write Quorum | Read Quorum |
|-------|--------|-------------|---------|-------------|-------------|
| 3     | 2+1    | 1.5x        | 1 site  | 2 of 3      | 2 of 3      |
| 4     | 2+2    | 2.0x        | 2 sites | 2 of 4      | 2 of 4      |
| 5     | 3+2    | 1.67x       | 2 sites | 3 of 5      | 3 of 5      |
| 6     | 4+2    | 1.5x        | 2 sites | 4 of 6      | 4 of 6      |

### Per-Bucket Storage Mode Override

```toml
# In web UI or via API
bucket "compliance-data":
  storage_mode: "replica"              # Always full replicas, regardless of size

bucket "media-archive":
  storage_mode: "erasure"              # Always erasure coded
  erasure_scheme: "3+2"

bucket "general":
  storage_mode: "hybrid"              # Default: small=replica, large=erasure
  hybrid_threshold: "1MB"
```

## Versioning (Future — Phase 3)

> **Not in MVP scope.** The architecture below is designed to be additive — the metadata store and chunk naming scheme are compatible with versioning from day one, but the implementation is deferred. MVP treats all objects as unversioned (latest-write-wins).

S3 versioning is complex enough with a single backend — with multiple regions and erasure coding, it needs careful design.

### How It Works

Each object "version" in S3Prism maps to a set of chunks (or replicas) across storage sites. The gateway maintains its own version history in the metadata store.

```
Virtual bucket: "my-data" (versioning enabled)

PUT "doc.txt" v1 -->  chunk-0@us-east-1, chunk-1@us-east-2, parity-0@us-west-1
PUT "doc.txt" v2 -->  chunk-0@us-east-1, chunk-1@us-east-2, parity-0@us-west-1
                      (different chunk keys, different data)
DELETE "doc.txt"  --> delete marker in metadata (chunks preserved)
```

### Metadata Model

```rust
struct ObjectVersion {
    version_id: String,             // Unique version ID (UUID)
    key: String,
    bucket: String,
    is_latest: bool,
    is_delete_marker: bool,
    size: u64,
    etag: String,
    last_modified: DateTime<Utc>,
    storage_mode: StorageMode,
    chunks: Vec<ChunkInfo>,         // Empty for delete markers
    user_metadata: HashMap<String, String>,
}
```

### Operations

| S3 Operation | S3Prism Behavior |
|-------------|-----------------|
| PutObject (versioning on) | Creates new version with new chunk set. Old chunks preserved. |
| GetObject | Returns latest non-delete-marker version. Reconstructs from chunks. |
| GetObject?versionId=X | Returns specific version. Reconstructs from that version's chunks. |
| DeleteObject | Creates a delete marker in metadata. Does NOT delete chunks. |
| DeleteObject?versionId=X | Permanently deletes that version's chunks from all sites. |
| ListObjectVersions | Returns all versions from metadata, including delete markers. |

### Backend Chunk Naming

Each version has its own chunks, namespaced by version ID:

```
us-east-1: s3prism-my-data-use1/doc.txt/v-{version_id}/chunk-0
us-east-2: s3prism-my-data-use2/doc.txt/v-{version_id}/chunk-1
us-west-1: s3prism-my-data-usw1/doc.txt/v-{version_id}/parity-0
```

### Versioning + Storage Modes

- **Replica mode**: Each version is a complete S3 object at each site. Backend provider's native versioning is NOT used on backend buckets (S3Prism manages versions in its own metadata). Backend bucket keys include the version ID.
- **Erasure mode**: Each version has its own chunk set. Old version chunks persist until explicitly deleted.
- **Hybrid mode**: Small object versions are full replicas, large object versions are erasure coded. Each version independently decides based on its size.

### Why Not Use Backend Provider's Native Versioning?

Tempting, but problematic:
- In EC mode, each chunk is a separate S3 object — the backend would version each chunk independently, not the logical object
- Version IDs would be per-chunk, not per-object
- Delete markers would be per-chunk — nonsensical
- ListObjectVersions would return chunk versions, not object versions
- S3Prism needs its own version tracking regardless

Backend backend buckets have versioning **disabled**. S3Prism handles all versioning in its metadata store.

## Object Lock (Future — Phase 3, WORM Compliance)

> **Not in MVP scope.** Object Lock requires versioning as a prerequisite (S3 spec mandates versioning-enabled buckets for Object Lock). This will be implemented after versioning support lands. The design is included here to ensure architectural decisions don't paint us into a corner.

Object Lock is the hardest feature to get right with distributed storage. It has legal and compliance implications — getting it wrong can mean regulatory violations.

### The Problem

S3 Object Lock guarantees:
1. Objects cannot be deleted or overwritten during a retention period
2. Compliance mode: nobody can shorten the retention or delete the object, not even the root account
3. Governance mode: privileged users can override
4. Legal holds: indefinite retention until explicitly removed

With S3Prism, the "object" is spread across multiple storage sites as chunks or replicas. The lock has to be enforced at every level.

### Design: Defense in Depth

Object Lock is enforced at **two layers**:

**Layer 1: S3Prism Metadata (Gateway-Level Lock)**
- Lock state stored in S3Prism's metadata per object version
- Gateway refuses delete/overwrite requests for locked objects
- This is the primary enforcement point for all S3 API operations

**Layer 2: Backend Storage Object Lock (Storage-Level Lock)**
- When Object Lock is enabled on a virtual bucket, S3Prism enables Object Lock on all backend backend buckets
- When writing chunks/replicas, S3Prism sets matching Object Lock retention on each backend object
- This means even if the S3Prism metadata is lost or corrupted, the backend data cannot be deleted by anyone

### Supported Modes

| Mode | Gateway Enforcement | Backend Enforcement | Override |
|------|-------------------|-------------------|----------|
| **Compliance** | Metadata lock, no override possible | Backend Compliance lock on all chunks/replicas | Cannot be shortened or removed by anyone |
| **Governance** | Metadata lock, bypass with `x-amz-bypass-governance-retention` | Backend Governance lock on all chunks/replicas | Privileged users can override |
| **Legal Hold** | Metadata flag per version | Legal hold set on all backend objects | Removed explicitly, no expiry |

### Object Lock + Storage Modes

**Replica mode (recommended for Object Lock):**
- Each replica is a complete S3 object at each storage site
- Object Lock retention is set on each replica directly via the backend provider's native Object Lock
- If S3Prism is lost, data is still locked and intact at each storage site
- **This is the safest option for compliance workloads**

**Erasure coded mode (use with caution):**
- Object Lock retention is set on each chunk/parity shard at each storage site
- Individual chunks are useless without reconstruction, but they're still locked
- If S3Prism metadata is lost, the recovery tool can reconstruct objects, but only if all chunks are still present (which Object Lock guarantees)
- Risk: if the metadata is corrupted and the recovery tool doesn't know which chunks belong together, the data is technically there but harder to reconstruct
- **Recommendation**: For Object Lock workloads, use replica mode or ensure metadata snapshots are also under Object Lock retention

### Object Lock Configuration

```toml
# Per-bucket Object Lock (set at bucket creation, cannot be disabled)
bucket "compliance-vault":
  storage_mode: "replica"              # Recommended for Object Lock
  object_lock:
    enabled: true
    default_retention:
      mode: "COMPLIANCE"
      days: 365                        # Or use "years: 1"
```

### Object Lock Metadata

```rust
struct ObjectLockState {
    mode: Option<LockMode>,            // Compliance or Governance
    retain_until: Option<DateTime<Utc>>,
    legal_hold: bool,
}

enum LockMode {
    Compliance,                        // Cannot be overridden
    Governance,                        // Can be overridden with bypass header
}
```

### Object Lock Operations

| S3 Operation | S3Prism Behavior |
|-------------|-----------------|
| PutObjectRetention | Sets retention on object version in metadata + all backend chunks/replicas |
| GetObjectRetention | Returns retention from metadata |
| PutObjectLegalHold | Sets legal hold in metadata + on all backend objects |
| GetObjectLegalHold | Returns legal hold from metadata |
| DeleteObject (locked) | **Rejected** with 403 AccessDenied |
| PutObject (overwrite locked) | **Rejected** — locked versions are immutable |

### Audit Trail

All Object Lock operations are logged with full detail for compliance auditing:
- Who set/changed the lock
- What mode and retention period
- Timestamp
- All backend objects that were locked
- Any override attempts (successful or rejected)

Audit logs are stored in the internal database and synced to S3. They should themselves be protected from deletion (stored in a separate locked bucket if possible).

## Internal Database

The gateway needs a local database that is fast, self-contained, and replicates to backend storage for durability. No external database dependencies — the user shouldn't have to run Postgres or etcd.

### Engine: RocksDB (Embedded, Battle-Tested)

- Industry standard embedded KV store — used by CockroachDB, TiKV, YugabyteDB, Kafka
- LSM tree architecture — excellent write throughput and mixed read/write workloads
- Microsecond reads, crash-safe writes with WAL
- Built-in compression (LZ4/zstd per level), bloom filters, prefix iteration
- Tunable compaction strategies for different workload profiles
- Rust bindings via `rust-rocksdb` crate (requires C++ toolchain for build)

### What's Stored

**Object metadata:**
- Object key -> ObjectMeta (size, etag, created, modified, content-type, user metadata)
- Object key -> ChunkMap (which chunk is at which site, chunk sizes, checksums)
- Object version history (if versioning enabled)

**Bucket state:**
- Bucket list and configuration
- Listing indexes (for efficient ListObjects with prefix/delimiter)

**Operational state:**
- Pending replication queue (chunks that haven't landed at all sites yet)
- Reconciler state (last scan position, findings, repair queue)
- Site health history (latency, availability over time)

**Metrics history:**
- Request counts, latencies, error rates (rolling windows)
- Per-site throughput and availability percentages
- Storage utilization per site per bucket
- Erasure coding performance stats
- These power both the Prometheus endpoint and any built-in dashboard

### S3 Replication (Database -> Backend Storage)

The local database replicates to backend storage so it's never a single point of failure:

**Periodic snapshots:**
- Configurable interval (default: every 5 minutes, or after N writes)
- RocksDB checkpoint compressed with zstd
- Uploaded to a dedicated metadata bucket at **every** storage site (full redundancy)
- Naming: `s3prism-meta-{instance-id}/snapshots/{timestamp}.rocksdb.zst`
- Retention: keep last N snapshots, prune older ones

**Write-ahead log (WAL) sync:**
- Between snapshots, WAL entries are appended to S3 as small objects
- Enables point-in-time recovery to any moment, not just snapshot boundaries
- WAL objects: `s3prism-meta-{instance-id}/wal/{sequence}.wal.zst`

**What this gives you:**
- Gateway can be destroyed and rebuilt — restore from S3
- Can migrate to a new machine — just point at the same metadata bucket
- Metadata is as durable as the data itself (replicated across all storage sites)
- No external database to manage, backup, or worry about

### Recovery

On startup:
1. If local RocksDB exists and is valid, use it
2. If local DB is missing or corrupt, download latest snapshot from S3
3. Replay any WAL entries after the snapshot timestamp
4. If no snapshot exists anywhere, start fresh (empty gateway)
5. Log the recovery path taken and any WAL entries replayed

### Metrics Database

The gateway stores rolling metrics internally for introspection:

- **Request metrics**: counts, latencies, error rates per operation per bucket (1h, 24h, 7d windows)
- **Site metrics**: availability percentage, latency P50/P95/P99, throughput per site
- **Redundancy metrics**: objects with full redundancy vs degraded, pending replication queue depth
- **Storage metrics**: total bytes stored (logical), total bytes across all sites (physical), ratio

These are exposed via:
- **Prometheus endpoint** (`/metrics`) for Grafana/alerting
- **Built-in status API** (`/status`, `/health`) for quick checks
- **CLI**: `s3prism status` shows a dashboard summary

## Authentication

The gateway uses the user's own storage access keys. No separate key management.

- User provides their storage access key and secret key during setup
- Gateway uses those credentials to create backend buckets and store chunks at each region
- Client requests to the gateway can either:
  - **Use the same storage credentials** — gateway validates the signature and re-signs requests to backends
  - **Use a simple local auth** — gateway generates a local key pair on init for convenience
- The user's storage credentials need permission to create buckets and read/write objects in all selected regions
- No special API access required — standard S3 operations only

## Bucket Mapping

Virtual buckets on the gateway map to real backend buckets at each site:

```
Gateway bucket: "my-data"
    --> us-east-1: "s3prism-{instance-id}-my-data"  (or "s3prism-my-data-use1")
    --> us-east-2: "s3prism-{instance-id}-my-data"
    --> us-west-1: "s3prism-{instance-id}-my-data"
```

Each site gets an identically-named backend bucket. Chunks within are named:

```
Object: "photos/vacation.jpg"
    --> us-east-1: "s3prism-my-data-use1/photos/vacation.jpg.chunk-0"
    --> us-east-2: "s3prism-my-data-use2/photos/vacation.jpg.chunk-1"
    --> us-west-1: "s3prism-my-data-usw1/photos/vacation.jpg.parity-0"
```

Small objects (< threshold) are stored as full copies:
```
    --> us-east-1: "s3prism-my-data-use1/photos/icon.png"
    --> us-east-2: "s3prism-my-data-use2/photos/icon.png"
    --> us-west-1: "s3prism-my-data-usw1/photos/icon.png"
```

## Reconciler (Background Consistency)

A background task that continuously ensures data integrity:

1. **Pending queue processor** — Retries chunk uploads that failed during write
2. **Integrity scanner** — Periodically walks metadata, verifies chunks exist at all sites (HEAD requests)
3. **Repair** — If a chunk is missing (site was down during write, or data loss), reconstruct from remaining chunks and re-upload
4. **Stale cleanup** — Removes orphaned chunks that don't match any metadata entry

## Health Monitor

Tracks each storage site's availability and latency:

- Periodic health checks (HEAD request to a canary object at each site)
- Latency tracking (rolling average, P99)
- Circuit breaker per site (if a site fails N checks, mark it degraded)
- Degraded sites are deprioritized but not excluded (erasure coding handles it)
- Alert if too many sites are degraded to maintain the erasure coding scheme

## Rust Project Structure

```
s3prism/
|-- Cargo.toml
|-- src/
|   |-- main.rs                      Entry point, CLI args, server startup
|   |-- config.rs                    Configuration (sites, scheme, thresholds)
|   |
|   |-- api/                         S3-compatible API frontend
|   |   |-- mod.rs
|   |   |-- server.rs                axum HTTP server setup, TLS
|   |   |-- router.rs                Route S3 operations to handlers
|   |   |-- auth.rs                  Access key validation
|   |   |-- handlers/
|   |   |   |-- mod.rs
|   |   |   |-- object.rs            GetObject, PutObject, DeleteObject, HeadObject
|   |   |   |-- bucket.rs            CreateBucket, DeleteBucket, ListBuckets
|   |   |   |-- list.rs              ListObjects, ListObjectsV2
|   |   |   |-- multipart.rs         CreateMultipartUpload, UploadPart, Complete
|   |   |   +-- copy.rs              CopyObject
|   |   |-- xml.rs                   S3 XML request/response serialization
|   |   +-- error.rs                 S3 error response formatting
|   |
|   |-- backend/                     S3 client to storage regional sites
|   |   |-- mod.rs
|   |   |-- client.rs                Per-site HTTP client (reqwest + aws-sigv4)
|   |   |-- signing.rs               SigV4 request signing via aws-sigv4
|   |   |-- dns.rs                   DNS resolution, IP tracking, TTL refresh
|   |   |-- pool.rs                  Per-IP connection pooling and health tracking
|   |   +-- fanout.rs                Fan-out executor (Pike pattern)
|   |
|   |-- erasure/                     Reed-Solomon erasure coding
|   |   |-- mod.rs
|   |   |-- encoder.rs               Object -> chunks (data + parity)
|   |   |-- decoder.rs               Chunks -> object (reconstruct)
|   |   +-- streaming.rs             Streaming encode/decode for large objects
|   |
|   |-- metadata/                    Metadata store
|   |   |-- mod.rs
|   |   |-- store.rs                 RocksDB-backed metadata operations
|   |   |-- models.rs                ObjectMeta, ChunkMap, BucketMeta types
|   |   |-- index.rs                 Prefix/delimiter indexes for ListObjects
|   |   +-- sync.rs                  Periodic S3 snapshot + WAL sync
|   |
|   |-- reconciler/                  Background consistency
|   |   |-- mod.rs
|   |   |-- pending.rs               Retry incomplete chunk writes
|   |   |-- scanner.rs               Integrity verification scan
|   |   +-- repair.rs                Reconstruct + re-upload missing chunks
|   |
|   |-- health/                      Site health monitoring
|   |   |-- mod.rs
|   |   |-- checker.rs               Health check loop per site
|   |   +-- circuit.rs               Circuit breaker logic
|   |
|   |-- metrics/                     Prometheus metrics
|   |   +-- mod.rs                   Counters, histograms, gauges
|   |
|   |-- stats/                       Transfer and analytics tracking
|   |   |-- mod.rs
|   |   |-- transfer.rs              Per-transfer recording
|   |   |-- aggregator.rs            Roll-up into time windows
|   |   |-- retention.rs             Historical data retention/pruning
|   |   +-- query.rs                 Query interface for API/web UI
|   |
|   |-- web/                         Built-in web interface
|   |   |-- mod.rs
|   |   |-- api.rs                   Management REST API handlers
|   |   |-- websocket.rs             Live log streaming
|   |   +-- assets/                  Embedded SPA frontend
|   |       |-- index.html
|   |       |-- app.js               (or Leptos/WASM bundle)
|   |       +-- style.css
|   |
|   |-- retry/                       Retry and circuit breaker logic
|   |   |-- mod.rs
|   |   |-- policy.rs                Retry policies and backoff
|   |   +-- circuit.rs               Per-site circuit breaker
|   |
|   |-- balancer/                    Load balancing
|   |   |-- mod.rs
|   |   |-- dns.rs                   DNS resolution and IP tracking
|   |   |-- host_group.rs            Per-region host group management
|   |   |-- strategy.rs              LB strategies (least-conn, round-robin, etc.)
|   |   +-- picker.rs                IP selection logic
|   |
|   |-- ratelimit/                   Rate limiting
|   |   |-- mod.rs
|   |   |-- limiter.rs               Token bucket implementation
|   |   |-- scopes.rs                Global/region/server/client/bucket scopes
|   |   +-- distributed.rs           Valkey-backed distributed rate limits
|   |
|   +-- coordination/               Multi-instance coordination
|       |-- mod.rs
|       |-- valkey.rs                Valkey connection and helpers
|       +-- config_sync.rs           Configuration propagation
|
+-- tests/
    |-- integration/
    |   |-- basic_ops.rs             PUT/GET/DELETE round-trip
    |   |-- erasure_recovery.rs      Simulate site failure, verify reconstruction
    |   |-- fanout_latency.rs        Verify fastest-response behavior
    |   +-- metadata_sync.rs         Snapshot/restore cycle
    +-- helpers/
        +-- mock_s3.rs               Mock S3 endpoint for testing
```

## Erasure Coding Libraries

Two strong Rust options evaluated:

### `reed-solomon-simd` (Recommended for MVP)
- Pure Rust, no C dependencies — easy cross-platform builds
- Based on Leopard-RS algorithm, O(n log n) complexity
- SIMD-accelerated: AVX2, SSSE3 (x86), NEON (ARM64), with scalar fallback
- **6-10 GiB/s encoding on a single core** — orders of magnitude faster than network I/O to backend storage
- Simple API: `encode()`/`decode()`
- Up to 32,768 shards (we need 3-6)
- https://github.com/AndersTrier/reed-solomon-simd

### `rlnc` (Future Option)
- Random Linear Network Coding — any k-of-n coded pieces can recover original
- Has a **recoding** feature: intermediate nodes can create new coded pieces without decoding
- Could enable gateway-to-gateway forwarding or re-balancing without full decode
- Also SIMD-accelerated (AVX512, AVX2, NEON)
- More complex API, newer library
- https://github.com/itzmeanjan/rlnc

Start with `reed-solomon-simd`. The encoding throughput is so far beyond network speeds that the erasure coding will never be the bottleneck. RLNC's recoding feature could be interesting later for multi-gateway topologies.

## Cargo.toml Dependencies

```toml
[package]
name = "s3prism"
version = "0.1.0"
edition = "2024"

[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# HTTP server (S3 frontend)
axum = { version = "0.8", features = ["multipart"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "cors"] }
hyper = { version = "1", features = ["full"] }

# S3 request signing (SigV4 only — no SDK HTTP layer)
aws-sigv4 = "1"
aws-credential-types = "1"

# HTTP client (full control over DNS, pooling, streaming, timeouts)
reqwest = { version = "0.12", features = ["stream", "rustls-tls"] }

# Erasure coding
reed-solomon-simd = "3"

# Metadata store
rocksdb = "0.22"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
quick-xml = { version = "0.37", features = ["serialize"] }
toml = "0.8"

# Crypto
md-5 = "0.10"
sha2 = "0.10"
hmac = "0.12"
hex = "0.4"

# Compression (for metadata snapshots)
zstd = "0.13"

# Metrics
prometheus = "0.13"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# CLI
clap = { version = "4", features = ["derive"] }
dialoguer = "0.11"

# Web UI (embedded assets)
rust-embed = "8"
mime_guess = "2"

# WebSocket (live log streaming)
tokio-tungstenite = "0.24"

# TLS
rustls = "0.23"
tokio-rustls = "0.26"
rustls-pemfile = "2"
webpki-roots = "0.26"

# Hot-reload config
arc-swap = "1"
notify = "7"                  # File watcher for config/cert changes

# Valkey/Redis (Tier 2+ coordination)
redis = { version = "0.27", features = ["tokio-comp", "connection-manager"] }

# Utilities
bytes = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
thiserror = "2"
anyhow = "1"

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
```

## Key Data Types

```rust
/// Configuration for a storage regional site
struct SiteConfig {
    name: String,              // "us-east-1"
    region: String,            // AWS region string
    endpoint: String,          // "https://s3.us-east-1.example.com"
    access_key: String,
    secret_key: String,
    priority: u8,              // Lower = preferred for primary writes
}

/// Erasure coding configuration
struct ErasureConfig {
    data_chunks: usize,        // k (e.g., 2)
    parity_chunks: usize,      // m (e.g., 1)
    small_object_threshold: u64, // Below this, replicate instead of erasure code
}

/// Metadata for a stored object
struct ObjectMeta {
    key: String,
    bucket: String,
    size: u64,
    etag: String,
    content_type: String,
    created: DateTime<Utc>,
    modified: DateTime<Utc>,
    user_metadata: HashMap<String, String>,
    storage_mode: StorageMode,
    chunks: Vec<ChunkInfo>,
}

/// How an object is stored
enum StorageMode {
    Replicated,                // Full copy at each site (small objects)
    ErasureCoded {             // Reed-Solomon coded (large objects)
        data_chunks: usize,
        parity_chunks: usize,
    },
}

/// Information about a single chunk
struct ChunkInfo {
    index: usize,              // Chunk number (0..k+m)
    chunk_type: ChunkType,     // Data or Parity
    site: String,              // Which storage site holds it
    s3_key: String,            // Key in the backend bucket
    size: u64,
    checksum: String,          // SHA-256 of chunk data
    status: ChunkStatus,
}

enum ChunkType {
    Data,
    Parity,
}

enum ChunkStatus {
    Confirmed,                 // Successfully written
    Pending,                   // Write in progress or queued for retry
    Missing,                   // Failed verification, needs repair
}
```

## Fan-Out Executor (The Pike Pattern)

```rust
/// Execute an operation against multiple sites, return after quorum
async fn fanout_write(
    clients: &[SiteClient],
    chunks: Vec<(usize, Bytes)>,
    quorum: usize,
) -> Result<Vec<ChunkResult>> {
    let (tx, mut rx) = mpsc::channel(chunks.len());

    // Spawn all chunk uploads concurrently
    for (chunk_idx, chunk_data) in chunks {
        let client = clients[chunk_idx].clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let result = client.put_chunk(chunk_idx, chunk_data).await;
            let _ = tx.send((chunk_idx, result)).await;
        });
    }
    drop(tx);

    // Collect results until quorum
    let mut confirmed = Vec::new();
    let mut pending = Vec::new();

    while let Some((idx, result)) = rx.recv().await {
        match result {
            Ok(info) => {
                confirmed.push(info);
                if confirmed.len() >= quorum {
                    // Quorum reached -- return success
                    // Remaining uploads continue in background
                    break;
                }
            }
            Err(e) => pending.push((idx, e)),
        }
    }

    if confirmed.len() < quorum {
        return Err(anyhow!("failed to reach write quorum"));
    }

    Ok(confirmed)
}

/// Read from multiple sites, reconstruct after k chunks arrive
async fn fanout_read(
    clients: &[SiteClient],
    chunk_map: &[ChunkInfo],
    data_chunks: usize,
) -> Result<Bytes> {
    let (tx, mut rx) = mpsc::channel(chunk_map.len());
    let cancel = CancellationToken::new();

    // Request all chunks concurrently
    for info in chunk_map {
        let client = clients[info.site_index].clone();
        let tx = tx.clone();
        let cancel = cancel.clone();
        let key = info.s3_key.clone();
        tokio::spawn(async move {
            tokio::select! {
                result = client.get_chunk(&key) => {
                    let _ = tx.send((info.index, result)).await;
                }
                _ = cancel.cancelled() => {}
            }
        });
    }
    drop(tx);

    // Collect until we have enough to reconstruct
    let mut chunks: Vec<Option<Bytes>> = vec![None; chunk_map.len()];
    let mut received = 0;

    while let Some((idx, result)) = rx.recv().await {
        if let Ok(data) = result {
            chunks[idx] = Some(data);
            received += 1;
            if received >= data_chunks {
                cancel.cancel(); // Cancel remaining requests
                break;
            }
        }
    }

    // Reconstruct original object from available chunks
    erasure::decode(&chunks, data_chunks)
}
```

## Metadata S3 Sync

```rust
/// Periodically snapshot local metadata to S3
async fn metadata_sync_loop(
    store: Arc<MetadataStore>,
    clients: &[SiteClient],
    interval: Duration,
    gateway_id: &str,
) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;

        // Create RocksDB checkpoint
        let snapshot = store.checkpoint().await?;

        // Compress with zstd
        let compressed = zstd::encode_all(&snapshot[..], 3)?;

        // Upload to metadata bucket at each site
        let key = format!(
            "metadata/snapshots/{}/{}.rocksdb.zst",
            gateway_id,
            Utc::now().format("%Y%m%d-%H%M%S")
        );

        for client in clients {
            let _ = client
                .put_object("s3prism-meta", &key, compressed.clone())
                .await;
        }

        // Prune old snapshots (keep last N)
        prune_old_snapshots(clients, gateway_id, 10).await;
    }
}

/// Restore metadata from S3 on startup
async fn restore_from_s3(
    clients: &[SiteClient],
    gateway_id: &str,
) -> Result<Option<Vec<u8>>> {
    // Try each site until we find a snapshot
    for client in clients {
        let prefix = format!("metadata/snapshots/{}/", gateway_id);
        if let Ok(snapshots) = client.list_objects("s3prism-meta", &prefix).await {
            if let Some(latest) = snapshots.last() {
                let compressed = client.get_object("s3prism-meta", &latest.key).await?;
                let data = zstd::decode_all(&compressed[..])?;
                return Ok(Some(data));
            }
        }
    }
    Ok(None)
}
```

## S3 API Coverage (MVP)

### Phase 1 — Core Operations
- PutObject (single + chunked transfer encoding)
- GetObject (with Range support for partial reads)
- HeadObject
- DeleteObject (tombstone + async backend reaper)
- DeleteObjects (batch delete, up to 1000 keys per request)
- ListObjectsV2
- CreateBucket / DeleteBucket / HeadBucket / ListBuckets
- CopyObject (server-side, between gateway buckets)

### Phase 2 — Multipart
- CreateMultipartUpload
- UploadPart
- CompleteMultipartUpload
- AbortMultipartUpload
- ListParts
- ListMultipartUploads

### Phase 3 — Advanced (Post-MVP)
- Object versioning (see [Versioning](#versioning-future--phase-3) section)
- Object Lock / WORM compliance (see [Object Lock](#object-lock-future--phase-3-worm-compliance) section)
- Presigned URLs
- Bucket policies
- CORS configuration
- Object tagging
- Lifecycle rules (delegate to backend provider's native lifecycle?)

### Phase 4 — IAM / Multi-Tenant
- S3Prism-native access keys (independent of storage credentials)
- Multiple users/tenants with separate key pairs
- IAM-style policies: per-user bucket access, read-only vs read-write, IP restrictions
- API key management via web UI (create, rotate, revoke)
- storage credentials become backend-only — clients never see them
- Audit log of all authenticated operations per user

## Load Balancing

storage regions have multiple IP addresses behind their DNS endpoints. The gateway needs to be smart about how it distributes requests across those IPs, and across its own instances when scaled out.

### DNS-Aware Backend Load Balancing

Each storage region endpoint (e.g., `s3.us-east-1.example.com`) resolves to multiple IPs. The gateway must:

1. **Resolve and track all IPs** per region endpoint
2. **Re-resolve periodically** (respect DNS TTL, or configurable interval)
3. **Track per-IP stats**: active connections, request count, latency, error rate
4. **Load balance across IPs** within each region

```rust
struct HostGroup {
    region: String,
    endpoint: String,                    // DNS name
    ips: Vec<HostEntry>,                 // Resolved IPs
    strategy: LoadBalanceStrategy,
    last_resolved: Instant,
    resolve_interval: Duration,
}

struct HostEntry {
    addr: IpAddr,
    active_connections: AtomicU32,
    total_requests: AtomicU64,
    total_errors: AtomicU64,
    avg_latency: AtomicU64,              // Microseconds, rolling average
    last_error: Option<Instant>,
    circuit: CircuitState,               // Per-IP circuit breaker
    weight: f64,                         // Adjustable weight (default 1.0)
    enabled: bool,                       // Can be disabled via API/UI
}
```

### Load Balancing Strategies

Configurable per region or globally:

| Strategy | How It Works | Best For |
|----------|-------------|----------|
| **Least Connections** | Route to IP with fewest active connections | Default — natural load distribution |
| **Least Latency** | Route to IP with lowest rolling avg latency | Latency-sensitive workloads |
| **Round Robin** | Cycle through IPs sequentially | Simple, predictable |
| **Weighted Round Robin** | Round robin with configurable weights per IP | When IPs have different capacity |
| **Random** | Random IP selection | Simplest, good enough at scale |
| **Power of Two Choices** | Pick 2 random IPs, choose the one with fewer connections | Good balance of simplicity and fairness |

Default: **Least Connections** — simple, self-balancing, and naturally adapts to slow IPs.

### Host Group Management

Visible and configurable in the web UI:

- View all resolved IPs per region with live stats (connections, latency, errors)
- Manually disable/enable individual IPs
- Adjust weights
- Change load balancing strategy per region
- See DNS resolution history (when IPs changed)
- Alert when DNS resolution returns different IPs (endpoint migration)

### Gateway-Level Load Balancing (Multi-Instance)

When running multiple gateway instances, a load balancer sits in front:

```
Clients
    |
    v
[DNS / L4 LB / L7 LB]
    |
    +---> Gateway Instance 1 (gw-01)
    +---> Gateway Instance 2 (gw-02)
    +---> Gateway Instance 3 (gw-03)
```

Options:
- **DNS round-robin**: Simplest. Each gateway instance registers in DNS.
- **L4 load balancer** (HAProxy, nginx stream): TCP-level, low overhead.
- **L7 load balancer** (nginx, envoy): Can route by bucket/path if needed.
- **Kubernetes Service**: If running in K8s, native service load balancing.

The gateway exposes `/health` for health-check integration with any load balancer.

## Rate Limiting

Rate limits at every level — per IP, per region, per server, and total. Prevents runaway clients from overwhelming backend storage or the gateway itself.

### Rate Limit Hierarchy

```
Total gateway limit (global ceiling)
    |
    +-- Per-region limits (cap traffic to any single storage region)
    |       |
    |       +-- Per-server limits (cap traffic to individual IPs within a region)
    |
    +-- Per-client limits (cap traffic from individual client IPs or access keys)
    |
    +-- Per-bucket limits (cap traffic to specific virtual buckets)
```

### Rate Limit Types

| Scope | What's Limited | Default | Configurable |
|-------|---------------|---------|-------------|
| **Global** | Total requests/sec across entire gateway | 10,000 rps | Yes |
| **Global bandwidth** | Total ingress + egress bytes/sec | 10 Gbps | Yes |
| **Per-region** | Requests/sec to a single storage region | 5,000 rps | Yes, per region |
| **Per-region bandwidth** | Bytes/sec to a single storage region | 5 Gbps | Yes, per region |
| **Per-server (IP)** | Requests/sec to a single backend IP | 1,000 rps | Yes, per IP |
| **Per-client IP** | Requests/sec from a single client source | 1,000 rps | Yes |
| **Per-client key** | Requests/sec per access key | 1,000 rps | Yes |
| **Per-bucket** | Requests/sec to a specific virtual bucket | Unlimited | Yes, per bucket |
| **Per-operation** | Requests/sec for specific operations (e.g., LIST, DELETE) | Unlimited | Yes |

### Rate Limiter Implementation

Token bucket per scope, with the option for distributed rate limiting when scaled out:

```rust
struct RateLimiter {
    scope: RateLimitScope,
    rate: f64,                    // Tokens per second
    burst: u64,                   // Maximum burst size
    tokens: AtomicU64,            // Current available tokens (fixed-point)
    last_refill: AtomicU64,       // Last refill timestamp
}

enum RateLimitScope {
    Global,
    Region(String),
    Server(IpAddr),
    ClientIp(IpAddr),
    ClientKey(String),
    Bucket(String),
    Operation(OperationType),
}
```

### Rate Limit Responses

When a rate limit is hit:
- Return **429 Too Many Requests** with `Retry-After` header
- Log the event with scope, current rate, limit, client info
- Increment rate limit metrics counter
- Web UI shows rate limit events in real-time

### Rate Limit Configuration via Web UI

- View all active rate limits with current utilization (gauge showing how close to limit)
- Adjust limits in real-time (no restart required)
- Per-region overrides: set different limits for different storage regions
- Per-server overrides: lower limits for IPs that are slower or erroring
- Allowlist/blocklist: exempt or block specific client IPs
- Scheduled limits: different limits at different times of day (future)

## Scaling Architecture

The gateway starts simple (single binary, embedded database) but needs to scale horizontally. This means shared state.

### Scaling Tiers

**Tier 1: Single Instance (MVP)**
- Embedded RocksDB database
- All state is local
- Periodic snapshots to S3 for durability
- Handles moderate traffic on one machine

**Tier 2: Multi-Instance with Shared Cache (Valkey/Redis)**
- Multiple gateway instances behind a load balancer
- Valkey (Redis-compatible) for shared real-time state:
  - Distributed rate limit counters (atomic increments across instances)
  - Active connection counts per backend IP (for cross-instance least-connections LB)
  - Circuit breaker state (if one instance trips a breaker, all instances know)
  - Health check consensus (majority vote on site health)
  - Transfer stats aggregation (combine stats from all instances)
- Each instance still has local RocksDB for metadata (fast reads)
- Sled syncs to S3 as before — Valkey is for ephemeral coordination, not durability

**Tier 3: Multi-Instance with Shared Database**
- Shared persistent database for configuration and metadata:
  - Configuration changes propagate instantly to all instances
  - Metadata (object->chunk mappings) is consistent across instances
  - No split-brain on writes
- Options:
  - **PostgreSQL**: Proven, widely available, good for config + metadata
  - **CockroachDB**: Distributed SQL, multi-region native — overkill for most deployments but available
  - **FoundationDB**: High-performance distributed KV — if you need extreme throughput
  - **SQLite + Litestream**: SQLite replicated to S3 — simpler than Postgres, might be enough

### State Classification

What goes where at each tier:

| Data | Tier 1 (Single) | Tier 2 (Valkey) | Tier 3 (Shared DB) |
|------|-----------------|-----------------|-------------------|
| Object metadata | RocksDB (local) | RocksDB (local) + S3 sync | Shared DB |
| Configuration | TOML file | TOML + Valkey pub/sub | Shared DB |
| Rate limit counters | Local atomics | Valkey (shared) | Valkey (shared) |
| Active connections/IP | Local tracking | Valkey (shared) | Valkey (shared) |
| Circuit breaker state | Local | Valkey (shared) | Valkey (shared) |
| Transfer stats | RocksDB (local) | Valkey (aggregated) | Shared DB |
| Health check results | Local | Valkey (consensus) | Valkey (consensus) |
| Pending replication queue | RocksDB (local) | Shared DB or Valkey | Shared DB |
| Reconciler state | RocksDB (local) | Shared DB | Shared DB |
| Session/request tracking | Memory | Memory | Memory |

### Valkey Integration

```rust
/// Distributed rate limiter using Valkey
async fn check_rate_limit_distributed(
    valkey: &ValkeyPool,
    key: &str,           // e.g., "ratelimit:region:us-east-1"
    limit: u64,
    window_secs: u64,
) -> Result<bool> {
    // Sliding window rate limit via Valkey MULTI/EXEC
    // Uses sorted set with timestamp scores
    let now = Utc::now().timestamp_millis();
    let window_start = now - (window_secs * 1000) as i64;

    let count: u64 = valkey
        .pipe()
        .zremrangebyscore(key, 0, window_start)      // Prune old entries
        .zcard(key)                                     // Count current entries
        .zadd(key, now, uuid::Uuid::new_v4().to_string()) // Add this request
        .expire(key, window_secs as i64 * 2)           // TTL for cleanup
        .query_async()
        .await?;

    Ok(count <= limit)
}

/// Distributed least-connections tracking
async fn get_least_connected_ip(
    valkey: &ValkeyPool,
    region: &str,
) -> Result<IpAddr> {
    // Each gateway instance increments/decrements connection count per IP
    let key = format!("connections:{}", region);
    let entries: Vec<(String, f64)> = valkey
        .zrangebyscore_withscores(&key, 0, "+inf")
        .await?;

    // Pick IP with lowest connection count
    entries.first()
        .map(|(ip, _)| ip.parse::<IpAddr>())
        .transpose()?
        .ok_or_else(|| anyhow!("no IPs available for region {}", region))
}
```

### Configuration Database

For multi-instance deployments, configuration needs to be shared and changes need to propagate:

**Option A: Valkey Pub/Sub + TOML**
- Primary config stays in TOML file
- Changes via web UI write to Valkey and publish on a channel
- All instances subscribe and apply changes
- Simple, but Valkey becomes a dependency for config changes

**Option B: PostgreSQL**
- Configuration stored in tables
- Instances poll for changes (or use LISTEN/NOTIFY)
- More operational overhead but more robust
- Natural fit if you already need Postgres for metadata at Tier 3

**Option C: S3 as Config Store**
- Configuration written to a well-known S3 key
- Instances poll periodically (e.g., every 30s)
- No additional infrastructure needed
- Higher latency for config propagation
- Good for small deployments that don't want to run Valkey or Postgres

Recommendation: Start with **TOML file (Tier 1)**, add **Valkey pub/sub (Tier 2)** when scaling, offer **PostgreSQL (Tier 3)** for large deployments.

## Deployment

### Single Instance (Tier 1)

Simplest deployment — one binary, one RocksDB database, talks to N storage sites.

```
s3prism \
    --config gateway.toml \
    --listen 0.0.0.0:443 \
    --tls-cert /etc/ssl/cert.pem \
    --tls-key /etc/ssl/key.pem
```

### Multi-Instance with Valkey (Tier 2)

```
s3prism \
    --config gateway.toml \
    --valkey redis://valkey.internal:6379 \
    --instance-id gw-02
```

### Multi-Instance with Shared DB (Tier 3)

```
s3prism \
    --config gateway.toml \
    --valkey redis://valkey.internal:6379 \
    --database postgres://user:pass@db.internal/gateway \
    --instance-id gw-03
```

### Docker Compose (Tier 2 Example)

```yaml
services:
  gateway-1:
    image: s3prism:latest
    ports: ["8443:8443", "9090:9090"]
    environment:
      S3_ACCESS_KEY: "AKID..."
      S3_SECRET_KEY: "..."
      VALKEY_URL: "redis://valkey:6379"
      INSTANCE_ID: "s3prism-01"
    volumes:
      - gw1-data:/var/lib/s3prism

  gateway-2:
    image: s3prism:latest
    ports: ["8444:8443", "9091:9090"]
    environment:
      S3_ACCESS_KEY: "AKID..."
      S3_SECRET_KEY: "..."
      VALKEY_URL: "redis://valkey:6379"
      INSTANCE_ID: "s3prism-02"
    volumes:
      - gw2-data:/var/lib/s3prism

  valkey:
    image: valkey/valkey:latest
    ports: ["6379:6379"]

  load-balancer:
    image: nginx:latest
    ports: ["443:443"]
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf
```

## Configuration Example

```toml
[gateway]
id = "s3prism-na-01"
listen = "0.0.0.0:8443"
management_listen = "0.0.0.0:9090"
tls_cert = "/etc/ssl/cert.pem"
tls_key = "/etc/ssl/key.pem"

[erasure]
data_chunks = 2
parity_chunks = 1
small_object_threshold = "1MB"

[metadata]
path = "/var/lib/s3prism/meta.rocksdb"
sync_interval = "5m"
snapshot_retention = 10

# Optional: Valkey for multi-instance coordination (Tier 2+)
[valkey]
url = "redis://localhost:6379"
# prefix = "wgw:"                    # Key prefix to avoid collisions

# Optional: Shared database for config/metadata (Tier 3)
# [database]
# url = "postgres://user:pass@localhost/s3prism"

[load_balancing]
strategy = "least_connections"         # least_connections, least_latency, round_robin, weighted, random, power_of_two
dns_resolve_interval = "60s"
max_connections_per_ip = 100

[rate_limits]
global_rps = 10000
global_bandwidth = "10Gbps"
per_client_ip_rps = 1000
per_client_key_rps = 1000

[rate_limits.per_region]
"us-east-1" = { rps = 5000, bandwidth = "5Gbps" }
"us-east-2" = { rps = 5000, bandwidth = "5Gbps" }
"us-west-1" = { rps = 3000, bandwidth = "3Gbps" }

[retry]
max_retries = 5
initial_backoff = "100ms"
max_backoff = "30s"
backoff_multiplier = 2.0
jitter = 0.25

[circuit_breaker]
failure_threshold = 10                 # Failures in window to trip
failure_window = "60s"
cooldown = "30s"

[[sites]]
name = "us-east-1"
endpoint = "https://s3.us-east-1.example.com"
region = "us-east-1"
access_key = "AKID..."
secret_key = "..."
priority = 3
```

## Hot-Reload Configuration

Everything must be changeable at runtime without restarting the gateway. No downtime for config changes.

### Reload Mechanisms

**1. SIGHUP (Unix signal)**
```
kill -HUP $(pidof s3prism)
```
- Re-reads TOML config file from disk
- Applies all changes that can be hot-reloaded
- Logs what changed and what was applied

**2. Web UI**
- All configuration editable in the Configuration page
- Changes apply immediately and persist to the config file (or shared DB)
- Shows diff of what changed before applying
- Undo/rollback to previous configuration

**3. Management API**
```
PUT /api/v1/config
Content-Type: application/json

{ "rate_limits": { "global_rps": 20000 } }
```
- Partial updates — only send what changed
- Returns the full effective config after applying
- Propagates to other instances via Valkey pub/sub (Tier 2+)

**4. CLI**
```
s3prism config set rate_limits.global_rps 20000
s3prism config set erasure.small_object_threshold "2MB"
s3prism config reload
s3prism config show
s3prism config diff          # Show pending changes vs running
s3prism config history       # Show recent config changes
```

### What Can Be Hot-Reloaded

| Setting | Hot-Reload? | Notes |
|---------|-------------|-------|
| Rate limits (all scopes) | Yes | Immediate |
| Load balancing strategy | Yes | Next request uses new strategy |
| Circuit breaker thresholds | Yes | Immediate |
| Retry policy | Yes | Next retry uses new policy |
| TLS certificates (frontend) | Yes | New connections use new cert, existing unaffected |
| TLS/SSL backend settings | Yes | New connections use new settings |
| Log level | Yes | Immediate |
| DNS resolve interval | Yes | Next resolve cycle |
| Site add/remove | Yes | Triggers data rebalancing in background |
| Site credentials | Yes | Next request to that site uses new creds |
| Erasure coding scheme | Partial | Applies to new objects only. Existing objects keep their scheme. |
| Small object threshold | Yes | Applies to new objects |
| Metadata sync interval | Yes | Next sync cycle |
| Management port | No | Requires restart |
| Listen address/port | No | Requires restart |

### Config Versioning

Every config change is versioned:

```rust
struct ConfigChange {
    version: u64,                   // Monotonically increasing
    timestamp: DateTime<Utc>,
    source: ConfigSource,           // File, API, WebUI, CLI, Signal
    diff: Vec<ConfigDiff>,          // What changed
    instance_id: String,            // Which instance applied it
}

enum ConfigDiff {
    Added { key: String, value: String },
    Removed { key: String, old_value: String },
    Changed { key: String, old_value: String, new_value: String },
}
```

- Last 100 config changes stored in the database
- Rollback to any previous version via API or web UI
- Config changes logged at INFO level with full diff

### Implementation

The gateway uses an `Arc<ArcSwap<Config>>` pattern — readers get lock-free access to the current config, and a reload atomically swaps in the new config:

```rust
use arc_swap::ArcSwap;

struct Gateway {
    config: Arc<ArcSwap<Config>>,
    // ...
}

impl Gateway {
    fn reload_config(&self, new_config: Config) {
        let old = self.config.load();
        let diff = compute_diff(&old, &new_config);
        log::info!("Config reload: {:?}", diff);
        self.config.store(Arc::new(new_config));
        // Notify subsystems of specific changes
        self.notify_config_change(&diff);
    }

    fn current_config(&self) -> Arc<Config> {
        self.config.load_full()
    }
}
```

## TLS / SSL Configuration

### Frontend TLS (Client -> Gateway)

The gateway terminates TLS from clients connecting to the S3 endpoint.

```toml
[tls]
cert = "/etc/ssl/gateway.crt"
key = "/etc/ssl/gateway.key"
# ca_cert = "/etc/ssl/ca.crt"               # For mTLS client verification

# Protocol versions
min_version = "1.2"                          # "1.0", "1.1", "1.2", "1.3"
max_version = "1.3"

# Cipher suites (TLS 1.2)
# Default: secure modern ciphers. Override to restrict or expand.
# ciphers = [
#     "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
#     "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
#     "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
# ]

# ALPN protocols
# alpn = ["h2", "http/1.1"]

# Client certificate verification (mTLS)
client_auth = "none"                         # "none", "optional", "required"
# client_ca = "/etc/ssl/client-ca.crt"       # CA for verifying client certs
```

**Hot-reload**: Certificate and key files are watched for changes. When updated, new connections use the new certificate. Existing connections are unaffected. Also reloadable via API:

```
POST /api/v1/config/tls/reload
```

### Backend TLS (Gateway -> Storage Sites)

The gateway connects to backend storage over HTTPS. These settings control that connection.

```toml
[tls.backend]
# Certificate verification
verify = true                                # Verify backend TLS certificate
# verify = false                             # DANGER: Skip verification (testing only)

# Custom CA bundle (if backend uses non-standard CA)
# ca_bundle = "/etc/ssl/custom-ca-bundle.crt"

# SNI (Server Name Indication)
sni = true                                   # Send SNI hostname (default: true)
# sni_override = "s3.example.com"          # Override SNI hostname

# Protocol versions
min_version = "1.2"
max_version = "1.3"

# Client certificate (if backend requires mTLS)
# client_cert = "/etc/ssl/client.crt"
# client_key = "/etc/ssl/client.key"

# Connection settings
handshake_timeout = "10s"
```

### Per-Site TLS Overrides

Each storage site can have its own TLS settings, overriding the defaults:

```toml
[[sites]]
name = "us-east-1"
endpoint = "https://s3.us-east-1.example.com"
region = "us-east-1"
access_key = "AKID..."
secret_key = "..."
priority = 1

# Per-site TLS overrides
[sites.tls]
verify = true                                # Override backend default
# ca_bundle = "/etc/ssl/us-east-1-ca.crt"    # Site-specific CA
# min_version = "1.3"                        # Require TLS 1.3 for this site
# sni_override = "custom.endpoint.com"
```

### SSL Validation Options

| Option | Values | Default | Hot-Reload? |
|--------|--------|---------|-------------|
| Backend cert verification | `true` / `false` | `true` | Yes |
| Custom CA bundle | File path | System CAs | Yes |
| SNI enabled | `true` / `false` | `true` | Yes |
| SNI hostname override | String | From endpoint URL | Yes |
| Min TLS version | `1.0` - `1.3` | `1.2` | Yes (new connections) |
| Max TLS version | `1.0` - `1.3` | `1.3` | Yes (new connections) |
| Handshake timeout | Duration | `10s` | Yes |
| Client cert (mTLS to backend storage) | File path | None | Yes |
| Frontend client auth | `none` / `optional` / `required` | `none` | Yes (new connections) |
| Frontend cipher suites | List | Secure defaults | Yes (new connections) |

### Web UI TLS Management

- View current TLS configuration for frontend and each backend site
- Upload new certificates via drag-and-drop
- View certificate details: issuer, subject, expiry, SANs
- Certificate expiry warnings (30d, 14d, 7d, 1d)
- Test backend TLS connectivity per site (handshake probe with results)
- Toggle verification per site (with confirmation warning for disabling)
- View TLS handshake stats: protocol version distribution, cipher usage, handshake latency

## Logging and Observability

Data resilience is the entire point of this project. Every operation must be fully traceable.

### Request Logging

Every S3 request gets a structured log entry:
```json
{
  "ts": "2026-03-06T14:23:01.442Z",
  "request_id": "a1b2c3d4",
  "op": "PutObject",
  "bucket": "my-data",
  "key": "photos/vacation.jpg",
  "size": 104857600,
  "storage_mode": "erasure_coded",
  "scheme": "2+1",
  "chunks": [
    {"index": 0, "type": "data", "site": "us-east-1", "status": "confirmed", "latency_ms": 142},
    {"index": 1, "type": "data", "site": "us-east-2", "status": "confirmed", "latency_ms": 168},
    {"index": 2, "type": "parity", "site": "us-west-1", "status": "pending", "latency_ms": null}
  ],
  "quorum_reached": true,
  "quorum_latency_ms": 168,
  "client_ip": "10.0.1.50",
  "result": "200 OK"
}
```

### What Gets Logged

| Event | Level | Why |
|-------|-------|-----|
| Every S3 request (method, key, size, result) | INFO | Audit trail |
| Chunk write success/failure per site | INFO | Track which sites have which data |
| Chunk read per site (latency, success/fail) | INFO | Performance and failover tracking |
| Quorum reached/failed | INFO/ERROR | Data placement confirmation |
| Background replication complete | INFO | Confirms all chunks landed |
| Background replication failure + retry | WARN | Data at risk until resolved |
| Metadata snapshot to S3 | INFO | Metadata durability tracking |
| Site health check pass/fail | DEBUG/WARN | Site availability |
| Reconciler found missing chunk | WARN | Integrity issue detected |
| Reconciler repaired chunk | INFO | Integrity issue resolved |
| Erasure encode/decode timing | DEBUG | Performance profiling |
| Startup/shutdown, config loaded | INFO | Operational |

### Log Destinations

- **Structured JSON** to stdout (for container/systemd capture)
- **Log file rotation** for direct deployments (configurable path, size, retention)
- **Prometheus metrics** for dashboards:
  - Requests per second by operation
  - Chunk write/read latency per site (histogram)
  - Quorum success/failure rate
  - Background replication queue depth
  - Site health status
  - Reconciler findings (missing chunks, repairs)
  - Erasure coding throughput
  - Metadata store size and sync status

### Data Resilience Logging

The critical path — making sure data is never silently lost:

1. **Write confirmation log** — After a PUT, log exactly which chunks are confirmed vs pending. If quorum was barely met (e.g., 2 of 3), flag it.
2. **Pending replication alerts** — If a background chunk write has been pending for > N minutes, escalate to WARN. If > N hours, ERROR.
3. **Integrity scan results** — The reconciler logs every scan: how many objects checked, how many chunks verified, how many missing, how many repaired.
4. **Degraded mode warnings** — If a site is down and the gateway is operating with reduced redundancy, log continuously until resolved.

The gateway manages the redundancy, logs everything, and the user sees a single namespace. Think of storage regions as the "disks" in a RAID array — the gateway is the RAID controller, operating at a global scale.

## Read Path Resilience Detail

Reads must never silently return bad data. The full read path:

1. Look up object in metadata store -> get chunk map (which chunks at which sites)
2. Fan out GET requests to all sites concurrently
3. As chunks arrive, verify checksums against metadata
4. Once k valid chunks received, reconstruct the object via Reed-Solomon decode
5. Verify reconstructed object checksum matches stored ETag
6. Stream to client
7. Cancel remaining in-flight requests
8. Log: which sites responded, latency of each, which chunks were used, total reconstruction time

If a chunk arrives but fails checksum: discard it, wait for another. If a site is down: the remaining sites cover it. If fewer than k chunks are available: return 503 with a clear error, log ERROR with full details of what's missing.

This is the Rob Pike pattern applied to data integrity — race the sites, take the fastest valid responses, and never trust a single source.

## Transfer Performance Tracking

Every transfer — client-to-gateway and gateway-to-backend — is tracked end-to-end.

### Per-Transfer Stats

Every PUT, GET, and chunk operation records:

```rust
struct TransferRecord {
    request_id: String,
    operation: Operation,           // Put, Get, Delete, Copy, List
    bucket: String,
    key: String,
    object_size: u64,               // Original object size
    storage_mode: StorageMode,      // Replicated or ErasureCoded

    // Client-side timing
    client_ip: String,
    client_first_byte: Duration,    // Time to first byte from client
    client_total: Duration,         // Total client transfer time
    client_throughput_bps: u64,     // Client transfer speed

    // Per-site chunk timing
    chunks: Vec<ChunkTransferRecord>,

    // Aggregate
    quorum_latency: Duration,       // Time until quorum reached
    total_latency: Duration,        // Time until all chunks complete
    erasure_encode_time: Duration,  // Reed-Solomon encode time
    erasure_decode_time: Duration,  // Reed-Solomon decode time (reads)
    result: TransferResult,         // Success, PartialSuccess, Failed
    timestamp: DateTime<Utc>,
}

struct ChunkTransferRecord {
    chunk_index: usize,
    chunk_type: ChunkType,          // Data or Parity
    site: String,                   // storage region
    size: u64,
    latency_connect: Duration,      // TCP + TLS handshake to backend storage
    latency_first_byte: Duration,   // Time to first byte
    latency_total: Duration,        // Total transfer time
    throughput_bps: u64,            // Bytes per second
    retries: u32,                   // How many retries needed
    result: ChunkResult,            // Success, Failed, Cancelled
}
```

### Aggregated Performance Stats

Rolled up and stored in the internal database:

**Per-site stats** (rolling windows: 1min, 5min, 1hr, 24hr, 7d):
- Throughput: avg, P50, P95, P99 bytes/sec
- Latency: connect, first-byte, total — avg, P50, P95, P99
- Error rate: percentage of failed chunk operations
- Availability: percentage of time site is reachable
- Retry rate: average retries per chunk operation

**Per-bucket stats:**
- Total objects, total logical size, total physical size (with erasure overhead)
- Read/write IOPS
- Ingress/egress bandwidth
- Error rates by operation type

**Global stats:**
- Total throughput across all sites
- Cross-region latency matrix (site-to-site as observed by the gateway)
- Redundancy health: percentage of objects fully replicated vs degraded
- Pending replication queue depth and age

**Erasure coding stats:**
- Encode throughput (MB/s)
- Decode throughput (MB/s)
- Reconstruction events (how often reads required parity chunks)

### Historical Retention

Performance data is kept at decreasing granularity:
- Last hour: per-second resolution
- Last 24 hours: per-minute resolution
- Last 7 days: per-hour resolution
- Last 90 days: per-day resolution
- Older: monthly summaries

All stored in the internal RocksDB database and replicated to backend storage with the regular snapshots.

## Web Interface

The gateway includes a built-in web UI served on the management port. No separate frontend deployment needed — it's compiled into the binary.

### Technology

- **Backend**: axum serving the API + static assets
- **Frontend**: Embedded SPA (Single Page Application)
  - Built with a lightweight framework (Leptos for Rust-native, or a pre-built JS bundle)
  - Compiled into the binary via `include_dir!` or `rust-embed`
  - Zero external dependencies at runtime — no npm, no node, no CDN

### Pages

**Dashboard (home):**
- Real-time overview: requests/sec, throughput, active transfers
- Site health cards: green/yellow/red per storage region with latency sparklines
- Redundancy status: "All objects fully redundant" or "47 objects degraded — repair in progress"
- Storage utilization: logical vs physical, per-bucket breakdown
- Recent alerts/warnings
- Mini charts: request rate (24h), throughput (24h), error rate (24h)

**Analytics:**
- **API request breakdown**: stacked bar chart by operation type (GET, PUT, DELETE, LIST, HEAD, multipart) over selectable time range
- **API request counts**: total requests, requests/sec, by hour/day/week/month
- **Operation type distribution**: pie/donut chart (what percentage of traffic is reads vs writes vs lists)
- **Region breakdown**: per-site request volume, throughput, and storage — side-by-side bar charts
- **Region utilization heatmap**: time-of-day vs region showing request density
- **Cross-region traffic flow**: Sankey or chord diagram showing where data flows (which regions serve which reads)
- **Storage growth**: line chart of total storage over time (logical vs physical), per-bucket
- **Cost estimation**: estimated storage cost based on actual usage per region (storage + API calls)
- **Top buckets**: by size, by request count, by throughput
- **Top objects**: most accessed objects, largest objects, most recently modified
- **Error breakdown**: by error type (timeout, 5xx, auth failure), by region, over time
- **Bandwidth**: ingress/egress per region, per bucket, over time — stacked area chart
- **Latency breakdown**: per-region latency over time (line chart with P50/P95/P99 bands)
- **Retry analytics**: retry rate per region over time, retry success rate
- **Erasure coding overhead**: encode/decode time distribution, reconstruction frequency
- All charts support: zoom, pan, time range selection, export as PNG/CSV

**Buckets:**
- List all virtual buckets
- Per-bucket stats: object count, size, read/write activity
- Create/delete buckets
- Browse objects (with prefix navigation)
- Object detail: chunk locations, redundancy status, transfer history

**Sites:**
- All configured storage regions with live health status
- Per-site: latency chart, throughput chart, availability history, error rate
- Add/remove sites (with data rebalancing)
- Latency matrix: cross-site latency heatmap
- **Run Benchmark** button — re-run full performance test against all or selected regions
- Benchmark history: compare performance over time (detect region degradation)
- Discover new regions: scan for available storage endpoints not yet configured
- Per-site connection pool stats: active, idle, queued connections

**Performance:**
- Transfer throughput over time (line chart, per-site)
- Latency distribution (histogram, per-site)
- Erasure coding performance
- Top objects by size, by access frequency
- Slow transfer log: transfers that exceeded P95 latency

**Redundancy:**
- Overall redundancy health percentage
- Objects at risk (fewer chunks than required)
- Pending replication queue with age
- Reconciler status: last scan, findings, repairs
- Repair history

**Configuration:**
- Erasure coding settings (scheme, small object threshold)
- Site configuration (add/remove/reorder regions)
- Credentials management (update storage credentials)
- Metadata sync settings (snapshot interval, retention)
- Retry policy tuning
- Log level configuration
- TLS certificate management

**Logs:**
- Searchable, filterable log viewer
- Filter by: level, operation, bucket, key, site, request_id
- Live tail mode (websocket streaming)

### Management API

The web UI is backed by a REST API on the management port. Everything the UI can do, the API can do:

```
GET    /api/v1/status                    Overall gateway status
GET    /api/v1/dashboard                 Dashboard summary data

GET    /api/v1/buckets                   List buckets with stats
POST   /api/v1/buckets                   Create bucket
DELETE /api/v1/buckets/{name}            Delete bucket
GET    /api/v1/buckets/{name}/objects    Browse objects
GET    /api/v1/buckets/{name}/stats      Bucket performance stats

GET    /api/v1/sites                     List sites with health
POST   /api/v1/sites                     Add site
DELETE /api/v1/sites/{name}              Remove site
GET    /api/v1/sites/{name}/stats        Site performance history
GET    /api/v1/sites/latency-matrix      Cross-site latency

GET    /api/v1/performance               Global performance stats
GET    /api/v1/performance/transfers     Recent transfer log
GET    /api/v1/performance/slow          Slow transfer report

GET    /api/v1/redundancy                Redundancy health overview
GET    /api/v1/redundancy/degraded       Objects with degraded redundancy
GET    /api/v1/redundancy/pending        Pending replication queue
GET    /api/v1/redundancy/repairs        Repair history

GET    /api/v1/config                    Current configuration
PUT    /api/v1/config                    Update configuration
POST   /api/v1/config/credentials        Update storage credentials

GET    /api/v1/logs                      Query logs (with filters)
WS     /api/v1/logs/stream               Live log stream (websocket)

GET    /metrics                          Prometheus metrics endpoint
GET    /health                           Health check endpoint
```

## Retry Patterns

Data resilience depends on robust retry logic everywhere. Every network call to backend storage can fail — transient errors, timeouts, rate limits, region outages. The gateway must handle all of these gracefully.

### Retry Policy

```rust
struct RetryPolicy {
    max_retries: u32,              // Maximum retry attempts (default: 5)
    initial_backoff: Duration,     // First retry delay (default: 100ms)
    max_backoff: Duration,         // Cap on backoff (default: 30s)
    backoff_multiplier: f64,       // Exponential factor (default: 2.0)
    jitter: f64,                   // Random jitter 0.0-1.0 (default: 0.25)
    retryable_errors: Vec<ErrorClass>,
}
```

### Exponential Backoff with Jitter

Every retryable operation uses exponential backoff with jitter to avoid thundering herd:

```
Attempt 1: immediate
Attempt 2: 100ms  + jitter (75-125ms)
Attempt 3: 200ms  + jitter (150-250ms)
Attempt 4: 400ms  + jitter (300-500ms)
Attempt 5: 800ms  + jitter (600-1000ms)
... capped at max_backoff
```

### What Gets Retried (and What Doesn't)

| Operation | Retry? | Policy | Notes |
|-----------|--------|--------|-------|
| Chunk PUT to backend storage | Yes | 5 retries, 100ms base | Transient failures, 500/503 |
| Chunk GET from backend storage | Yes | 3 retries, 50ms base | Fast retry, other sites cover |
| Chunk DELETE from backend storage | Yes | 5 retries, 100ms base | Must eventually succeed |
| Metadata snapshot to S3 | Yes | 10 retries, 1s base | Critical, more patient |
| WAL entry to S3 | Yes | 5 retries, 500ms base | Important for durability |
| Health check | No | N/A | Failure is the signal |
| Client request (overall) | No | N/A | Client handles their own retries |
| Backend400 Bad Request | No | N/A | Client error, won't change |
| Backend404 Not Found | No | N/A | Object doesn't exist |
| Backend403 Forbidden | No | N/A | Auth issue, won't change on retry |
| Backend429 Too Many Requests | Yes | Use Retry-After header | Respect rate limits |
| Backend500 Internal Error | Yes | Standard backoff | Transient |
| Backend503 Service Unavailable | Yes | Standard backoff | Transient |
| TCP connection refused | Yes | Standard backoff | Site might be restarting |
| TCP timeout | Yes | Standard backoff, shorter timeout on retry | Site might be slow |
| TLS handshake failure | Yes | 2 retries only | Usually persistent |
| DNS resolution failure | Yes | 3 retries, 1s base | Might be transient |

### Per-Operation Retry Behavior

**PUT (write path):**
1. Erasure encode object into chunks
2. Fan out chunk uploads to all sites
3. If a chunk upload fails, retry **to the same site** with backoff
4. If retries exhausted for a site, the chunk is marked `Pending` in metadata
5. As long as quorum (k) chunks succeed, return 200 to client
6. Background reconciler picks up `Pending` chunks and retries indefinitely
7. If quorum cannot be reached after all retries, return 503 to client
8. Log everything: which sites failed, which retries succeeded, final state

**GET (read path):**
1. Fan out chunk requests to all sites
2. If a chunk request fails, retry to the same site (fast, 2-3 attempts)
3. Don't wait for retries to block — other sites' chunks may arrive first
4. If k chunks arrive from other sites before retry succeeds, cancel the retry
5. If fewer than k chunks available after all attempts, return 503
6. Never return partial or corrupt data

**DELETE (delete path):**
1. Fan out delete requests to all sites
2. If a site fails, retry with backoff
3. If retries exhausted, queue for background cleanup
4. Return 204 to client once metadata is updated (deletes are best-effort at each site)
5. Background task ensures all chunks are eventually removed

**Metadata snapshot:**
1. Create RocksDB checkpoint
2. Compress with zstd
3. Upload to metadata bucket at each site
4. If a site fails, retry with longer backoff (this is background, no rush)
5. If all sites fail, hold snapshot locally and retry next cycle
6. Never lose a snapshot — worst case it stays local until connectivity returns

### Circuit Breaker Per Site

Beyond individual retries, each site has a circuit breaker:

```
States:
  CLOSED   -> Normal operation. Errors increment failure counter.
  OPEN     -> Site considered down. Skip sending requests, fail fast.
  HALF-OPEN -> After cooldown, send a single probe request.

Transitions:
  CLOSED -> OPEN:      Failure count exceeds threshold within window (e.g., 10 failures in 60s)
  OPEN -> HALF-OPEN:   After cooldown period (e.g., 30s)
  HALF-OPEN -> CLOSED: Probe request succeeds
  HALF-OPEN -> OPEN:   Probe request fails, reset cooldown
```

When a site is OPEN:
- Chunk uploads to that site are queued for later (marked `Pending`)
- Chunk reads skip that site (rely on other sites for reconstruction)
- Health checker continues probing
- Dashboard shows the site as degraded
- All queued operations replay when circuit closes

### Retry Metrics

Track retry behavior for observability:
- Retry rate per site (should be low in normal operation)
- Retry success rate (what percentage of retries eventually succeed)
- Circuit breaker state transitions (how often sites go OPEN)
- Time spent in degraded mode per site
- Background replication queue depth and drain rate

All visible in the web UI and exposed via Prometheus.

## Notifications and Alerting

The gateway should proactively notify operators when things need attention.

### Built-in Alert Rules

| Alert | Severity | Condition |
|-------|----------|-----------|
| Site unreachable | Critical | Health check failed for > 60s |
| Redundancy degraded | Critical | Any object has fewer than k+m confirmed chunks |
| Pending replication stale | Warning | Chunk pending > 15 minutes |
| Pending replication critical | Critical | Chunk pending > 1 hour |
| Write quorum failure | Critical | PUT failed because quorum couldn't be reached |
| High error rate | Warning | > 5% error rate on any site over 5 minutes |
| Certificate expiring | Warning | TLS cert expires within 14 days |
| Certificate expired | Critical | TLS cert has expired |
| Disk space low | Warning | RocksDB database partition > 80% full |
| Metadata sync failed | Warning | Snapshot upload failed to all sites |
| Rate limit active | Info | Any rate limit is being hit |
| Circuit breaker open | Warning | Any site/IP circuit breaker tripped |

### Alert Destinations

- **Web UI** — Alert banner + alert history page
- **Webhook** — POST JSON to a configurable URL (Slack, PagerDuty, Discord, custom)
- **Email** — SMTP integration for critical alerts (optional)
- **Prometheus alertmanager** — Via standard Prometheus metrics + alert rules
- **Log** — All alerts are logged at appropriate severity

```toml
[alerts]
enabled = true

[[alerts.webhooks]]
url = "https://hooks.slack.com/services/T00/B00/xxx"
events = ["critical", "warning"]
# headers = { "Authorization" = "Bearer token" }

# [alerts.email]
# smtp_host = "smtp.example.com"
# smtp_port = 587
# from = "s3prism@example.com"
# to = ["ops@example.com"]
# events = ["critical"]
```

## Data Recovery and Migration

### Recovery Tool

A companion CLI for recovering data if the gateway is lost:

```
s3prism recover \
    --from-snapshot s3://s3prism-meta-gw01/snapshots/latest.rocksdb.zst \
    --site us-east-1 --site us-east-2 --site us-west-1 \
    --output /recovered/
```

- Downloads metadata snapshot from S3
- Walks all objects in the metadata
- Fetches chunks from storage sites and reconstructs original objects
- Writes recovered files locally or to another S3 destination
- Progress bar, resume capability, parallel downloads

### Migration

Moving data between S3Prism configurations:

- **Change erasure scheme** — Re-encode existing objects from 2+1 to 3+2 (background job)
- **Add/remove sites** — Rebalance chunks to include new sites or evacuate removed ones
- **Export** — Bulk export all objects as plain files (de-prism)
- **Import** — Bulk import from existing backend buckets into S3Prism virtual buckets

All migration operations run as background tasks with progress tracking in the web UI.

## Concurrency and Connection Management

### Connection Pools

Per-site HTTP connection pools to backend storage:

```toml
[connections]
pool_size_per_site = 256               # Max concurrent connections per storage site
idle_timeout = "90s"                   # Close idle connections after this
connect_timeout = "5s"                 # TCP connect timeout
request_timeout = "300s"               # Overall request timeout (large objects)
pool_idle_size = 32                    # Keep this many idle connections warm
```

### Request Queuing

When connection pools are full:
- Queue incoming requests (bounded queue per site)
- Apply backpressure to clients if queue is full (503 with Retry-After)
- Track queue depth and wait time in metrics

### Graceful Shutdown

On SIGTERM or SIGINT:
1. Stop accepting new client connections
2. Drain in-flight requests (configurable timeout, default 30s)
3. Flush pending replication queue to metadata (mark as pending for next startup)
4. Final metadata snapshot to S3
5. Close all backend connections
6. Exit

```toml
[shutdown]
drain_timeout = "30s"                  # Max time to wait for in-flight requests
snapshot_on_shutdown = true             # Force metadata snapshot before exit
```

## Testing Strategy

### Unit Tests
- Erasure encoding/decoding correctness
- Metadata store CRUD operations
- Rate limiter token bucket math
- Config parsing and validation
- S3 XML serialization/deserialization
- Retry policy backoff calculations
- Load balancer strategy selection

### Integration Tests
- Full PUT/GET/DELETE round-trip through the gateway against mock S3 backends
- Erasure recovery: simulate site failure, verify reconstruction from parity
- Fan-out latency: verify fastest-response behavior
- Metadata snapshot/restore cycle
- Rate limiting enforcement
- Circuit breaker tripping and recovery
- Config hot-reload
- TLS certificate rotation

### Chaos Tests
- Kill a mock site mid-transfer — verify quorum still succeeds
- Introduce latency spikes — verify fan-out selects fastest
- Corrupt a chunk — verify checksum catches it and parity reconstructs
- Fill pending queue — verify reconciler drains it
- Simulate full disk — verify graceful degradation

### S3 Compatibility Tests
- Run the AWS S3 compatibility test suite against the gateway
- Validate with real tools: aws-cli, boto3, rclone, s3cmd, cyberduck
- Multipart upload edge cases (abort, resume, zero-byte parts)

## Open Questions

1. **Streaming** — Can we stream-encode erasure chunks without buffering the full object? For very large objects this matters. Reed-Solomon works on fixed-size blocks, so we can process in blocks and stream. Each block (e.g., 64MB) is independently erasure coded — this lets us stream encode/decode without holding the full object in memory.
2. **Consistency** — What happens if a client reads immediately after a write and the metadata hasn't been updated? Need write-through metadata updates (RocksDB WriteBatch is sync with WAL).
3. **Multi-gateway coordination** — If multiple gateways share the same buckets, how do they coordinate metadata? Deferred to Phase 2.
4. **Cost visibility** — Should the gateway expose metrics showing actual storage cost (1.5x) vs apparent storage (1x)?
5. **Versioning** — Deferred to Phase 3. Design is in the doc; each version will be independently coded. MVP is unversioned (latest-write-wins).
6. **Range reads** — How do range requests (byte ranges) work with erasure coding? Need to map the byte range to the right chunk(s) and decode only what's needed.
7. **Bandwidth cost** — Fan-out reads to all sites means pulling data from multiple regions even though we discard some. Worth it for latency? Should there be a "latency-optimized" vs "cost-optimized" read mode?
8. **Object size limits** — What's the maximum object size? Streaming erasure coding with 64MB blocks means no hard limit, but need to test with multi-TB objects.
