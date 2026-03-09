# S3Prism

**Multi-region erasure-coded S3 gateway.**

One beam in, many regions out. Recombine from anywhere.

S3Prism is a standalone S3-compatible gateway that distributes your data across multiple S3-compatible storage regions using Reed-Solomon erasure coding. It presents a single S3 endpoint backed by cross-region durability, automatic failover, and best-latency reads.

Think of your storage regions as disks in a RAID array — S3Prism is the RAID controller, operating at a global scale.

## Features

- **Erasure coding** — Reed-Solomon encoding splits objects across regions (e.g., 2+1, 4+2, 8+4). Survive full region outages without storing full copies everywhere.
- **Replica mode** — Full copies at every region for maximum durability and simplest reads. Switch per-bucket.
- **Hybrid mode** — Small objects replicated, large objects erasure-coded. Configurable threshold.
- **Quorum writes** — Returns success once minimum data chunks confirm (1 for replica, data_chunks for EC). Remaining writes complete in background for lower latency.
- **S3-compatible API** — Works with any S3 client: AWS CLI, s5cmd, boto3, rclone, MinIO warp, etc. Supports PUT, GET, HEAD, DELETE, CopyObject, multipart uploads, batch delete, ListObjectsV2 with pagination.
- **AWS Signature V4** — Full SigV4 signing for backend requests, plus transparent handling of `aws-chunked` streaming uploads from clients.
- **Virtual buckets** — Client-facing bucket names map to unique backend buckets per region. Backend keys are opaque UUIDs — fully decoupled from client keys.
- **Fan-out writes** — Chunks/replicas written to all target regions concurrently.
- **Configurable write distribution** — Shuffle mode randomly distributes chunks across regions. Priority mode uses a fixed ordering.
- **Event-driven purge queue** — Deletes are instant for clients. Backend cleanup fires immediately via notification, batching multi-object deletes across regions in parallel.
- **Multipart upload support** — Full S3 multipart protocol: CreateMultipartUpload, UploadPart, CompleteMultipartUpload, AbortMultipartUpload, ListMultipartUploads. Parts stored on backend storage (not local disk) with configurable concurrency limits.
- **Management web UI** — Configure sites, storage modes, erasure coding parameters, and monitor bucket stats from a clean web interface.
- **Region discovery** — Auto-discover available storage regions with latency benchmarking to find the best sites for your deployment.
- **Single binary** — No external dependencies. RocksDB for metadata, your S3-compatible storage for data.

## Screenshots
<img width="1222" height="494" alt="image" src="https://github.com/user-attachments/assets/fbca4d43-d3aa-4f6d-905a-884853605ecb" />

<img width="1231" height="558" alt="image" src="https://github.com/user-attachments/assets/124ba241-280a-458e-98e4-75c027037976" />

<img width="981" height="1146" alt="image" src="https://github.com/user-attachments/assets/3a647c62-485a-488a-85f4-5746a3d55a96" />

<img width="1238" height="607" alt="image" src="https://github.com/user-attachments/assets/6557f24d-54a5-4a74-a8a9-348d419fff5f" />



## Quick Start

### Build

```
cargo build --release
```

### Run

```
./target/release/s3prism serve
```

```
S3Prism starting...
Management UI: http://0.0.0.0:9090
No configuration found — open the management UI to complete setup
S3 endpoint will return 503 until setup is complete
```

On first launch, S3Prism starts in **unconfigured mode**. Open the management UI at `http://localhost:9090` to add your storage sites and configure erasure coding.

### Configure

1. Open `http://localhost:9090`
2. Add your S3-compatible storage sites (endpoint, region, access key, secret key)
3. Set your erasure coding parameters (data chunks, parity chunks, storage mode)
4. Create buckets via any S3 client

### Use

```bash
# Create a bucket
aws s3 mb s3://my-bucket --endpoint-url http://localhost:8443

# Upload a file
aws s3 cp file.bin s3://my-bucket/file.bin --endpoint-url http://localhost:8443

# Download
aws s3 cp s3://my-bucket/file.bin ./downloaded.bin --endpoint-url http://localhost:8443

# List objects
aws s3 ls s3://my-bucket/ --endpoint-url http://localhost:8443
```

Works with any S3 client:

