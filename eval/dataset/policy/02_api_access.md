# API Access Specification

Defines requirements for external API consumers.

## Rate Limiting

Controls request frequency to protect service availability.

- Clients must not exceed 1000 requests per minute per API key.
- Clients that exceed the rate limit must receive a 429 response.
- Retry-After headers should be included in all 429 responses.
- Clients may cache responses to reduce request volume.

## Authentication

- All API requests must include a valid Bearer token in the Authorization header.
- Expired tokens must be rejected with a 401 response.
- API keys must not be transmitted in URL query parameters.
- Clients should refresh tokens at least 60 seconds before expiry.

## Error Handling

Standardises error response format.

- Error responses must include a machine-readable code field.
- Error responses must include a human-readable message field.
- Stack traces must not be included in production error responses.
- Clients should implement exponential backoff on 5xx responses.
- Clients may retry idempotent requests on network timeout.

How to handle partial failures across federated services is unclear.
