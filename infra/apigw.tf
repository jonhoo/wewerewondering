resource "aws_apigatewayv2_api" "www" {
  name          = "wewerewondering"
  protocol_type = "HTTP"
}

import {
  to = aws_apigatewayv2_api.www
  id = "je8z4t28h4"
}

data "aws_iam_policy_document" "apigw_assume" {
  statement {
    principals {
      type        = "Service"
      identifiers = ["apigateway.amazonaws.com"]
    }
    actions = ["sts:AssumeRole"]
  }
}

resource "aws_iam_role" "apigw_cw" {
  name                = "wewerewondering-api-gw"
  description         = "Allows API Gateway to push logs to CloudWatch Logs."
  assume_role_policy  = data.aws_iam_policy_document.apigw_assume.json
  managed_policy_arns = ["arn:aws:iam::aws:policy/service-role/AmazonAPIGatewayPushToCloudWatchLogs"]
}

import {
  to = aws_iam_role.apigw_cw
  id = "wewerewondering-api-gw"
}

resource "aws_api_gateway_account" "www" {
  cloudwatch_role_arn = aws_iam_role.apigw_cw.arn
}

import {
  to = aws_api_gateway_account.www
  id = "api-gateway-account"
}

resource "aws_apigatewayv2_stage" "www" {
  api_id      = aws_apigatewayv2_api.www.id
  name        = "$default"
  auto_deploy = true
  access_log_settings {
    destination_arn = aws_cloudwatch_log_group.apigw.arn
    format = jsonencode({
      "requestId" : "$context.requestId",
      "ip" : "$context.identity.sourceIp",
      "requestTime" : "$context.requestTime",
      "httpMethod" : "$context.httpMethod",
      "routeKey" : "$context.routeKey",
      "status" : "$context.status",
      "protocol" : "$context.protocol",
      "responseLength" : "$context.responseLength"
    })
  }
  default_route_settings {
    throttling_burst_limit = 250
    throttling_rate_limit  = 50
  }
}

import {
  to = aws_apigatewayv2_stage.www
  id = "je8z4t28h4/$default"
}

resource "aws_apigatewayv2_integration" "www" {
  api_id                 = aws_apigatewayv2_api.www.id
  integration_type       = "AWS_PROXY"
  integration_method     = "POST"
  integration_uri        = aws_lambda_function.www.invoke_arn
  payload_format_version = "2.0"
}

import {
  to = aws_apigatewayv2_integration.www
  id = "je8z4t28h4/4y6aomd"
}

resource "aws_apigatewayv2_route" "api_event_post" {
  api_id    = aws_apigatewayv2_api.www.id
  route_key = "POST /api/event"
  target    = "integrations/${aws_apigatewayv2_integration.www.id}"
}

import {
  to = aws_apigatewayv2_route.api_event_post
  id = "je8z4t28h4/lmcxybh"
}

resource "aws_apigatewayv2_route" "api_event_eid_post" {
  api_id    = aws_apigatewayv2_api.www.id
  route_key = "POST /api/event/{eid}"
  target    = "integrations/${aws_apigatewayv2_integration.www.id}"
}

import {
  to = aws_apigatewayv2_route.api_event_eid_post
  id = "je8z4t28h4/cyva0m4"
}

resource "aws_apigatewayv2_route" "api_event_eid_get" {
  api_id    = aws_apigatewayv2_api.www.id
  route_key = "GET /api/event/{eid}"
  target    = "integrations/${aws_apigatewayv2_integration.www.id}"
}

import {
  to = aws_apigatewayv2_route.api_event_eid_get
  id = "je8z4t28h4/iih0hlf"
}

resource "aws_apigatewayv2_route" "api_route" {
  for_each = {
    get_eeq     = "GET /api/event/{eid}/questions",
    get_eeqs    = "GET /api/event/{eid}/questions/{secret}",
    post_toggle = "POST /api/event/{eid}/questions/{secret}/{qid}/toggle/{property}",
    get_q       = "GET /api/questions/{qids}",
    post_vote   = "POST /api/vote/{qid}/{updown}",
  }

  api_id    = aws_apigatewayv2_api.www.id
  route_key = each.value
  target    = "integrations/${aws_apigatewayv2_integration.www.id}"
}

import {
  to = aws_apigatewayv2_route.api_route["get_eeq"]
  id = "je8z4t28h4/ezhnbti"
}

import {
  to = aws_apigatewayv2_route.api_route["get_eeqs"]
  id = "je8z4t28h4/fb0pv8e"
}

import {
  to = aws_apigatewayv2_route.api_route["post_toggle"]
  id = "je8z4t28h4/0y2fhvt"
}

import {
  to = aws_apigatewayv2_route.api_route["get_q"]
  id = "je8z4t28h4/5j62zea"
}

import {
  to = aws_apigatewayv2_route.api_route["post_vote"]
  id = "je8z4t28h4/d6f5hnm"
}
