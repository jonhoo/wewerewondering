locals {
  athena = "wewerewondering-athena"
  db     = "default"
  tbl    = "cloudfront_logs"
}

resource "aws_s3_bucket" "athena" {
  bucket = local.athena

  # TODO: lifecycle configuration
  # https://docs.aws.amazon.com/athena/latest/ug/querying.html#query-results-specify-location
}

resource "aws_s3_bucket_ownership_controls" "athena" {
  bucket = aws_s3_bucket.athena.id

  rule {
    object_ownership = "BucketOwnerEnforced"
  }
}

resource "aws_s3_bucket_acl" "athena" {
  depends_on = [aws_s3_bucket_ownership_controls.athena]

  bucket = aws_s3_bucket.athena.id
  acl    = "private"
}

# https://docs.aws.amazon.com/athena/latest/ug/cloudfront-logs.html
resource "aws_glue_catalog_table" "cf_logs" {
  name          = local.tbl
  database_name = local.db
  table_type    = "EXTERNAL_TABLE"
  owner         = "hadoop"
  parameters = {
    EXTERNAL                 = "TRUE"
    "skip.header.line.count" = 2
  }
  storage_descriptor {
    input_format  = "org.apache.hadoop.mapred.TextInputFormat"
    location      = "s3://${aws_s3_bucket.logs.id}/"
    output_format = "org.apache.hadoop.hive.ql.io.HiveIgnoreKeyTextOutputFormat"

    columns {
      name = "date"
      type = "date"
    }
    columns {
      name = "time"
      type = "string"
    }
    columns {
      name = "location"
      type = "string"
    }
    columns {
      name = "bytes"
      type = "bigint"
    }
    columns {
      name = "request_ip"
      type = "string"
    }
    columns {
      name = "method"
      type = "string"
    }
    columns {
      name = "host"
      type = "string"
    }
    columns {
      name = "uri"
      type = "string"
    }
    columns {
      name = "status"
      type = "int"
    }
    columns {
      name = "referrer"
      type = "string"
    }
    columns {
      name = "user_agent"
      type = "string"
    }
    columns {
      name = "query_string"
      type = "string"
    }
    columns {
      name = "cookie"
      type = "string"
    }
    columns {
      name = "result_type"
      type = "string"
    }
    columns {
      name = "request_id"
      type = "string"
    }
    columns {
      name = "host_header"
      type = "string"
    }
    columns {
      name = "request_protocol"
      type = "string"
    }
    columns {
      name = "request_bytes"
      type = "bigint"
    }
    columns {
      name = "time_taken"
      type = "float"
    }
    columns {
      name = "xforwarded_for"
      type = "string"
    }
    columns {
      name = "ssl_protocol"
      type = "string"
    }
    columns {
      name = "ssl_cipher"
      type = "string"
    }
    columns {
      name = "response_result_type"
      type = "string"
    }
    columns {
      name = "http_version"
      type = "string"
    }
    columns {
      name = "fle_status"
      type = "string"
    }
    columns {
      name = "fle_encrypted_fields"
      type = "int"
    }
    columns {
      name = "c_port"
      type = "int"
    }
    columns {
      name = "time_to_first_byte"
      type = "float"
    }
    columns {
      name = "x_edge_detailed_result_type"
      type = "string"
    }
    columns {
      name = "sc_content_type"
      type = "string"
    }
    columns {
      name = "sc_content_len"
      type = "bigint"
    }
    columns {
      name = "sc_range_start"
      type = "bigint"
    }
    columns {
      name = "sc_range_end"
      type = "bigint"
    }

    ser_de_info {
      parameters = {
        "field.delim"          = "\t"
        "serialization.format" = "\t"
      }
      serialization_library = "org.apache.hadoop.hive.serde2.lazy.LazySimpleSerDe"
    }
  }
}

resource "aws_athena_workgroup" "www" {
  name = "primary"

  configuration {
    enforce_workgroup_configuration    = false
    publish_cloudwatch_metrics_enabled = false

    result_configuration {
      output_location = "s3://${aws_s3_bucket.athena.bucket}/"
    }
  }
}

resource "aws_athena_named_query" "common_errs" {
  name      = "Common errors"
  workgroup = aws_athena_workgroup.www.name
  database  = local.db
  query     = <<-EOF
  SELECT
    request_ip,
    method,
    uri,
    status,
    COUNT(*) AS n
  FROM "${local.db}"."${local.tbl}"
  WHERE status >= 400
    AND from_iso8601_timestamp(concat(to_iso8601("date"), 'T', time)) > current_timestamp - interval '14' day
  GROUP BY status, method, uri, request_ip
  HAVING COUNT(*) > 1
  ORDER BY n DESC;
  EOF
}

resource "aws_athena_named_query" "recent_errs" {
  name      = "Recent errors"
  workgroup = aws_athena_workgroup.www.name
  database  = local.db
  query     = <<-EOF
  SELECT
    from_iso8601_timestamp(concat(to_iso8601("date"), 'T', time)) AT TIME ZONE 'Europe/Oslo' as "when",
    request_ip,
    method,
    uri,
    status
  FROM "${local.db}"."${local.tbl}"
  WHERE status >= 400
    AND status <= 599
    AND from_iso8601_timestamp(concat(to_iso8601("date"), 'T', time)) > current_timestamp - interval '8' hour
  ORDER BY "when" DESC
  LIMIT 25;
  EOF
}
