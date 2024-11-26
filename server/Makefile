DYNAMODB_CONTAINER_NAME=dynamodb-local
DYNAMODB_ADMIN_CONTAINER_NAME=dynamodb-admin
ENDPOINT_URL=http://localhost:8000

.PHONY: fmt
fmt:
	cargo fmt

.PHONY: check
check:
	cargo fmt --check
	cargo clippy --all-features
	cargo d --no-deps --all-features

.PHONY: run
run:
	USE_DYNAMODB=1 AWS_ENDPOINT_URL=${ENDPOINT_URL} cargo run

.PHONY: test/e2e
test/e2e:
	AWS_ENDPOINT_URL=${ENDPOINT_URL} cargo t -- --ignored

.PHONY: dynamodb
dynamodb:
	@./run-dynamodb-local.sh ${DYNAMODB_CONTAINER_NAME} ${ENDPOINT_URL}

.PHONY: dynamodb/admin
dynamodb/admin:
	@docker ps | grep ${DYNAMODB_ADMIN_CONTAINER_NAME} > /dev/null && \
	echo "Already running. Use 'make dynamodb/admin/kill' first." && \
	exit 0

	echo "🚀 Spinning up a container with DynamoDB Admin..."
	@(docker run -d --rm --net host --name ${DYNAMODB_ADMIN_CONTAINER_NAME} aaronshaf/dynamodb-admin) > /dev/null
	echo "🔎 DynamoDB Admin is available at http://localhost:8001"

.PHONY: dynamodb/admin/kill
dynamodb/admin/kill:
	@docker ps | grep ${DYNAMODB_ADMIN_CONTAINER_NAME} > /dev/null && \
	docker stop ${DYNAMODB_ADMIN_CONTAINER_NAME} > /dev/null && \
	echo "🧹 Done!" && exit 0
	echo "Container '${DYNAMODB_ADMIN_CONTAINER_NAME}' not found.

.PHONY: dynamodb/list
dynamodb/list:
	@aws dynamodb list-tables --endpoint-url ${ENDPOINT_URL} | jq .TableNames

.PHONY: dynamodb/describe/events
dynamodb/describe/events:
	@aws dynamodb describe-table --table-name events --endpoint-url ${ENDPOINT_URL} | jq .Table

.PHONY: dynamodb/describe/questions
dynamodb/describe/questions:
	@aws dynamodb describe-table --table-name questions --endpoint-url ${ENDPOINT_URL} | jq .Table
	
.PHONY: dynamodb/describe/gsi
dynamodb/describe/questions/gsi:
	@aws dynamodb describe-table --table-name questions --endpoint-url ${ENDPOINT_URL} | jq .Table.GlobalSecondaryIndexes

.PHONY: dynamodb/kill
.ONESHELL: dynamodb/kill
dynamodb/kill:
	@docker ps | grep ${DYNAMODB_CONTAINER_NAME} > /dev/null && \
	docker stop ${DYNAMODB_CONTAINER_NAME} > /dev/null && \
	echo "🧹 Done!" && exit 0
	echo "Container '${DYNAMODB_CONTAINER_NAME}' not found."
