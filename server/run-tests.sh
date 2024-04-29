#!/usr/bin/env bash
json=$(aws sts assume-role --role-arn "arn:aws:iam::880545379339:role/OrganizationAccountAccessRole" --role-session-name cargo-test)
AWS_ACCESS_KEY_ID=$(jq -r .Credentials.AccessKeyId <<<"$json")
AWS_SECRET_ACCESS_KEY=$(jq -r .Credentials.SecretAccessKey <<<"$json")
AWS_SESSION_TOKEN=$(jq -r .Credentials.SessionToken <<<"$json")
export AWS_ACCESS_KEY_ID
export AWS_SECRET_ACCESS_KEY
export AWS_SESSION_TOKEN
export AWS_REGION=eu-north-1
cargo t "$@" -- --ignored
