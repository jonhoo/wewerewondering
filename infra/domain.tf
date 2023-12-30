locals {
  domain = "wewerewondering.com"
}

resource "aws_route53_zone" "www" {
  name = local.domain
}

resource "aws_route53_record" "www_mx" {
  zone_id = aws_route53_zone.www.zone_id
  name    = local.domain
  type    = "MX"
  ttl     = 3600
  records = [
    "10 mx1.improvmx.com",
    "20 mx2.improvmx.com"
  ]
}

resource "aws_route53_record" "www_spf" {
  zone_id = aws_route53_zone.www.zone_id
  name    = local.domain
  type    = "TXT"
  ttl     = 3600
  records = [
    "v=spf1 include:spf.improvmx.com ~all",
  ]
}

resource "aws_route53_record" "www_cf" {
  zone_id = aws_route53_zone.www.zone_id
  name    = local.domain
  type    = "A"
  alias {
    name                   = aws_cloudfront_distribution.www.domain_name
    zone_id                = aws_cloudfront_distribution.www.hosted_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "www_cf_v6" {
  zone_id = aws_route53_zone.www.zone_id
  name    = local.domain
  type    = "AAAA"
  alias {
    name                   = aws_cloudfront_distribution.www.domain_name
    zone_id                = aws_cloudfront_distribution.www.hosted_zone_id
    evaluate_target_health = false
  }
}

resource "aws_acm_certificate" "www" {
  provider          = aws.us-east-1
  domain_name       = local.domain
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_route53_record" "www_cert" {
  for_each = {
    for dvo in aws_acm_certificate.www.domain_validation_options : dvo.domain_name => {
      name   = dvo.resource_record_name
      record = dvo.resource_record_value
      type   = dvo.resource_record_type
    }
  }

  allow_overwrite = true
  name            = each.value.name
  records         = [each.value.record]
  ttl             = 60
  type            = each.value.type
  zone_id         = aws_route53_zone.www.zone_id
}

resource "aws_acm_certificate_validation" "www" {
  provider                = aws.us-east-1
  certificate_arn         = aws_acm_certificate.www.arn
  validation_record_fqdns = [for record in aws_route53_record.www_cert : record.fqdn]
}
