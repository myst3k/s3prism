// Prefix/delimiter indexes for ListObjects
//
// RocksDB's sorted key iteration handles prefix scanning natively.
// Keys are stored as "bucket\0object_key" which gives us natural
// lexicographic ordering for prefix + delimiter queries.
//
// Future optimization: maintain a separate CF with prefix counts
// for faster common-prefix aggregation on large buckets.
