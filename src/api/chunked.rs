use axum::body::Bytes;
use axum::http::HeaderMap;

/// Decode an AWS Signature V4 chunked body (`aws-chunked` content encoding).
///
/// The format is:
///   <hex-size>;chunk-signature=<sig>\r\n
///   <data>\r\n
///   ...
///   0;chunk-signature=<sig>\r\n
///   \r\n
///
/// Returns the raw decoded payload. If the body is not aws-chunked, returns it as-is.
pub fn decode_aws_chunked(headers: &HeaderMap, body: Bytes) -> Bytes {
    let is_chunked = headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("aws-chunked"))
        .unwrap_or(false)
        || headers
            .get("x-amz-content-sha256")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.starts_with("STREAMING-"))
            .unwrap_or(false);

    if !is_chunked {
        return body;
    }

    // Use x-amz-decoded-content-length as a capacity hint if available
    let capacity = headers
        .get("x-amz-decoded-content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(body.len());

    let mut output = Vec::with_capacity(capacity);
    let mut pos = 0;
    let raw = body.as_ref();

    while pos < raw.len() {
        // Find the end of the chunk header line (terminated by \r\n)
        let header_end = match find_crlf(raw, pos) {
            Some(end) => end,
            None => break,
        };

        // Parse hex chunk size (everything before the first ';')
        let header_line = &raw[pos..header_end];
        let size_end = header_line.iter().position(|&b| b == b';').unwrap_or(header_line.len());
        let hex_str = match std::str::from_utf8(&header_line[..size_end]) {
            Ok(s) => s.trim(),
            Err(_) => break,
        };
        let chunk_size = match usize::from_str_radix(hex_str, 16) {
            Ok(n) => n,
            Err(_) => break,
        };

        if chunk_size == 0 {
            break;
        }

        // Data starts after the \r\n
        let data_start = header_end + 2;
        let data_end = data_start + chunk_size;

        if data_end > raw.len() {
            break;
        }

        output.extend_from_slice(&raw[data_start..data_end]);

        // Skip past the data and the trailing \r\n
        pos = data_end + 2;
    }

    Bytes::from(output)
}

fn find_crlf(data: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < data.len() {
        if data[i] == b'\r' && data[i + 1] == b'\n' {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_non_chunked() {
        let headers = HeaderMap::new();
        let body = Bytes::from("hello world");
        let result = decode_aws_chunked(&headers, body.clone());
        assert_eq!(result, body);
    }

    #[test]
    fn decode_single_chunk() {
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", "aws-chunked".parse().unwrap());

        // 5 bytes of "hello"
        let raw = b"5;chunk-signature=abc123\r\nhello\r\n0;chunk-signature=def456\r\n\r\n";
        let body = Bytes::from(raw.to_vec());
        let result = decode_aws_chunked(&headers, body);
        assert_eq!(result.as_ref(), b"hello");
    }

    #[test]
    fn decode_multiple_chunks() {
        let mut headers = HeaderMap::new();
        headers.insert("x-amz-content-sha256", "STREAMING-AWS4-HMAC-SHA256-PAYLOAD".parse().unwrap());

        let raw = b"5;chunk-signature=aaa\r\nhello\r\n6;chunk-signature=bbb\r\n world\r\n0;chunk-signature=ccc\r\n\r\n";
        let body = Bytes::from(raw.to_vec());
        let result = decode_aws_chunked(&headers, body);
        assert_eq!(result.as_ref(), b"hello world");
    }
}
