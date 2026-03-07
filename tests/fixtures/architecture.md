# System Architecture

## Overview

Our system uses a microservices architecture with three main components:
the API Gateway, the Processing Engine, and the Storage Layer.

## API Gateway

The API Gateway handles all incoming HTTP requests and routes them to
appropriate backend services. It performs authentication, rate limiting,
and request validation.

**Decision**: We chose Nginx over HAProxy because of better WebSocket support.

**Important**: All API endpoints must use HTTPS in production.

## Processing Engine

The Processing Engine is responsible for:
- Data transformation and normalization
- Business logic execution
- Event-driven processing via message queues

**Preference**: We prefer async processing over synchronous calls for any
operation that takes more than 100ms.

**TODO**: Migrate the legacy batch processor to the new streaming architecture.

## Storage Layer

We use PostgreSQL for relational data and Redis for caching.

**Constraint**: All database queries must complete within 500ms.
Response times above this threshold trigger alerts.

**Definition**: SLA (Service Level Agreement) - Our commitment to 99.9%
uptime for the API Gateway.

## Deployment

Deployments follow a blue-green strategy. Each release goes through
staging before production.

**Fact**: The system handles approximately 10,000 requests per second
at peak load.
