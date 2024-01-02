locals {
  static = "wewerewondering-${data.aws_region.current.name}-static"
}

resource "aws_s3_bucket" "static" {
  bucket        = local.static
  force_destroy = true
}

resource "aws_s3_bucket_ownership_controls" "static" {
  bucket = aws_s3_bucket.static.id

  rule {
    object_ownership = "BucketOwnerEnforced"
  }
}

data "aws_iam_policy_document" "cloudfront_s3" {
  policy_id = "PolicyForCloudFrontPrivateContent"

  statement {
    sid = "AllowCloudFrontServicePrincipal"

    principals {
      type        = "Service"
      identifiers = ["cloudfront.amazonaws.com"]
    }

    actions = [
      "s3:GetObject",
      "s3:ListBucket",
    ]

    resources = [
      aws_s3_bucket.static.arn,
      "${aws_s3_bucket.static.arn}/*",
    ]

    condition {
      test     = "StringEquals"
      variable = "AWS:SourceArn"

      values = [aws_cloudfront_distribution.www.arn]
    }
  }
}

resource "aws_s3_bucket_policy" "cloudfront" {
  bucket = aws_s3_bucket.static.id
  policy = data.aws_iam_policy_document.cloudfront_s3.json
}

check "static-built" {
  assert {
    condition     = fileexists("${path.module}/../client/dist/index.html")
    error_message = "Run `npm run build` in ../client"
  }
}

locals {
  content_types = {
    css  = "text/css"
    html = "text/html"
    js   = "application/javascript"
    json = "application/json"
    txt  = "text/plain"
    png  = "image/png"
    ico  = "image/vnd.microsoft.icon"
  }
}

resource "aws_s3_object" "dist" {
  for_each = fileset("${path.module}/../client/dist", "**")

  force_destroy = true
  bucket        = aws_s3_bucket.static.id
  key           = each.value
  source        = "${path.module}/../client/dist/${each.value}"
  # etag makes the file update when it changes; see https://stackoverflow.com/questions/56107258/terraform-upload-file-to-s3-on-every-apply
  etag = filemd5("${path.module}/../client/dist/${each.value}")

  cache_control = each.value == "index.html" ? "max-age=300" : null
  content_type  = local.content_types[element(split(".", each.value), length(split(".", each.value)) - 1)]
}

# TODO: delete old files in assets/ ?
