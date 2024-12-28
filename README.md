**If you want to contribute, see `CONTRIBUTING.md`.**

This is the code that fuels <https://wewerewondering.com/>, a website
that is aimed at facilitating live Q&A sessions. To use it, just go to
that URL and click "Create Event". Then, click "Share Event" and share
the URL that just got copied to your clipboard to anyone you want to be
able to ask questions. You'll see them come in live in the host view.
You can share the host view by copy-pasting the URL in your browser
address bar.

What it provides:

- Zero-hassle experience for you and your audience.
- Audience question voting.
- List of already-answered questions.
- Ability to hide questions.

What it doesn't provide:

- Protection against **malicious** double-voting.
- Live question feed for the audience (it is ~10s out-of-date).
- Long-lived Q&A sessions -- questions go away after 30 days.

---

If you're curious about the technology behind the site, it's all run on
AWS. Here's the rough architecture behind the scenes:

**Account.**

I've set up an [AWS Organization] for my personal AWS account. In that
organization, I've created a dedicated AWS account that holds all the
infrastructure for wewerewondering.com. That way, at least in theory,
it's cleanly separated from everything else, and could even be given
away to elsewhere should that become relevant.

**Domain.**

The domain is registered with [Hover], my registrar of choice for no
particularly good reason. The nameservers are set to point at [Route
53], which hold a single public hosted zone. It has MX records and SPF
pointing to [ImprovMX] (which is great btw), A and AAAA records that use
"[aliasing]" to point at the CloudFront distribution for the site (see
below). Finally, it has a CNAME record used for domain verification for
[AWS Certificate Manager].

The process for setting up the cert was a little weird. First, the
certificate **must** be in `us-east-1` to work with CloudFront for
[_reasons_]. Second, the CNAME record for domain verification wasn't
auto-added. Instead, I had to go into the Certificate Manager control
panel for the domain, and click a button named "Create records in Route
53". Not too bad, but wasn't immediately obvious. Once I did that
though, verification went through just fine.

**CDN.**

The main entry point for the site is [AWS CloudFront]. I have a single
"distribution", and the Route 53 A/AAAA entries are pointed at that one
distribution's CloudFront domain name. The distribution also has
wewerewondering.com configured as an [alternate domain name], and is
configured to use the Certificate Manager domain from earlier and the
most up-to-date TLS configuration. The distribution has "[standard
logging]" (to S3) enabled for now, and has a "default root object" of
`index.html` (more on that later).

CloudFront ties "[behaviors]" to "[origins]". Behaviors are ~= routes
and origins are ~= backends. There are two behaviors: the default route
and the `/api` route. There are two origins: [S3] and [API Gateway].
You get three internet points if you can guess which behavior connects
to which origin.

_Static components_. The default route (behavior) is set up to send
requests to the S3 origin, which in turn just points at an S3 bucket
that holds the output of building the stuff in `client/`. The behavior
redirects HTTP to HTTPS, only allows GET and HEAD requests, and uses the
`CachingOptimized` caching policy which basically means it has a long
default timeout (1 day) and compression enabled. In S3, I've
specifically overridden the "metadata" for `index.html` to set
cache-control to `max-age=300` since it gets updated in-place (the
`assets/` files have hashes in their names and can be cached forever).
In addition, it has the `SecurityHeaderPolicy` response header policy to
set `X-Frame-Options` and friends.

There's one non-obvious trick in use here to make the single-page app
approach work with "pretty" URLs that don't involve `#`. Ultimately we
want URLs that the single-page app handles to all be routed to
`index.html` rather than try to request, say, `/event/foo` from S3.
There are multiple ways to achieve this. The one I went with was to
define a [CloudFront function] to rewrite request URLs that I then
associate with the "Viewer request" hook. It looks like this:

```javascript
function handler(event) {
    var req = event.request;
    if (req.uri.startsWith('/event/')) {
        req.uri = '/index.html';
    }
    return req;
}
```

I did it this way rather than using a [custom error response] because
that _also_ rewrites 404 errors from the API origin, which I don't want.
Not to mention I wanted unhandled URLs to still give 404s. And I didn't
want to use [S3 Static Web Hosting] (which allows you to set up
[conditional redirects]) because then CloudFront can't access S3
"natively" and will instead redirect to the bucket and require it to be
publicly accessible.

