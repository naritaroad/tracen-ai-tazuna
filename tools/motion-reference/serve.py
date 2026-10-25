#!/usr/bin/env python3
"""Loopback-only review server with byte ranges for reliable MP4 seeking."""
import argparse
import functools
import http.server
import re
from pathlib import Path


class RangeHandler(http.server.SimpleHTTPRequestHandler):
    def send_head(self):
        path = Path(self.translate_path(self.path))
        self.byte_count = None
        header = self.headers.get('Range')
        if not header or not path.is_file():
            return super().send_head()
        size = path.stat().st_size
        match = re.fullmatch(r'bytes=(\d*)-(\d*)', header.strip())
        if not match or not any(match.groups()):
            self.send_error(416, 'Invalid byte range')
            return None
        start, end = match.groups()
        if start:
            start = int(start)
            end = min(size-1, int(end)) if end else size-1
        else:
            start, end = max(0, size-int(end)), size-1
        if start >= size or start > end:
            self.send_response(416)
            self.send_header('Content-Range', f'bytes */{size}')
            self.end_headers()
            return None
        f = path.open('rb')
        f.seek(start)
        self.send_response(206)
        self.send_header('Content-Type', self.guess_type(str(path)))
        self.send_header('Content-Length', str(end-start+1))
        self.send_header('Content-Range', f'bytes {start}-{end}/{size}')
        self.send_header('Accept-Ranges', 'bytes')
        self.end_headers()
        self.byte_count = end-start+1
        return f

    def copyfile(self, source, outputfile):
        if self.byte_count is None:
            return super().copyfile(source, outputfile)
        remaining = self.byte_count
        try:
            while remaining:
                block = source.read(min(1024*1024, remaining))
                if not block:
                    break
                outputfile.write(block)
                remaining -= len(block)
        except (BrokenPipeError, ConnectionResetError):
            pass


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument('--port', type=int, default=5181)
    args = parser.parse_args()
    handler = functools.partial(RangeHandler, directory=str(args.root.resolve()))
    server = http.server.ThreadingHTTPServer(('127.0.0.1', args.port), handler)
    print(f'Review server: http://127.0.0.1:{args.port}', flush=True)
    server.serve_forever()