```bash
# s5cmd
s5cmd --endpoint-url http://localhost:8443 cp file.bin s3://my-bucket/

# warp benchmark
warp mixed --host 127.0.0.1:8443 --bucket my-bucket --duration 60s --concurrent 20 --obj.size 10MiB --obj.randsize --lookup path
```

## Configuration

### Bootstrap Config (`s3prism.toml`)

```toml
s3_port = 8443
mgmt_port = 9090
bind_addr = "0.0.0.0"
db_path = "data/s3prism.rocksdb"
log_level = "info"

# Optional: protect the management UI
# mgmt_password = "your-password-here"
```

### Runtime Config

All runtime configuration (sites, erasure coding, storage modes) is managed through the web UI and stored in the embedded RocksDB database. Export/import configuration as JSON for backup or migration.

## Architecture

```
┌─────────────┐
│  S3 Client  │
└──────┬──────┘
       │ S3 API (PUT, GET, DELETE, ...)
┌──────▼──────┐
│   S3Prism   │──── Management UI (:9090)
│  Gateway    │──── RocksDB (metadata)
└──┬───┬───┬──┘
   │   │   │  Fan-out (parallel)
   ▼   ▼   ▼
┌───┐┌───┐┌───┐
│ A ││ B ││ C │  S3-compatible storage regions
└───┘└───┘└───┘
```

### Storage Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| **Replica** | Full copy at every region | Maximum durability, simplest reads |
| **Erasure** | Reed-Solomon encoded chunks distributed across regions | Storage efficient, survives region failures |
| **Hybrid** | Small objects replicated, large objects erasure-coded | Best of both worlds (configurable threshold) |

### Write Path

1. Client PUTs an object to S3Prism
2. Gateway generates a UUID backend key (fully decoupled from client key)
3. Based on storage mode:
   - **Replica**: object written to all regions in parallel, returns after first confirms (quorum=1)
   - **Erasure**: object encoded into data + parity chunks, each written to a different region, returns after data chunks confirm
4. Metadata (client key → backend key mapping, chunk locations) stored in RocksDB
5. Remaining background writes complete asynchronously

### Read Path

1. Client GETs an object from S3Prism
2. Gateway looks up metadata to find chunk/replica locations
3. Fetches from backend regions in parallel (fan-out)
4. For erasure-coded objects, reconstructs from any sufficient subset of chunks
5. Returns the object to the client

### Delete Path

1. Client DELETEs an object
2. Gateway immediately removes metadata and returns success
3. Backend objects queued for async purge
4. Purge reaper wakes immediately, batches deletes per region using S3 multi-object delete API

## Project Structure

```
src/
├── api/            S3-compatible API (axum)
│   ├── handlers/   PUT, GET, DELETE, multipart, list, buckets
│   ├── chunked.rs  AWS SigV4 chunked upload decoder
│   ├── router.rs   Query-param dispatching
│   └── xml.rs      S3 XML request/response handling
├── backend/        S3 client for backend storage regions
│   ├── client.rs   Per-site HTTP client with SigV4 signing
│   ├── fanout.rs   Parallel fan-out for reads/writes
│   └── signing.rs  AWS Signature V4
├── erasure_coding/ Reed-Solomon encode/decode (reed-solomon-simd)
├── metadata/       RocksDB metadata store
│   ├── models.rs   Data models (ObjectMeta, BucketMeta, ChunkInfo)
│   ├── store.rs    Storage trait
│   └── purge_queue.rs  Event-driven backend cleanup
├── web/            Management UI and API
├── config.rs       Bootstrap + runtime configuration
└── main.rs         Entry point
```

## Performance

Benchmarked with [warp](https://github.com/minio/warp) against 3 backend regions with 2+1 erasure coding, quorum writes enabled:

```
PUT (random objects up to 10 MiB, 20 concurrent):
  Average: ~60 MiB/s, ~33 obj/s
  0 errors

GET (random objects up to 10 MiB, 4 concurrent):
  Average: ~38 MiB/s, ~23 obj/s
  0 errors
```

Throughput scales with concurrency and object size. Quorum writes reduce PUT latency by returning once minimum data chunks confirm.

## Requirements

- Rust 1.85+ (2024 edition)
- One or more S3-compatible storage endpoints

## License

Apache License 2.0 — see [LICENSE](LICENSE).
