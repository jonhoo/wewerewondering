resource "aws_cloudwatch_dashboard" "www" {
  dashboard_name = "ApiGatewayHttp"

  dashboard_body = jsonencode({
    "widgets" : [
      {
        "height" : 4,
        "width" : 8,
        "y" : 0,
        "x" : 0,
        "type" : "metric",
        "properties" : {
          "metrics" : [
            ["AWS/ApiGateway", "Count", "ApiId", aws_apigatewayv2_api.www.id, { "period" : 300, "stat" : "Sum" }]
          ],
          "legend" : {
            "position" : "bottom"
          },
          "region" : data.aws_region.current.name,
          "liveData" : false,
          "title" : "Count: Sum",
          "period" : 300,
          "view" : "timeSeries",
          "stacked" : false
        }
      },
      {
        "height" : 4,
        "width" : 8,
        "y" : 0,
        "x" : 8,
        "type" : "metric",
        "properties" : {
          "metrics" : [
            ["AWS/ApiGateway", "5xx", "ApiId", aws_apigatewayv2_api.www.id, { "period" : 300, "stat" : "Sum" }]
          ],
          "legend" : {
            "position" : "bottom"
          },
          "region" : data.aws_region.current.name,
          "liveData" : false,
          "title" : "5XXError: Sum",
          "period" : 300,
          "view" : "timeSeries",
          "stacked" : false
        }
      },
      {
        "height" : 4,
        "width" : 8,
        "y" : 0,
        "x" : 16,
        "type" : "metric",
        "properties" : {
          "metrics" : [
            ["AWS/ApiGateway", "4xx", "ApiId", aws_apigatewayv2_api.www.id, { "period" : 300, "stat" : "Sum" }]
          ],
          "legend" : {
            "position" : "bottom"
          },
          "region" : data.aws_region.current.name,
          "liveData" : false,
          "title" : "4XXError: Sum",
          "period" : 300,
          "view" : "timeSeries",
          "stacked" : false
        }
      },
      {
        "height" : 4,
        "width" : 12,
        "y" : 4,
        "x" : 0,
        "type" : "metric",
        "properties" : {
          "metrics" : [
            ["AWS/ApiGateway", "Latency", "ApiId", aws_apigatewayv2_api.www.id, { "period" : 300, "stat" : "Average" }]
          ],
          "legend" : {
            "position" : "bottom"
          },
          "region" : data.aws_region.current.name,
          "liveData" : false,
          "title" : "Latency: Average",
          "period" : 300,
          "view" : "timeSeries",
          "stacked" : false
        }
      },
      {
        "height" : 4,
        "width" : 12,
        "y" : 4,
        "x" : 12,
        "type" : "metric",
        "properties" : {
          "metrics" : [
            ["AWS/ApiGateway", "IntegrationLatency", "ApiId", aws_apigatewayv2_api.www.id, { "period" : 300, "stat" : "Average" }]
          ],
          "legend" : {
            "position" : "bottom"
          },
          "region" : data.aws_region.current.name,
          "liveData" : false,
          "title" : "IntegrationLatency: Average",
          "period" : 300,
          "view" : "timeSeries",
          "stacked" : false
        }
      },
      {
        "height" : 4,
        "width" : 24,
        "y" : 8,
        "x" : 0,
        "type" : "metric",
        "properties" : {
          "metrics" : [
            ["AWS/ApiGateway", "DataProcessed", "ApiId", aws_apigatewayv2_api.www.id, { "period" : 300, "stat" : "Sum" }]
          ],
          "legend" : {
            "position" : "bottom"
          },
          "region" : data.aws_region.current.name,
          "liveData" : false,
          "title" : "DataProcessed: Sum",
          "period" : 300,
          "view" : "timeSeries",
          "stacked" : false
        }
      }
    ]
  })
}

import {
  to = aws_cloudwatch_dashboard.www
  id = "ApiGatewayHttp"
}
