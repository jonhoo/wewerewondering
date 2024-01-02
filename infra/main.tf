terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.31.0"
    }
  }

  required_version = ">= 1.6.6"
}

variable "tfc_aws_dynamic_credentials" {
  description = "Object containing AWS dynamic credentials configuration"
  type = object({
    default = object({
      shared_config_file = string
    })
    aliases = map(object({
      shared_config_file = string
    }))
  })
}

provider "aws" {
  region              = "eu-north-1"
  shared_config_files = [var.tfc_aws_dynamic_credentials.default.shared_config_file]
  # assume_role {
  #   role_arn    = "arn:aws:iam::880545379339:role/OrganizationAccountAccessRole"
  #   external_id = "terraform"
  # }
}

# for ACM cert for CloudFront
# https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/cnames-and-https-requirements.html#https-requirements-aws-region
provider "aws" {
  region              = "us-east-1"
  alias               = "us-east-1"
  shared_config_files = [var.tfc_aws_dynamic_credentials.default.shared_config_file]
  # assume_role {
  #   role_arn    = "arn:aws:iam::880545379339:role/OrganizationAccountAccessRole"
  #   external_id = "terraform"
  # }
}

data "aws_region" "current" {}
data "aws_caller_identity" "current" {}
data "aws_canonical_user_id" "current" {}

terraform {
  cloud {
    organization = "wewerewondering"
    workspaces {
      name = "wewerewondering"
    }
  }
}

provider "tfe" {
  hostname = var.tfc_hostname
}
