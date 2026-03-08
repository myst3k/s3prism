use crate::metadata::models::BucketMeta;
use crate::metadata::store::ListObjectsResult;

pub fn list_objects_v2_response(
    bucket: &str,
    prefix: &str,
    delimiter: Option<&str>,
    max_keys: usize,
    result: &ListObjectsResult,
) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push_str(r#"<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#);
    xml.push_str(&format!("<Name>{}</Name>", escape_xml(bucket)));
    xml.push_str(&format!("<Prefix>{}</Prefix>", escape_xml(prefix)));
    xml.push_str(&format!("<MaxKeys>{max_keys}</MaxKeys>"));
    xml.push_str(&format!(
        "<IsTruncated>{}</IsTruncated>",
        result.truncated
    ));
    xml.push_str(&format!("<KeyCount>{}</KeyCount>", result.entries.len()));

    if result.truncated {
        if let Some(last) = result.entries.last() {
            xml.push_str(&format!(
                "<NextContinuationToken>{}</NextContinuationToken>",
                escape_xml(&last.key)
            ));
        }
    }

    if let Some(d) = delimiter {
        xml.push_str(&format!("<Delimiter>{}</Delimiter>", escape_xml(d)));
    }

    for entry in &result.entries {
        xml.push_str("<Contents>");
        xml.push_str(&format!("<Key>{}</Key>", escape_xml(&entry.key)));
        xml.push_str(&format!(
            "<LastModified>{}</LastModified>",
            entry.last_modified.to_rfc3339()
        ));
        xml.push_str(&format!("<ETag>\"{}\"</ETag>", escape_xml(&entry.etag)));
        xml.push_str(&format!("<Size>{}</Size>", entry.size));
        xml.push_str(&format!(
            "<StorageClass>{}</StorageClass>",
            entry.storage_class
        ));
        xml.push_str("</Contents>");
    }

    for prefix in &result.common_prefixes {
        xml.push_str("<CommonPrefixes>");
        xml.push_str(&format!("<Prefix>{}</Prefix>", escape_xml(prefix)));
        xml.push_str("</CommonPrefixes>");
    }

    xml.push_str("</ListBucketResult>");
    xml
}

pub fn list_buckets_response(buckets: &[BucketMeta]) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push_str(r#"<ListAllMyBucketsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#);
    xml.push_str("<Owner><ID>s3prism</ID><DisplayName>s3prism</DisplayName></Owner>");
    xml.push_str("<Buckets>");

    for bucket in buckets {
        xml.push_str("<Bucket>");
        xml.push_str(&format!("<Name>{}</Name>", escape_xml(&bucket.name)));
        xml.push_str(&format!(
            "<CreationDate>{}</CreationDate>",
            bucket.created.to_rfc3339()
        ));
        xml.push_str("</Bucket>");
    }

    xml.push_str("</Buckets>");
    xml.push_str("</ListAllMyBucketsResult>");
    xml
}

pub fn delete_result_response(deleted: &[(String, bool)]) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push_str(r#"<DeleteResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#);

    for (key, success) in deleted {
        if *success {
            xml.push_str("<Deleted>");
            xml.push_str(&format!("<Key>{}</Key>", escape_xml(key)));
            xml.push_str("</Deleted>");
        } else {
            xml.push_str("<Error>");
            xml.push_str(&format!("<Key>{}</Key>", escape_xml(key)));
            xml.push_str("<Code>InternalError</Code>");
            xml.push_str("<Message>Failed to delete</Message>");
            xml.push_str("</Error>");
        }
    }

    xml.push_str("</DeleteResult>");
    xml
}

pub fn parse_delete_objects_request(body: &str) -> Result<Vec<String>, super::error::S3Error> {
    let mut keys = Vec::new();
    for line in body.split("<Key>") {
        if let Some(end) = line.find("</Key>") {
            keys.push(line[..end].to_string());
        }
    }
    if keys.is_empty() {
        return Err(super::error::S3Error::MalformedXML);
    }
    Ok(keys)
}

pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