Another modification I made to the defaults was to slightly modify the
S3 bucket policy compared to the one CloudFlare recommends in order to
allow LIST requests so that you get 404s instead of 403s. The part of
the policy I used was:

```json
"Action": [
	"s3:GetObject",
	"s3:ListBucket"
],
"Resource": [
	"arn:aws:s3:::wewerewondering-static",
	"arn:aws:s3:::wewerewondering-static/*"
],
```

_The `/api` endpoints._ The behavior for the `/api` URLs is defined for
the path `/api/*`, is configured to allow all HTTP methods but only
HTTPS, and also uses the `SecurityHeaderPolicy` response header
policy. For caching, I created my own policy that is basically
`CachingOptimized` but has a default TTL of 1s, because if I fail to set
a cache header I'd rather things mostly keep working rather than
everything looking like nothing updates.

The origin for `/api` is a [custom origin] that holds the "Invoke URL"
of the [API Gateway] API (and requires HTTPS). Which brings us to:

**The API.**

As previously mentioned, the API is a single [AWS Lambda] backed by the
[Lambda Rust Runtime] (see `server/` for more details). But, it's hosted
through AWS' [API Gateway] service, mostly because it gives me
throttling, metrics, and logging out of the box. For more elaborate
services I'm sure the multi-stage and authorization bits come in handy
too, but I haven't made use of any of that. The site also uses the [HTTP
API] configuration because it's a) cheaper, b) simpler to set up, and c)
worked out of the box with the [Lambda Rust Runtime], which the [REST
API] stuff didn't (for me at least). There are [other differences], but
none that seemed compelling for this site's use-case.

All of the routes supported by the API implementation (in `server/`) are
registered in API Gateway and are all pointed at the same Lambda. This
has the nice benefit that other routes won't even invoke the Lambda,
which (I assume) is cheaper. I've set up the `$default` stage to have
fairly conservative throttling (for now) just to avoid any surprise
jumps in cost. It also has "Access logging" [set up][api-gw-log].

One thing noting about using [API Gateway] with the [HTTP API] is that
the automatic dashboard it adds to CloudWatch [doesn't work][cw-api-gw]
because it expects the metrics from the [REST API], which are named
differently from the ones [used by the HTTP API]. The (annoying) fix was
to copy the automatic dashboard over into a new (custom) dashboard and
edit the source for every widget to replace
```
"ApiName", "wewerewondering"
```
with
```
"ApiId", "<the ID of the API Gateway API>"
```
and replace the metric names by the [correct ones][used by the HTTP
API].

The Lambda itself is mostly just what `cargo lambda deploy` sets up,
though I've specifically add `RUST_LOG` as an environment variable to
get more verbose logs (for now). It's also set up to log to CloudWatch,
which I think happened more or less automatically. Crucially though, the
IAM role used to execute the Lambda is also granted read/write (but not
delete/admin) access to the database, like so:

```json
{
    "Sid": "VisualEditor0",
    "Effect": "Allow",
    "Action": [
        "dynamodb:BatchGetItem",
        "dynamodb:PutItem",
        "dynamodb:GetItem",
        "dynamodb:Scan",
        "dynamodb:Query",
        "dynamodb:UpdateItem"
    ],
    "Resource": [
        "arn:aws:dynamodb:*:<account id>:table/events",
        "arn:aws:dynamodb:*:<account id>:table/questions",
        "arn:aws:dynamodb:*:<account id>:table/questions/index/top"
    ]
}
```

**The database.**

The site uses [DynamoDB] as its storage backend, because frankly, that's
all it needs. And it's fairly fast and cheap if you can get away with
its limited feature set. There are two tables, `events` and `questions`,
both of which are set up to use [on-demand provisioning]. `events` just
holds the ULID of an event, which is also the partition key (DynamoDB
[doesn't have] auto-increment integer primary keys because they don't
scale), the event's secret key, and its creation and [auto-deletion]
timestamp. `questions` has:

- the question ULID (as the partition key)
- the event ULID
- the question text
- the question author (if given)
- the number of votes
- whether the question is answered
- whether the question is hidden
- creation and [auto-deletion] timestamps

The ULIDs, the timestamps, and the question text + author never change
This is why the API to look up event info and question texts/authors is
separated from looking up vote counts -- the former can have much longer
cache time.

To allow querying questions for a given event, `questions` also has a
[global secondary index] called `top` whose partition key is the event
ULID. We don't use a sort key, since we want to sort by a [more complex
function]. That index also projects out the "answered" and "hidden"
fields so that a single query to that index gives all the mutable state
for an event's question list (and can thus be queried with a single
DynamoDB call by the Lambda).

