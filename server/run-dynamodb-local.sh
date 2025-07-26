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
#
# NB. In order to work with the tables and indexes created under these credentials, 
# applications (including DynamoDB Admin) should use these very credentials.
export AWS_ACCESS_KEY_ID=carpe
export AWS_SECRET_ACCESS_KEY=diem
export AWS_DEFAULT_REGION=us-east-1

DYNAMODB_NETWORK_NAME=wewerewondering
DYNAMODB_CONTAINER_NAME=dynamodb-local
DYNAMODB_ADMIN_CONTAINER_NAME=dynamodb-admin
DYNAMODB_HOST=127.0.0.1
DYNAMODB_PORT=8000
DYNAMODB_ADMIN_HOST=127.0.0.1
DYNAMODB_ADMIN_PORT=8001
ENDPOINT_URL=http://${DYNAMODB_HOST}:${DYNAMODB_PORT}

docker ps | grep ${DYNAMODB_CONTAINER_NAME} >/dev/null &&
    echo "🚫 Container \"${DYNAMODB_CONTAINER_NAME}\" with DynamoDB Local service is already running." && exit 0

echo "🖴 Preparing volumes for DynamoDB..."
rm -rf dynamodb-data
mkdir dynamodb-data

if docker network inspect ${DYNAMODB_NETWORK_NAME} 2>&1 >/dev/null; then
    echo "🚫 Network ${DYNAMODB_NETWORK_NAME} already exists, re-using..."
else
    docker network create ${DYNAMODB_NETWORK_NAME}
fi

echo "🚀 Spinning up a container with DynamoDB..."
(
    docker run --rm -d -p ${DYNAMODB_HOST}:${DYNAMODB_PORT}:8000 \
        -w /home/dynamodblocal --name ${DYNAMODB_CONTAINER_NAME} --network ${DYNAMODB_NETWORK_NAME} \
        amazon/dynamodb-local:latest
) >/dev/null

while ! (aws dynamodb list-tables --endpoint-url ${ENDPOINT_URL} >/dev/null); do
    echo "⏳ Waiting for the database to start accepting connections..."
done

./run-migrations.sh "${ENDPOINT_URL}"

echo "✅ Container \"${DYNAMODB_CONTAINER_NAME}\" with DynamoDB Local is ready!"

docker ps | grep ${DYNAMODB_ADMIN_CONTAINER_NAME} >/dev/null &&
    echo "🚫 Container "${DYNAMODB_ADMIN_CONTAINER_NAME}" with DynamoDB Admin service is already running." &&
    exit 0

echo "🚀 Spinning up a container with DynamoDB Admin..."
(
    docker run -d --rm -p ${DYNAMODB_ADMIN_HOST}:${DYNAMODB_ADMIN_PORT}:8001 \
        --name ${DYNAMODB_ADMIN_CONTAINER_NAME} \
        --network ${DYNAMODB_NETWORK_NAME} \
        -e AWS_ACCESS_KEY_ID=$AWS_ACCESS_KEY_ID \
        -e AWS_SECRET_ACCESS_KEY=$AWS_SECRET_ACCESS_KEY \
        -e DYNAMO_ENDPOINT=http://${DYNAMODB_CONTAINER_NAME}:8000 \
        aaronshaf/dynamodb-admin
) >/dev/null
echo "🔎 DynamoDB Admin is available at http://${DYNAMODB_ADMIN_HOST}:${DYNAMODB_ADMIN_PORT}"

echo "✅ Done!"
