from http.server import SimpleHTTPRequestHandler, HTTPServer
import requests
import logging
import os

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

logger.info("Starting proxy server on port 9799")


class CustomHandler(SimpleHTTPRequestHandler):
    def add_cors_headers(self):
        """Utility method to add consistent CORS headers to every response."""
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header(
            "Access-Control-Allow-Methods", "GET, POST, PUT, OPTIONS, PATCH, PUT"
        )
        self.send_header("Access-Control-Expose-Headers", "*")
        self.send_header("Access-Control-Allow-Headers", "*")
        self.send_header("Access-Control-Max-Age", "86400")  # 24 hours

    def do_OPTIONS(self):
        logger.info(f"Received OPTIONS request to {self.path}")
        self.send_response(200)
        self.add_cors_headers()
        self.end_headers()

    def do_POST(self):
        logger.info(f"Received POST request to {self.path}")
        if self.path.startswith("/metrics"):
            self.proxy_metrics()
        else:
            super().do_POST()

    def do_GET(self):
        logger.info(f"Received GET request to {self.path}")
        if self.path.startswith("/metrics"):
            self.proxy_metrics()
        else:
            self.serve_static_file()

    def serve_static_file(self):
        """Serve a static file if it exists, or return 404 if not."""
        file_path = self.translate_path(self.path)
        logger.info(f"Attempting to serve file: {file_path}")

        if os.path.isfile(file_path):
            logger.info(f"Serving file: {file_path}")
            self.send_response(200)
            self.add_cors_headers()
            content_type = self.guess_type(file_path)
            self.send_header("Content-Type", content_type)
            self.end_headers()

            with open(file_path, "rb") as file:
                self.wfile.write(file.read())
        else:
            logger.warning(f"File not found: {file_path}")
            self.send_response(404)
            self.add_cors_headers()
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"404 Not Found")

    def do_PUT(self):
        logger.info(f"Received PUT request to {self.path}")
        if self.path.startswith("/metrics"):
            self.proxy_metrics()
        else:
            super().do_PUT()

    def proxy_metrics(self):
        content_len = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_len) if content_len > 0 else None
        logger.info(f"Received metrics body: {body}")
        path = self.path.split("/")
        logger.info(f"Path parts: {path}")

        # Handle both /metrics/job/{instance} and /metrics paths
        instance = path[3] if len(path) > 3 else "default"
        target_url = f"http://pushgateway:9091/metrics/job/{instance}"
        logger.info(f"Forwarding to: {target_url}")

        try:
            resp = requests.post(
                target_url,
                data=body,
                headers={"Content-Type": "application/x-www-form-urlencoded"},
            )
            logger.info(f"Pushgateway response status: {resp.status_code}")
        except Exception as e:
            logger.error(f"Error forwarding metrics: {str(e)}")
            self.send_error(500, f"Error forwarding metrics: {str(e)}")
            return

        self.add_cors_headers()
        self.send_response(resp.status_code)
        self.end_headers()
        self.wfile.write(resp.content)


httpd = HTTPServer(("0.0.0.0", 9799), CustomHandler)
logger.info("Serving files and proxying /metrics to pushgateway on port 9799")
httpd.serve_forever()
