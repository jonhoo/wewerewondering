Run the server:

```console
$ cd server && cargo run
```

Run the client (after `cd client && npm install`):

```console
$ cd client && npm run dev
```

Then go to <https://localhost:5173/>.

<!-- TODO: how the AWS parts are set up, especially DynamoDB. -->

---

**Notes for me**

To deploy server:

```console
cd server
cargo lambda build --release --arm64
cargo lambda deploy --env-var RUST_LOG=info,tower_http=debug,wewerewondering_api=trace --profile qa
```

To deploy client:

```console
cd client
npm run build
aws --profile qa s3 sync --delete dist/ s3://wewerewondering-static
```
