locals {
  logs = "wewerewondering-logs"
}

data "aws_canonical_user_id" "current" {}

resource "aws_s3_bucket" "logs" {
  bucket = local.logs
}

import {
  to = aws_s3_bucket.logs
  id = local.logs
}

resource "aws_s3_bucket_ownership_controls" "logs" {
  bucket = aws_s3_bucket.logs.id

  rule {
    object_ownership = "BucketOwnerPreferred"
  }
}

import {
  to = aws_s3_bucket_ownership_controls.logs
  id = local.logs
}

resource "aws_s3_bucket_acl" "logs" {
  depends_on = [aws_s3_bucket_ownership_controls.logs]

  bucket = aws_s3_bucket.logs.id

  access_control_policy {
    grant {
      grantee {
        id   = data.aws_canonical_user_id.current.id
        type = "CanonicalUser"
      }
      permission = "FULL_CONTROL"
    }

    # https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/AccessLogs.html#AccessLogsBucketAndFileOwnership
    grant {
      grantee {
        type = "CanonicalUser"
        uri  = "c4c1ede66af53448b93c283ce9448c4ba468c9432aa01d700d3878632f77d2d0"
      }
      permission = "FULL_CONTROL"
    }

    owner {
      display_name = "admin"
      id           = data.aws_canonical_user_id.current.id
    }
  }
}

import {
  to = aws_s3_bucket_acl.logs
  id = local.logs
}

resource "aws_cloudwatch_log_group" "lambda" {
  name = "/aws/lambda/wewerewondering-api"
  # TODO
  retention_in_days = 0
}

import {
  to = aws_cloudwatch_log_group.lambda
  id = "/aws/lambda/wewerewondering-api"
}

resource "aws_cloudwatch_log_group" "apigw" {
  name = "/aws/api-gateway/wewerewondering"
  # TODO
  retention_in_days = 0
}

import {
  to = aws_cloudwatch_log_group.apigw
  id = "/aws/api-gateway/wewerewondering"
}

# arn:aws:iam::aws:policy/service-role/AmazonAPIGatewayPushToCloudWatchLogs
