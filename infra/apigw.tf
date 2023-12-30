resource "aws_apigatewayv2_api" "www" {
  name          = "wewerewondering"
  protocol_type = "HTTP"
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

resource "aws_api_gateway_account" "www" {
  cloudwatch_role_arn = aws_iam_role.apigw_cw.arn
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

resource "aws_apigatewayv2_integration" "www" {
  api_id                 = aws_apigatewayv2_api.www.id
  integration_type       = "AWS_PROXY"
  integration_method     = "POST"
  integration_uri        = aws_lambda_function.www.invoke_arn
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_route" "api_event_post" {
  api_id    = aws_apigatewayv2_api.www.id
  route_key = "POST /api/event"
  target    = "integrations/${aws_apigatewayv2_integration.www.id}"
}

resource "aws_apigatewayv2_route" "api_event_eid_post" {
  api_id    = aws_apigatewayv2_api.www.id
  route_key = "POST /api/event/{eid}"
  target    = "integrations/${aws_apigatewayv2_integration.www.id}"
}

resource "aws_apigatewayv2_route" "api_event_eid_get" {
  api_id    = aws_apigatewayv2_api.www.id
  route_key = "GET /api/event/{eid}"
  target    = "integrations/${aws_apigatewayv2_integration.www.id}"
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

resource "aws_lambda_permission" "www" {
  statement_id  = "AllowExecutionFromAPIGateway"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.www.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_stage.www.execution_arn}/*"
}