[more complex function]: https://www.evanmiller.org/ranking-news-items-with-upvotes.html

**Metrics and Logging.**

<!-- TODO: Athena in particular -->

---

**Scaling further.**

Currently, everything is in `us-east-1`. That's sad. CDN helps
(potentially a lot), but mainly for guests, and not when voting. It's
mostly because DynamoDB [global tables] do reconciliation-by-overwrite,
which doesn't work very well for counters. Could make it store every
vote separately and do a count, but that's sad. Alternatively, if we
assume that most guests are near the host, we could:

1. Make `events` a global table (but not `questions`).
2. Have a separate `questions` in each region.
3. Add a `region` column to `events` which is set to the region that
   hosts the Lambda that serves the "create event" request.
4. Update the server code to always access `questions` in the region of
   the associated event.

We'd probably need to tweak CloudFlare (and maybe Route 53?) a little
bit to make it to do geo-aware routing, but I think that's a thing it
supports.

[AWS Organization]: https://docs.aws.amazon.com/organizations/latest/userguide/orgs_introduction.html
[Hover]: https://www.hover.com/
[Route 53]: https://aws.amazon.com/route53/
[ImprovMX]: https://improvmx.com/
[aliasing]: https://docs.aws.amazon.com/Route53/latest/DeveloperGuide/resource-record-sets-choosing-alias-non-alias.html
[AWS Certificate Manager]: https://aws.amazon.com/certificate-manager/
[_reasons_]: https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/cnames-and-https-requirements.html
[AWS CloudFront]: https://aws.amazon.com/cloudfront/
[alternate domain name]: https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/CNAMEs.html
[standard logging]: https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/AccessLogs.html
[behaviors]: https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/RequestAndResponseBehavior.html
[origins]: https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/DownloadDistS3AndCustomOrigins.html
[S3]: https://aws.amazon.com/s3/
[API Gateway]: https://aws.amazon.com/api-gateway/
[CloudFront function]: https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/cloudfront-functions.html?icmpid=docs_cf_help_panel
[custom error response]: https://stackoverflow.com/questions/38475329/single-page-application-in-aws-cloudfront
[S3 Static Web Hosting]: https://docs.aws.amazon.com/AmazonS3/latest/userguide/WebsiteHosting.html
[conditional redirects]: https://docs.aws.amazon.com/AmazonS3/latest/userguide/how-to-page-redirect.html#advanced-conditional-redirects
[custom origin]: https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/DownloadDistS3AndCustomOrigins.html#concept_CustomOrigin
[AWS Lambda]: https://aws.amazon.com/lambda/
[Lambda Rust Runtime]: https://github.com/awslabs/aws-lambda-rust-runtime
[HTTP API]: https://docs.aws.amazon.com/apigateway/latest/developerguide/http-api.html
[REST API]: https://docs.aws.amazon.com/apigateway/latest/developerguide/apigateway-rest-api.html
[other differences]: https://docs.aws.amazon.com/apigateway/latest/developerguide/http-api-vs-rest.html
[cw-api-gw]: https://repost.aws/questions/QURsag9V3pQjio1m0ZWebjIQ/cannot-find-http-api-by-name-in-cloudwatch-metrics
[used by the HTTP API]: https://docs.aws.amazon.com/apigateway/latest/developerguide/http-api-metrics.html
[api-gw-log]: https://docs.aws.amazon.com/apigateway/latest/developerguide/set-up-logging.html
[DynamoDB]: https://aws.amazon.com/dynamodb/
[on-demand provisioning]: https://aws.amazon.com/blogs/aws/amazon-dynamodb-on-demand-no-capacity-planning-and-pay-per-request-pricing/
[auto-deletion]: https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/TTL.html
[doesn't have]: https://aws.amazon.com/premiumsupport/knowledge-center/primary-key-dynamodb-table/
[global secondary index]: https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/GSI.html

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
