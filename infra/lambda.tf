data "aws_iam_policy_document" "xray" {
  statement {
    actions = [
      "xray:PutTraceSegments",
      "xray:PutTelemetryRecords",
    ]
    resources = ["*"]
  }
}

resource "aws_iam_policy" "xray" {
  # TODO
  name   = "AWSLambdaTracerAccessExecutionRole-14a6d1b5-3a03-4b02-94ca-fec2eced24ab"
  path   = "/service-role/"
  policy = data.aws_iam_policy_document.xray.json
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

resource "aws_iam_policy" "cloudwatch" {
  # TODO
  name   = "AWSLambdaBasicExecutionRole-b586114a-ba08-47b0-afe0-82c4d81857a0"
  path   = "/service-role/"
  policy = data.aws_iam_policy_document.cloudwatch.json
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
    aws_iam_policy.cloudwatch.arn,
    aws_iam_policy.xray.arn,
    "arn:aws:iam::aws:policy/CloudWatchLambdaInsightsExecutionRolePolicy"
  ]
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

resource "aws_iam_role_policy" "dynamodb" {
  name   = "api-db-access"
  role   = aws_iam_role.www.id
  policy = data.aws_iam_policy_document.dynamodb.json
}

resource "terraform_data" "cargo_lambda" {
  triggers_replace = {
    cargo_toml = "${base64sha256(file("${path.module}/../server/Cargo.toml"))}"
    main_rs    = "${base64sha256(file("${path.module}/../server/src/main.rs"))}"
  }

  provisioner "local-exec" {
    command     = "cargo lambda build --release --arm64"
    working_dir = "../server"
  }
}

data "archive_file" "lambda" {
  type        = "zip"
  source_file = "${path.module}/../server/target/lambda/wewerewondering-api/bootstrap"
  output_path = "lambda_function_payload.zip"
  depends_on  = [terraform_data.cargo_lambda]
}

resource "aws_lambda_function" "www" {
  function_name = "wewerewondering-api"
  role          = aws_iam_role.www.arn
  handler       = "bootstrap"
  runtime       = "provided.al2"
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
