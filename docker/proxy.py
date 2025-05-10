from http.server import SimpleHTTPRequestHandler, HTTPServer
import requests
import logging

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

logger.info("Starting proxy server on port 9799")


class CustomHandler(SimpleHTTPRequestHandler):
    def do_OPTIONS(self):
        logger.info(f"Received OPTIONS request to {self.path}")
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, PUT, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.send_header("Access-Control-Max-Age", "86400")  # 24 hours
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
            super().do_GET()

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

        self.send_response(resp.status_code)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(resp.content)


httpd = HTTPServer(("0.0.0.0", 9799), CustomHandler)
logger.info("Serving files and proxying /metrics to pushgateway on port 9799")
httpd.serve_forever()
