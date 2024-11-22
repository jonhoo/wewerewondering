DYNAMODB_CONTAINER_NAME=dynamodb-local

.PHONY: fmt
fmt:
	cargo fmt

.PHONY: check
check:
	cargo fmt --check
	cargo clippy --all-features
	cargo d --no-deps --all-features

.PHONY: dynamodb
.ONESHELL: dynamodb
dynamodb:
	@docker ps | grep ${DYNAMODB_CONTAINER_NAME} > /dev/null && \
	echo "Already running. Use 'make dynamodb/kill' first." && \
	exit 0

	echo "Preparing volumes for DynamoDB..."
	@rm -rf dynamodb-data
	@mkdir dynamodb-data

	echo "Spinning up a container with DynamoDB..."
	@(docker run --rm -d \
	-v ./dynamodb-data:/home/dynamodblocal/data \
	-p 127.0.0.1:8000:8000 \
	-w /home/dynamodblocal \
	--name ${DYNAMODB_CONTAINER_NAME} \
	amazon/dynamodb-local:latest \
	-jar DynamoDBLocal.jar -sharedDb -dbPath ./data) > /dev/null
	
	while ! (aws dynamodb list-tables --endpoint-url http://localhost:8000 > /dev/null 2>&1); do
		echo "Waiting for the database to start accepting connections..."
	done

	echo "Creating 'events' table..."
	@aws dynamodb create-table \
	 --table-name events \
	 --attribute-definitions AttributeName=id,AttributeType=S \
	 --key-schema AttributeName=id,KeyType=HASH \
	 --billing-mode PAY_PER_REQUEST \
	 --endpoint-url http://localhost:8000 > /dev/null
	
	@aws dynamodb update-time-to-live \
	 --table-name events \
	 --time-to-live-specification Enabled=true,AttributeName=expire \
	 --endpoint-url http://localhost:8000 > /dev/null

	echo "Tables in the database:"
	@aws dynamodb list-tables --endpoint-url http://localhost:8000 | jq .TableNames

.PHONY: dynamodb/kill
.ONESHELL: dynamodb/kill
dynamodb/kill:
	@docker ps | grep ${DYNAMODB_CONTAINER_NAME} > /dev/null && \
	docker stop ${DYNAMODB_CONTAINER_NAME} > /dev/null && \
	echo "Done!" && exit 0

	echo "Container '${DYNAMODB_CONTAINER_NAME}' not found."
