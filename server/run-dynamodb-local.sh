#!/usr/bin/env bash

if ! [ -x "$(command -v docker)" ]; then
    echo '❌ Error: please make sure docker is installed and is in the PATH' >&2
    exit 1
fi

if ! [ -x "$(command -v aws)" ]; then
    echo '❌ Error: please make sure AWS CLI is installed and is in the PATH' >&2
    exit 1
fi

# AWS CLI wants us to either run `aws configure` or provide the three essential variables
# from the environment. Most of us will have aws profile(s) configured on the workstation,
# but this should not be a requirement to be able to spin up and query a DynamoDB Local
# instance. This is why we are setting those variables in the current shell. Note that
# the  credential values themselves do not matter, it is rather the fact that they _are_ set.
#
# see: https://docs.aws.amazon.com/cli/v1/userguide/cli-configure-envvars.html#envvars-set
export AWS_ACCESS_KEY_ID=lorem
export AWS_SECRET_ACCESS_KEY=ipsum
export AWS_DEFAULT_REGION=us-east-1

DYNAMODB_CONTAINER_NAME=dynamodb-local
DYNAMODB_ADMIN_CONTAINER_NAME=dynamodb-admin
DYNAMODB_HOST=127.0.0.1
DYNAMODB_PORT=8000
ENDPOINT_URL=http://${DYNAMODB_HOST}:${DYNAMODB_PORT}

docker ps | grep ${DYNAMODB_CONTAINER_NAME} >/dev/null &&
    echo "🚫 Container \"${DYNAMODB_CONTAINER_NAME}\" with DynamoDB Local service is already running." && exit 0

echo "🖴 Preparing volumes for DynamoDB..."
rm -rf dynamodb-data
mkdir dynamodb-data

echo "🚀 Spinning up a container with DynamoDB..."
(
    docker run --rm -d -v ./dynamodb-data:/home/dynamodblocal/data -p ${DYNAMODB_HOST}:${DYNAMODB_PORT}:8000 \
        -w /home/dynamodblocal --name ${DYNAMODB_CONTAINER_NAME} amazon/dynamodb-local:latest \
        -jar DynamoDBLocal.jar -sharedDb -dbPath ./data
) >/dev/null

while ! (aws dynamodb list-tables --endpoint-url ${ENDPOINT_URL} >/dev/null); do
    echo "⏳ Waiting for the database to start accepting connections..."
done

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
    AttributeName=votes,AttributeType=N \
    --key-schema AttributeName=id,KeyType=HASH \
    --global-secondary-indexes 'IndexName=top,KeySchema=[{AttributeName=eid,KeyType=HASH},{AttributeName=votes,KeyType=RANGE}],Projection={ProjectionType=INCLUDE,NonKeyAttributes=[answered,hidden]}' \
    --billing-mode PAY_PER_REQUEST \
    --endpoint-url ${ENDPOINT_URL} >/dev/null

aws dynamodb update-time-to-live \
    --table-name questions \
    --time-to-live-specification Enabled=true,AttributeName=expire \
    --endpoint-url ${ENDPOINT_URL} >/dev/null

echo "✅ Container \"${DYNAMODB_CONTAINER_NAME}\" with DynamoDB Local is ready!"

docker ps | grep ${DYNAMODB_ADMIN_CONTAINER_NAME} >/dev/null &&
    echo "🚫 Container "${DYNAMODB_ADMIN_CONTAINER_NAME}" with DynamoDB Admin service is already running." &&
    exit 0

echo "🚀 Spinning up a container with DynamoDB Admin..."
(docker run -d --rm --net host --name ${DYNAMODB_ADMIN_CONTAINER_NAME} aaronshaf/dynamodb-admin) >/dev/null
echo "🔎 DynamoDB Admin is available at http://localhost:8001"

echo "✅ Done!"
