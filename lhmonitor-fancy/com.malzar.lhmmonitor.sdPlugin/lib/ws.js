// Cliente WebSocket mínimo (RFC 6455, solo texto) para no depender de la versión de Node.
// Node 20 (el que trae Stream Deck de Elgato) no incluye WebSocket global.
'use strict';

const http = require('http');
const crypto = require('crypto');
const { EventEmitter } = require('events');

class MiniWS extends EventEmitter {
  constructor(url) {
    super();
    this.readyState = 0;
    this.buf = Buffer.alloc(0);
    this.frag = [];
    const u = new URL(url);
    const key = crypto.randomBytes(16).toString('base64');
    const req = http.request({
      host: u.hostname, port: u.port, path: u.pathname + u.search,
      headers: { Connection: 'Upgrade', Upgrade: 'websocket', 'Sec-WebSocket-Version': '13', 'Sec-WebSocket-Key': key },
    });
    req.on('upgrade', (res, socket, head) => {
      const expect = crypto.createHash('sha1').update(key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
      if (res.headers['sec-websocket-accept'] !== expect) { socket.destroy(); this.emit('error', new Error('Handshake inválido')); return; }
      this.socket = socket;
      socket.setNoDelay(true);
      this.readyState = 1;
      socket.on('data', (d) => this._data(d));
      socket.on('close', () => { this.readyState = 3; this.emit('close'); });
      socket.on('error', (e) => this.emit('error', e));
      this.emit('open');
      if (head && head.length) this._data(head);
    });
    req.on('response', () => this.emit('error', new Error('El servidor no aceptó WebSocket')));
    req.on('error', (e) => { this.readyState = 3; this.emit('error', e); this.emit('close'); });
    req.end();
  }

  _data(d) {
    this.buf = Buffer.concat([this.buf, d]);
    for (;;) {
      if (this.buf.length < 2) return;
      const b0 = this.buf[0], b1 = this.buf[1];
      let len = b1 & 0x7f, off = 2;
      if (len === 126) { if (this.buf.length < 4) return; len = this.buf.readUInt16BE(2); off = 4; }
      else if (len === 127) { if (this.buf.length < 10) return; len = Number(this.buf.readBigUInt64BE(2)); off = 10; }
      const masked = b1 & 0x80;
      const mask = masked ? this.buf.subarray(off, off + 4) : null;
      if (masked) off += 4;
      if (this.buf.length < off + len) return;
      let payload = this.buf.subarray(off, off + len);
      if (mask) { payload = Buffer.from(payload); for (let i = 0; i < payload.length; i++) payload[i] ^= mask[i & 3]; }
      this.buf = this.buf.subarray(off + len);
      const op = b0 & 0x0f, fin = b0 & 0x80;
      if (op === 0x8) { this.close(); return; }
      if (op === 0x9) { this._send(0xa, payload); continue; }
      if (op === 0xa) continue;
      if (op === 0x1 || op === 0x2 || op === 0x0) {
        this.frag.push(Buffer.from(payload));
        if (fin) { const msg = Buffer.concat(this.frag).toString('utf8'); this.frag = []; this.emit('message', msg); }
      }
    }
  }

  _send(op, payload) {
    if (this.readyState !== 1) return;
    const len = payload.length;
    let header;
    if (len < 126) { header = Buffer.alloc(2); header[1] = 0x80 | len; }
    else if (len < 65536) { header = Buffer.alloc(4); header[1] = 0x80 | 126; header.writeUInt16BE(len, 2); }
    else { header = Buffer.alloc(10); header[1] = 0x80 | 127; header.writeBigUInt64BE(BigInt(len), 2); }
    header[0] = 0x80 | op;
    const mask = crypto.randomBytes(4);
    const body = Buffer.from(payload);
    for (let i = 0; i < body.length; i++) body[i] ^= mask[i & 3];
    this.socket.write(Buffer.concat([header, mask, body]));
  }

  send(text) { this._send(0x1, Buffer.from(String(text), 'utf8')); }

  close() {
    if (this.readyState === 1) { try { this._send(0x8, Buffer.alloc(0)); } catch (_) {} }
    this.readyState = 3;
    if (this.socket) this.socket.end();
  }
}

module.exports = MiniWS;
