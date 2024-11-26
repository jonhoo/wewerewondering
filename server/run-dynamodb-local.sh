#!/usr/bin/env bash

DYNAMODB_CONTAINER_NAME=$1
ENDPOINT_URL=$2

docker ps | grep ${DYNAMODB_CONTAINER_NAME} >/dev/null &&
    echo "Already running. Use 'make dynamodb/kill' first." &&
    exit 0

echo "🖴 Preparing volumes for DynamoDB..."
rm -rf dynamodb-data
mkdir dynamodb-data

echo "🚀 Spinning up a container with DynamoDB..."
(
    docker run --rm -d -v ./dynamodb-data:/home/dynamodblocal/data -p 127.0.0.1:8000:8000 \
        -w /home/dynamodblocal --name ${DYNAMODB_CONTAINER_NAME} amazon/dynamodb-local:latest \
        -jar DynamoDBLocal.jar -sharedDb -dbPath ./data
) >/dev/null

while ! (aws dynamodb list-tables --endpoint-url ${ENDPOINT_URL} >/dev/null 2>&1); do
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

echo "✅ Done!"
echo
echo "💡To get details on a table, run 'make dynamodb/describe/<table_name>'"
echo "💡To spin up a Web UI for your local DynamoDB instance, hit 'make dynamodb/admin'"
