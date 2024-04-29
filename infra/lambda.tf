data "aws_iam_policy_document" "xray" {
  statement {
    actions = [
      "xray:PutTraceSegments",
      "xray:PutTelemetryRecords",
    ]
    resources = ["*"]
  }
}

data "aws_iam_policy_document" "cloudwatch" {
  statement {
    actions = [
      "logs:CreateLogGroup",
    ]
    resources = [aws_cloudwatch_log_group.lambda.arn]
  }

  statement {
    actions = [
      "logs:CreateLogStream",
      "logs:PutLogEvents",
    ]
    resources = ["${aws_cloudwatch_log_group.lambda.arn}:*"]
  }
}

data "aws_iam_policy_document" "dynamodb" {
  statement {
    actions = [
      "dynamodb:UpdateItem",
      "dynamodb:Scan",
      "dynamodb:Query",
      "dynamodb:PutItem",
      "dynamodb:GetItem",
      "dynamodb:BatchGetItem",
    ]
    resources = [
      aws_dynamodb_table.events.arn,
      aws_dynamodb_table.questions.arn,
      "${aws_dynamodb_table.questions.arn}/index/top"
    ]
  }
}

data "aws_iam_policy_document" "assume_role" {
  statement {
    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
    actions = ["sts:AssumeRole"]
  }
}

resource "aws_iam_role" "www" {
  name               = "wewerewondering-api"
  assume_role_policy = data.aws_iam_policy_document.assume_role.json
  path               = "/service-role/"

  managed_policy_arns = [
    "arn:aws:iam::aws:policy/CloudWatchLambdaInsightsExecutionRolePolicy"
  ]
  inline_policy {
    name   = "xray"
    policy = data.aws_iam_policy_document.xray.json
  }
  inline_policy {
    name   = "cloudwatch"
    policy = data.aws_iam_policy_document.cloudwatch.json
  }
  inline_policy {
    name   = "api-db-access"
    policy = data.aws_iam_policy_document.dynamodb.json
  }
}

check "lambda-built" {
  assert {
    condition     = fileexists("${path.module}/../server/target/lambda/wewerewondering-api/bootstrap")
    error_message = "Run `cargo lambda build --release --arm64` in ../server"
  }
}

data "archive_file" "lambda" {
  type        = "zip"
  source_file = "${path.module}/../server/target/lambda/wewerewondering-api/bootstrap"
  output_path = "lambda_function_payload.zip"
}

resource "aws_lambda_function" "www" {
  function_name = "wewerewondering-api"
  role          = aws_iam_role.www.arn
  handler       = "bootstrap"
  runtime       = "provided.al2023"
  architectures = ["arm64"]
  timeout       = 30
  layers = [
    "arn:aws:lambda:${data.aws_region.current.name}:580247275435:layer:LambdaInsightsExtension-Arm64:5"
  ]

  filename         = "lambda_function_payload.zip"
  source_code_hash = data.archive_file.lambda.output_base64sha256

  environment {
    variables = {
      RUST_LOG = "info,tower_http=debug,wewerewondering_api=trace"
    }
  }

  depends_on = [
    aws_cloudwatch_log_group.lambda,
  ]
}
