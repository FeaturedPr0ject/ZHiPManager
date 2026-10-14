const http = require('node:http');
const crypto = require('node:crypto');

const PORT = Number(process.env.PORT || 10000);
const PRO_CODE = String(process.env.PRO_CODE || process.env.ZHIP_PRO_CODE || '');

function send(res, status, body) {
  const payload = JSON.stringify(body);
  res.writeHead(status, {
    'Content-Type': 'application/json; charset=utf-8',
    'Cache-Control': 'no-store',
    'Access-Control-Allow-Origin': process.env.ALLOWED_ORIGIN || '*',
    'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type'
  });
  res.end(payload);
}

function safeEqual(a, b) {
  const left = Buffer.from(String(a));
  const right = Buffer.from(String(b));
  return left.length === right.length && crypto.timingSafeEqual(left, right);
}

const server = http.createServer((req, res) => {
  if (req.method === 'OPTIONS') {
    res.writeHead(204, {
      'Access-Control-Allow-Origin': process.env.ALLOWED_ORIGIN || '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type',
      'Access-Control-Max-Age': '86400'
    });
    return res.end();
  }

  const url = new URL(req.url, 'http://localhost');

  if (req.method === 'GET' && url.pathname === '/health') {
    return send(res, 200, { ok: true, service: 'ZHiPManager Render API' });
  }

  if (req.method === 'POST' && url.pathname === '/api/pro/validate') {
    let body = '';
    req.on('data', chunk => {
      body += chunk;
      if (body.length > 8192) {
        send(res, 413, { valid: false, error: 'Request too large' });
        req.destroy();
      }
    });
    req.on('end', () => {
      let input;
      try {
        input = JSON.parse(body || '{}');
      } catch {
        return send(res, 400, { valid: false, error: 'Invalid JSON' });
      }

      if (!PRO_CODE) {
        return send(res, 503, { valid: false, error: 'Pro validation is not configured on the server' });
      }

      const supplied = typeof input.code === 'string' ? input.code.trim() : '';
      return send(res, 200, { valid: safeEqual(supplied, PRO_CODE) });
    });
    return;
  }

  return send(res, 404, { error: 'Not found' });
});

server.listen(PORT, '0.0.0.0', () => {
  console.log('ZHiPManager Render API listening on port ' + PORT);
});
