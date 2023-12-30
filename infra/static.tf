locals {
  static = "wewerewondering-static"
}

resource "aws_s3_bucket" "static" {
  bucket = local.static
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
