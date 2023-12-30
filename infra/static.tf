locals {
  static = "wewerewondering-static"
}

resource "aws_s3_bucket" "static" {
  bucket        = local.static
  force_destroy = true
}

import {
  to = aws_s3_bucket.static
  id = local.static
}

resource "aws_s3_bucket_ownership_controls" "static" {
  bucket = aws_s3_bucket.static.id

  rule {
    object_ownership = "BucketOwnerEnforced"
  }
}

import {
  to = aws_s3_bucket_ownership_controls.static
  id = local.static
}

resource "aws_s3_bucket_acl" "static" {
  depends_on = [aws_s3_bucket_ownership_controls.static]

  bucket = aws_s3_bucket.static.id
  acl    = "private"
}

import {
  to = aws_s3_bucket_acl.static
  id = "${local.static},private"
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

import {
  to = aws_s3_bucket_policy.cloudfront
  id = local.static
}

check "static-built" {
  assert {
    condition     = fileexists("${path.module}/../client/dist/index.html")
    error_message = "Run `npm run build` in ../client"
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
}

# TODO: delete old files in assets/ ?

# TODO: requires 1.7: https://github.com/hashicorp/terraform/pull/33932#issuecomment-1761821359
#import {
#  for_each = fileset("${path.module}/../client/dist", "**")
#
#  to = aws_s3_object.dist[each.value]
#  id = "${aws_s3_bucket.static.id}/${each.value}"
#}
import {
  to = aws_s3_object.dist["index.html"]
  id = "${aws_s3_bucket.static.id}/index.html"
}
import {
  to = aws_s3_object.dist["robots.txt"]
  id = "${aws_s3_bucket.static.id}/robots.txt"
}
import {
  to = aws_s3_object.dist["favicon.ico"]
  id = "${aws_s3_bucket.static.id}/favicon.ico"
}
import {
  to = aws_s3_object.dist["favicon.png"]
  id = "${aws_s3_bucket.static.id}/favicon.png"
}
import {
  to = aws_s3_object.dist["apple-touch-icon.png"]
  id = "${aws_s3_bucket.static.id}/apple-touch-icon.png"
}
