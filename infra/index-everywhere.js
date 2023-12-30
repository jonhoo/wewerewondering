function handler(event) {
    var req = event.request;
    if (
        req.uri.startsWith('/event/')
    ) {
        req.uri = '/index.html';
    }
    return req;
}
