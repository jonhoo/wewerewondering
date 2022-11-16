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
<http://localhost:5173/#/event/00000000-0000-0000-0000-000000000000/secret>.
It will also auto-generate user votes over time for the questions there.

If you're curious about the technologies used in the server and client,
see their respective `README.md` files.
