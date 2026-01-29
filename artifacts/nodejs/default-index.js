const http = require('http');

const hostname = '0.0.0.0';
const port = process.env.SERVER_PORT || 3000;

const server = http.createServer((req, res) => {
    res.statusCode = 200;
    res.setHeader('Content-Type', 'text/html');
    res.end('<html><body><h1>Hello from Raptor Panel!</h1><p>Your Node.js server is running.</p></body></html>');
});

server.listen(port, hostname, () => {
    console.log(`Server listening on port ${port}`);
    console.log(`Server running at http://${hostname}:${port}/`);
});
