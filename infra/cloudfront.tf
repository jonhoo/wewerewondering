locals {
  s3_origin_id = "wewerewondering"
  gw_origin_id = "wewerewondering-api"
}

resource "aws_cloudfront_origin_access_control" "static" {
  name                              = aws_s3_bucket.static.bucket_regional_domain_name
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

import {
  to = aws_cloudfront_origin_access_control.static
  id = "E2O0QG272YYJYR"
}

resource "aws_cloudfront_cache_policy" "cache_when_requested" {
  name        = "CacheWhenRequested"
  default_ttl = 1
  max_ttl     = 31536000
  min_ttl     = 1
  parameters_in_cache_key_and_forwarded_to_origin {
    cookies_config {
      cookie_behavior = "none"
    }
    headers_config {
      header_behavior = "none"
    }
    query_strings_config {
      query_string_behavior = "none"
    }
    enable_accept_encoding_brotli = true
    enable_accept_encoding_gzip   = true
  }
}

import {
  to = aws_cloudfront_cache_policy.cache_when_requested
  id = "fcc8df6d-6613-4210-8246-f45d18f04835"
}

resource "aws_cloudfront_function" "index_everywhere" {
  name    = "index-everywhere"
  runtime = "cloudfront-js-1.0"
  code    = file("${path.module}/index-everywhere.js")
}

import {
  to = aws_cloudfront_function.index_everywhere
  id = "index-everywhere"
}

resource "aws_cloudfront_distribution" "www" {
  origin {
    origin_id = local.gw_origin_id
    # NOTE: this is stupid
    domain_name = "${aws_apigatewayv2_api.www.id}.execute-api.${data.aws_region.current.name}.amazonaws.com"

    custom_origin_config {
      http_port              = 80
      https_port             = 443
      origin_protocol_policy = "https-only"
      origin_ssl_protocols   = ["TLSv1.2"]
    }
  }

  origin {
    origin_id                = local.s3_origin_id
    domain_name              = aws_s3_bucket.static.bucket_regional_domain_name
    origin_access_control_id = aws_cloudfront_origin_access_control.static.id
  }

  enabled             = true
  is_ipv6_enabled     = true
  default_root_object = "index.html"
  aliases             = ["wewerewondering.com"]
  price_class         = "PriceClass_All"
  http_version        = "http2"

  logging_config {
    include_cookies = false
    bucket          = aws_s3_bucket.logs.bucket_domain_name
  }

  default_cache_behavior {
    allowed_methods  = ["GET", "HEAD"]
    cached_methods   = ["GET", "HEAD"]
    target_origin_id = local.s3_origin_id

    # Using the CachingOptimized managed policy ID:
    cache_policy_id = "658327ea-f89d-4fab-a63d-7e88639e58f6"
    # Using the SecurityHeadersPolicy managed policy ID:
    response_headers_policy_id = "67f7725c-6f97-4210-82d7-5512b31e9d03"

    compress               = true
    viewer_protocol_policy = "redirect-to-https"

    function_association {
      event_type   = "viewer-request"
      function_arn = aws_cloudfront_function.index_everywhere.arn
    }
  }

  # Cache behavior with precedence 0
  ordered_cache_behavior {
    path_pattern     = "/api/*"
    allowed_methods  = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods   = ["GET", "HEAD"]
    target_origin_id = local.gw_origin_id
    compress         = true

    cache_policy_id = aws_cloudfront_cache_policy.cache_when_requested.id
    # Using the SecurityHeadersPolicy managed policy ID:
    response_headers_policy_id = "67f7725c-6f97-4210-82d7-5512b31e9d03"

    viewer_protocol_policy = "https-only"
  }

  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }

  viewer_certificate {
    acm_certificate_arn      = aws_acm_certificate_validation.www.certificate_arn
    minimum_protocol_version = "TLSv1.2_2021"
    ssl_support_method       = "sni-only"
  }
}

import {
  to = aws_cloudfront_distribution.www
  id = "E1ECZRHBXFKMHK"
}
