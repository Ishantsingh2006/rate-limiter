# rate-limiter

A high-performance rate limiting service built with Rust and Axum. Supports sliding window and fixed window algorithms with Redis (with TLS/SSL support) and an in-memory storage fallback. Exposes standard HTTP rate limit headers and a real-time web dashboard.

Live demo: [https://rate-limiter-tau.vercel.app/](https://rate-limiter-tau.vercel.app/)

## Architecture & Deployment

The application is architected as a distributed system deployed across modern cloud services:
- **Frontend**: Hosted on **Vercel** ([https://rate-limiter-tau.vercel.app/](https://rate-limiter-tau.vercel.app/)) serving a responsive web dashboard.
- **Backend API**: Deployed on **Render** (built with Rust's Axum & Tokio runtime), utilizing a custom CORS configuration to securely expose standardized rate limit headers to the frontend.
- **Database/Cache**: Backed by **Upstash Serverless Redis** for distributed rate limit state tracking, connected securely using Rust's `tls-rustls` SSL connector.

## Features

- **Multiple Algorithms** - Supports both **Sliding Window** and **Fixed Window** rate limiting strategies.
- **Dual Storage Backends** - Distributed Redis-backed storage (with `tls-rustls` support) and a memory-safe local fallback if Redis is unreachable.
- **Non-Incrementing Status Queries** - Inspect quota details (`/api/limiter-status`) without consuming available requests.
- **RFC Rate Limit Headers** - Returns standard headers (`RateLimit-Limit`, `RateLimit-Remaining`, and `RateLimit-Reset`).
- **Real-Time Dashboard** - Built-in web visualization using modern typography and layout.
- **Fail-Closed Architecture** - Protects backend database capacity if the rate limiter backend goes offline.

## Quick Start

### Prerequisites

- Rust 1.70+
- Docker and Docker Compose (optional, for Redis)

### Installation

```bash
# Clone the repository
git clone https://github.com/Ishantsingh2006/rate-limiter.git
cd rate-limiter

# Copy template configurations
cp .env.example .env
```

### Running Locally

```bash
# Start Redis (optional)
docker-compose up -d

# Run the API server
cargo run
```

The server will start on `http://127.0.0.1:3000` by default.

## Testing

```bash
# Run all unit and integration tests (tests both memory and Redis strategies)
cargo test --all

# Run only strategy integration tests
cargo test -p limiter_engine
```

## Usage in Terminal

Query the protected API endpoint (each request consumes 1 quota token):

```bash
curl -i -X GET \
  -H "Authorization: Bearer demo-client-token" \
  http://127.0.0.1:3000/api/data
```

**Response headers:**
```http
HTTP/1.1 200 OK
ratelimit-limit: 5
ratelimit-remaining: 4
ratelimit-reset: 59
cache-control: no-store, no-cache, must-revalidate
```

Query the rate limit status without consuming your quota:

```bash
curl -i -X GET \
  -H "Authorization: Bearer demo-client-token" \
  http://127.0.0.1:3000/api/limiter-status
```

**Response body:**
```json
{
  "limit": 5,
  "remaining": 4,
  "rest_in_sec": 59
}
```
