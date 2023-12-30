terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.31.0"
    }
  }

  required_version = ">= 1.6.6"
}

provider "aws" {
  region = "eu-north-1"
  assume_role {
    role_arn    = "arn:aws:iam::880545379339:role/OrganizationAccountAccessRole"
    external_id = "terraform"
  }
}

# for ACM cert for CloudFront
# https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/cnames-and-https-requirements.html#https-requirements-aws-region
provider "aws" {
  region = "us-east-1"
  alias  = "us-east-1"
  assume_role {
    role_arn    = "arn:aws:iam::880545379339:role/OrganizationAccountAccessRole"
    external_id = "terraform"
  }
}

data "aws_region" "current" {}

terraform {
  cloud {
    organization = "wewerewondering"
    workspaces {
      name = "wewerewondering"
    }
  }
}
