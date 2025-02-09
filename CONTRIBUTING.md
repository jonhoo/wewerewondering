Hello there!

So, you want to help improve the site — great!

### Setup

Local setup is fairly straightforward:

1. Run the server (you'll need [Rust](https://www.rust-lang.org/)):
   ```console
   $ cd server && cargo run
   ```
2. Install the client components (you'll need [npm](https://www.npmjs.com/)):
   ```console
   $ cd client && npm install
   ```
3. Run the client:
   ```console
   $ cd client && npm run dev
   ```
4. Open <http://localhost:5173/>.

If you modify the files under `client/`, the browser view should
auto-update. If you modify files under `server/`, you'll have to re-run
`cargo run` to see its effects.

Note that when run this way, to aid in development, the server will
auto-populate an event with a set of questions from a past live Q&A
session I ran at
<http://localhost:5173/event/00000000000000000000000000/secret>.
It will also auto-generate user votes over time for the questions there.

If you're curious about the technologies used in the server and client,
see their respective `README.md` files.

### DynamoDB Local

To run tests against a DynamoDB instance running [locally](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/DynamoDBLocal.html), make sure
you got [`docker`](https://docs.docker.com/engine/install/) and
[`AWS CLI`](https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html#getting-started-install-instructions) installed, then hit:

```console
$ cd server
$ ./run-dynamodb-local.sh
```

This will also spin a [Web UI](https://github.com/aaronshaf/dynamodb-admin?tab=readme-ov-file)
for your local DynamoDB instance.

You can now run tests with:

```sh
USE_DYNAMODB=local cargo t -- --include-ignored
```

Assuming you are staying in the `server` directory, to run the back-end application against
your local DynamoDB instance, hit:

```sh
USE_DYNAMODB=local cargo run
```

### API Gateway Local

Prerequisites:

- [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html#binary-releases)
- [SAM CLI](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/install-sam-cli.html)
- DynamoDB Local [container](#backend-with-dynamodb-local)

NB! `API Gateway Local` will only work when the binary is built in release mode (`sam build` will do this for us).
See how we are wrapping the `axum` app in the `LambdaLayer` in release mode in [main](./server/src/main.rs).

To build and launch the application as a `Lambda` function behind `API Gateway` locally, `cd` to the server
directory, and hit:

```sh
sam build
sam local start-api
```

Once you make changes to the back-end code, open a separate terminal window and rebuild the app with:

```sh
sam build
```

The `sam local` process we've lauched previously will then pick up the new binary from `./server/.aws-sam` directory.

Here is how our `API Gateway Local` plus `DynamoDB Local` setup look like:

```sh
 ______________________________                                  _______________________________________________
|           Browser            |                                |       Docker Network: wewerewondering         |
|   _______________________    |     _______________________    |     __________________________________        |
|  |                       |   |    | API Gateway Proxy     |   |    | WeWereWondering Server Container |       |
|  | WeWereWodering Client |-- |--> | http://localhost:3000 | --|--> | ports: SAM assigns dynamically   | --|   |
|  | http://localhost:5173 |   |    |_______________________|   |    |__________________________________|   |   |
|  |_______________________|   |                                |                                           |   |
|   _______________________    |                                |                                           |   |
|  |                       |   |                                |     _____________________________         |   |
|  | DynamoDB Admin Client |---|--------------------------------|--> | DynamoDB Local Container    |        |   |
|  | http://localhost:8001 |   |                                |    | ports: 127.0.0.1:8000:8000  |        |   |
|  |_______________________|   |                                |    | host: dynamodb-local        | <------|   |
|                              |                                |    |_____________________________|            |
|______________________________|                                |_______________________________________________|
```

### End-to-end Testing

Prerequisites:

- [google-chrome](https://www.google.com/chrome/)
- [chromedriver](https://googlechromelabs.github.io/chrome-for-testing/#stable)
- DynamoDB Local [container](#backend-with-dynamodb-local)
- `wewerewondering` client [setup](#setup)

Make sure chrome binaries are in your path and launch the driver process:

```sh
chromedriver --port=4444
```

Prepare the client distribution first:

```sh
cd client && npm run build
```

Now, to run the e2e test suite, go to the `server` directory and issue:

```sh
USE_DYNAMODB=local cargo t --release --test e2e --features e2e-test
```

To run the e2e tests in a headless mode, hit:

```sh
USE_DYNAMODB=local HEADLESS=1 cargo t --release --test e2e --features e2e-test
```

Note, that if you've launch a web driver on a diffrent port (which you may want to do in order
to test with a different engine), you will want to provide `WEBDRIVER_PORT` to the test
run command:

```sh
WEBDRIVER_PORT=<port_goes_here> USE_DYNAMODB=local cargo t --release --test e2e --features e2e-test
```
