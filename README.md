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
distribution's CloudFront domain name. The distribution also has wewerewondering.com configured as an
[alternate domain name], and is configured to use the Certificate
Manager domain from earlier and the most up-to-date TLS configuration.
The distribution has "[standard logging]" (to S3) enabled for now.

CloudFront ties "[behaviors]" to "[origins]". Behaviors are ~= routes
and origins are ~= backends. There are two behaviors: the default route
and the `/api` route. There are two origins: [S3] and [API Gateway].
Three internet points if you can guess which behavior connects to which
origin.


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
