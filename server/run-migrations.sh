#!/usr/bin/env bash

if ! [ -x "$(command -v aws)" ]; then
    echo '❌ Error: please make sure AWS CLI is installed and is in the PATH' >&2
    exit 1
fi

if [ "$1" = "" ]; then
    echo "
    ❌ Please provide DynamoDB endpoint url.

    Usage: $0 <endpoint_url>
    e.g.: $0 http://localhost:8000

    Also make sure 'AWS_ACCESS_KEY_ID', 'AWS_SECRET_ACCESS_KEY', and 'AWS_DEFAULT_REGION'
    are available in the environment or your '~/.aws' configuration files.
    "
    exit 1
fi

ENDPOINT_URL="$1"

echo "🗒️ Creating 'events' table..."
aws dynamodb create-table \
    --table-name events \
    --attribute-definitions AttributeName=id,AttributeType=S \
    --key-schema AttributeName=id,KeyType=HASH \
    --billing-mode PAY_PER_REQUEST \
    --endpoint-url ${ENDPOINT_URL} >/dev/null

aws dynamodb update-time-to-live \
    --table-name events \
    --time-to-live-specification Enabled=true,AttributeName=expire \
    --endpoint-url ${ENDPOINT_URL} >/dev/null

echo "🗒️ Creating 'questions' table and 🚄 GSI..."
aws dynamodb create-table \
    --table-name questions \
    --attribute-definitions AttributeName=id,AttributeType=S \
    AttributeName=eid,AttributeType=S \
    --key-schema AttributeName=id,KeyType=HASH \
    --global-secondary-indexes 'IndexName=top,KeySchema=[{AttributeName=eid,KeyType=HASH}],Projection={ProjectionType=INCLUDE,NonKeyAttributes=[answered,hidden,votes]}' \
    --billing-mode PAY_PER_REQUEST \
    --endpoint-url ${ENDPOINT_URL} >/dev/null

aws dynamodb update-time-to-live \
    --table-name questions \
    --time-to-live-specification Enabled=true,AttributeName=expire \
    --endpoint-url ${ENDPOINT_URL} >/dev/null
