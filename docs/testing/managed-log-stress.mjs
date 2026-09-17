// Manual Windows retention check: register `node managed-log-stress.mjs 32109`
// in Port Lens, using this file's folder as cwd. Stop the App when finished.
import http from "node:http";
const port = Number(process.argv[2] || 32109);
const payload = "x".repeat(32 * 1024);
let sequence = 0;
const server = http.createServer((_request, response) => {
  response.end(`log retention probe pid=${process.pid} sequence=${sequence}\n`);
});
server.listen(port, "127.0.0.1", () => {
  setInterval(() => {
    const marker = `${new Date().toISOString()} sequence=${++sequence}`;
    process.stdout.write(`stdout ${marker} ${payload}\n`);
    process.stderr.write(`stderr ${marker} ${payload}\n`);
  }, 50);
});
