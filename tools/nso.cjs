// 仅用于可选的本地 NSO 导出比对；不依赖工作区临时分析脚本。
function lz4BlockDecode(src, start, end, outSize) {
  const out = Buffer.alloc(outSize);
  let ip = start;
  let op = 0;
  while (ip < end && op < outSize) {
    const token = src[ip++];
    let litLen = token >> 4;
    if (litLen === 15) {
      let b;
      do {
        b = src[ip++];
        litLen += b;
      } while (b === 255 && ip < end);
    }
    if (litLen > 0) {
      if (ip + litLen > end || op + litLen > outSize) throw new Error('literal overrun');
      src.copy(out, op, ip, ip + litLen);
      ip += litLen;
      op += litLen;
    }
    if (ip >= end) break;
    const offset = src[ip] | (src[ip + 1] << 8);
    ip += 2;
    if (offset === 0 || offset > op) throw new Error('bad match offset');
    let matchLen = token & 0x0F;
    if (matchLen === 15) {
      let b;
      do {
        b = src[ip++];
        matchLen += b;
      } while (b === 255 && ip < end);
    }
    matchLen += 4;
    if (op + matchLen > outSize) throw new Error('match overrun');
    let mp = op - offset;
    for (let i = 0; i < matchLen; i++) out[op++] = out[mp++];
  }
  if (op !== outSize) throw new Error('short output ' + op + '/' + outSize);
  return out;
}

function nsoHeader(b) {
  return {
    flags: b.readUInt32LE(0x0C),
    textFileOff: b.readUInt32LE(0x10),
    textMemOff: b.readUInt32LE(0x14),
    textSize: b.readUInt32LE(0x18),
    roFileOff: b.readUInt32LE(0x20),
    roMemOff: b.readUInt32LE(0x24),
    roSize: b.readUInt32LE(0x28),
    dataFileOff: b.readUInt32LE(0x30),
    dataMemOff: b.readUInt32LE(0x34),
    dataSize: b.readUInt32LE(0x38),
    bssSize: b.readUInt32LE(0x3C),
    textFileSize: b.readUInt32LE(0x60),
    roFileSize: b.readUInt32LE(0x64),
    dataFileSize: b.readUInt32LE(0x68),
  };
}

function decompressSegment(b, off, compSize, decompSize) {
  return lz4BlockDecode(b, off, off + compSize, decompSize);
}

module.exports = { nsoHeader, decompressSegment };
