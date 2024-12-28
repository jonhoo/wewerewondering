Hello there!

So, you want to help improve the site — great!

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
