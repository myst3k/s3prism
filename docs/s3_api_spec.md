# AWS S3 API Complete Specification Reference

> Generated: 2026-03-07
> Source: https://docs.aws.amazon.com/AmazonS3/latest/API/API_Operations_Amazon_Simple_Storage_Service.html
> This document contains the complete specification for every Amazon S3 API operation.

---

# Table of Contents

1. [Object Operations](#1-object-operations)
2. [Bucket Operations](#2-bucket-operations)
3. [Multipart Upload Operations](#3-multipart-upload-operations)
4. [Access Control Operations](#4-access-control-operations)
5. [Bucket Configuration Operations](#5-bucket-configuration-operations)
6. [Tagging Operations](#6-tagging-operations)
7. [Legal Hold / Object Lock / Retention](#7-legal-hold--object-lock--retention)
8. [Analytics / Metrics / Inventory / Intelligent Tiering](#8-analytics--metrics--inventory--intelligent-tiering)
9. [Other Operations](#9-other-operations)

---

# 1. Object Operations

## 1.1 GetObject

Retrieves objects from Amazon S3.

**Request**
```
GET /{Key}?partNumber={PartNumber}&response-cache-control={ResponseCacheControl}&response-content-disposition={ResponseContentDisposition}&response-content-encoding={ResponseContentEncoding}&response-content-language={ResponseContentLanguage}&response-content-type={ResponseContentType}&response-expires={ResponseExpires}&versionId={VersionId} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**URI Parameters**

| Parameter | Required | Type | Description |
|-----------|----------|------|-------------|
| Bucket | Yes | String | Bucket name. Directory buckets: `bucket-name--zone-id--x-s3` |
| Key | Yes | String | Object key (min length: 1) |

**Query Parameters**

| Parameter | Required | Type | Description |
|-----------|----------|------|-------------|
| partNumber | No | Integer | Part number (1-10,000) for ranged GET of multipart objects |
| versionId | No | String | Version ID (directory buckets: only `null`) |
| response-cache-control | No | String | Sets Cache-Control response header (must be signed) |
| response-content-disposition | No | String | Sets Content-Disposition response header (must be signed) |
| response-content-encoding | No | String | Sets Content-Encoding response header (must be signed) |
| response-content-language | No | String | Sets Content-Language response header (must be signed) |
| response-content-type | No | String | Sets Content-Type response header (must be signed) |
| response-expires | No | String | Sets Expires response header (must be signed) |

**Request Headers**

| Header | Required | Type | Description |
|--------|----------|------|-------------|
| If-Match | No | String | Return only if ETag matches; otherwise 412 |
| If-Modified-Since | No | DateTime | Return only if modified since; otherwise 304 |
| If-None-Match | No | String | Return only if ETag differs; otherwise 304 |
| If-Unmodified-Since | No | DateTime | Return only if not modified since; otherwise 412 |
| Range | No | String | Byte range: `bytes=start-end` (single range only) |
| x-amz-server-side-encryption-customer-algorithm | No | String | SSE-C algorithm (e.g., `AES256`). Not for directory buckets |
| x-amz-server-side-encryption-customer-key | No | String | SSE-C encryption key. Not for directory buckets |
| x-amz-server-side-encryption-customer-key-MD5 | No | String | MD5 of SSE-C key. Not for directory buckets |
| x-amz-request-payer | No | String | `requester`. Not for directory buckets |
| x-amz-expected-bucket-owner | No | String | Account ID of expected bucket owner (403 if mismatch) |
| x-amz-checksum-mode | No | String | `ENABLED` to retrieve object checksum |

**Request Body**: None

**Response Status Codes**

| Code | Description |
|------|-------------|
| 200 | Success |
| 206 | Partial Content (Range request) |
| 304 | Not Modified |
| 403 | InvalidObjectState (archived) or Access Denied |
| 404 | NoSuchKey |
| 405 | Method Not Allowed (delete marker) |
| 412 | Precondition Failed |

**Response Headers**

| Header | Type | Description |
|--------|------|-------------|
| accept-ranges | String | Byte range support |
| Cache-Control | String | Caching behavior |
| Content-Disposition | String | Presentational information |
| Content-Encoding | String | Content encodings applied |
| Content-Language | String | Content language |
| Content-Length | Integer | Body size in bytes |
| Content-Range | String | Byte range portion (Range requests) |
| Content-Type | String | MIME type |
| ETag | String | Entity tag |
| Expires | DateTime | Cache expiration |
| Last-Modified | DateTime | Last modification time |
| x-amz-checksum-crc32 | String | CRC32 checksum (if uploaded) |
| x-amz-checksum-crc32c | String | CRC32C checksum (if uploaded) |
| x-amz-checksum-crc64nvme | String | CRC64NVME checksum |
| x-amz-checksum-sha1 | String | SHA1 checksum (if uploaded) |
| x-amz-checksum-sha256 | String | SHA256 checksum (if uploaded) |
| x-amz-checksum-type | String | `COMPOSITE` or `FULL_OBJECT` |
| x-amz-delete-marker | Boolean | Whether object is a delete marker |
| x-amz-expiration | String | Expiration config (expiry-date, rule-id). Not for directory buckets |
| x-amz-missing-meta | Integer | Metadata entries not returned. Not for directory buckets |
| x-amz-mp-parts-count | Integer | Part count (if partNumber specified) |
| x-amz-object-lock-legal-hold | String | `ON` or `OFF`. Not for directory buckets |
| x-amz-object-lock-mode | String | `GOVERNANCE` or `COMPLIANCE`. Not for directory buckets |
| x-amz-object-lock-retain-until-date | DateTime | Lock expiration. Not for directory buckets |
| x-amz-replication-status | String | `COMPLETE`, `PENDING`, `FAILED`, `REPLICA`, `COMPLETED`. Not for directory buckets |
| x-amz-request-charged | String | `requester`. Not for directory buckets |
| x-amz-restore | String | Archive restoration info. Not for directory buckets |
| x-amz-server-side-encryption | String | `AES256`, `aws:fsx`, `aws:kms`, `aws:kms:dsse` |
| x-amz-server-side-encryption-aws-kms-key-id | String | KMS key ID |
| x-amz-server-side-encryption-bucket-key-enabled | Boolean | S3 Bucket Key enabled |
| x-amz-server-side-encryption-customer-algorithm | String | SSE-C algorithm. Not for directory buckets |
| x-amz-server-side-encryption-customer-key-MD5 | String | SSE-C key MD5. Not for directory buckets |
| x-amz-storage-class | String | Storage class |
| x-amz-tagging-count | Integer | Tag count. Not for directory buckets |
| x-amz-version-id | String | Version ID. Not for directory buckets |
| x-amz-website-redirect-location | String | Redirect URL. Not for directory buckets |

**Response Body**: Binary object data

**Error Codes**

| Code | Status | Description |
|------|--------|-------------|
| InvalidObjectState | 403 | Object archived, must restore first |
| NoSuchKey | 404 | Key does not exist |

---

## 1.2 PutObject

Adds an object to a bucket.

**Request**
```
PUT /{Key+} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**URI Parameters**

| Parameter | Required | Type | Description |
|-----------|----------|------|-------------|
| Bucket | Yes | String | Bucket name |
| Key | Yes | String | Object key (min length: 1) |

**Request Headers**

| Header | Required | Type | Description |
|--------|----------|------|-------------|
| Cache-Control | No | String | Caching behavior |
| Content-Disposition | No | String | Presentational information |
| Content-Encoding | No | String | Content encodings |
| Content-Language | No | String | Content language |
| Content-Length | No | Integer | Body size |
| Content-MD5 | No | String | Base64 MD5 digest |
| Content-Type | No | String | MIME type |
| Expires | No | DateTime | Cache expiration |
| If-Match | No | String | Upload only if ETag matches |
| If-None-Match | No | String | Upload only if key doesn't exist (`*`) |
| x-amz-acl | No | String | Canned ACL: `private`, `public-read`, `public-read-write`, `authenticated-read`, `aws-exec-read`, `bucket-owner-read`, `bucket-owner-full-control`. Not for directory buckets |
| x-amz-grant-full-control | No | String | Grant all permissions. Not for directory buckets |
| x-amz-grant-read | No | String | Grant read. Not for directory buckets |
| x-amz-grant-read-acp | No | String | Grant read ACL. Not for directory buckets |
| x-amz-grant-write-acp | No | String | Grant write ACL. Not for directory buckets |
| x-amz-checksum-crc32 | No | String | CRC32 checksum |
| x-amz-checksum-crc32c | No | String | CRC32C checksum |
| x-amz-checksum-crc64nvme | No | String | CRC64NVME checksum |
| x-amz-checksum-sha1 | No | String | SHA1 checksum |
| x-amz-checksum-sha256 | No | String | SHA256 checksum |
| x-amz-sdk-checksum-algorithm | No | String | `CRC32`, `CRC32C`, `SHA1`, `SHA256`, `CRC64NVME` |
| x-amz-server-side-encryption | No | String | `AES256`, `aws:fsx`, `aws:kms`, `aws:kms:dsse` |
| x-amz-server-side-encryption-customer-algorithm | No | String | SSE-C algorithm. Not for directory buckets |
| x-amz-server-side-encryption-customer-key | No | String | SSE-C key. Not for directory buckets |
| x-amz-server-side-encryption-customer-key-MD5 | No | String | SSE-C key MD5. Not for directory buckets |
| x-amz-server-side-encryption-aws-kms-key-id | No | String | KMS key ID |
| x-amz-server-side-encryption-context | No | String | Base64-encoded KMS context |
| x-amz-server-side-encryption-bucket-key-enabled | No | Boolean | Enable S3 Bucket Key |
| x-amz-object-lock-mode | No | String | `GOVERNANCE` or `COMPLIANCE`. Not for directory buckets |
| x-amz-object-lock-retain-until-date | No | Timestamp | Lock expiration. Not for directory buckets |
| x-amz-object-lock-legal-hold | No | String | `ON` or `OFF`. Not for directory buckets |
| x-amz-storage-class | No | String | Storage class (see GetObject for values) |
| x-amz-tagging | No | String | URL-encoded tag-set. Not for directory buckets |
| x-amz-website-redirect-location | No | String | Redirect URL. Not for directory buckets |
| x-amz-write-offset-bytes | No | Integer | Append offset (S3 Express One Zone only) |
| x-amz-expected-bucket-owner | No | String | Account ID validation |
| x-amz-request-payer | No | String | `requester`. Not for directory buckets |

**Request Body**: Binary object data (required)

**Response Status Codes**

| Code | Description |
|------|-------------|
| 200 | Success |
| 400 | EncryptionTypeMismatch, InvalidRequest, InvalidWriteOffset, TooManyParts |
| 403 | Access Denied |
| 409 | ConditionalRequestConflict |
| 412 | Precondition Failed |

**Response Headers**

| Header | Type | Description |
|--------|------|-------------|
| ETag | String | Entity tag |
| x-amz-version-id | String | Version ID. Not for directory buckets |
| x-amz-checksum-crc32 | String | CRC32 checksum |
| x-amz-checksum-crc32c | String | CRC32C checksum |
| x-amz-checksum-crc64nvme | String | CRC64NVME checksum |
| x-amz-checksum-sha1 | String | SHA1 checksum |
| x-amz-checksum-sha256 | String | SHA256 checksum |
| x-amz-checksum-type | String | `COMPOSITE` or `FULL_OBJECT` |
| x-amz-server-side-encryption | String | Encryption algorithm |
| x-amz-server-side-encryption-customer-algorithm | String | SSE-C confirmation |
| x-amz-server-side-encryption-customer-key-MD5 | String | SSE-C key verification |
| x-amz-server-side-encryption-aws-kms-key-id | String | KMS key ID |
| x-amz-server-side-encryption-context | String | KMS context |
| x-amz-server-side-encryption-bucket-key-enabled | Boolean | Bucket Key enabled |
| x-amz-expiration | String | Lifecycle expiration info |
| x-amz-object-size | Integer | Object size (append operations) |
| x-amz-request-charged | String | `requester` |

**Response Body**: None

---

## 1.3 HeadObject

Retrieves metadata without returning the object body.

**Request**
```
HEAD /{Key}?partNumber={PartNumber}&versionId={VersionId} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**URI Parameters**

| Parameter | Required | Type | Description |
|-----------|----------|------|-------------|
| Bucket | Yes | String | Bucket name |
| Key | Yes | String | Object key |
| partNumber | No | Integer | Part number (1-10,000) |
| versionId | No | String | Version ID |

**Request Headers**: Same conditional headers as GetObject (If-Match, If-Modified-Since, If-None-Match, If-Unmodified-Since, Range), plus SSE-C headers, x-amz-request-payer, x-amz-expected-bucket-owner, x-amz-checksum-mode.

**Request Body**: None

**Response Status Codes**: 200, 304, 400, 403, 404, 405, 412

**Response Headers**: Same as GetObject (all metadata headers) but no response body.

**Response Body**: None (HEAD request)

---

## 1.4 DeleteObject

Removes an object from a bucket.

**Request**
```
DELETE /{Key+}?versionId={VersionId} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**URI Parameters**

| Parameter | Required | Type | Description |
|-----------|----------|------|-------------|
| Bucket | Yes | String | Bucket name |
| Key | Yes | String | Object key |
| versionId | No | String | Version ID |

**Request Headers**

| Header | Required | Type | Description |
|--------|----------|------|-------------|
| x-amz-mfa | No | String | MFA serial + code. Not for directory buckets |
| x-amz-request-payer | No | String | `requester`. Not for directory buckets |
| x-amz-bypass-governance-retention | No | Boolean | Bypass governance lock. Not for directory buckets |
| x-amz-expected-bucket-owner | No | String | Account ID validation |
| If-Match | No | String | Delete only if ETag matches |
| x-amz-if-match-last-modified-time | No | Timestamp | Delete only if modification time matches (directory buckets) |
| x-amz-if-match-size | No | Integer | Delete only if size matches (directory buckets) |

**Request Body**: None

**Response Status Codes**: 204 (success), 403, 412

**Response Headers**

| Header | Type | Description |
|--------|------|-------------|
| x-amz-delete-marker | Boolean | Whether deleted version was a delete marker |
| x-amz-version-id | String | Version ID of delete marker created |
| x-amz-request-charged | String | `requester` |

**Response Body**: None

---

## 1.5 DeleteObjects

Deletes multiple objects (up to 1,000) in a single request.

**Request**
```
POST /?delete HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Request Headers**

| Header | Required | Type | Description |
|--------|----------|------|-------------|
| x-amz-mfa | Conditional | String | MFA token for MFA-enabled buckets |
| x-amz-request-payer | No | String | `requester` |
| x-amz-bypass-governance-retention | No | Boolean | Bypass governance lock |
| x-amz-expected-bucket-owner | No | String | Account ID validation |
| Content-MD5 | Required (GP) | String | MD5 of request body |
| x-amz-sdk-checksum-algorithm | No | String | Checksum algorithm |

**Request Body (XML)**
```xml
<Delete>
    <Object>
        <Key>string</Key>
        <VersionId>string</VersionId>
        <ETag>string</ETag>
        <LastModifiedTime>timestamp</LastModifiedTime>
        <Size>long</Size>
    </Object>
    <Quiet>boolean</Quiet>
</Delete>
```

**Response Status Codes**: 200 (GP), 204 (directory), 400, 403

**Response Body (XML)**
```xml
<DeleteResult>
    <Deleted>
        <Key>string</Key>
        <VersionId>string</VersionId>
        <DeleteMarker>boolean</DeleteMarker>
        <DeleteMarkerVersionId>string</DeleteMarkerVersionId>
    </Deleted>
    <Error>
        <Key>string</Key>
        <Code>string</Code>
        <Message>string</Message>
        <VersionId>string</VersionId>
    </Error>
</DeleteResult>
```

---

## 1.6 CopyObject

Creates a copy of an existing object (up to 5 GB single copy).

**Request**
```
PUT /{Key} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
x-amz-copy-source: /{SourceBucket}/{SourceKey}
```

**Request Headers**

| Header | Required | Type | Description |
|--------|----------|------|-------------|
| x-amz-copy-source | Yes | String | Source: `/bucket/key` or ARN. Supports `?versionId=` |
| x-amz-metadata-directive | No | String | `COPY` (default) or `REPLACE` |
| x-amz-tagging-directive | No | String | `COPY` (default) or `REPLACE` |
| x-amz-copy-source-if-match | No | String | Copy if source ETag matches |
| x-amz-copy-source-if-modified-since | No | Timestamp | Copy if source modified since |
| x-amz-copy-source-if-none-match | No | String | Copy if source ETag differs |
| x-amz-copy-source-if-unmodified-since | No | Timestamp | Copy if source not modified since |
| If-Match | No | String | Copy if destination ETag matches |
| If-None-Match | No | String | Copy if destination key doesn't exist |
| x-amz-acl | No | String | Canned ACL for destination |
| x-amz-checksum-algorithm | No | String | Checksum algorithm |
| x-amz-server-side-encryption | No | String | Encryption algorithm |
| x-amz-server-side-encryption-aws-kms-key-id | No | String | KMS key ID |
| x-amz-server-side-encryption-context | No | String | KMS context |
| x-amz-server-side-encryption-bucket-key-enabled | No | Boolean | Bucket Key |
| x-amz-server-side-encryption-customer-algorithm | No | String | SSE-C destination algorithm |
| x-amz-server-side-encryption-customer-key | No | String | SSE-C destination key |
| x-amz-server-side-encryption-customer-key-MD5 | No | String | SSE-C destination key MD5 |
| x-amz-copy-source-server-side-encryption-customer-algorithm | No | String | SSE-C source algorithm |
| x-amz-copy-source-server-side-encryption-customer-key | No | String | SSE-C source key |
| x-amz-copy-source-server-side-encryption-customer-key-MD5 | No | String | SSE-C source key MD5 |
| x-amz-storage-class | No | String | Destination storage class |
| x-amz-tagging | No | String | Destination tags |
| x-amz-object-lock-mode | No | String | `GOVERNANCE` or `COMPLIANCE` |
| x-amz-object-lock-retain-until-date | No | Timestamp | Lock expiration |
| x-amz-object-lock-legal-hold | No | String | `ON` or `OFF` |
| x-amz-expected-bucket-owner | No | String | Destination account validation |
| x-amz-source-expected-bucket-owner | No | String | Source account validation |
| x-amz-request-payer | No | String | `requester` |
| x-amz-grant-full-control | No | String | Grant permissions |
| x-amz-grant-read | No | String | Grant read |
| x-amz-grant-read-acp | No | String | Grant read ACL |
| x-amz-grant-write-acp | No | String | Grant write ACL |
| x-amz-website-redirect-location | No | String | Redirect location |
| Cache-Control | No | String | Caching |
| Content-Disposition | No | String | Presentational info |
| Content-Encoding | No | String | Content encoding |
| Content-Language | No | String | Language |
| Content-Type | No | String | MIME type |
| Expires | No | String | Cache expiration |

**Request Body**: None

**Response Status Codes**: 200, 400, 403, 404, 409, 412

**Response Headers**: x-amz-expiration, x-amz-copy-source-version-id, x-amz-version-id, x-amz-server-side-encryption, x-amz-server-side-encryption-aws-kms-key-id, x-amz-server-side-encryption-bucket-key-enabled, x-amz-server-side-encryption-context, x-amz-server-side-encryption-customer-algorithm, x-amz-server-side-encryption-customer-key-MD5, x-amz-request-charged

**Response Body (XML)**
```xml
<CopyObjectResult>
   <ETag>string</ETag>
   <LastModified>timestamp</LastModified>
   <ChecksumType>string</ChecksumType>
   <ChecksumCRC32>string</ChecksumCRC32>
   <ChecksumCRC32C>string</ChecksumCRC32C>
   <ChecksumCRC64NVME>string</ChecksumCRC64NVME>
   <ChecksumSHA1>string</ChecksumSHA1>
   <ChecksumSHA256>string</ChecksumSHA256>
</CopyObjectResult>
```

**Error Codes**: ObjectNotInActiveTierError (403), NoSuchBucket (404), NoSuchKey (404)

---

## 1.7 GetObjectAttributes

Retrieves object metadata (combines HeadObject and ListParts).

**Request**
```
GET /{Key+}?attributes&versionId={VersionId} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Request Headers**

| Header | Required | Type | Description |
|--------|----------|------|-------------|
| x-amz-object-attributes | Yes | String | Fields: `ETag`, `Checksum`, `ObjectParts`, `StorageClass`, `ObjectSize` |
| x-amz-max-parts | No | Integer | Max parts to return |
| x-amz-part-number-marker | No | Integer | Part listing start |
| x-amz-expected-bucket-owner | No | String | Account validation |
| x-amz-request-payer | No | String | `requester` |
| x-amz-server-side-encryption-customer-algorithm | No | String | SSE-C algorithm |
| x-amz-server-side-encryption-customer-key | No | String | SSE-C key |
| x-amz-server-side-encryption-customer-key-MD5 | No | String | SSE-C key MD5 |

**Response Body (XML)**
```xml
<GetObjectAttributesResponse>
    <ETag>string</ETag>
    <Checksum>
        <ChecksumCRC32>string</ChecksumCRC32>
        <ChecksumCRC32C>string</ChecksumCRC32C>
        <ChecksumCRC64NVME>string</ChecksumCRC64NVME>
        <ChecksumSHA1>string</ChecksumSHA1>
        <ChecksumSHA256>string</ChecksumSHA256>
        <ChecksumType>string</ChecksumType>
    </Checksum>
    <ObjectParts>
        <IsTruncated>boolean</IsTruncated>
        <MaxParts>integer</MaxParts>
        <NextPartNumberMarker>integer</NextPartNumberMarker>
        <PartNumberMarker>integer</PartNumberMarker>
        <Part>
            <PartNumber>integer</PartNumber>
            <Size>long</Size>
            <ChecksumCRC32>string</ChecksumCRC32>
            <!-- other checksums -->
        </Part>
        <PartsCount>integer</PartsCount>
    </ObjectParts>
    <StorageClass>string</StorageClass>
    <ObjectSize>long</ObjectSize>
</GetObjectAttributesResponse>
```

---

## 1.8 RestoreObject

Restores an archived object. Not for directory buckets.

**Request**
```
POST /{Key+}?restore&versionId={VersionId} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Request Headers**

| Header | Required | Description |
|--------|----------|-------------|
| x-amz-expected-bucket-owner | No | Account validation |
| x-amz-request-payer | No | `requester` |
| x-amz-sdk-checksum-algorithm | No | Checksum algorithm |

**Request Body (XML)**
```xml
<RestoreRequest>
    <Days>integer</Days>
    <GlacierJobParameters>
        <Tier>Standard|Bulk|Expedited</Tier>
    </GlacierJobParameters>
    <Type>SELECT</Type>
    <Description>string</Description>
    <SelectParameters>...</SelectParameters>
    <OutputLocation>
        <S3>
            <BucketName>string</BucketName>
            <Prefix>string</Prefix>
            <CannedACL>string</CannedACL>
            <StorageClass>string</StorageClass>
            <Encryption>...</Encryption>
            <Tagging>...</Tagging>
            <UserMetadata>...</UserMetadata>
            <AccessControlList>...</AccessControlList>
        </S3>
    </OutputLocation>
</RestoreRequest>
```

**Response Status Codes**

| Code | Description |
|------|-------------|
| 200 | Previously restored; expiry updated |
| 202 | Restore initiated |
| 403 | ObjectAlreadyInActiveTierError |
| 409 | RestoreAlreadyInProgress |
| 503 | GlacierExpeditedRetrievalNotAvailable |

---

## 1.9 SelectObjectContent

Filters object contents using SQL. Not for directory buckets.

**Request**
```
POST /{Key+}?select&select-type=2 HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Request Headers**: x-amz-expected-bucket-owner, SSE-C headers

**Request Body (XML)**
```xml
<SelectObjectContentRequest>
   <Expression>string</Expression>
   <ExpressionType>SQL</ExpressionType>
   <InputSerialization>
      <CompressionType>GZIP|BZIP2|NONE</CompressionType>
      <CSV>...</CSV>
      <JSON><Type>DOCUMENT|LINES</Type></JSON>
      <Parquet/>
   </InputSerialization>
   <OutputSerialization>
      <CSV>...</CSV>
      <JSON><RecordDelimiter>string</RecordDelimiter></JSON>
   </OutputSerialization>
   <RequestProgress><Enabled>boolean</Enabled></RequestProgress>
   <ScanRange><Start>long</Start><End>long</End></ScanRange>
</SelectObjectContentRequest>
```

**Response**: Streamed XML with Records, Stats, Progress, Cont, End events.

---

## 1.10 GetObjectTorrent

Returns BitTorrent file for an object. Not for directory buckets.

**Request**
```
GET /{Key+}?torrent HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Constraints**: Objects must be < 5 GB and not SSE-C encrypted.

**Response**: Binary bencoded BitTorrent dictionary. Content-Type: `application/x-bittorrent`

---

## 1.11 RenameObject

Renames an object in a directory bucket (S3 Express One Zone only).

**Request**
```
PUT /{Key+}?renameObject HTTP/1.1
Host: {Bucket}.s3express-{zone-id}.{region}.amazonaws.com
x-amz-rename-source: /{SourceKey}
```

**Request Headers**

| Header | Required | Description |
|--------|----------|-------------|
| x-amz-rename-source | Yes | Source object path (URL encoded) |
| x-amz-client-token | No | Idempotency token (max 64 chars) |
| If-Match | No | Rename only if destination ETag matches |
| If-None-Match | No | Rename only if destination doesn't exist (`*`) |
| x-amz-rename-source-if-match | No | Rename only if source ETag matches |
| x-amz-rename-source-if-none-match | No | Rename only if source ETag differs |
| x-amz-rename-source-if-modified-since | No | Rename if source modified since |
| x-amz-rename-source-if-unmodified-since | No | Rename if source not modified since |

**Response**: 200 OK (empty body), 400 (IdempotencyParameterMismatch), 412 (Precondition Failed)

---

## 1.12 UpdateObjectEncryption

Updates encryption settings for an existing object. General purpose buckets only.

**Request**
```
PUT /{Key+}?encryption&versionId={VersionId} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Request Headers**: x-amz-request-payer, x-amz-expected-bucket-owner, Content-MD5, x-amz-sdk-checksum-algorithm

**Request Body (XML)**
```xml
<ObjectEncryption>
   <SSE-KMS>
      <BucketKeyEnabled>boolean</BucketKeyEnabled>
      <KMSKeyArn>string</KMSKeyArn>
   </SSE-KMS>
</ObjectEncryption>
```

**Response**: 200 (success), 400 (InvalidRequest), 403 (AccessDenied), 404 (NoSuchKey)

---

# 2. Bucket Operations

## 2.1 CreateBucket

**Request**
```
PUT / HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Request Headers**

| Header | Required | Description |
|--------|----------|-------------|
| x-amz-acl | No | Canned ACL: `private`, `public-read`, `public-read-write`, `authenticated-read`. Not for directory buckets |
| x-amz-grant-full-control | No | Grant all permissions. Not for directory buckets |
| x-amz-grant-read | No | Grant list objects. Not for directory buckets |
| x-amz-grant-read-acp | No | Grant read ACL. Not for directory buckets |
| x-amz-grant-write | No | Grant create objects. Not for directory buckets |
| x-amz-grant-write-acp | No | Grant write ACL. Not for directory buckets |
| x-amz-bucket-object-lock-enabled | No | Enable Object Lock. Not for directory buckets |
| x-amz-object-ownership | No | `BucketOwnerPreferred`, `ObjectWriter`, `BucketOwnerEnforced` (default). Not for directory buckets |

**Request Body (XML)**
```xml
<CreateBucketConfiguration>
   <LocationConstraint>string</LocationConstraint>
   <Location>
      <Name>string</Name>
      <Type>string</Type>
   </Location>
   <Bucket>
      <DataRedundancy>string</DataRedundancy>
      <Type>string</Type>
   </Bucket>
   <Tags><Tag><Key>string</Key><Value>string</Value></Tag></Tags>
</CreateBucketConfiguration>
```

**Response**: 200 OK. Headers: Location (bucket path), x-amz-bucket-arn (directory buckets)

**Error Codes**: BucketAlreadyExists (409), BucketAlreadyOwnedByYou (409)

---

## 2.2 DeleteBucket

**Request**
```
DELETE / HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Headers**: x-amz-expected-bucket-owner (optional, not for directory buckets)

**Response**: 204 No Content

**Prerequisites**: All objects and versions must be deleted first.

---

## 2.3 HeadBucket

Checks bucket existence and access permissions.

**Request**
```
HEAD / HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Headers**: x-amz-expected-bucket-owner (optional)

**Response Status Codes**: 200, 301, 400, 403, 404

**Response Headers**

| Header | Description |
|--------|-------------|
| x-amz-bucket-arn | Bucket ARN (directory buckets only) |
| x-amz-bucket-location-type | `AvailabilityZone` or `LocalZone` (directory buckets) |
| x-amz-bucket-location-name | Zone ID |
| x-amz-bucket-region | AWS Region |
| x-amz-access-point-alias | Whether name is access point alias |

---

## 2.4 ListBuckets

**Request**
```
GET /?bucket-region={Region}&continuation-token={Token}&max-buckets={Max}&prefix={Prefix} HTTP/1.1
Host: s3.amazonaws.com
```

**Query Parameters**: bucket-region, continuation-token (0-1024 chars), max-buckets (1-10000), prefix

**Response Body (XML)**
```xml
<ListAllMyBucketsResult>
    <Buckets>
        <Bucket>
            <BucketArn>string</BucketArn>
            <BucketRegion>string</BucketRegion>
            <CreationDate>timestamp</CreationDate>
            <Name>string</Name>
        </Bucket>
    </Buckets>
    <Owner><DisplayName>string</DisplayName><ID>string</ID></Owner>
    <ContinuationToken>string</ContinuationToken>
    <Prefix>string</Prefix>
</ListAllMyBucketsResult>
```

---

## 2.5 ListDirectoryBuckets

**Request**
```
GET /?continuation-token={Token}&max-directory-buckets={Max} HTTP/1.1
Host: s3express-control.{region}.amazonaws.com
```

**Response Body (XML)**
```xml
<ListAllMyDirectoryBucketsResult>
    <Buckets><Bucket><BucketArn>string</BucketArn><BucketRegion>string</BucketRegion><CreationDate>timestamp</CreationDate><Name>string</Name></Bucket></Buckets>
    <ContinuationToken>string</ContinuationToken>
</ListAllMyDirectoryBucketsResult>
```

---

## 2.6 ListObjects (Deprecated - use ListObjectsV2)

**Request**
```
GET /?delimiter={D}&encoding-type={E}&marker={M}&max-keys={N}&prefix={P} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Response Body (XML)**: ListBucketResult with IsTruncated, Marker, NextMarker, Contents (Key, LastModified, ETag, Size, StorageClass, Owner), CommonPrefixes.

---

## 2.7 ListObjectsV2

**Request**
```
GET /?list-type=2&continuation-token={Token}&delimiter={D}&encoding-type={E}&fetch-owner={Bool}&max-keys={N}&prefix={P}&start-after={S} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Query Parameters**

| Parameter | Required | Description |
|-----------|----------|-------------|
| list-type | Yes | Must be `2` |
| continuation-token | No | Pagination token |
| delimiter | No | Grouping character (directory buckets: `/` only) |
| encoding-type | No | `url` |
| fetch-owner | No | Include Owner in response |
| max-keys | No | 1-1000 (default: 1000) |
| prefix | No | Filter by prefix |
| start-after | No | Start after this key. Not for directory buckets |

**Request Headers**: x-amz-request-payer, x-amz-expected-bucket-owner, x-amz-optional-object-attributes (`RestoreStatus`)

**Response Body (XML)**
```xml
<ListBucketResult>
    <Name>string</Name>
    <Prefix>string</Prefix>
    <Delimiter>string</Delimiter>
    <MaxKeys>integer</MaxKeys>
    <IsTruncated>boolean</IsTruncated>
    <KeyCount>integer</KeyCount>
    <ContinuationToken>string</ContinuationToken>
    <NextContinuationToken>string</NextContinuationToken>
    <Contents>
        <Key>string</Key>
        <LastModified>timestamp</LastModified>
        <ETag>string</ETag>
        <Size>long</Size>
        <StorageClass>string</StorageClass>
        <Owner><ID>string</ID><DisplayName>string</DisplayName></Owner>
        <ChecksumAlgorithm>string</ChecksumAlgorithm>
        <ChecksumType>string</ChecksumType>
        <RestoreStatus><IsRestoreInProgress>boolean</IsRestoreInProgress><RestoreExpiryDate>timestamp</RestoreExpiryDate></RestoreStatus>
    </Contents>
    <CommonPrefixes><Prefix>string</Prefix></CommonPrefixes>
</ListBucketResult>
```

---

## 2.8 ListObjectVersions

Not supported for directory buckets.

**Request**
```
GET /?versions&delimiter={D}&encoding-type={E}&key-marker={K}&max-keys={N}&prefix={P}&version-id-marker={V} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Response Body (XML)**
```xml
<ListVersionsResult>
    <IsTruncated>boolean</IsTruncated>
    <KeyMarker>string</KeyMarker>
    <VersionIdMarker>string</VersionIdMarker>
    <NextKeyMarker>string</NextKeyMarker>
    <NextVersionIdMarker>string</NextVersionIdMarker>
    <Version>
        <Key>string</Key>
        <VersionId>string</VersionId>
        <IsLatest>boolean</IsLatest>
        <LastModified>timestamp</LastModified>
        <ETag>string</ETag>
        <Size>long</Size>
        <StorageClass>string</StorageClass>
        <Owner><ID>string</ID><DisplayName>string</DisplayName></Owner>
        <ChecksumAlgorithm>string</ChecksumAlgorithm>
        <ChecksumType>string</ChecksumType>
    </Version>
    <DeleteMarker>
        <Key>string</Key>
        <VersionId>string</VersionId>
        <IsLatest>boolean</IsLatest>
        <LastModified>timestamp</LastModified>
        <Owner><ID>string</ID><DisplayName>string</DisplayName></Owner>
    </DeleteMarker>
    <CommonPrefixes><Prefix>string</Prefix></CommonPrefixes>
</ListVersionsResult>
```

---

## 2.9 GetBucketLocation (Deprecated - use HeadBucket)

**Request**: `GET /?location`

**Response Body (XML)**
```xml
<LocationConstraint>{region}</LocationConstraint>
```

Returns `null` for us-east-1, `EU` for eu-west-1.

---

# 3. Multipart Upload Operations

## 3.1 CreateMultipartUpload

Initiates a multipart upload.

**Request**
```
POST /{Key+}?uploads HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Request Headers**: All standard object metadata headers (Cache-Control, Content-Disposition, Content-Encoding, Content-Language, Content-Type, Expires), ACL headers, SSE headers (SSE-KMS, SSE-C), Object Lock headers, checksum headers, x-amz-storage-class, x-amz-tagging, x-amz-website-redirect-location, x-amz-request-payer, x-amz-expected-bucket-owner.

**Request Body**: None

**Response Body (XML)**
```xml
<InitiateMultipartUploadResult>
   <Bucket>string</Bucket>
   <Key>string</Key>
   <UploadId>string</UploadId>
</InitiateMultipartUploadResult>
```

**Response Headers**: x-amz-server-side-encryption, x-amz-server-side-encryption-aws-kms-key-id, x-amz-server-side-encryption-context, x-amz-server-side-encryption-bucket-key-enabled, x-amz-server-side-encryption-customer-algorithm, x-amz-server-side-encryption-customer-key-MD5, x-amz-abort-date, x-amz-abort-rule-id, x-amz-checksum-algorithm, x-amz-checksum-type, x-amz-request-charged

---

## 3.2 UploadPart

Uploads a part of a multipart upload.

**Request**
```
PUT /{Key+}?partNumber={N}&uploadId={UploadId} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**URI Parameters**: Bucket, Key, partNumber (1-10,000), uploadId

**Request Headers**: Content-Length, Content-MD5, x-amz-checksum-* (crc32, crc32c, crc64nvme, sha1, sha256), x-amz-sdk-checksum-algorithm, SSE-C headers, x-amz-request-payer, x-amz-expected-bucket-owner

**Request Body**: Binary part data

**Response Headers**: ETag (required for CompleteMultipartUpload), x-amz-checksum-* headers, x-amz-server-side-encryption, x-amz-server-side-encryption-aws-kms-key-id, x-amz-server-side-encryption-bucket-key-enabled, x-amz-server-side-encryption-customer-algorithm, x-amz-server-side-encryption-customer-key-MD5, x-amz-request-charged

**Error Codes**: NoSuchUpload (404)

---

## 3.3 UploadPartCopy

Copies data from an existing object as part of a multipart upload.

**Request**
```
PUT /{Key+}?partNumber={N}&uploadId={UploadId} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
x-amz-copy-source: /{SourceBucket}/{SourceKey}
```

**Request Headers**: x-amz-copy-source (required), x-amz-copy-source-if-match, x-amz-copy-source-if-modified-since, x-amz-copy-source-if-none-match, x-amz-copy-source-if-unmodified-since, x-amz-copy-source-range (`bytes=first-last`), SSE-C source/destination headers, x-amz-expected-bucket-owner, x-amz-source-expected-bucket-owner, x-amz-request-payer

**Response Body (XML)**
```xml
<CopyPartResult>
    <ETag>string</ETag>
    <LastModified>timestamp</LastModified>
    <ChecksumCRC32>string</ChecksumCRC32>
    <ChecksumCRC32C>string</ChecksumCRC32C>
    <ChecksumCRC64NVME>string</ChecksumCRC64NVME>
    <ChecksumSHA1>string</ChecksumSHA1>
    <ChecksumSHA256>string</ChecksumSHA256>
</CopyPartResult>
```

---

## 3.4 CompleteMultipartUpload

**Request**
```
POST /{Key}?uploadId={UploadId} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Request Headers**: x-amz-checksum-* headers, x-amz-checksum-type (`COMPOSITE`|`FULL_OBJECT`), x-amz-mp-object-size, x-amz-request-payer, x-amz-expected-bucket-owner, SSE-C headers, If-Match, If-None-Match

**Request Body (XML)**
```xml
<CompleteMultipartUpload>
    <Part>
        <PartNumber>integer</PartNumber>
        <ETag>string</ETag>
        <ChecksumCRC32>string</ChecksumCRC32>
        <!-- other checksums -->
    </Part>
</CompleteMultipartUpload>
```

**Response Body (XML)**
```xml
<CompleteMultipartUploadResult>
    <Location>string</Location>
    <Bucket>string</Bucket>
    <Key>string</Key>
    <ETag>string</ETag>
    <ChecksumCRC32>string</ChecksumCRC32>
    <!-- other checksums -->
    <ChecksumType>string</ChecksumType>
</CompleteMultipartUploadResult>
```

**Error Codes**: EntityTooSmall (400), InvalidPart (400), InvalidPartOrder (400), NoSuchUpload (404), ConditionalRequestConflict (409)

---

## 3.5 AbortMultipartUpload

**Request**
```
DELETE /{Key+}?uploadId={UploadId} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Headers**: x-amz-request-payer, x-amz-expected-bucket-owner, x-amz-if-match-initiated-time (directory buckets)

**Response**: 204 No Content

**Error Codes**: NoSuchUpload (404)

---

## 3.6 ListMultipartUploads

**Request**
```
GET /?uploads&delimiter={D}&encoding-type={E}&key-marker={K}&max-uploads={N}&prefix={P}&upload-id-marker={U} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Response Body (XML)**
```xml
<ListMultipartUploadsResult>
    <Bucket>string</Bucket>
    <KeyMarker>string</KeyMarker>
    <UploadIdMarker>string</UploadIdMarker>
    <NextKeyMarker>string</NextKeyMarker>
    <NextUploadIdMarker>string</NextUploadIdMarker>
    <MaxUploads>integer</MaxUploads>
    <IsTruncated>boolean</IsTruncated>
    <Upload>
        <Key>string</Key>
        <UploadId>string</UploadId>
        <Initiated>timestamp</Initiated>
        <StorageClass>string</StorageClass>
        <ChecksumAlgorithm>string</ChecksumAlgorithm>
        <Initiator><ID>string</ID><DisplayName>string</DisplayName></Initiator>
        <Owner><ID>string</ID><DisplayName>string</DisplayName></Owner>
    </Upload>
    <CommonPrefixes><Prefix>string</Prefix></CommonPrefixes>
</ListMultipartUploadsResult>
```

---

## 3.7 ListParts

**Request**
```
GET /{Key}?max-parts={N}&part-number-marker={M}&uploadId={UploadId} HTTP/1.1
Host: {Bucket}.s3.amazonaws.com
```

**Response Headers**: x-amz-abort-date, x-amz-abort-rule-id, x-amz-request-charged

**Response Body (XML)**
```xml
<ListPartsResult>
   <Bucket>string</Bucket>
   <Key>string</Key>
   <UploadId>string</UploadId>
   <PartNumberMarker>integer</PartNumberMarker>
   <NextPartNumberMarker>integer</NextPartNumberMarker>
   <MaxParts>integer</MaxParts>
   <IsTruncated>boolean</IsTruncated>
   <Part>
      <PartNumber>integer</PartNumber>
      <LastModified>timestamp</LastModified>
      <ETag>string</ETag>
      <Size>long</Size>
      <ChecksumCRC32>string</ChecksumCRC32>
      <!-- other checksums -->
   </Part>
   <Initiator><DisplayName>string</DisplayName><ID>string</ID></Initiator>
   <Owner><DisplayName>string</DisplayName><ID>string</ID></Owner>
   <StorageClass>string</StorageClass>
   <ChecksumAlgorithm>string</ChecksumAlgorithm>
   <ChecksumType>string</ChecksumType>
</ListPartsResult>
```

---

# 4. Access Control Operations

## 4.1 GetBucketAcl

Not supported for directory buckets.

**Request**: `GET /?acl` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<AccessControlPolicy>
  <Owner><ID>string</ID><DisplayName>string</DisplayName></Owner>
  <AccessControlList>
    <Grant>
      <Grantee xsi:type="CanonicalUser|Group|AmazonCustomerByEmail">
        <ID>string</ID><DisplayName>string</DisplayName><EmailAddress>string</EmailAddress><URI>string</URI>
      </Grantee>
      <Permission>FULL_CONTROL|READ|WRITE|READ_ACP|WRITE_ACP</Permission>
    </Grant>
  </AccessControlList>
</AccessControlPolicy>
```

---

## 4.2 PutBucketAcl

Not supported for directory buckets.

**Request**: `PUT /?acl`

**Headers**: x-amz-acl (`private`|`public-read`|`public-read-write`|`authenticated-read`), x-amz-grant-full-control, x-amz-grant-read, x-amz-grant-read-acp, x-amz-grant-write, x-amz-grant-write-acp, Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: AccessControlPolicy XML (same schema as response above). Cannot use both headers and body.

**Response**: 200 OK (empty body)

**Error Codes**: AccessControlListNotSupported (BucketOwnerEnforced), InvalidRequest

---

## 4.3 GetObjectAcl

Not supported for directory buckets.

**Request**: `GET /{Key+}?acl&versionId={VersionId}`

**Headers**: x-amz-expected-bucket-owner, x-amz-request-payer

**Response Body**: Same AccessControlPolicy XML as GetBucketAcl

**Response Headers**: x-amz-request-charged

**Error Codes**: NoSuchKey (404)

---

## 4.4 PutObjectAcl

Not supported for directory buckets.

**Request**: `PUT /{Key+}?acl&versionId={VersionId}`

**Headers**: x-amz-acl, x-amz-grant-* headers, Content-MD5, x-amz-request-payer, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: AccessControlPolicy XML (optional - can use headers instead)

**Response**: 200 OK (empty body)

**Error Codes**: NoSuchKey (404), AccessControlListNotSupported

---

## 4.5 GetBucketPolicy

**Request**: `GET /?policy` | **Headers**: x-amz-expected-bucket-owner (not for directory buckets)

**Response Body**: JSON policy document

**Response Status Codes**: 200, 403, 405, 501

---

## 4.6 PutBucketPolicy

**Request**: `PUT /?policy`

**Headers**: Content-MD5, x-amz-confirm-remove-self-bucket-access, x-amz-expected-bucket-owner (not for directory buckets), x-amz-sdk-checksum-algorithm

**Request Body**: JSON policy document

**Response**: 200/204 (empty body)

---

## 4.7 DeleteBucketPolicy

**Request**: `DELETE /?policy` | **Headers**: x-amz-expected-bucket-owner (not for directory buckets)

**Response**: 204 No Content

---

## 4.8 GetBucketPolicyStatus

Not supported for directory buckets.

**Request**: `GET /?policyStatus` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<PolicyStatus>
    <IsPublic>boolean</IsPublic>
</PolicyStatus>
```

---

## 4.9 GetPublicAccessBlock

Not supported for directory buckets.

**Request**: `GET /?publicAccessBlock` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<PublicAccessBlockConfiguration>
    <BlockPublicAcls>boolean</BlockPublicAcls>
    <IgnorePublicAcls>boolean</IgnorePublicAcls>
    <BlockPublicPolicy>boolean</BlockPublicPolicy>
    <RestrictPublicBuckets>boolean</RestrictPublicBuckets>
</PublicAccessBlockConfiguration>
```

---

## 4.10 PutPublicAccessBlock

Not supported for directory buckets.

**Request**: `PUT /?publicAccessBlock`

**Headers**: Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: PublicAccessBlockConfiguration XML (same schema as above)

**Response**: 200 OK (empty body)

---

## 4.11 DeletePublicAccessBlock

Not supported for directory buckets.

**Request**: `DELETE /?publicAccessBlock` | **Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content

---

## 4.12 GetBucketOwnershipControls

Not supported for directory buckets.

**Request**: `GET /?ownershipControls` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<OwnershipControls>
  <Rule>
    <ObjectOwnership>BucketOwnerEnforced|BucketOwnerPreferred|ObjectWriter</ObjectOwnership>
  </Rule>
</OwnershipControls>
```

---

## 4.13 PutBucketOwnershipControls

Not supported for directory buckets.

**Request**: `PUT /?ownershipControls`

**Headers**: Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: OwnershipControls XML (same schema)

**Response**: 200 OK (empty body)

---

## 4.14 DeleteBucketOwnershipControls

Not supported for directory buckets.

**Request**: `DELETE /?ownershipControls` | **Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content

---

## 4.15 GetBucketAbac

Gets ABAC (attribute-based access control) status. General purpose buckets only.

**Request**: `GET /?abac` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<AbacStatus>
   <Status>Enabled|Disabled</Status>
</AbacStatus>
```

---

## 4.16 PutBucketAbac

Sets ABAC status. General purpose buckets only.

**Request**: `PUT /?abac`

**Headers**: Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body (XML)**
```xml
<AbacStatus>
    <Status>Enabled|Disabled</Status>
</AbacStatus>
```

**Response**: 200 OK (empty body)

---

# 5. Bucket Configuration Operations

## 5.1 GetBucketVersioning

Not supported for directory buckets.

**Request**: `GET /?versioning` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<VersioningConfiguration>
    <Status>Enabled|Suspended</Status>
    <MfaDelete>Enabled|Disabled</MfaDelete>
</VersioningConfiguration>
```

---

## 5.2 PutBucketVersioning

Not supported for directory buckets.

**Request**: `PUT /?versioning`

**Headers**: Content-MD5, x-amz-mfa (serial + code for MFA Delete), x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body (XML)**
```xml
<VersioningConfiguration>
    <Status>Enabled|Suspended</Status>
    <MfaDelete>Enabled|Disabled</MfaDelete>
</VersioningConfiguration>
```

**Response**: 200 OK (empty body). Note: 15-minute propagation delay.

---

## 5.3 GetBucketLifecycleConfiguration

**Request**: `GET /?lifecycle` | **Headers**: x-amz-expected-bucket-owner (GP only)

**Response Headers**: x-amz-transition-default-minimum-object-size (`all_storage_classes_128K`|`varies_by_storage_class`)

**Response Body (XML)**
```xml
<LifecycleConfiguration>
    <Rule>
        <ID>string</ID>
        <Status>Enabled|Disabled</Status>
        <Filter>
            <Prefix>string</Prefix>
            <Tag><Key>string</Key><Value>string</Value></Tag>
            <ObjectSizeGreaterThan>long</ObjectSizeGreaterThan>
            <ObjectSizeLessThan>long</ObjectSizeLessThan>
            <And>...</And>
        </Filter>
        <Expiration><Date>timestamp</Date><Days>integer</Days><ExpiredObjectDeleteMarker>boolean</ExpiredObjectDeleteMarker></Expiration>
        <Transition><Date>timestamp</Date><Days>integer</Days><StorageClass>string</StorageClass></Transition>
        <NoncurrentVersionExpiration><NoncurrentDays>integer</NoncurrentDays><NewerNoncurrentVersions>integer</NewerNoncurrentVersions></NoncurrentVersionExpiration>
        <NoncurrentVersionTransition><NoncurrentDays>integer</NoncurrentDays><NewerNoncurrentVersions>integer</NewerNoncurrentVersions><StorageClass>string</StorageClass></NoncurrentVersionTransition>
        <AbortIncompleteMultipartUpload><DaysAfterInitiation>integer</DaysAfterInitiation></AbortIncompleteMultipartUpload>
    </Rule>
</LifecycleConfiguration>
```

**Error Codes**: NoSuchLifecycleConfiguration (404)

---

## 5.4 PutBucketLifecycleConfiguration

**Request**: `PUT /?lifecycle`

**Headers**: x-amz-sdk-checksum-algorithm, x-amz-expected-bucket-owner (GP only), x-amz-transition-default-minimum-object-size

**Request Body**: LifecycleConfiguration XML (same schema as above, max 1,000 rules)

**Response**: 200 OK (empty body)

---

## 5.5 DeleteBucketLifecycle

**Request**: `DELETE /?lifecycle` | **Headers**: x-amz-expected-bucket-owner (GP only)

**Response**: 204 No Content

---

## 5.6 GetBucketEncryption

**Request**: `GET /?encryption` | **Headers**: x-amz-expected-bucket-owner (not for directory buckets)

**Response Body (XML)**
```xml
<ServerSideEncryptionConfiguration>
  <Rule>
    <ApplyServerSideEncryptionByDefault>
      <SSEAlgorithm>AES256|aws:kms|aws:kms:dsse</SSEAlgorithm>
      <KMSMasterKeyID>string</KMSMasterKeyID>
    </ApplyServerSideEncryptionByDefault>
    <BlockedEncryptionTypes><EncryptionType>SSE-C</EncryptionType></BlockedEncryptionTypes>
    <BucketKeyEnabled>boolean</BucketKeyEnabled>
  </Rule>
</ServerSideEncryptionConfiguration>
```

---

## 5.7 PutBucketEncryption

**Request**: `PUT /?encryption`

**Headers**: Content-MD5, x-amz-expected-bucket-owner (not for directory buckets), x-amz-sdk-checksum-algorithm

**Request Body**: ServerSideEncryptionConfiguration XML (same schema)

**Response**: 200 OK (empty body)

---

## 5.8 DeleteBucketEncryption

**Request**: `DELETE /?encryption` | **Headers**: x-amz-expected-bucket-owner (not for directory buckets)

**Response**: 204 No Content. Resets to SSE-S3 default.

---

## 5.9 GetBucketCors

Not supported for directory buckets.

**Request**: `GET /?cors` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<CORSConfiguration>
    <CORSRule>
        <AllowedOrigin>string</AllowedOrigin>
        <AllowedMethod>string</AllowedMethod>
        <AllowedHeader>string</AllowedHeader>
        <ExposeHeader>string</ExposeHeader>
        <MaxAgeSeconds>integer</MaxAgeSeconds>
        <ID>string</ID>
    </CORSRule>
</CORSConfiguration>
```

---

## 5.10 PutBucketCors

Not supported for directory buckets.

**Request**: `PUT /?cors`

**Headers**: Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: CORSConfiguration XML (same schema, max 100 rules, max 64 KB)

**Response**: 200 OK (empty body)

---

## 5.11 DeleteBucketCors

Not supported for directory buckets.

**Request**: `DELETE /?cors` | **Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content

---

## 5.12 GetBucketLogging

Not supported for directory buckets.

**Request**: `GET /?logging` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<BucketLoggingStatus>
    <LoggingEnabled>
        <TargetBucket>string</TargetBucket>
        <TargetPrefix>string</TargetPrefix>
        <TargetGrants>
            <Grant><Grantee>...</Grantee><Permission>string</Permission></Grant>
        </TargetGrants>
        <TargetObjectKeyFormat>
            <PartitionedPrefix><PartitionDateSource>string</PartitionDateSource></PartitionedPrefix>
            <SimplePrefix/>
        </TargetObjectKeyFormat>
    </LoggingEnabled>
</BucketLoggingStatus>
```

---

## 5.13 PutBucketLogging

Not supported for directory buckets.

**Request**: `PUT /?logging`

**Headers**: Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: BucketLoggingStatus XML (same schema). Empty element to disable: `<BucketLoggingStatus />`

**Response**: 200 OK (empty body)

---

## 5.14 GetBucketNotificationConfiguration

Not supported for directory buckets.

**Request**: `GET /?notification` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<NotificationConfiguration>
  <TopicConfiguration>
    <Id>string</Id>
    <Topic>string (ARN)</Topic>
    <Event>string</Event>
    <Filter><S3Key><FilterRule><Name>prefix|suffix</Name><Value>string</Value></FilterRule></S3Key></Filter>
  </TopicConfiguration>
  <QueueConfiguration>
    <Id>string</Id><Queue>string (ARN)</Queue><Event>string</Event><Filter>...</Filter>
  </QueueConfiguration>
  <CloudFunctionConfiguration>
    <Id>string</Id><CloudFunction>string (ARN)</CloudFunction><Event>string</Event><Filter>...</Filter>
  </CloudFunctionConfiguration>
  <EventBridgeConfiguration/>
</NotificationConfiguration>
```

---

## 5.15 PutBucketNotificationConfiguration

Not supported for directory buckets.

**Request**: `PUT /?notification`

**Headers**: x-amz-expected-bucket-owner, x-amz-skip-destination-validation (boolean)

**Request Body**: NotificationConfiguration XML (same schema). Empty element to disable.

**Response**: 200 OK (empty body). Header: x-amz-sns-test-message-id (if applicable)

---

## 5.16 GetBucketReplication

Not supported for directory buckets.

**Request**: `GET /?replication` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<ReplicationConfiguration>
    <Role>string (IAM ARN)</Role>
    <Rule>
        <ID>string</ID>
        <Status>Enabled|Disabled</Status>
        <Priority>integer</Priority>
        <DeleteMarkerReplication><Status>Enabled|Disabled</Status></DeleteMarkerReplication>
        <Filter><Prefix>string</Prefix><Tag><Key>string</Key><Value>string</Value></Tag><And>...</And></Filter>
        <Destination>
            <Bucket>string (ARN)</Bucket>
            <Account>string</Account>
            <StorageClass>string</StorageClass>
            <AccessControlTranslation><Owner>Destination</Owner></AccessControlTranslation>
            <EncryptionConfiguration><ReplicaKmsKeyID>string</ReplicaKmsKeyID></EncryptionConfiguration>
            <ReplicationTime><Status>Enabled|Disabled</Status><Time><Minutes>integer</Minutes></Time></ReplicationTime>
            <Metrics><Status>Enabled|Disabled</Status><EventThreshold><Minutes>integer</Minutes></EventThreshold></Metrics>
        </Destination>
        <ExistingObjectReplication><Status>Enabled|Disabled</Status></ExistingObjectReplication>
        <SourceSelectionCriteria>
            <SseKmsEncryptedObjects><Status>Enabled|Disabled</Status></SseKmsEncryptedObjects>
            <ReplicaModifications><Status>Enabled|Disabled</Status></ReplicaModifications>
        </SourceSelectionCriteria>
    </Rule>
</ReplicationConfiguration>
```

---

## 5.17 PutBucketReplication

Not supported for directory buckets.

**Request**: `PUT /?replication`

**Headers**: Content-MD5 (required), x-amz-bucket-object-lock-token, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: ReplicationConfiguration XML (same schema, max 1,000 rules)

**Response**: 200 OK (empty body)

---

## 5.18 DeleteBucketReplication

Not supported for directory buckets.

**Request**: `DELETE /?replication` | **Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content

---

## 5.19 GetBucketAccelerateConfiguration

Not supported for directory buckets.

**Request**: `GET /?accelerate` | **Headers**: x-amz-expected-bucket-owner, x-amz-request-payer

**Response Body (XML)**
```xml
<AccelerateConfiguration>
   <Status>Enabled|Suspended</Status>
</AccelerateConfiguration>
```

---

## 5.20 PutBucketAccelerateConfiguration

Not supported for directory buckets.

**Request**: `PUT /?accelerate`

**Headers**: x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: AccelerateConfiguration XML (same schema)

**Response**: 200 OK (empty body). Propagation: up to 30 minutes.

---

## 5.21 GetBucketRequestPayment

Not supported for directory buckets.

**Request**: `GET /?requestPayment` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<RequestPaymentConfiguration>
  <Payer>Requester|BucketOwner</Payer>
</RequestPaymentConfiguration>
```

---

## 5.22 PutBucketRequestPayment

Not supported for directory buckets.

**Request**: `PUT /?requestPayment`

**Headers**: Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: RequestPaymentConfiguration XML (same schema)

**Response**: 200 OK (empty body)

---

## 5.23 GetBucketWebsite

Not supported for directory buckets.

**Request**: `GET /?website` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<WebsiteConfiguration>
   <IndexDocument><Suffix>string</Suffix></IndexDocument>
   <ErrorDocument><Key>string</Key></ErrorDocument>
   <RedirectAllRequestsTo><HostName>string</HostName><Protocol>http|https</Protocol></RedirectAllRequestsTo>
   <RoutingRules>
      <RoutingRule>
         <Condition><HttpErrorCodeReturnedEquals>string</HttpErrorCodeReturnedEquals><KeyPrefixEquals>string</KeyPrefixEquals></Condition>
         <Redirect><HostName>string</HostName><HttpRedirectCode>string</HttpRedirectCode><Protocol>http|https</Protocol><ReplaceKeyPrefixWith>string</ReplaceKeyPrefixWith><ReplaceKeyWith>string</ReplaceKeyWith></Redirect>
      </RoutingRule>
   </RoutingRules>
</WebsiteConfiguration>
```

---

## 5.24 PutBucketWebsite

Not supported for directory buckets.

**Request**: `PUT /?website`

**Headers**: Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: WebsiteConfiguration XML (same schema, max 50 routing rules, max 128 KB)

**Response**: 200 OK (empty body)

---

## 5.25 DeleteBucketWebsite

Not supported for directory buckets.

**Request**: `DELETE /?website` | **Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content. Idempotent.

---

## 5.26 CreateSession

Creates temporary session credentials for S3 Express One Zone directory buckets.

**Request**
```
GET /?session HTTP/1.1
Host: {Bucket}.s3express-{zone-id}.{region}.amazonaws.com
```

**Request Headers**

| Header | Required | Description |
|--------|----------|-------------|
| x-amz-create-session-mode | No | `ReadWrite` (default) or `ReadOnly` |
| x-amz-server-side-encryption | No | `AES256`, `aws:kms` |
| x-amz-server-side-encryption-aws-kms-key-id | Conditional | KMS key ID (required with `aws:kms`) |
| x-amz-server-side-encryption-context | No | Base64-encoded KMS context |
| x-amz-server-side-encryption-bucket-key-enabled | No | Always enabled for directory buckets |

**Response Body (XML)**
```xml
<CreateSessionResult>
    <Credentials>
        <AccessKeyId>string</AccessKeyId>
        <SecretAccessKey>string</SecretAccessKey>
        <SessionToken>string</SessionToken>
        <Expiration>timestamp</Expiration>
    </Credentials>
</CreateSessionResult>
```

Session credentials expire after 5 minutes.

---

# 6. Tagging Operations

## 6.1 GetBucketTagging

Not supported for directory buckets.

**Request**: `GET /?tagging` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<Tagging>
   <TagSet>
      <Tag><Key>string</Key><Value>string</Value></Tag>
   </TagSet>
</Tagging>
```

**Error Codes**: NoSuchTagSet

---

## 6.2 PutBucketTagging

Not supported for directory buckets.

**Request**: `PUT /?tagging`

**Headers**: Content-MD5 (required), x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: Tagging XML (same schema)

**Response**: 200/204 (empty body)

**Error Codes**: InvalidTag, MalformedXML, OperationAborted, InternalError

---

## 6.3 DeleteBucketTagging

Not supported for directory buckets.

**Request**: `DELETE /?tagging` | **Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content

---

## 6.4 GetObjectTagging

Not supported for directory buckets.

**Request**: `GET /{Key+}?tagging&versionId={VersionId}`

**Headers**: x-amz-expected-bucket-owner, x-amz-request-payer

**Response Headers**: x-amz-version-id

**Response Body**: Tagging XML (same schema)

---

## 6.5 PutObjectTagging

Not supported for directory buckets. Max 10 tags per object.

**Request**: `PUT /{Key+}?tagging&versionId={VersionId}`

**Headers**: Content-MD5 (required), x-amz-expected-bucket-owner, x-amz-request-payer, x-amz-sdk-checksum-algorithm

**Request Body**: Tagging XML (same schema)

**Response**: 200 OK. Header: x-amz-version-id

**Error Codes**: InvalidTag, MalformedXML, OperationAborted, InternalError

---

## 6.6 DeleteObjectTagging

Not supported for directory buckets.

**Request**: `DELETE /{Key+}?tagging&versionId={VersionId}`

**Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content. Header: x-amz-version-id

---

# 7. Legal Hold / Object Lock / Retention

All operations in this section are not supported for directory buckets.

## 7.1 GetObjectLegalHold

**Request**: `GET /{Key+}?legal-hold&versionId={VersionId}`

**Headers**: x-amz-expected-bucket-owner, x-amz-request-payer

**Response Body (XML)**
```xml
<LegalHold>
    <Status>ON|OFF</Status>
</LegalHold>
```

---

## 7.2 PutObjectLegalHold

**Request**: `PUT /{Key+}?legal-hold&versionId={VersionId}`

**Headers**: Content-MD5, x-amz-request-payer, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body (XML)**
```xml
<LegalHold>
    <Status>ON|OFF</Status>
</LegalHold>
```

**Response**: 200 OK. Header: x-amz-request-charged

---

## 7.3 GetObjectLockConfiguration

**Request**: `GET /?object-lock` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<ObjectLockConfiguration>
    <ObjectLockEnabled>Enabled</ObjectLockEnabled>
    <Rule>
        <DefaultRetention>
            <Mode>GOVERNANCE|COMPLIANCE</Mode>
            <Days>integer</Days>
            <Years>integer</Years>
        </DefaultRetention>
    </Rule>
</ObjectLockConfiguration>
```

---

## 7.4 PutObjectLockConfiguration

**Request**: `PUT /?object-lock`

**Headers**: Content-MD5, x-amz-bucket-object-lock-token, x-amz-expected-bucket-owner, x-amz-request-payer, x-amz-sdk-checksum-algorithm

**Request Body**: ObjectLockConfiguration XML (same schema). Days OR Years, not both.

**Response**: 200 OK. Header: x-amz-request-charged

---

## 7.5 GetObjectRetention

**Request**: `GET /{Key+}?retention&versionId={VersionId}`

**Headers**: x-amz-expected-bucket-owner, x-amz-request-payer

**Response Body (XML)**
```xml
<Retention>
   <Mode>GOVERNANCE|COMPLIANCE</Mode>
   <RetainUntilDate>timestamp</RetainUntilDate>
</Retention>
```

---

## 7.6 PutObjectRetention

**Request**: `PUT /{Key+}?retention&versionId={VersionId}`

**Headers**: Content-MD5, x-amz-bypass-governance-retention, x-amz-request-payer, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body**: Retention XML (same schema)

**Response**: 200 OK. Header: x-amz-request-charged

---

# 8. Analytics / Metrics / Inventory / Intelligent Tiering

All operations in this section are not supported for directory buckets.

## 8.1 GetBucketAnalyticsConfiguration

**Request**: `GET /?analytics&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<AnalyticsConfiguration>
   <Id>string</Id>
   <Filter>
      <Prefix>string</Prefix>
      <Tag><Key>string</Key><Value>string</Value></Tag>
      <And><Prefix>string</Prefix><Tag>...</Tag></And>
   </Filter>
   <StorageClassAnalysis>
      <DataExport>
         <OutputSchemaVersion>V_1</OutputSchemaVersion>
         <Destination>
            <S3BucketDestination>
               <Bucket>string</Bucket>
               <BucketAccountId>string</BucketAccountId>
               <Format>CSV</Format>
               <Prefix>string</Prefix>
            </S3BucketDestination>
         </Destination>
      </DataExport>
   </StorageClassAnalysis>
</AnalyticsConfiguration>
```

---

## 8.2 PutBucketAnalyticsConfiguration

**Request**: `PUT /?analytics&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Request Body**: AnalyticsConfiguration XML (same schema). Max 1,000 per bucket.

**Response**: 200 OK (empty body)

**Error Codes**: TooManyConfigurations (400)

---

## 8.3 DeleteBucketAnalyticsConfiguration

**Request**: `DELETE /?analytics&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content

---

## 8.4 ListBucketAnalyticsConfigurations

**Request**: `GET /?analytics&continuation-token={Token}` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<ListBucketAnalyticsConfigurationResult>
    <IsTruncated>boolean</IsTruncated>
    <ContinuationToken>string</ContinuationToken>
    <NextContinuationToken>string</NextContinuationToken>
    <AnalyticsConfiguration>...</AnalyticsConfiguration>
</ListBucketAnalyticsConfigurationResult>
```

Returns max 100 per request.

---

## 8.5 GetBucketMetricsConfiguration

**Request**: `GET /?metrics&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<MetricsConfiguration>
  <Id>string</Id>
  <Filter>
    <AccessPointArn>string</AccessPointArn>
    <Prefix>string</Prefix>
    <Tag><Key>string</Key><Value>string</Value></Tag>
    <And><AccessPointArn>string</AccessPointArn><Prefix>string</Prefix><Tag>...</Tag></And>
  </Filter>
</MetricsConfiguration>
```

---

## 8.6 PutBucketMetricsConfiguration

**Request**: `PUT /?metrics&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Request Body**: MetricsConfiguration XML (same schema). Max 1,000 per bucket.

**Response**: 200 OK (empty body)

**Error Codes**: TooManyConfigurations (400)

---

## 8.7 DeleteBucketMetricsConfiguration

**Request**: `DELETE /?metrics&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content

---

## 8.8 ListBucketMetricsConfigurations

**Request**: `GET /?metrics&continuation-token={Token}` | **Headers**: x-amz-expected-bucket-owner

**Response Body**: ListMetricsConfigurationsResult XML with IsTruncated, NextContinuationToken, MetricsConfiguration array. Returns max 100 per request.

---

## 8.9 GetBucketInventoryConfiguration

**Request**: `GET /?inventory&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<InventoryConfiguration>
   <Id>string</Id>
   <IsEnabled>boolean</IsEnabled>
   <Destination>
      <S3BucketDestination>
         <AccountId>string</AccountId>
         <Bucket>string</Bucket>
         <Prefix>string</Prefix>
         <Format>CSV|ORC|Parquet</Format>
         <Encryption><SSE-S3/><SSE-KMS><KeyId>string</KeyId></SSE-KMS></Encryption>
      </S3BucketDestination>
   </Destination>
   <Schedule><Frequency>Daily|Weekly</Frequency></Schedule>
   <Filter><Prefix>string</Prefix></Filter>
   <IncludedObjectVersions>All|Current</IncludedObjectVersions>
   <OptionalFields><Field>string</Field></OptionalFields>
</InventoryConfiguration>
```

---

## 8.10 PutBucketInventoryConfiguration

**Request**: `PUT /?inventory&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Request Body**: InventoryConfiguration XML (same schema). Max 1,000 per bucket.

**Response**: 200 OK (empty body)

---

## 8.11 DeleteBucketInventoryConfiguration

**Request**: `DELETE /?inventory&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content

---

## 8.12 ListBucketInventoryConfigurations

**Request**: `GET /?inventory&continuation-token={Token}` | **Headers**: x-amz-expected-bucket-owner

**Response Body**: ListInventoryConfigurationsResult XML with IsTruncated, NextContinuationToken, InventoryConfiguration array. Returns max 100 per request.

---

## 8.13 GetBucketIntelligentTieringConfiguration

**Request**: `GET /?intelligent-tiering&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<IntelligentTieringConfiguration>
    <Id>string</Id>
    <Filter>
        <Prefix>string</Prefix>
        <Tag><Key>string</Key><Value>string</Value></Tag>
        <And><Prefix>string</Prefix><Tag>...</Tag></And>
    </Filter>
    <Status>Enabled|Disabled</Status>
    <Tiering>
        <AccessTier>string</AccessTier>
        <Days>integer</Days>
    </Tiering>
</IntelligentTieringConfiguration>
```

---

## 8.14 PutBucketIntelligentTieringConfiguration

**Request**: `PUT /?intelligent-tiering&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Request Body**: IntelligentTieringConfiguration XML (same schema). Max 1,000 per bucket.

**Response**: 200 OK (empty body)

---

## 8.15 DeleteBucketIntelligentTieringConfiguration

**Request**: `DELETE /?intelligent-tiering&id={Id}` | **Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content

---

## 8.16 ListBucketIntelligentTieringConfigurations

**Request**: `GET /?intelligent-tiering&continuation-token={Token}` | **Headers**: x-amz-expected-bucket-owner

**Response Body**: ListBucketIntelligentTieringConfigurationsOutput XML with IsTruncated, NextContinuationToken, IntelligentTieringConfiguration array.

---

# 9. Other Operations

## 9.1 WriteGetObjectResponse

Used with Object Lambda access points to return transformed objects. Not for directory buckets.

**Request**
```
POST /WriteGetObjectResponse HTTP/1.1
Host: {RequestRoute}.s3-object-lambda.{Region}.amazonaws.com
```

**Required Headers**: x-amz-request-route, x-amz-request-token

**Forwarding Headers** (all prefixed with `x-amz-fwd-` or `x-amz-fwd-header-`):
- x-amz-fwd-status (HTTP status code)
- x-amz-fwd-error-code, x-amz-fwd-error-message
- x-amz-fwd-header-Content-Type, Content-Encoding, Content-Language, Content-Disposition, Content-Range, Cache-Control, accept-ranges, ETag, Last-Modified, Expires
- x-amz-fwd-header-x-amz-checksum-* (crc32, crc32c, crc64nvme, sha1, sha256)
- x-amz-fwd-header-x-amz-server-side-encryption, x-amz-server-side-encryption-aws-kms-key-id
- x-amz-fwd-header-x-amz-storage-class, x-amz-version-id, x-amz-delete-marker
- x-amz-fwd-header-x-amz-object-lock-mode, x-amz-object-lock-legal-hold, x-amz-object-lock-retain-until-date
- x-amz-fwd-header-x-amz-expiration, x-amz-restore, x-amz-replication-status
- x-amz-fwd-header-x-amz-mp-parts-count, x-amz-tagging-count, x-amz-missing-meta
- x-amz-fwd-header-x-amz-request-charged
- x-amz-meta-* (custom metadata)

**Request Body**: Binary transformed object data

**Response**: 200 OK (empty body)

---

## 9.2 Deprecated Operations

### GetBucketLifecycle (Deprecated)

Use GetBucketLifecycleConfiguration instead. Same endpoint `GET /?lifecycle` but uses older schema without Filter element.

### PutBucketLifecycle (Deprecated)

Use PutBucketLifecycleConfiguration instead. Same endpoint `PUT /?lifecycle` but uses older schema with Prefix at Rule level instead of Filter.

### GetBucketNotification (Deprecated)

Use GetBucketNotificationConfiguration instead. Same endpoint but returns older schema with InvocationRole in CloudFunctionConfiguration.

### PutBucketNotification (Deprecated)

Use PutBucketNotificationConfiguration instead. Same endpoint but uses older schema.

---

## 9.3 Metadata Table Configuration Operations

### CreateBucketMetadataTableConfiguration (Deprecated - use V2: CreateBucketMetadataConfiguration)

**Request**: `POST /?metadataTable`

**Headers**: Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

**Request Body (XML)**
```xml
<MetadataTableConfiguration>
    <S3TablesDestination>
        <TableBucketArn>string</TableBucketArn>
        <TableName>string</TableName>
    </S3TablesDestination>
</MetadataTableConfiguration>
```

**Response**: 200 OK (empty body)

### GetBucketMetadataTableConfiguration (Deprecated - use V2: GetBucketMetadataConfiguration)

**Request**: `GET /?metadataTable` | **Headers**: x-amz-expected-bucket-owner

**Response Body (XML)**
```xml
<GetBucketMetadataTableConfigurationResult>
   <MetadataTableConfigurationResult>
      <S3TablesDestinationResult>
         <TableArn>string</TableArn>
         <TableBucketArn>string</TableBucketArn>
         <TableName>string</TableName>
         <TableNamespace>string</TableNamespace>
      </S3TablesDestinationResult>
   </MetadataTableConfigurationResult>
   <Status>CREATING|ACTIVE|FAILED</Status>
   <Error><ErrorCode>string</ErrorCode><ErrorMessage>string</ErrorMessage></Error>
</GetBucketMetadataTableConfigurationResult>
```

### DeleteBucketMetadataTableConfiguration (Deprecated - use V2: DeleteBucketMetadataConfiguration)

**Request**: `DELETE /?metadataTable` | **Headers**: x-amz-expected-bucket-owner

**Response**: 204 No Content

### CreateBucketMetadataConfiguration (V2)

**Request**: `POST /?metadataConfiguration` | **Headers**: Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

### GetBucketMetadataConfiguration (V2)

**Request**: `GET /?metadataConfiguration` | **Headers**: x-amz-expected-bucket-owner

### DeleteBucketMetadataConfiguration (V2)

**Request**: `DELETE /?metadataConfiguration` | **Headers**: x-amz-expected-bucket-owner

### UpdateBucketMetadataInventoryTableConfiguration

**Request**: `PUT /?metadataTable&inventory` | **Headers**: Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

### UpdateBucketMetadataJournalTableConfiguration

**Request**: `PUT /?metadataTable&journal` | **Headers**: Content-MD5, x-amz-expected-bucket-owner, x-amz-sdk-checksum-algorithm

---

# Appendix A: Common Headers

## Common Request Headers

| Header | Description |
|--------|-------------|
| Authorization | AWS Signature Version 4 |
| Content-Length | Length of request body |
| Content-Type | MIME type of request body |
| Date | Request date |
| Host | Bucket endpoint |
| x-amz-content-sha256 | SHA256 hash of request payload |
| x-amz-date | Alternative to Date header |
| x-amz-security-token | Temporary security credentials token |

## Common Response Headers

| Header | Description |
|--------|-------------|
| Content-Length | Response body length |
| Content-Type | Response MIME type |
| Connection | Connection status |
| Date | Response date |
| ETag | Entity tag |
| Server | `AmazonS3` |
| x-amz-delete-marker | Whether object is a delete marker |
| x-amz-id-2 | Extended request ID |
| x-amz-request-id | Request ID |
| x-amz-version-id | Object version ID |

---

# Appendix B: Storage Classes

| Storage Class | Description |
|---------------|-------------|
| STANDARD | Default, frequently accessed |
| REDUCED_REDUNDANCY | Non-critical, reproducible data |
| STANDARD_IA | Infrequent access, rapid retrieval |
| ONEZONE_IA | Infrequent access, single AZ |
| INTELLIGENT_TIERING | Automatic tiering |
| GLACIER | Archive, minutes-to-hours retrieval |
| DEEP_ARCHIVE | Long-term archive, 12-48 hour retrieval |
| GLACIER_IR | Archive with instant retrieval |
| OUTPOSTS | S3 on Outposts |
| SNOW | S3 on Snow devices |
| EXPRESS_ONEZONE | S3 Express One Zone (directory buckets) |
| FSX_OPENZFS | FSx for OpenZFS |
| FSX_ONTAP | FSx for NetApp ONTAP |

---

# Appendix C: Server-Side Encryption Options

| Algorithm | Header Value | Description |
|-----------|-------------|-------------|
| SSE-S3 | `AES256` | Amazon S3 managed keys |
| SSE-KMS | `aws:kms` | AWS KMS managed keys |
| DSSE-KMS | `aws:kms:dsse` | Dual-layer SSE with KMS |
| SSE-C | Customer headers | Customer-provided keys |
| FSx | `aws:fsx` | FSx integration |

---

# Appendix D: Complete Operation Index

## Object Operations
1. GetObject
2. PutObject
3. HeadObject
4. DeleteObject
5. DeleteObjects
6. CopyObject
7. GetObjectAttributes
8. RestoreObject
9. SelectObjectContent
10. GetObjectTorrent
11. RenameObject
12. UpdateObjectEncryption

## Bucket Operations
13. CreateBucket
14. DeleteBucket
15. HeadBucket
16. ListBuckets
17. ListDirectoryBuckets
18. ListObjects (deprecated)
19. ListObjectsV2
20. ListObjectVersions
21. GetBucketLocation (deprecated)

## Multipart Upload Operations
22. CreateMultipartUpload
23. UploadPart
24. UploadPartCopy
25. CompleteMultipartUpload
26. AbortMultipartUpload
27. ListMultipartUploads
28. ListParts

## Access Control Operations
29. GetBucketAcl
30. PutBucketAcl
31. GetObjectAcl
32. PutObjectAcl
33. GetBucketPolicy
34. PutBucketPolicy
35. DeleteBucketPolicy
36. GetBucketPolicyStatus
37. GetPublicAccessBlock
38. PutPublicAccessBlock
39. DeletePublicAccessBlock
40. GetBucketOwnershipControls
41. PutBucketOwnershipControls
42. DeleteBucketOwnershipControls
43. GetBucketAbac
44. PutBucketAbac

## Bucket Configuration Operations
45. GetBucketVersioning
46. PutBucketVersioning
47. GetBucketLifecycleConfiguration
48. PutBucketLifecycleConfiguration
49. DeleteBucketLifecycle
50. GetBucketEncryption
51. PutBucketEncryption
52. DeleteBucketEncryption
53. GetBucketCors
54. PutBucketCors
55. DeleteBucketCors
56. GetBucketLogging
57. PutBucketLogging
58. GetBucketNotificationConfiguration
59. PutBucketNotificationConfiguration
60. GetBucketReplication
61. PutBucketReplication
62. DeleteBucketReplication
63. GetBucketAccelerateConfiguration
64. PutBucketAccelerateConfiguration
65. GetBucketRequestPayment
66. PutBucketRequestPayment
67. GetBucketWebsite
68. PutBucketWebsite
69. DeleteBucketWebsite
70. CreateSession

## Tagging Operations
71. GetBucketTagging
72. PutBucketTagging
73. DeleteBucketTagging
74. GetObjectTagging
75. PutObjectTagging
76. DeleteObjectTagging

## Legal Hold / Object Lock / Retention
77. GetObjectLegalHold
78. PutObjectLegalHold
79. GetObjectLockConfiguration
80. PutObjectLockConfiguration
81. GetObjectRetention
82. PutObjectRetention

## Analytics / Metrics / Inventory / Intelligent Tiering
83. GetBucketAnalyticsConfiguration
84. PutBucketAnalyticsConfiguration
85. DeleteBucketAnalyticsConfiguration
86. ListBucketAnalyticsConfigurations
87. GetBucketMetricsConfiguration
88. PutBucketMetricsConfiguration
89. DeleteBucketMetricsConfiguration
90. ListBucketMetricsConfigurations
91. GetBucketInventoryConfiguration
92. PutBucketInventoryConfiguration
93. DeleteBucketInventoryConfiguration
94. ListBucketInventoryConfigurations
95. GetBucketIntelligentTieringConfiguration
96. PutBucketIntelligentTieringConfiguration
97. DeleteBucketIntelligentTieringConfiguration
98. ListBucketIntelligentTieringConfigurations

## Other Operations
99. WriteGetObjectResponse
100. CreateBucketMetadataTableConfiguration (deprecated)
101. GetBucketMetadataTableConfiguration (deprecated)
102. DeleteBucketMetadataTableConfiguration (deprecated)
103. CreateBucketMetadataConfiguration (V2)
104. GetBucketMetadataConfiguration (V2)
105. DeleteBucketMetadataConfiguration (V2)
106. UpdateBucketMetadataInventoryTableConfiguration
107. UpdateBucketMetadataJournalTableConfiguration

## Deprecated Operations
108. GetBucketLifecycle
109. PutBucketLifecycle
110. GetBucketNotification
111. PutBucketNotification
