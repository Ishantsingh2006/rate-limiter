# rate-limiter

A high-performance rate limiting service built with Rust and Axum. Supports sliding window and fixed window algorithms with Redis (with TLS/SSL support) and an in-memory storage fallback. Exposes standard HTTP rate limit headers and a real-time web dashboard.

Live demo: [https://rate-limiter-tau.vercel.app/](https://rate-limiter-tau.vercel.app/)

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

Choose one of the two options below to run the service locally:

#### Option 1: Run everything in Docker (API + Redis)
Use this option to spin up both the Redis database and the API server containerized:
```bash
# Start both the API server and Redis in Docker
docker compose up -d --build
```
The server will start on `http://127.0.0.1:3000`.

#### Option 2: Run Redis in Docker, and the API server natively
Use this option for active development, allowing you to run and debug the Rust code locally on your host machine while using a containerized Redis instance:
```bash
# Start only the Redis container
docker compose up -d redis

# Run the API server natively on your host
cargo run
```
The native server will start on `http://127.0.0.1:3000` (or the port set in your `.env` file).

## Usage

You can test the rate limiting behavior locally or directly against the live production deployment using `curl`.

> [!TIP]
> By default, `curl` only displays the JSON response body. To inspect the rate-limiting HTTP response headers returned by the server (such as `ratelimit-limit`, `ratelimit-remaining`, and `ratelimit-reset`), add the `-i` flag to the `curl` commands below.

### 1. Consuming Quota
Each request to the data endpoint consumes 1 quota token.

#### Test Locally:
```bash
curl -X GET \
  -H "Authorization: Bearer demo-client-token" \
  http://127.0.0.1:3000/api/data
```

#### Test Production:
```bash
curl -X GET \
  -H "Authorization: Bearer demo-client-token" \
  https://rate-limiter-0p1u.onrender.com/api/data
```

### 2. Checking Status
Query the rate limit status without consuming quota:

#### Test Locally:
```bash
curl -X GET \
  -H "Authorization: Bearer demo-client-token" \
  http://127.0.0.1:3000/api/limiter-status
```

#### Test Production:
```bash
curl -X GET \
  -H "Authorization: Bearer demo-client-token" \
  https://rate-limiter-0p1u.onrender.com/api/limiter-status
```

## Architecture & Deployment

The application is architected as a distributed system deployed across modern cloud services:
- **Frontend**: Hosted on **Vercel** ([https://rate-limiter-tau.vercel.app/](https://rate-limiter-tau.vercel.app/)) serving a responsive web dashboard.
- **Backend API**: Deployed on **Render**([https://rate-limiter-0p1u.onrender.com](https://rate-limiter-0p1u.onrender.com)) (built with Rust's Axum & Tokio runtime).
- **Database/Cache**: Backed by **Upstash Serverless Redis** for distributed rate limit state tracking, connected securely using Rust's `tls-rustls` SSL connector.
- **Cross-Origin Resource Sharing (CORS)**: Configured with custom CORS policies to securely authorize cross-origin requests from the Vercel frontend and explicitly expose standard HTTP rate limit response headers (`ratelimit-*`) so client browsers can read them.
